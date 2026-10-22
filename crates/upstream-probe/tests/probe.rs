use std::{
    sync::{Arc, Mutex},
    time::Duration,
};

use axum::{
    body::Body, extract::State, http::HeaderMap, response::Response, routing::post, Json, Router,
};
use futures_util::{stream, StreamExt};
use reqwest::StatusCode;
use serde_json::{json, Value};
use upstream_probe::{probe, responses_body, ProbeApi};

const COMPLETE: &str = "event: response.completed\r\ndata: {\"type\":\"response.completed\",\"response\":{\"status\":\"completed\"}}\r\n\r\n";

#[derive(Clone)]
struct MockState {
    chat_status: StatusCode,
    responses_status: StatusCode,
    body: String,
    content_type: &'static str,
    keep_open: bool,
    requests: Arc<Mutex<Vec<(String, String, Value)>>>,
}

struct MockUpstream {
    base: String,
    state: MockState,
    task: tokio::task::JoinHandle<()>,
}

impl Drop for MockUpstream {
    fn drop(&mut self) {
        self.task.abort();
    }
}

impl MockUpstream {
    async fn start(
        chat_status: u16,
        responses_status: u16,
        body: &str,
        content_type: &'static str,
        keep_open: bool,
    ) -> Self {
        let state = MockState {
            chat_status: StatusCode::from_u16(chat_status).unwrap(),
            responses_status: StatusCode::from_u16(responses_status).unwrap(),
            body: body.to_string(),
            content_type,
            keep_open,
            requests: Arc::new(Mutex::new(Vec::new())),
        };
        let app = Router::new()
            .route("/v1/chat/completions", post(chat))
            .route("/v1/responses", post(responses))
            .with_state(state.clone());
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let base = format!("http://{}", listener.local_addr().unwrap());
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        Self { base, state, task }
    }

    async fn probe(&self) -> Result<upstream_probe::ProbeResult, reqwest::Error> {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(2))
            .build()
            .unwrap();
        probe(
            client.post(format!("{}/v1/chat/completions", self.base))
                .bearer_auth("test-key").json(&json!({"model":"test-model","messages":[{"role":"user","content":"ping"}],"max_tokens":1})),
            client.post(format!("{}/v1/responses", self.base))
                .bearer_auth("test-key").header("accept", "text/event-stream").json(&responses_body("test-model")),
        ).await
    }
}

fn record(state: &MockState, path: &str, headers: &HeaderMap, body: Value) {
    state.requests.lock().unwrap().push((
        path.to_string(),
        headers["authorization"].to_str().unwrap().to_string(),
        body,
    ));
}

