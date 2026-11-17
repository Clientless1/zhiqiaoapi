use std::{env, time::Duration};

use anyhow::{Context, Result};

const DEFAULT_UPSTREAM_SECRET_KEY: &str = "change-me-upstream-secret-key-in-production";

#[derive(Clone, Debug)]
pub(crate) struct Config {
    pub database_url: String,
    pub redis_url: Option<String>,
    pub redis_key_prefix: String,
    pub upstream_secret_key: String,
    pub tick_interval: Duration,
    pub channel_probe_enabled: bool,
    pub channel_probe_interval: Duration,
    pub upstream_models_interval: Duration,
    pub upstream_connect_timeout: Duration,
    pub upstream_timeout: Duration,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            database_url: required("DATABASE_URL")?,
            redis_url: optional("REDIS_URL"),
            redis_key_prefix: "zhiqiaoapi".to_string(),
            upstream_secret_key: required_secret("UPSTREAM_SECRET_KEY")?,
            tick_interval: duration_ms("SCHEDULER_TICK_INTERVAL_MS", 1_000)?,
            channel_probe_enabled: parse_probe_enabled(
                env::var("CHANNEL_PROBE_ENABLED").ok().as_deref(),
            )?,
            channel_probe_interval: duration_secs_with_alias(
                "CHANNEL_PROBE_INTERVAL_SECONDS",
                "UPSTREAM_HEALTH_CHECK_INTERVAL_SECONDS",
                600,
            )?,
            upstream_models_interval: duration_secs(
                "UPSTREAM_MODEL_SYNC_INTERVAL_SECONDS",
                86_400,
            )?,
            upstream_connect_timeout: Duration::from_secs(10),
            upstream_timeout: duration_secs("UPSTREAM_TIMEOUT_SECONDS", 60)?,
        })
    }
}

fn optional(name: &str) -> Option<String> {
    env::var(name)
        .map(|value| value.trim().to_string())
        .ok()
        .filter(|value| !value.is_empty())
}

fn required(name: &str) -> Result<String> {
    env::var(name)
        .map(|value| value.trim().to_string())
        .ok()
        .filter(|value| !value.is_empty())
        .with_context(|| format!("{name} is required"))
}

fn required_secret(name: &str) -> Result<String> {
    let value = required(name)?;
    if value == DEFAULT_UPSTREAM_SECRET_KEY || value.len() < 32 {
        anyhow::bail!("{name} must be configured before scheduler can start");
    }
    Ok(value)
}

fn duration_secs(name: &str, default: u64) -> Result<Duration> {
    Ok(Duration::from_secs(parse_u64(name, default)?))
}

fn duration_secs_with_alias(name: &str, alias: &str, default: u64) -> Result<Duration> {
    Ok(Duration::from_secs(parse_u64_with_alias(
        name, alias, default,
    )?))
}

fn duration_ms(name: &str, default: u64) -> Result<Duration> {
    Ok(Duration::from_millis(parse_u64(name, default)?.max(1)))
}

fn parse_u64(name: &str, default: u64) -> Result<u64> {
    match env::var(name) {
        Ok(value) => value
            .parse()
            .with_context(|| format!("{name} must be an unsigned integer")),
        Err(env::VarError::NotPresent) => Ok(default),
        Err(err) => Err(err).with_context(|| format!("failed to read {name}")),
    }
}

fn parse_u64_with_alias(name: &str, alias: &str, default: u64) -> Result<u64> {
    if env::var(name).is_ok() {
        parse_u64(name, default)
    } else {
        parse_u64(alias, default)
    }
}

// Active probes submit generation requests and may incur upstream charges.
// Require an explicit opt-in, including on the first scheduler tick after restart.
fn parse_probe_enabled(value: Option<&str>) -> Result<bool> {
    match value.map(str::trim).map(str::to_ascii_lowercase).as_deref() {
        None | Some("") | Some("0" | "false" | "no" | "off") => Ok(false),
        Some("1" | "true" | "yes" | "on") => Ok(true),
        _ => anyhow::bail!("CHANNEL_PROBE_ENABLED must be a boolean"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn active_probes_require_explicit_opt_in() {
        for value in [
            None,
            Some(""),
            Some("  "),
            Some("false"),
            Some("0"),
            Some("off"),
            Some("no"),
        ] {
            assert!(!parse_probe_enabled(value).unwrap());
        }
        for value in ["true", "1", "yes", "on", " TRUE "] {
            assert!(parse_probe_enabled(Some(value)).unwrap());
        }
        assert!(parse_probe_enabled(Some("flase")).is_err());
    }
}
