//! Shared OpenAI-compatible text probes for manual diagnostics and the scheduler.

use reqwest::{RequestBuilder, Response, StatusCode};
use serde_json::{json, Value};

const RESPONSE_LIMIT: usize = 256 * 1024;
const ERROR_LIMIT: usize = 16 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProbeApi {
    ChatCompletions,
    Responses,
}

pub struct ProbeResult {
    pub api: ProbeApi,
    pub status: StatusCode,
    /// None only when the probe passed, including a valid terminal Responses event.
    pub failure_body: Option<String>,
    pub chat_status: Option<StatusCode>,
}

/// A bounded request that also works with upstreams requiring streaming Responses.
pub fn responses_body(model: &str) -> Value {
    json!({
        "model": model,
        "input": [{"role": "user", "content": [{"type": "input_text", "text": "Reply OK only."}]}],
        "instructions": "Reply briefly.",
        "max_output_tokens": 16,
        "store": false,
        "stream": true
    })
}

/// Try Responses once on a protocol/authentication rejection. Never retry timeouts,
/// quota/rate limits or transient server failures. Callers supply authenticated,
/// time-limited requests so their URL adapters and timeout policy remain in effect.
pub async fn probe(
    chat_request: RequestBuilder,
    responses_request: RequestBuilder,
) -> Result<ProbeResult, reqwest::Error> {
    let response = chat_request.send().await?;
    let chat_status = response.status();
    if chat_status.is_success() {
        return Ok(ProbeResult {
            api: ProbeApi::ChatCompletions,
            status: chat_status,
            failure_body: None,
            chat_status: None,
        });
    }
    if !matches!(
        chat_status.as_u16(),
        400 | 401 | 403 | 404 | 405 | 422 | 501
    ) {
        return Ok(ProbeResult {
            api: ProbeApi::ChatCompletions,
            status: chat_status,
            failure_body: Some(read_error_body(response).await?),
            chat_status: None,
        });
    }
    // Release the rejected response before opening a second request.
    drop(response);
    let response = responses_request.send().await?;
    let status = response.status();
    let failure_body = if status.is_success() {
        validate_responses(response).await?
    } else {
        Some(read_error_body(response).await?)
    };
    Ok(ProbeResult {
        api: ProbeApi::Responses,
        status,
        failure_body,
        chat_status: Some(chat_status),
    })
}

async fn read_error_body(mut response: Response) -> Result<String, reqwest::Error> {
    let mut body = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        let remaining = ERROR_LIMIT - body.len();
        body.extend_from_slice(&chunk[..chunk.len().min(remaining)]);
        if body.len() == ERROR_LIMIT {
            break;
        }
    }
    Ok(String::from_utf8_lossy(&body).into_owned())
}

async fn validate_responses(mut response: Response) -> Result<Option<String>, reqwest::Error> {
    let is_sse = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value.to_ascii_lowercase().starts_with("text/event-stream"));
    let mut buffer = Vec::new();
    let mut total = 0;
    let mut event = String::new();
    let mut data = String::new();
    while let Some(chunk) = response.chunk().await? {
        total += chunk.len();
        if total > RESPONSE_LIMIT {
            return Ok(Some("Responses 探测响应超出大小限制".to_string()));
        }
        buffer.extend_from_slice(&chunk);
        if !is_sse {
            continue;
        }
        let mut consumed = 0;
        while let Some(end) = buffer[consumed..].iter().position(|byte| *byte == b'\n') {
            let end = consumed + end;
            let line = String::from_utf8_lossy(&buffer[consumed..end]);
            let line = line.trim_end_matches('\r');
            consumed = end + 1;
            if line.is_empty() {
                if let Some(result) = terminal_event(&event, &data) {
                    return Ok(result);
                }
                event.clear();
                data.clear();
            } else if let Some(value) = line.strip_prefix("event:") {
                event = value.trim_start_matches(' ').to_string();
            } else if let Some(value) = line.strip_prefix("data:") {
                if !data.is_empty() {
                    data.push('\n');
                }
                data.push_str(value.strip_prefix(' ').unwrap_or(value));
            }
        }
        buffer.drain(..consumed);
    }
    if is_sse {
        // SSE events need a blank line terminator; a partial final event is a failure.
        return Ok(Some("Responses 流已结束，但未收到完成事件".to_string()));
    }
    Ok(Some(match serde_json::from_slice::<Value>(&buffer) {
        Ok(value) => match response_status(&value) {
            Some(result) => return Ok(result),
            None => "Responses 响应未包含有效的完成状态".to_string(),
        },
        Err(_) => "Responses 响应不是有效 JSON 或事件流".to_string(),
    }))
}

// Outer Option: terminal event; inner Option: failure message (None means success).
fn terminal_event(event: &str, data: &str) -> Option<Option<String>> {
    let value: Value = serde_json::from_str(data).ok()?;
    let kind = value.get("type").and_then(Value::as_str).unwrap_or(event);
    match kind {
        "error" | "response.failed" => {
            Some(Some(value.get("response").unwrap_or(&value).to_string()))
        }
        "response.completed" | "response.incomplete" => Some(
            response_status(value.get("response").unwrap_or(&value))
                .unwrap_or_else(|| Some("Responses 完成事件缺少有效状态".to_string())),
        ),
        _ => None,
    }
}

fn response_status(value: &Value) -> Option<Option<String>> {
    if value.get("error").is_some_and(|error| !error.is_null()) {
        return Some(Some(value.to_string()));
    }
    match value.get("status").and_then(Value::as_str) {
        Some("completed") => Some(None),
        // A deliberately small output budget can truncate an otherwise working call.
        Some("incomplete")
            if value
                .pointer("/incomplete_details/reason")
                .and_then(Value::as_str)
                == Some("max_output_tokens") =>
        {
            Some(None)
        }
        Some("failed" | "incomplete" | "cancelled") => Some(Some(value.to_string())),
        _ => None,
    }
}