async fn chat(
    State(state): State<MockState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> (StatusCode, Json<Value>) {
    record(&state, "chat", &headers, body);
    let response = if state.chat_status.is_success() {
        json!({"choices": [{"message": {"role": "assistant", "content": "OK"}, "finish_reason": "stop"}]})
    } else {
        json!({"error":{"message":"请使用AccessToken进行认证"}})
    };
    (state.chat_status, Json(response))
}

async fn responses(
    State(state): State<MockState>,
    headers: HeaderMap,
    Json(body): Json<Value>,
) -> Response {
    record(&state, "responses", &headers, body);
    // Break every event across chunks, including UTF-8 and CRLF boundaries.
    let chunks: Vec<Result<Vec<u8>, std::io::Error>> = state
        .body
        .as_bytes()
        .chunks(7)
        .map(|bytes| Ok(bytes.to_vec()))
        .collect();
    let body = if state.keep_open {
        Body::from_stream(stream::iter(chunks).chain(stream::pending()))
    } else {
        Body::from_stream(stream::iter(chunks))
    };
    Response::builder()
        .status(state.responses_status)
        .header("content-type", state.content_type)
        .body(body)
        .unwrap()
}

#[tokio::test]
async fn responses_only_key_passes_without_changing_auth_or_waiting_for_connection_close() {
    let server =
        MockUpstream::start(401, 200, COMPLETE, "text/event-stream; charset=utf-8", true).await;
    let result = server.probe().await.unwrap();
    assert_eq!(result.api, ProbeApi::Responses);
    assert_eq!(result.chat_status, Some(StatusCode::UNAUTHORIZED));
    assert_eq!(result.status, StatusCode::OK);
    assert!(result.failure_body.is_none());
    let requests = server.state.requests.lock().unwrap();
    assert_eq!(requests.len(), 2);
    assert!(requests
        .iter()
        .all(|(_, auth, _)| auth == "Bearer test-key"));
    assert_eq!(requests[1].2["model"], "test-model");
    assert_eq!(requests[1].2["store"], false);
    assert_eq!(requests[1].2["stream"], true);
    assert_eq!(requests[1].2["max_output_tokens"], 16);
}

#[tokio::test]
async fn working_chat_does_not_make_a_second_call() {
    let server = MockUpstream::start(200, 401, "", "application/json", false).await;
    let result = server.probe().await.unwrap();
    assert_eq!(result.api, ProbeApi::ChatCompletions);
    assert!(result.failure_body.is_none());
    assert_eq!(server.state.requests.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn invalid_key_still_fails_after_one_fallback() {
    let server = MockUpstream::start(
        401,
        401,
        r#"{"error":{"message":"invalid key"}}"#,
        "application/json",
        false,
    )
    .await;
    let result = server.probe().await.unwrap();
    assert_eq!(result.status, StatusCode::UNAUTHORIZED);
    assert!(result.failure_body.unwrap().contains("invalid key"));
    assert_eq!(server.state.requests.lock().unwrap().len(), 2);
}

#[tokio::test]
async fn protocol_rejections_try_responses_once() {
    for status in [400, 403, 404, 405, 422, 501] {
        let server = MockUpstream::start(status, 200, COMPLETE, "text/event-stream", false).await;
        assert!(
            server.probe().await.unwrap().failure_body.is_none(),
            "HTTP {status}"
        );
        assert_eq!(server.state.requests.lock().unwrap().len(), 2);
    }
}

#[tokio::test]
async fn quota_limits_and_transient_failures_do_not_retry() {
    for status in [429, 500, 502, 503] {
        let server = MockUpstream::start(status, 200, COMPLETE, "text/event-stream", false).await;
        let result = server.probe().await.unwrap();
        assert_eq!(result.status.as_u16(), status);
        assert!(result.failure_body.is_some());
        assert_eq!(server.state.requests.lock().unwrap().len(), 1);
    }
}

#[tokio::test]
async fn http_200_is_not_enough_to_pass_responses() {
    for body in [
        "event: error\ndata: {\"type\":\"error\",\"error\":{\"message\":\"no permission 权限不足\"}}\n\n",
        "data: {\"type\":\"response.failed\",\"response\":{\"status\":\"failed\",\"error\":{\"message\":\"failed\"}}}\n\n",
        "data: {\"type\":\"response.created\"}\n\n",
        "data: [DONE]\n\n",
        "data: {\"type\":\"response.completed\",\"response\":{\"status\":\"completed\"}}",
    ] {
        let server = MockUpstream::start(401, 200, body, "text/event-stream", false).await;
        let result = server.probe().await.unwrap();
        assert_eq!(result.status, StatusCode::OK);
        assert!(result.failure_body.is_some(), "{body}");
    }
}

#[tokio::test]
async fn json_responses_require_a_valid_terminal_status() {
    for (body, passes) in [
        (r#"{"status":"completed","output":[]}"#, true),
        (
            r#"{"status":"incomplete","incomplete_details":{"reason":"max_output_tokens"}}"#,
            true,
        ),
        (
            r#"{"status":"incomplete","incomplete_details":{"reason":"content_filter"}}"#,
            false,
        ),
        (r#"{"status":"failed","error":{"message":"denied"}}"#, false),
        (r#"{"status":"in_progress"}"#, false),
        (r#"{"error":{"message":"denied"}}"#, false),
        ("<html>Not an API</html>", false),
    ] {
        let server = MockUpstream::start(404, 200, body, "application/json", false).await;
        assert_eq!(
            server.probe().await.unwrap().failure_body.is_none(),
            passes,
            "{body}"
        );
    }
}

#[tokio::test]
async fn output_budget_exhaustion_is_a_working_stream() {
    let body = "data: {\"type\":\"response.incomplete\",\"response\":{\"status\":\"incomplete\",\"incomplete_details\":{\"reason\":\"max_output_tokens\"}}}\n\n";
    let server = MockUpstream::start(401, 200, body, "text/event-stream", false).await;
    assert!(server.probe().await.unwrap().failure_body.is_none());
}

#[tokio::test]
async fn unfinished_stream_is_bounded_by_request_timeout() {
    let server = MockUpstream::start(
        401,
        200,
        "data: {\"type\":\"response.created\"}\n\n",
        "text/event-stream",
        true,
    )
    .await;
    assert!(server.probe().await.err().unwrap().is_timeout());
}

#[tokio::test]
async fn oversized_responses_fail_with_bounded_reading() {
    let body = "x".repeat(256 * 1024 + 1);
    let server = MockUpstream::start(401, 200, &body, "application/json", false).await;
    assert!(server
        .probe()
        .await
        .unwrap()
        .failure_body
        .unwrap()
        .contains("大小限制"));
}
