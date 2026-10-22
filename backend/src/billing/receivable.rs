use sqlx::{PgPool, Postgres, Row, Transaction};
use uuid::Uuid;

use crate::{
    error::{AppError, AppResult},
    id::DbId,
};

use super::{account, BillingCharge, CreditAccountId, DebitPart};

pub(super) async fn unpaid_accounts(
    pool: &PgPool,
    user_id: DbId,
    accounts: &[CreditAccountId],
) -> AppResult<Vec<CreditAccountId>> {
    let ids = accounts
        .iter()
        .map(|account| account.id)
        .collect::<Vec<_>>();
    let rows: Vec<DbId> = sqlx::query_scalar(
        "SELECT DISTINCT r.credit_account_id FROM billing_receivable r
         JOIN credit_account w ON w.id=r.credit_account_id
         WHERE r.settled_at IS NULL AND (w.owner_user_id=$1 OR w.id=ANY($2))
         ORDER BY r.credit_account_id",
    )
    .bind(user_id)
    .bind(ids)
    .fetch_all(pool)
    .await?;
    Ok(rows.into_iter().map(CreditAccountId::new).collect())
}

/// Recovery released reserved credit, not an actual debit. Collect late usage
/// from spendable funds, or retain a durable receivable until funds arrive.
pub(crate) async fn charge_recovered(
    tx: &mut Transaction<'_, Postgres>,
    usage_id: DbId,
    billing: &BillingCharge,
    part: &DebitPart,
    locked_accounts: &[CreditAccountId],
) -> AppResult<()> {
    if try_collect(
        tx,
        Some(usage_id),
        billing.transaction_id,
        part,
        Some(locked_accounts),
    )
    .await?
    {
        return Ok(());
    }
    sqlx::query(
        "INSERT INTO billing_receivable
         (transaction_id,usage_id,allocation_id,credit_account_id,amount_micros,billing_status)
         VALUES ($1,$2,$3,$4,$5,$6)
         ON CONFLICT (transaction_id,allocation_id) DO NOTHING",
    )
    .bind(billing.transaction_id)
    .bind(usage_id)
    .bind(part.allocation_id)
    .bind(part.credit_account.id)
    .bind(part.amount_micros)
    .bind(billing.status.as_str())
    .execute(&mut **tx)
    .await?;
    sqlx::query("UPDATE usage SET billing_status='payment_pending' WHERE id=$1")
        .bind(usage_id)
        .execute(&mut **tx)
        .await?;
    tracing::warn!(%usage_id, transaction_id=%billing.transaction_id,
        allocation_id=part.allocation_id, amount_micros=part.amount_micros,
        "late usage awaits available balance; durable receivable retained");
    Ok(())
}

