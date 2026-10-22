use std::{sync::Arc, time::Instant};

use axum::{
    body::Body,
    extract::State,
    http::{header, HeaderValue, Method},
    response::Response,
};
use bytes::Bytes;
use serde_json::Value;
use uuid::Uuid;

use crate::{
    auth::UserAuth,
    billing::{BillableUsage, BillingAccounts, BillingMeter, SettleRequest},
    error::{reqwest_status, AppError, AppResult},
    relay::{
        enqueue_relay_usage, finish_relay, forward_openai_bound, release_empty_hold,
        reserve_billable_credit, usage_from_context, RelayBody, RelayContext,
    },
    AppState,
};

const SPEECH_PATH: &str = "/v1/audio/speech";
const MAX_SPEECH_BYTES: usize = 64 * 1024 * 1024;

pub(crate) async fn openai_audio_speech(
    State(state): State<Arc<AppState>>,
    auth: UserAuth,
    RelayBody(body): RelayBody,
) -> AppResult<Response> {
    let (mut value, requested_model, format, estimated_seconds) = speech_request(&body)?;
    let resolved = crate::project::models::resolve_project_model(
        &state.db.pool,
        auth.project_id,
        &requested_model,
    )
    .await?;
    auth.ensure_model_scope(&state, &resolved.external_model)
        .await?;
    auth.ensure_model_scope(&state, &resolved.target_model)
        .await?;
    value["model"] = Value::String(resolved.target_model.clone());
    let body = Bytes::from(serde_json::to_vec(&value)?);
    let permit = state.user_request_limiter.try_acquire(auth.user_id).await?;
    let started = Instant::now();
    let (protocol, upstream) = super::select_upstream_excluding(
        &state,
        SPEECH_PATH,
        &resolved.target_model,
        resolved.target_channel_id,
        None,
        &[],
    )
    .await?;
    let price = state
        .billing
        .price_for(
            &state.db.pool,
            upstream.channel_id,
            &resolved.target_model,
            &auth.user_group,
        )
        .await?;
    if price.billing_meter != BillingMeter::Audio {
        return Err(AppError::BadRequest(
            "TTS 模型需要配置按音频秒计费的价格".into(),
        ));
    }
    let unit_price = price
        .unit_price_micros
        .filter(|p| *p > 0)
        .ok_or_else(|| AppError::BadRequest("TTS 模型需要配置每秒单价".into()))?;
    let model_account = auth.model_credit_account(&resolved.external_model).cloned();
    let hold = reserve_billable_credit(
        &state,
        &auth,
        model_account.as_ref(),
        estimated_seconds.saturating_mul(unit_price),
    )
    .await?;
    let mut ctx = RelayContext {
        state: state.clone(),
        auth,
        upstream: upstream.clone(),
        protocol,
        method: "POST",
        path: SPEECH_PATH,
        model: resolved.target_model.clone(),
        external_model: resolved.external_model,
        upstream_model: resolved.target_model,
        routing: resolved.routing,
        streamed: false,
        price,
        hold,
        user_key_model_credit_account: model_account,
        started,
        channel_affinity_key: None,
        relay_trace_id: Uuid::new_v4(),
        relay_attempt: 1,
        relay_final: true,
        request_body_bytes: body.len(),
        request_input_tokens_estimate: 0,
        request_params: Default::default(),
        request_permit: Some(permit),
        upstream_request_path: Some(SPEECH_PATH.into()),
        upstream_response_mode: Some("audio"),
    };
    let mut response = match forward_openai_bound(
        &state,
        &upstream,
        Method::POST,
        SPEECH_PATH,
        Some(body),
    )
    .await
    {
        Ok(response) => response,
        Err(error) => return finish_relay(ctx, Err(error)).await,
    };
    let status = reqwest_status(response.status());
    if !status.is_success() {
        return finish_relay(ctx, Ok(response)).await;
    }
    let content_type = response
        .headers()
        .get(header::CONTENT_TYPE)
        .cloned()
        .unwrap_or_else(|| HeaderValue::from_static("application/octet-stream"));
    let mut bytes = Vec::new();
    loop {
        match response.chunk().await {
            Ok(Some(chunk)) if bytes.len().saturating_add(chunk.len()) <= MAX_SPEECH_BYTES => {
                bytes.extend_from_slice(&chunk)
            }
            Ok(None) => break,
            Ok(Some(_)) => {
                release_empty_hold(
                    &state,
                    ctx.hold.clone(),
                    "speech response exceeds size limit",
                )
                .await;
                return Err(AppError::UpstreamUnavailable(
                    "TTS response exceeds 64 MB".into(),
                ));
            }
            Err(error) => {
                release_empty_hold(&state, ctx.hold.clone(), "speech response read failed").await;
                return Err(error.into());
            }
        }
    }
    ctx.release_request_permit();
    let seconds = match audio_seconds(&bytes, &format) {
        Ok(seconds) => seconds,
        Err(error) => {
            release_empty_hold(
                &state,
                ctx.hold.clone(),
                "speech response audio duration unavailable",
            )
            .await;
            return Err(error);
        }
    };
    let charge = state
        .billing
        .settle(
            &state.db.pool,
            SettleRequest {
                accounts: BillingAccounts {
                    user_id: ctx.auth.user_id,
                    project_id: ctx.auth.project_id,
                    user_key_id: ctx.auth.user_key_id,
                    user_key_model_credit_account: ctx.user_key_model_credit_account.as_ref(),
                    user_key_credit_account: &ctx.auth.user_key_credit_account,
                    project_credit_account: &ctx.auth.project_credit_account,
                },
                hold: ctx.hold.clone(),
                usage: Some(BillableUsage::audio_seconds(seconds)),
                price: &ctx.price,
                allow_supplemental: true,
            },
        )
        .await;
    let charge = match charge {
        Ok(charge) => charge,
        Err(error) => {
            release_empty_hold(&state, ctx.hold.clone(), "speech billing failed").await;
            return Err(error);
        }
    };
    let usage = usage_from_context(
        &ctx,
        Some(status.as_u16() as i32),
        None,
        None,
        None,
        Some(charge),
    );
    enqueue_relay_usage(&state, usage, None).await;
    Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, content_type)
        .header("x-zhiqiaoapi-audio-seconds", seconds.to_string())
        .body(Body::from(bytes))
        .map_err(|e| AppError::BadRequest(e.to_string()))
}

