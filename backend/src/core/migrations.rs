use sha2::{Digest, Sha384};
use sqlx::PgPool;

/// Only accept the three exact historical brand-renaming variants shipped by
/// this project. Unknown modifications still fail SQLx's normal checksum check.
/// Migration 14 then normalizes the corresponding task constraint and rows.
pub async fn run(pool: &PgPool) -> anyhow::Result<()> {
    reconcile_known_brand_variants(pool).await?;
    sqlx::migrate!("./migrations").run(pool).await?;
    Ok(())
}

async fn reconcile_known_brand_variants(pool: &PgPool) -> anyhow::Result<()> {
    let mut tx = pool.begin().await?;
    sqlx::query("SELECT pg_advisory_xact_lock(728904133)")
        .execute(&mut *tx)
        .await?;
    let exists: bool = sqlx::query_scalar("SELECT to_regclass('_sqlx_migrations') IS NOT NULL")
        .fetch_one(&mut *tx)
        .await?;
    if exists {
        for (version, canonical) in historical_sources() {
            let legacy = canonical.replace("zhiqiaoapi_response", "neogate_response");
            let legacy_checksum = Sha384::digest(legacy.as_bytes()).to_vec();
            let canonical_checksum = Sha384::digest(canonical.as_bytes()).to_vec();
            let result = sqlx::query(
                "UPDATE _sqlx_migrations SET checksum=$1
                 WHERE version=$2 AND success AND checksum=$3",
            )
            .bind(canonical_checksum)
            .bind(version)
            .bind(legacy_checksum)
            .execute(&mut *tx)
            .await?;
            if result.rows_affected() > 0 {
                tracing::info!(
                    version,
                    "reconciled known historical task-brand migration variant"
                );
            }
        }
    }
    tx.commit().await?;
    Ok(())
}

fn historical_sources() -> [(i64, &'static str); 3] {
    [
        (1, include_str!("../../migrations/0001_initial.sql")),
        (
            6,
            include_str!("../../migrations/0006_openai_video_tasks.sql"),
        ),
        (
            7,
            include_str!("../../migrations/0007_audio_transcriptions.sql"),
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[sqlx::test(migrations = false)]
    #[ignore = "requires PostgreSQL"]
    async fn known_checksums_are_reconciled_but_unknown_edits_are_not(pool: PgPool) {
        sqlx::query("CREATE TABLE _sqlx_migrations (version BIGINT PRIMARY KEY,checksum BYTEA NOT NULL,success BOOLEAN NOT NULL)")
            .execute(&pool).await.unwrap();
        for (version, canonical) in historical_sources() {
            let legacy = canonical.replace("zhiqiaoapi_response", "neogate_response");
            sqlx::query("INSERT INTO _sqlx_migrations VALUES ($1,$2,true)")
                .bind(version)
                .bind(Sha384::digest(legacy.as_bytes()).to_vec())
                .execute(&pool)
                .await
                .unwrap();
        }
        reconcile_known_brand_variants(&pool).await.unwrap();
        for (version, canonical) in historical_sources() {
            let checksum: Vec<u8> =
                sqlx::query_scalar("SELECT checksum FROM _sqlx_migrations WHERE version=$1")
                    .bind(version)
                    .fetch_one(&pool)
                    .await
                    .unwrap();
            assert_eq!(checksum, Sha384::digest(canonical.as_bytes()).to_vec());
        }
        sqlx::query("UPDATE _sqlx_migrations SET checksum=decode('abcdef','hex') WHERE version=1")
            .execute(&pool)
            .await
            .unwrap();
        reconcile_known_brand_variants(&pool).await.unwrap();
        let checksum: Vec<u8> =
            sqlx::query_scalar("SELECT checksum FROM _sqlx_migrations WHERE version=1")
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(checksum, vec![0xab, 0xcd, 0xef]);
    }
}
