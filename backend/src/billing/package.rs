//! Package grants use the same durable allocations, leases, ledger and recovery
//! as monetary credit, but never borrow from a key or project wallet.
use super::{CreditAccountId, DebitHold, DebitPart};
use crate::{
    error::{AppError, AppResult},
    id::DbId,
};
use chrono::{DateTime, Utc};
use serde::Serialize;
use sqlx::{PgPool, Postgres, Row, Transaction};
use uuid::Uuid;

const UNLIMITED_TOTAL: i64 = 9_000_000_000_000_000;

pub(crate) async fn grant(
    tx: &mut Transaction<'_, Postgres>,
    user_id: DbId,
    package_id: DbId,
    order_id: Option<Uuid>,
    redemption_id: Option<DbId>,
) -> AppResult<()> {
    let id: DbId = sqlx::query_scalar(
        "INSERT INTO user_package_grant(user_id,package_id,order_id,redemption_id,package_name,
         total_quota_micros,daily_quota_micros,weekly_quota_micros,expires_at)
         SELECT $1,id,$3,$4,name,total_quota_micros,daily_quota_micros,weekly_quota_micros,
         now()+make_interval(days=>duration_days) FROM user_package WHERE id=$2 RETURNING id",
    )
    .bind(user_id)
    .bind(package_id)
    .bind(order_id)
    .bind(redemption_id)
    .fetch_one(&mut **tx)
    .await?;
    sqlx::query(
        "INSERT INTO credit_account(owner_type,owner_id,owner_user_id,owner_name,balance_micros)
         SELECT 'package_grant',id,user_id,package_name,COALESCE(total_quota_micros,$2)
         FROM user_package_grant WHERE id=$1",
    )
    .bind(id)
    .bind(UNLIMITED_TOTAL)
    .execute(&mut **tx)
    .await?;
    Ok(())
}

// Natural days and Monday-based weeks, both in Asia/Shanghai. In-flight
// reservations count against both limits, even across a period boundary.
const PERIOD_USAGE: &str = "SELECT
    COALESCE(sum(CASE WHEN created_at >= date_trunc('day',now() AT TIME ZONE 'Asia/Shanghai') AT TIME ZONE 'Asia/Shanghai' THEN consumed_micros ELSE 0 END
      + CASE WHEN status='active' THEN amount_micros-consumed_micros-returned_micros ELSE 0 END),0)::BIGINT AS daily_used,
    COALESCE(sum(CASE WHEN created_at >= date_trunc('week',now() AT TIME ZONE 'Asia/Shanghai') AT TIME ZONE 'Asia/Shanghai' THEN consumed_micros ELSE 0 END
      + CASE WHEN status='active' THEN amount_micros-consumed_micros-returned_micros ELSE 0 END),0)::BIGINT AS weekly_used
    FROM credit_allocation WHERE credit_account_id=$1";

fn remaining(
    balance: i64,
    reserved: i64,
    daily: Option<i64>,
    weekly: Option<i64>,
    day_used: i64,
    week_used: i64,
) -> i64 {
    let mut available = (balance - reserved).max(0);
    if let Some(limit) = daily {
        available = available.min((limit - day_used).max(0));
    }
    if let Some(limit) = weekly {
        available = available.min((limit - week_used).max(0));
    }
    available
}