async fn try_collect(
    tx: &mut Transaction<'_, Postgres>,
    usage_id: Option<DbId>,
    transaction_id: Uuid,
    part: &DebitPart,
    locked_accounts: Option<&[CreditAccountId]>,
) -> AppResult<bool> {
    let mut accounts =
        account::repayment_accounts(tx, std::slice::from_ref(&part.credit_account)).await?;
    // A batch already owns its wallet locks. A project restored since that
    // snapshot must wait for reconciliation rather than reverse the lock order.
    if let Some(locked_accounts) = locked_accounts {
        accounts.retain(|account| locked_accounts.contains(account));
    }
    account::lock_accounts_for_update(tx, &accounts).await?;
    let mut available = Vec::with_capacity(accounts.len());
    for credit_account in &accounts {
        let amount: i64 = sqlx::query_scalar(
            "SELECT balance_micros-reserved_micros FROM credit_account WHERE id=$1",
        )
        .bind(credit_account.id)
        .fetch_one(&mut **tx)
        .await?;
        available.push(amount);
    }
    if available.iter().copied().fold(0_i64, i64::saturating_add) < part.amount_micros {
        return Ok(false);
    }
    let mut debits = Vec::new();
    let mut remaining = part.amount_micros;
    for (credit_account, available) in accounts.iter().zip(available) {
        let amount = remaining.min(available);
        if amount <= 0 {
            continue;
        }
        let balance: i64 = sqlx::query_scalar(
            "UPDATE credit_account SET balance_micros=balance_micros-$2,updated_at=now()
             WHERE id=$1 AND balance_micros-reserved_micros >= $2 RETURNING balance_micros",
        )
        .bind(credit_account.id)
        .bind(amount)
        .fetch_one(&mut **tx)
        .await?;
        debits.push((credit_account, amount, balance));
        remaining -= amount;
        if remaining == 0 {
            break;
        }
    }
    let updated = sqlx::query(
        "UPDATE credit_allocation
         SET consumed_micros=consumed_micros+$2,returned_micros=returned_micros-$2,updated_at=now()
         WHERE id=$1 AND status='recovered' AND returned_micros >= $2",
    )
    .bind(part.allocation_id)
    .bind(part.amount_micros)
    .execute(&mut **tx)
    .await?;
    if updated.rows_affected() != 1 {
        return Err(AppError::Conflict(
            "late billing exceeds recovered allocation".into(),
        ));
    }
    for (payer, amount, balance) in debits {
        sqlx::query(
            "INSERT INTO credit_ledger
         (credit_account_id,amount_micros,balance_after_micros,reason,
          usage_id,allocation_id,transaction_id,metadata)
         VALUES ($1,$2,$3,'usage',$4,$5,$6,$7)",
        )
        .bind(payer.id)
        .bind(-amount)
        .bind(balance)
        .bind(usage_id)
        .bind(part.allocation_id)
        .bind(transaction_id)
        .bind(serde_json::json!({
            "late_after_recovery": true,
            "original_credit_account_id": part.credit_account.id,
            "repayment_source": if payer == &part.credit_account { "original_account" } else { "owner_default_project" },
        }))
        .execute(&mut **tx)
        .await?;
    }
    Ok(true)
}

