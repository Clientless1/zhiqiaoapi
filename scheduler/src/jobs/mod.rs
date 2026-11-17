use std::time::{Duration, Instant};

use crate::{app::AppContext, config::Config};

mod channel_probe;
mod upstream_models;

pub(crate) struct JobCadence {
    next_channel_probe: Instant,
    next_upstream_models: Instant,
}

impl JobCadence {
    pub fn new(now: Instant, config: &Config) -> Self {
        Self {
            next_channel_probe: now,
            next_upstream_models: now + spread_initial_delay(config.upstream_models_interval),
        }
    }
}

pub(crate) async fn run_due(context: &AppContext, cadence: &mut JobCadence) -> anyhow::Result<()> {
    let now = Instant::now();

    if context.config.channel_probe_enabled && now >= cadence.next_channel_probe {
        channel_probe::run(context).await?;
        cadence.next_channel_probe = now + context.config.channel_probe_interval;
    }

    if now >= cadence.next_upstream_models {
        upstream_models::run(context).await?;
        cadence.next_upstream_models = now + context.config.upstream_models_interval;
    }

    Ok(())
}

fn spread_initial_delay(interval: Duration) -> Duration {
    interval.min(Duration::from_secs(15))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[tokio::test]
    async fn disabled_probes_make_no_database_or_upstream_requests_even_when_due() {
        let config = Config {
            database_url: "postgres://test@127.0.0.1:1/test".into(),
            redis_url: None,
            redis_key_prefix: "test".into(),
            upstream_secret_key: "test-secret-for-disabled-probe-check".into(),
            tick_interval: Duration::from_millis(1),
            channel_probe_enabled: false,
            channel_probe_interval: Duration::from_secs(1),
            upstream_models_interval: Duration::from_secs(86400),
            upstream_connect_timeout: Duration::from_secs(1),
            upstream_timeout: Duration::from_secs(1),
        };
        let context = AppContext {
            db: sqlx::postgres::PgPoolOptions::new()
                .acquire_timeout(Duration::from_millis(50))
                .connect_lazy(&config.database_url)
                .unwrap(),
            http: reqwest::Client::new(),
            secrets: crate::secrets::SecretStore::new(&config.upstream_secret_key),
            cache_invalidator: crate::cache::CacheInvalidator::new(None, "test")
                .await
                .unwrap(),
            config: Arc::new(config),
        };
        let mut cadence = JobCadence::new(Instant::now(), &context.config);
        cadence.next_channel_probe = Instant::now() - Duration::from_secs(600);
        for _ in 0..3 {
            run_due(&context, &mut cadence).await.unwrap();
        }
        // Model catalog synchronization retains its own schedule.
        cadence.next_upstream_models = Instant::now() - Duration::from_secs(1);
        assert!(run_due(&context, &mut cadence).await.is_err());
    }
}
