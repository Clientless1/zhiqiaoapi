use std::time::Duration;

use sqlx::{PgPool, Row};

use crate::{
    auth::{generate_user_key, hash_key},
    error::{AppError, AppResult},
    id::DbId,
};

/// Only the random credential's digest is persisted; email addresses and signed
/// legacy reset links alone can no longer authorize a password write.
pub(super) async fn issue(pool: &PgPool, email: &str, ttl: Duration) -> AppResult<Option<String>> {
    let mut tx = pool.begin().await?;
    let user = sqlx::query(
        "SELECT id, auth_version FROM \"user\" WHERE email=$1 AND status='enabled' FOR UPDATE",
    )
    .bind(email)
    .fetch_optional(&mut *tx)
    .await?;
    let Some(user) = user else { return Ok(None) };
    let user_id: DbId = user.try_get("id")?;
    sqlx::query("DELETE FROM user_password_reset WHERE user_id=$1")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    let token = format!("neo_reset_v2_{}", generate_user_key());
    sqlx::query(
        "INSERT INTO user_password_reset (token_hash,user_id,auth_version,expires_at)
         VALUES ($1,$2,$3,now()+make_interval(secs => $4))",
    )
    .bind(hash_key(&token))
    .bind(user_id)
    .bind(user.try_get::<i64, _>("auth_version")?)
    .bind(ttl.as_secs_f64())
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(Some(token))
}

pub(super) async fn consume(pool: &PgPool, token: &str, password_hash: &str) -> AppResult<()> {
    if !token.starts_with("neo_reset_v2_") || token.len() > 128 {
        return Err(AppError::Unauthorized);
    }
    let digest = hash_key(token);
    let mut tx = pool.begin().await?;
    let user_id = sqlx::query_scalar::<_, DbId>(
        "SELECT user_id FROM user_password_reset WHERE token_hash=$1",
    )
    .bind(&digest)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::Unauthorized)?;
    // The user is always locked before reset credentials, matching issue().
    let version = sqlx::query_scalar::<_, i64>(
        "SELECT auth_version FROM \"user\" WHERE id=$1 AND status='enabled' FOR UPDATE",
    )
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(AppError::Unauthorized)?;
    let consumed = sqlx::query(
        "UPDATE user_password_reset SET consumed_at=now()
         WHERE token_hash=$1 AND user_id=$2 AND auth_version=$3
           AND consumed_at IS NULL AND expires_at>now()",
    )
    .bind(digest)
    .bind(user_id)
    .bind(version)
    .execute(&mut *tx)
    .await?;
    if consumed.rows_affected() != 1 {
        return Err(AppError::Unauthorized);
    }
    sqlx::query(
        "UPDATE \"user\" SET password_hash=$2,password_changed_at=now(),updated_at=now() WHERE id=$1",
    )
    .bind(user_id)
    .bind(password_hash)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[sqlx::test(migrations = "./migrations")]
    #[ignore = "requires PostgreSQL"]
    async fn reset_is_single_use_expires_and_is_revoked_by_password_changes(pool: PgPool) {
        let id: DbId = sqlx::query_scalar(
            "INSERT INTO \"user\" (email,password_hash,user_group_id) VALUES ('reset@example.com','initial',(SELECT id FROM user_group WHERE is_default)) RETURNING id",
        ).fetch_one(&pool).await.unwrap();
        let token = issue(&pool, "reset@example.com", Duration::from_secs(60))
            .await
            .unwrap()
            .unwrap();
        let (a, b) = tokio::join!(
            consume(&pool, &token, "changed-a"),
            consume(&pool, &token, "changed-b")
        );
        assert_ne!(
            a.is_ok(),
            b.is_ok(),
            "exactly one concurrent reset must commit"
        );
        assert!(consume(&pool, &token, "replayed").await.is_err());
        let version: i64 = sqlx::query_scalar("SELECT auth_version FROM \"user\" WHERE id=$1")
            .bind(id)
            .fetch_one(&pool)
            .await
            .unwrap();
        assert_eq!(version, 1);

        let old = issue(&pool, "reset@example.com", Duration::from_secs(60))
            .await
            .unwrap()
            .unwrap();
        sqlx::query("UPDATE \"user\" SET password_hash='admin-password-reset' WHERE id=$1")
            .bind(id)
            .execute(&pool)
            .await
            .unwrap();
        assert!(consume(&pool, &old, "must-not-work").await.is_err());
        let expired = issue(&pool, "reset@example.com", Duration::ZERO)
            .await
            .unwrap()
            .unwrap();
        assert!(consume(&pool, &expired, "expired").await.is_err());
        let first = issue(&pool, "reset@example.com", Duration::from_secs(60))
            .await
            .unwrap()
            .unwrap();
        let latest = issue(&pool, "reset@example.com", Duration::from_secs(60))
            .await
            .unwrap()
            .unwrap();
        assert_ne!(first, latest);
        assert!(consume(&pool, &first, "superseded").await.is_err());
        assert!(consume(&pool, &latest, "latest").await.is_ok());
    }
}