pub(super) async fn reconcile(pool: &PgPool, limit: i64) -> AppResult<u64> {
    let mut collected = 0;
    for _ in 0..limit.max(0) {
        let mut tx = pool.begin().await?;
        let row = sqlx::query(
            "SELECT id,transaction_id,usage_id,allocation_id,credit_account_id,amount_micros,billing_status
             FROM billing_receivable WHERE settled_at IS NULL AND retry_after <= now()
             ORDER BY retry_after,id LIMIT 1 FOR UPDATE SKIP LOCKED",
        )
        .fetch_optional(&mut *tx)
        .await?;
        let Some(row) = row else {
            tx.commit().await?;
            break;
        };
        let id: DbId = row.try_get("id")?;
        let usage_id: Option<DbId> = row.try_get("usage_id")?;
        // Serialize separate receivables for the same usage before restoring its
        // final status, including concurrent collectors on different workers.
        if let Some(usage_id) = usage_id {
            sqlx::query("SELECT id FROM usage WHERE id=$1 FOR UPDATE")
                .bind(usage_id)
                .execute(&mut *tx)
                .await?;
        }
        let part = DebitPart {
            credit_account: CreditAccountId::new(row.try_get("credit_account_id")?),
            allocation_id: row.try_get("allocation_id")?,
            amount_micros: row.try_get("amount_micros")?,
        };
        sqlx::query("SELECT id FROM credit_allocation WHERE id=$1 FOR UPDATE")
            .bind(part.allocation_id)
            .execute(&mut *tx)
            .await?;
        if try_collect(
            &mut tx,
            usage_id,
            row.try_get("transaction_id")?,
            &part,
            None,
        )
        .await?
        {
            sqlx::query(
                "UPDATE billing_receivable SET settled_at=now(),attempts=attempts+1 WHERE id=$1",
            )
            .bind(id)
            .execute(&mut *tx)
            .await?;
            if let Some(usage_id) = usage_id {
                sqlx::query(
                    "UPDATE usage SET billing_status=$2 WHERE id=$1 AND billing_status='payment_pending'
                     AND NOT EXISTS (SELECT 1 FROM billing_receivable WHERE usage_id=$1 AND settled_at IS NULL)",
                ).bind(usage_id).bind(row.try_get::<String,_>("billing_status")?)
                    .execute(&mut *tx).await?;
            }
            collected += 1;
        } else {
            sqlx::query(
                "UPDATE billing_receivable SET retry_after=now()+interval '30 seconds',attempts=attempts+1 WHERE id=$1",
            ).bind(id).execute(&mut *tx).await?;
        }
        tx.commit().await?;
    }
    Ok(collected)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[sqlx::test(migrations = "./migrations")]
    #[ignore = "requires PostgreSQL; run with DATABASE_URL and --ignored"]
    async fn restored_default_project_waits_for_reconciliation_without_taking_new_wallet_locks(
        pool: PgPool,
    ) {
        sqlx::raw_sql(
            r#"
            INSERT INTO "user" (id,email,user_group_id)
              SELECT 99111,'restored-payer@example.test',id FROM user_group WHERE is_default;
            INSERT INTO project (id,name,owner_user_id,is_default,status)
              VALUES (99111,'Restored owner',99111,TRUE,'disabled');
            INSERT INTO credit_account (owner_type,owner_id,balance_micros)
              VALUES ('project',99111,100);
            INSERT INTO user_key (id,user_id,project_id,key_prefix,secret_ciphertext)
              VALUES (99111,99111,99111,'restored','unused');
            INSERT INTO user_key_model (id,user_key_id,model) VALUES (99111,99111,'gpt-4.1');
            "#,
        )
        .execute(&pool)
        .await
        .unwrap();
        let original_id: DbId = sqlx::query_scalar(
            "INSERT INTO credit_account (owner_type,owner_id)
             VALUES ('user_key_model',99111) RETURNING id",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        let allocation_id: DbId = sqlx::query_scalar(
            "INSERT INTO credit_allocation (credit_account_id,amount_micros,returned_micros,status)
             VALUES ($1,100,100,'recovered') RETURNING id",
        )
        .bind(original_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        let project_id: DbId = sqlx::query_scalar(
            "SELECT id FROM credit_account WHERE owner_type='project' AND owner_id=99111",
        )
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(project_id < original_id);
        let part = DebitPart {
            credit_account: CreditAccountId::new(original_id),
            allocation_id,
            amount_micros: 35,
        };
        let mut tx = pool.begin().await.unwrap();
        let locked_accounts = account::lock_debit_parts(&mut tx, &[&part]).await.unwrap();
        assert_eq!(locked_accounts, vec![part.credit_account.clone()]);
        sqlx::query("UPDATE project SET status='enabled' WHERE id=99111")
            .execute(&pool)
            .await
            .unwrap();
        let mut wallet_holder = pool.begin().await.unwrap();
        sqlx::query("SELECT id FROM credit_account WHERE id=$1 FOR UPDATE")
            .bind(project_id)
            .fetch_one(&mut *wallet_holder)
            .await
            .unwrap();
        let transaction_id = Uuid::new_v4();
        let collected = tokio::time::timeout(
            std::time::Duration::from_millis(500),
            try_collect(&mut tx, None, transaction_id, &part, Some(&locked_accounts)),
        )
        .await
        .expect("an already locked batch must not take the newly eligible wallet")
        .unwrap();
        assert!(!collected);
        wallet_holder.rollback().await.unwrap();
        tx.commit().await.unwrap();
        let mut reconciliation = pool.begin().await.unwrap();
        sqlx::query("SELECT id FROM credit_allocation WHERE id=$1 FOR UPDATE")
            .bind(allocation_id)
            .fetch_one(&mut *reconciliation)
            .await
            .unwrap();
        assert!(
            try_collect(&mut reconciliation, None, transaction_id, &part, None)
                .await
                .unwrap()
        );
        reconciliation.commit().await.unwrap();
        let balances: Vec<(DbId, i64)> = sqlx::query_as(
            "SELECT id,balance_micros FROM credit_account WHERE id=ANY($1) ORDER BY id",
        )
        .bind(vec![project_id, original_id])
        .fetch_all(&pool)
        .await
        .unwrap();
        assert_eq!(balances, vec![(project_id, 65), (original_id, 0)]);
    }
}