fn speech_request(body: &[u8]) -> AppResult<(Value, String, String, i64)> {
    let value: Value = serde_json::from_slice(body)?;
    let model = value
        .get("model")
        .and_then(Value::as_str)
        .filter(|v| !v.trim().is_empty())
        .ok_or_else(|| AppError::BadRequest("model is required".into()))?
        .trim()
        .to_string();
    let input = value
        .get("input")
        .and_then(Value::as_str)
        .filter(|v| !v.trim().is_empty())
        .ok_or_else(|| AppError::BadRequest("input is required".into()))?;
    if input.chars().count() > 4096 {
        return Err(AppError::BadRequest(
            "TTS input must be 4096 characters or fewer".into(),
        ));
    }
    if value
        .get("voice")
        .and_then(Value::as_str)
        .is_none_or(|v| v.trim().is_empty())
    {
        return Err(AppError::BadRequest("voice is required".into()));
    }
    let speed = value
        .get("speed")
        .map(|v| v.as_f64().unwrap_or(f64::NAN))
        .unwrap_or(1.0);
    if !speed.is_finite() || !(0.25..=4.0).contains(&speed) {
        return Err(AppError::BadRequest(
            "speed must be between 0.25 and 4".into(),
        ));
    }
    let format = value
        .get("response_format")
        .and_then(Value::as_str)
        .unwrap_or("mp3")
        .to_string();
    if !matches!(
        format.as_str(),
        "mp3" | "opus" | "aac" | "flac" | "wav" | "pcm"
    ) {
        return Err(AppError::BadRequest("unsupported TTS audio format".into()));
    }
    if value
        .get("stream_format")
        .and_then(Value::as_str)
        .is_some_and(|f| f != "audio")
    {
        return Err(AppError::BadRequest(
            "TTS stream_format must be audio".into(),
        ));
    }
    let estimated_seconds = ((input.chars().count() as f64 / 4.0 / speed).ceil() as i64).max(1);
    Ok((value, model, format, estimated_seconds))
}

fn audio_seconds(bytes: &[u8], format: &str) -> AppResult<i64> {
    if bytes.is_empty() {
        return Err(AppError::UpstreamUnavailable(
            "empty TTS audio response".into(),
        ));
    }
    let duration = if format == "pcm" {
        // OpenAI speech PCM: mono, 24 kHz, signed 16-bit little endian.
        bytes.len() as f64 / 48_000.0
    } else {
        super::audio::audio_info(bytes, format)
            .map_err(|_| AppError::UpstreamUnavailable("TTS response is not valid audio".into()))?
            .0
    };
    Ok(duration.ceil().max(1.0) as i64)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn speech_validates_input_and_pcm_billing_uses_actual_duration() {
        let input=br#"{"model":"gpt-4o-mini-tts","voice":"alloy","input":"hello","response_format":"pcm"}"#;
        let (_, model, format, _) = speech_request(input).unwrap();
        assert_eq!(model, "gpt-4o-mini-tts");
        assert_eq!(audio_seconds(&vec![0; 72_000], &format).unwrap(), 2);
        assert!(
            speech_request(br#"{"model":"tts-1","voice":"alloy","input":"hello","speed":0}"#)
                .is_err()
        );
        assert!(speech_request(br#"{"model":"tts-1","input":"hello"}"#).is_err());
        assert!(audio_seconds(&[], "pcm").is_err());
    }
}
