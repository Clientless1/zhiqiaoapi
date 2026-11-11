use std::{collections::BTreeMap, sync::Arc};

use axum::{extract::State, http::header, response::IntoResponse, Json};
use chrono::{DateTime, Duration, Utc};
use serde::Serialize;
use sqlx::{PgPool, Row};

use crate::{billing::BILLABLE_PRICE_CONDITION_CP, error::AppResult, AppState};

const WINDOW_HOURS: usize = 24;
const FRESH_MINUTES: i64 = 30;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
enum HealthStatus {
    Operational,
    Degraded,
    Outage,
    Unknown,
}

fn status_for_counts(samples: i64, successes: i64) -> HealthStatus {
    match (samples, successes) {
        (0, _) => HealthStatus::Unknown,
        (_, 0) => HealthStatus::Outage,
        (all, ok) if all == ok => HealthStatus::Operational,
        _ => HealthStatus::Degraded,
    }
}

#[derive(Debug, Serialize)]
struct HistoryBucket {
    start_at: DateTime<Utc>,
    status: HealthStatus,
    sample_count: i64,
    success_count: i64,
    latency_ms: Option<f64>,
}

#[derive(Debug, Serialize)]
struct ModelHealth {
    provider: String,
    model: String,
    status: HealthStatus,
    availability_percent: Option<f64>,
    /// Mean completed successful probe duration, not time to first token.
    latency_ms: Option<f64>,
    /// The existing probe does not record generated tokens. Never infer a rate.
    tokens_per_second: Option<f64>,
    sample_count: i64,
    last_checked_at: Option<DateTime<Utc>>,
    history: Vec<HistoryBucket>,
}

#[derive(Debug, Serialize)]
struct HealthSnapshot {
    window_hours: usize,
    bucket_minutes: usize,
    generated_at: DateTime<Utc>,
    models: Vec<ModelHealth>,
}

struct BucketRow {
    provider: String,
    model: String,
    start_at: DateTime<Utc>,
    samples: i64,
    successes: i64,
    latency_sum: Option<f64>,
    latency_count: i64,
    last_checked_at: DateTime<Utc>,
    latest_status: String,
}

pub(super) async fn public_model_health(
    State(state): State<Arc<AppState>>,
) -> AppResult<impl IntoResponse> {
    let snapshot = load_health(&state.db.pool, Utc::now()).await?;
    Ok((
        [(header::CACHE_CONTROL, "public, max-age=30")],
        Json(snapshot),
    ))
}

async fn load_health(pool: &PgPool, now: DateTime<Utc>) -> AppResult<HealthSnapshot> {
    // PostgreSQL stores microseconds; align the bucket origin before comparing
    // database timestamps so sub-microsecond rounding cannot shift a bucket.
    let now = DateTime::from_timestamp_micros(now.timestamp_micros()).unwrap_or(now);
    let published: Vec<_> = super::load_offers(pool, 1_000_000)
        .await?
        .into_iter()
        .map(|offer| (offer.provider, offer.model))
        .collect();
    let start = now - Duration::hours(WINDOW_HOURS as i64);
    // Exact probe model matching is essential: the scheduler may probe just one
    // model on a multi-model channel. No URLs, secrets, user usage, errors or
    // channel identities are projected into this public response.
    let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
        r#"SELECT c.provider, cps.model,
             date_bin(interval '1 hour', cps.created_at, $1) AS bucket_start,
             COUNT(*)::bigint AS samples,
             COUNT(*) FILTER (WHERE cps.status='ok')::bigint AS successes,
             (SUM(cps.latency_ms) FILTER (WHERE cps.status='ok' AND cps.latency_ms >= 0))::float8 AS latency_sum,
             COUNT(cps.latency_ms) FILTER (WHERE cps.status='ok' AND cps.latency_ms >= 0)::bigint AS latency_count,
             MAX(cps.created_at) AS last_checked_at,
             (ARRAY_AGG(cps.status ORDER BY cps.created_at DESC, cps.id DESC))[1] AS latest_status
           FROM channel_probe cps
           JOIN channel c ON c.id=cps.channel_id AND c.enabled
           JOIN provider p ON p.code=c.provider AND p.enabled
           WHERE cps.created_at >= $1 AND cps.created_at <= $2
             AND cps.status IN ('ok','failed')
             AND EXISTS (
               SELECT 1 FROM channel_price cp
               JOIN channel_model cm ON cm.channel_id=cp.channel_id AND cm.model=cp.model
               WHERE cp.channel_id=c.id AND cp.model=cps.model
                 AND cp.enabled AND {BILLABLE_PRICE_CONDITION_CP}
                 AND cm.enabled AND cm.status='available'
             )
           GROUP BY c.provider,cps.model,bucket_start
           ORDER BY c.provider,cps.model,bucket_start"#
    )))
    .bind(start)
    .bind(now)
    .fetch_all(pool)
    .await?;
    let mut buckets = Vec::with_capacity(rows.len());
    for row in rows {
        buckets.push(BucketRow {
            provider: row.try_get("provider")?,
            model: row.try_get("model")?,
            start_at: row.try_get("bucket_start")?,
            samples: row.try_get("samples")?,
            successes: row.try_get("successes")?,
            latency_sum: row.try_get("latency_sum")?,
            latency_count: row.try_get("latency_count")?,
            last_checked_at: row.try_get("last_checked_at")?,
            latest_status: row.try_get("latest_status")?,
        });
    }
    Ok(assemble_snapshot(published, buckets, now))
}

