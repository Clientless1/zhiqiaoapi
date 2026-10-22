use std::{collections::BTreeMap, sync::Arc, time::Duration};

use sqlx::{PgPool, Row};
use tokio::{task::JoinHandle, time};
use uuid::Uuid;

use crate::error::AppResult;

use super::DebitHold;

const HOLD_LEASE_SECONDS: i32 = 120;
const HOLD_HEARTBEAT: Duration = Duration::from_secs(10);

#[derive(Debug)]
pub(crate) struct HoldLease {
    task: JoinHandle<()>,
}

impl Drop for HoldLease {
    fn drop(&mut self) {
        self.task.abort();
    }
}

/// Serialize registration against recovery before allowing an upstream call.
pub(super) async fn register(pool: &PgPool, hold: &mut DebitHold) -> AppResult<Vec<i64>> {
    if hold.parts.is_empty() {
        return Ok(Vec::new());
    }
    let mut amounts = BTreeMap::new();
    for part in &hold.parts {
        *amounts.entry(part.allocation_id).or_insert(0_i64) += part.amount_micros;
    }
    let allocation_ids = amounts.keys().copied().collect::<Vec<_>>();
    let mut tx = pool.begin().await?;
    let rows = sqlx::query(
        "SELECT id, status, amount_micros - consumed_micros - returned_micros AS remaining
         FROM credit_allocation WHERE id = ANY($1) ORDER BY id FOR UPDATE",
    )
    .bind(&allocation_ids)
    .fetch_all(&mut *tx)
    .await?;
    let invalid = amounts
        .iter()
        .filter(|(id, amount)| {
            !rows.iter().any(|row| {
                row.get::<i64, _>("id") == **id
                    && row.get::<String, _>("status") == "active"
                    && row.get::<i64, _>("remaining") >= **amount
            })
        })
        .map(|(id, _)| *id)
        .collect::<Vec<_>>();
    if !invalid.is_empty() {
        tx.rollback().await?;
        return Ok(invalid);
    }
    sqlx::query(
        "INSERT INTO billing_hold (transaction_id, expires_at)
         VALUES ($1, clock_timestamp() + make_interval(secs => $2))",
    )
    .bind(hold.transaction_id)
    .bind(HOLD_LEASE_SECONDS as f64)
    .execute(&mut *tx)
    .await?;
    for (allocation_id, amount) in amounts {
        sqlx::query(
            "INSERT INTO billing_hold_allocation (transaction_id, allocation_id, amount_micros)
             VALUES ($1, $2, $3)",
        )
        .bind(hold.transaction_id)
        .bind(allocation_id)
        .bind(amount)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    hold.lease = Some(start_heartbeat(
        pool.clone(),
        hold.transaction_id,
        HOLD_HEARTBEAT,
    ));
    Ok(Vec::new())
}

fn start_heartbeat(pool: PgPool, transaction_id: Uuid, heartbeat: Duration) -> Arc<HoldLease> {
    let task = tokio::spawn(async move {
        let mut interval = time::interval(heartbeat);
        loop {
            interval.tick().await;
            match renew(&pool, transaction_id).await {
                Ok(false) => break,
                Ok(true) => {}
                Err(err) => tracing::warn!(%transaction_id, "failed to renew billing hold: {err}"),
            }
        }
    });
    Arc::new(HoldLease { task })
}

async fn renew(pool: &PgPool, transaction_id: Uuid) -> AppResult<bool> {
    let result = sqlx::query(
        "UPDATE billing_hold SET expires_at = clock_timestamp() + make_interval(secs => $2)
         WHERE transaction_id = $1",
    )
    .bind(transaction_id)
    .bind(HOLD_LEASE_SECONDS as f64)
    .execute(pool)
    .await?;
    Ok(result.rows_affected() > 0)
}

pub(crate) async fn complete(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    transaction_id: Uuid,
) -> AppResult<()> {
    sqlx::query("DELETE FROM billing_hold WHERE transaction_id = $1")
        .bind(transaction_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::billing::{CreditAccountId, DebitPart};

    #[sqlx::test(migrations = "./migrations")]
    #[ignore = "requires PostgreSQL; run with DATABASE_URL and --ignored"]
    async fn live_hold_heartbeat_renews_lease_and_survives_clones(pool: PgPool) {
        let account_id:i64=sqlx::query_scalar(
            "INSERT INTO credit_account(owner_type,owner_id,balance_micros,reserved_micros) VALUES ('project',99003,100,100) RETURNING id",
        ).fetch_one(&pool).await.unwrap();
        let allocation_id:i64=sqlx::query_scalar(
            "INSERT INTO credit_allocation(credit_account_id,amount_micros) VALUES ($1,100) RETURNING id",
        ).bind(account_id).fetch_one(&pool).await.unwrap();
        let mut hold = DebitHold::new(
            20,
            vec![DebitPart {
                credit_account: CreditAccountId::new(account_id),
                allocation_id,
                amount_micros: 20,
            }],
            true,
        );
        assert!(register(&pool, &mut hold).await.unwrap().is_empty());
        hold.lease.take();
        sqlx::query("UPDATE billing_hold SET expires_at=clock_timestamp()-interval '1 second' WHERE transaction_id=$1")
            .bind(hold.transaction_id).execute(&pool).await.unwrap();
        hold.lease = Some(start_heartbeat(
            pool.clone(),
            hold.transaction_id,
            Duration::from_millis(10),
        ));
        let clone = hold.clone();
        let transaction_id = hold.transaction_id;
        drop(hold);
        time::timeout(Duration::from_secs(3),async {
            loop {
                let fresh:bool=sqlx::query_scalar("SELECT expires_at>clock_timestamp()+interval '60 seconds' FROM billing_hold WHERE transaction_id=$1")
                    .bind(transaction_id).fetch_one(&pool).await.unwrap();
                if fresh { break; }
                time::sleep(Duration::from_millis(10)).await;
            }
        }).await.expect("a cloned live hold must keep renewing");
        drop(clone);
        sqlx::query("UPDATE billing_hold SET expires_at=clock_timestamp()-interval '1 second' WHERE transaction_id=$1")
            .bind(transaction_id).execute(&pool).await.unwrap();
        time::sleep(Duration::from_millis(30)).await;
        let expired: bool = sqlx::query_scalar(
            "SELECT expires_at<=clock_timestamp() FROM billing_hold WHERE transaction_id=$1",
        )
        .bind(transaction_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(expired, "dropping the final hold must stop its heartbeat");
    }
}