/// None means a legacy/monetary key; Some always means package-only billing.
pub(crate) async fn reserve(
    pool: &PgPool,
    user_id: DbId,
    user_key_id: DbId,
    amount: i64,
) -> AppResult<Option<DebitHold>> {
    let package_id: Option<DbId> = sqlx::query_scalar(
        "SELECT p.id FROM user_key k JOIN user_package p ON p.id=k.package_id
         WHERE k.id=$1 AND k.user_id=$2 AND p.billing_mode='quota'",
    )
    .bind(user_key_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?;
    let Some(package_id) = package_id else {
        return Ok(None);
    };
    let mut tx = pool.begin().await?;
    // Wallet rows are locked in the same order as settlement. Usage is read
    // after taking the lock, so concurrent requests cannot overspend a limit.
    let rows = sqlx::query(
        "SELECT g.*,w.id AS account_id,w.balance_micros,w.reserved_micros
         FROM user_package_grant g JOIN credit_account w ON w.owner_type='package_grant' AND w.owner_id=g.id
         JOIN user_package p ON p.id=g.package_id
         WHERE g.user_id=$1 AND g.package_id=$2 AND p.service_enabled
           AND g.starts_at<=now() AND g.expires_at>now()
         ORDER BY w.id FOR UPDATE OF w")
        .bind(user_id).bind(package_id).fetch_all(&mut *tx).await?;
    if rows.is_empty() {
        return Err(AppError::Forbidden("套餐尚未开通或已到期".into()));
    }
    let mut candidates = Vec::new();
    let mut available_total = 0_i64;
    for row in rows {
        let account_id: DbId = row.try_get("account_id")?;
        let used = sqlx::query(PERIOD_USAGE)
            .bind(account_id)
            .fetch_one(&mut *tx)
            .await?;
        let available = remaining(
            row.try_get("balance_micros")?,
            row.try_get("reserved_micros")?,
            row.try_get("daily_quota_micros")?,
            row.try_get("weekly_quota_micros")?,
            used.try_get("daily_used")?,
            used.try_get("weekly_used")?,
        );
        available_total = available_total.saturating_add(available);
        candidates.push((
            row.try_get::<DateTime<Utc>, _>("expires_at")?,
            account_id,
            available,
        ));
    }
    if available_total < amount.max(0) {
        return Err(AppError::InsufficientQuota {
            available_micros: available_total,
            required_micros: amount,
        });
    }
    candidates.sort_by_key(|(expires, id, _)| (*expires, *id));
    let mut needed = amount.max(0);
    let mut parts = Vec::new();
    for (_, account_id, available) in candidates {
        let take = available.min(needed);
        if take <= 0 {
            continue;
        }
        sqlx::query("UPDATE credit_account SET reserved_micros=reserved_micros+$2,updated_at=now() WHERE id=$1")
            .bind(account_id).bind(take).execute(&mut *tx).await?;
        let allocation_id = sqlx::query_scalar("INSERT INTO credit_allocation(credit_account_id,amount_micros) VALUES($1,$2) RETURNING id")
            .bind(account_id).bind(take).fetch_one(&mut *tx).await?;
        parts.push(DebitPart {
            credit_account: CreditAccountId::new(account_id),
            allocation_id,
            amount_micros: take,
        });
        needed -= take;
        if needed == 0 {
            break;
        }
    }
    tx.commit().await?;
    Ok(Some(DebitHold::new(amount.max(0), parts, true)))
}

#[derive(Debug, Serialize)]
pub(crate) struct GrantRecord {
    pub id: DbId,
    pub package_id: DbId,
    pub package_name: String,
    pub starts_at: DateTime<Utc>,
    pub expires_at: DateTime<Utc>,
    pub active: bool,
    pub total_quota_micros: Option<i64>,
    pub daily_quota_micros: Option<i64>,
    pub weekly_quota_micros: Option<i64>,
    pub total_used_micros: i64,
    pub reserved_micros: i64,
    pub daily_used_micros: i64,
    pub weekly_used_micros: i64,
    pub available_micros: i64,
}
pub(crate) async fn grants(pool: &PgPool, user_id: DbId) -> AppResult<Vec<GrantRecord>> {
    let rows = sqlx::query(
        "SELECT g.*,w.id AS account_id,w.balance_micros,w.reserved_micros,
         g.expires_at>now() AND p.service_enabled AS active FROM user_package_grant g
         JOIN credit_account w ON w.owner_type='package_grant' AND w.owner_id=g.id
         JOIN user_package p ON p.id=g.package_id WHERE g.user_id=$1 ORDER BY g.expires_at DESC,g.id DESC")
        .bind(user_id).fetch_all(pool).await?;
    let mut result = Vec::new();
    for row in rows {
        let usage = sqlx::query(PERIOD_USAGE)
            .bind(row.try_get::<DbId, _>("account_id")?)
            .fetch_one(pool)
            .await?;
        let total: Option<i64> = row.try_get("total_quota_micros")?;
        let daily = row.try_get("daily_quota_micros")?;
        let weekly = row.try_get("weekly_quota_micros")?;
        let balance = row.try_get("balance_micros")?;
        let reserved = row.try_get("reserved_micros")?;
        let day_used = usage.try_get("daily_used")?;
        let week_used = usage.try_get("weekly_used")?;
        let active = row.try_get("active")?;
        result.push(GrantRecord {
            id: row.try_get("id")?,
            package_id: row.try_get("package_id")?,
            package_name: row.try_get("package_name")?,
            starts_at: row.try_get("starts_at")?,
            expires_at: row.try_get("expires_at")?,
            active,
            total_quota_micros: total,
            daily_quota_micros: daily,
            weekly_quota_micros: weekly,
            total_used_micros: total.unwrap_or(UNLIMITED_TOTAL) - balance,
            reserved_micros: reserved,
            daily_used_micros: day_used,
            weekly_used_micros: week_used,
            available_micros: if active {
                remaining(balance, reserved, daily, weekly, day_used, week_used)
            } else {
                0
            },
        });
    }
    Ok(result)
}

#[cfg(test)]
mod tests {
    use super::remaining;
    #[test]
    fn package_limits_use_the_strictest_remaining_dimension() {
        assert_eq!(remaining(1000, 10, Some(100), Some(500), 40, 490), 10);
        assert_eq!(remaining(1000, 10, Some(100), None, 100, 0), 0);
        assert_eq!(remaining(20, 10, None, None, 0, 0), 10);
        assert_eq!(remaining(1000, 0, None, Some(50), 0, 100), 0);
        assert_eq!(remaining(1000, 0, Some(100), Some(500), 0, 0), 100);
    }
}