fn assemble_snapshot(
    published: Vec<(String, String)>,
    rows: Vec<BucketRow>,
    now: DateTime<Utc>,
) -> HealthSnapshot {
    let start = now - Duration::hours(WINDOW_HOURS as i64);
    let mut models: BTreeMap<_, _> = published
        .into_iter()
        .map(|(provider, model)| {
            let value = ModelHealth {
                provider: provider.clone(),
                model: model.clone(),
                status: HealthStatus::Unknown,
                availability_percent: None,
                latency_ms: None,
                tokens_per_second: None,
                sample_count: 0,
                last_checked_at: None,
                history: (0..WINDOW_HOURS)
                    .map(|hour| HistoryBucket {
                        start_at: start + Duration::hours(hour as i64),
                        status: HealthStatus::Unknown,
                        sample_count: 0,
                        success_count: 0,
                        latency_ms: None,
                    })
                    .collect(),
            };
            ((provider, model), value)
        })
        .collect();
    let mut totals: BTreeMap<(String, String), (i64, f64, i64)> = BTreeMap::new();
    for row in rows {
        let key = (row.provider, row.model);
        let Some(model) = models.get_mut(&key) else {
            continue;
        };
        let hour = (row.start_at - start).num_hours();
        if !(0..WINDOW_HOURS as i64).contains(&hour) {
            continue;
        }
        let bucket = &mut model.history[hour as usize];
        bucket.sample_count = row.samples;
        bucket.success_count = row.successes;
        bucket.status = status_for_counts(row.samples, row.successes);
        bucket.latency_ms = row
            .latency_sum
            .filter(|_| row.latency_count > 0)
            .map(|sum| sum / row.latency_count as f64);
        model.sample_count += row.samples;
        let total = totals.entry(key).or_default();
        total.0 += row.successes;
        total.1 += row.latency_sum.unwrap_or(0.0);
        total.2 += row.latency_count;
        if model
            .last_checked_at
            .is_none_or(|last| row.last_checked_at > last)
        {
            model.last_checked_at = Some(row.last_checked_at);
            model.status = if now - row.last_checked_at > Duration::minutes(FRESH_MINUTES) {
                HealthStatus::Unknown
            } else if row.latest_status == "failed" {
                if row.successes == 0 {
                    HealthStatus::Outage
                } else {
                    HealthStatus::Degraded
                }
            } else {
                HealthStatus::Operational
            };
        }
    }
    for (key, model) in &mut models {
        if let Some((successes, latency_sum, latency_count)) = totals.get(key) {
            model.availability_percent = (model.sample_count > 0)
                .then(|| *successes as f64 / model.sample_count as f64 * 100.0);
            model.latency_ms = (*latency_count > 0).then(|| *latency_sum / *latency_count as f64);
        }
    }
    HealthSnapshot {
        window_hours: WINDOW_HOURS,
        bucket_minutes: 60,
        generated_at: now,
        models: models.into_values().collect(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn row(now: DateTime<Utc>, model: &str, samples: i64, successes: i64) -> BucketRow {
        BucketRow {
            provider: "openai".into(),
            model: model.into(),
            start_at: now - Duration::hours(1),
            samples,
            successes,
            latency_sum: Some(successes as f64 * 200.0),
            latency_count: successes,
            last_checked_at: now - Duration::minutes(2),
            latest_status: "ok".into(),
        }
    }

    #[test]
    fn never_reuses_another_models_probe_or_invents_throughput() {
        let now = Utc::now();
        let snapshot = assemble_snapshot(
            vec![
                ("openai".into(), "model-a".into()),
                ("openai".into(), "model-b".into()),
            ],
            vec![row(now, "model-a", 4, 3)],
            now,
        );
        assert_eq!(snapshot.models[0].availability_percent, Some(75.0));
        assert_eq!(snapshot.models[0].latency_ms, Some(200.0));
        assert_eq!(snapshot.models[1].availability_percent, None);
        assert_eq!(snapshot.models[1].status, HealthStatus::Unknown);
        assert!(snapshot
            .models
            .iter()
            .all(|model| model.tokens_per_second.is_none()));
        assert_eq!(snapshot.models[1].history.len(), 24);
        assert!(snapshot.models[1]
            .history
            .iter()
            .all(|bucket| bucket.status == HealthStatus::Unknown));
    }

    #[test]
    fn excludes_unpublished_models_and_old_current_status() {
        let now = Utc::now();
        let mut stale = row(now, "model-a", 3, 3);
        stale.last_checked_at = now - Duration::minutes(31);
        let snapshot = assemble_snapshot(
            vec![("openai".into(), "model-a".into())],
            vec![stale, row(now, "private-model", 1, 1)],
            now,
        );
        assert_eq!(snapshot.models.len(), 1);
        assert_eq!(snapshot.models[0].status, HealthStatus::Unknown);
        assert_eq!(snapshot.models[0].availability_percent, Some(100.0));
    }

    #[test]
    fn distinguishes_missing_failed_and_mixed_history() {
        assert_eq!(status_for_counts(0, 0), HealthStatus::Unknown);
        assert_eq!(status_for_counts(2, 0), HealthStatus::Outage);
        assert_eq!(status_for_counts(2, 1), HealthStatus::Degraded);
        assert_eq!(status_for_counts(2, 2), HealthStatus::Operational);
    }

    #[sqlx::test(migrations = "./migrations")]
    #[ignore = "requires PostgreSQL"]
    async fn public_health_uses_only_exact_published_model_probe_samples(pool: PgPool) {
        let now = Utc::now();
        sqlx::raw_sql(r#"
            INSERT INTO channel (id,provider,name) VALUES
              (93001,'openai','visible channel'),(93002,'openai','hidden channel');
            INSERT INTO channel_endpoint (channel_id,protocol,base_url) VALUES
              (93001,'openai','https://secret-origin.invalid');
            INSERT INTO channel_key (channel_id,name,key_prefix,secret_ciphertext) VALUES
              (93001,'secret-key','private','encrypted-private-key');
            INSERT INTO channel_model (channel_id,model,enabled) VALUES
              (93001,'health-model-a',true),(93001,'health-model-b',true),
              (93001,'health-private-model',true),(93002,'health-model-a',true);
            INSERT INTO channel_price (channel_id,model,input_price_micros,output_price_micros,enabled) VALUES
              (93001,'health-model-a',1000000,1000000,true),
              (93001,'health-model-b',1000000,1000000,true),
              (93001,'health-private-model',1000000,1000000,false),
              (93002,'health-model-a',1000000,1000000,false);
        "#).execute(&pool).await.unwrap();
        for (channel, model, status, latency, minutes) in [
            (93001, "health-model-a", "ok", 200, 20),
            (93001, "health-model-a", "failed", 800, 10),
            (93001, "health-model-a", "ok", 400, 5),
            (93001, "health-model-a", "skipped", 0, 4),
            (93001, "health-model-a", "failed", 900, 1500),
            (93001, "health-private-model", "ok", 1, 3),
            (93002, "health-model-a", "ok", 1, 2),
        ] {
            sqlx::query("INSERT INTO channel_probe(channel_id,protocol,model,status,latency_ms,error_summary,created_at) VALUES($1,'openai',$2,$3,$4,'private-upstream-error',$5)")
              .bind(channel as i64).bind(model).bind(status).bind(latency as i64)
              .bind(now-Duration::minutes(minutes)).execute(&pool).await.unwrap();
        }
        let snapshot = load_health(&pool, now).await.unwrap();
        assert_eq!(snapshot.models.len(), 2);
        let model = &snapshot.models[0];
        assert_eq!(model.model, "health-model-a");
        assert_eq!(model.sample_count, 3);
        assert!((model.availability_percent.unwrap() - 200.0 / 3.0).abs() < 0.001);
        assert_eq!(model.latency_ms, Some(300.0));
        assert_eq!(model.status, HealthStatus::Operational);
        assert_eq!(model.history.last().unwrap().status, HealthStatus::Degraded);
        assert_eq!(snapshot.models[1].sample_count, 0);
        assert_eq!(snapshot.models[1].status, HealthStatus::Unknown);
        let json = serde_json::to_string(&snapshot).unwrap();
        for private in [
            "secret-origin",
            "private-upstream-error",
            "channel_id",
            "channel_key",
            "health-private-model",
        ] {
            assert!(!json.contains(private), "leaked {private}");
        }
    }
}
