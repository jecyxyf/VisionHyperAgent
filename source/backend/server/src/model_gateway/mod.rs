//! Optional in-process protocol adapter for Chat-only providers. Codex still owns all tools.
//! This route is never a browser API: it rejects Origin and requires a per-start secret.
mod protocol;

use crate::codex_config::{CodexSettings, ProviderProtocol};
use axum::{
    body::{Body, Bytes},
    extract::{DefaultBodyLimit, Request, State},
    http::{header, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::post,
    Json, Router,
};
use futures_util::{Stream, StreamExt};
use protocol::{ChatStream, SseDecoder};
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::convert::Infallible;
use std::pin::Pin;
use std::sync::Arc;
use std::time::Duration;

#[derive(Clone)]
struct Gateway {
    settings: Arc<CodexSettings>,
    client: reqwest::Client,
}

pub(crate) fn routes(settings: Option<Arc<CodexSettings>>) -> Result<Router, String> {
    let Some(settings) = settings.filter(|s| s.wire_api == ProviderProtocol::ChatCompletions)
    else {
        return Ok(Router::new());
    };
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(180))
        .build()
        .map_err(|_| "无法初始化模型协议兼容层")?;
    let gateway = Gateway { settings, client };
    Ok(Router::new()
        .route("/internal/model/v1/responses", post(responses))
        .layer(DefaultBodyLimit::max(32 * 1024 * 1024))
        .route_layer(middleware::from_fn_with_state(gateway.clone(), authorize))
        .with_state(gateway))
}

async fn authorize(State(gateway): State<Gateway>, request: Request, next: Next) -> Response {
    if request.headers().contains_key(header::ORIGIN) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let supplied = request
        .headers()
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or("");
    // Equal-length random tokens; no provider credential is exposed to the browser or Codex.
    let token = gateway.settings.gateway_token();
    let equal = supplied.len() == token.len()
        && supplied
            .bytes()
            .zip(token.bytes())
            .fold(0_u8, |diff, (a, b)| diff | (a ^ b))
            == 0;
    if !equal {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    next.run(request).await
}

fn failure(status: StatusCode, message: &str) -> Response {
    (status,Json(json!({"error":{"type":"invalid_request_error","code":"model_gateway_error","message":message}}))).into_response()
}

async fn responses(State(gateway): State<Gateway>, Json(request): Json<Value>) -> Response {
    if request.get("model").and_then(Value::as_str) != Some(&gateway.settings.model) {
        return failure(
            StatusCode::BAD_REQUEST,
            "Model is not enabled in backend configuration",
        );
    }
    let converted = match protocol::convert_request(&request) {
        Ok(converted) => converted,
        Err(message) => {
            log::warn!("Model compatibility request rejected: {message}");
            return failure(StatusCode::BAD_REQUEST, &message);
        }
    };
    let url = format!(
        "{}/chat/completions",
        gateway.settings.base_url.trim_end_matches('/')
    );
    let upstream = match gateway
        .client
        .post(url)
        .bearer_auth(gateway.settings.provider_key())
        .json(&converted.body)
        .send()
        .await
    {
        Ok(response) => response,
        Err(_) => {
            log::error!("Model provider connection failed");
            return failure(StatusCode::BAD_GATEWAY, "Model provider connection failed");
        }
    };
    if !upstream.status().is_success() {
        let status =
            StatusCode::from_u16(upstream.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
        log::warn!("Model provider rejected request, HTTP status={status}");
        return failure(if status.is_redirection(){StatusCode::BAD_GATEWAY}else{status},"Model provider rejected the request; check authentication, model parameters or rate limits");
    }
    let mut adapter = ChatStream::new(gateway.settings.model.clone(), converted.custom_tools)
        .with_tool_names(converted.tool_names);
    let pending = adapter.begin().into_iter().map(encode).collect();
    let state = StreamState {
        upstream: Box::pin(upstream.bytes_stream()),
        decoder: SseDecoder::default(),
        adapter,
        pending,
        finished: false,
    };
    let stream = futures_util::stream::unfold(state, |mut state| async move {
        loop {
            if let Some(bytes) = state.pending.pop_front() {
                return Some((Ok::<_, Infallible>(bytes), state));
            }
            if state.finished {
                return None;
            }
            match state.upstream.next().await {
                Some(Ok(bytes)) => match state.decoder.push(&bytes) {
                    Ok(events) => {
                        for event in events {
                            let result = if event.trim() == "[DONE]" {
                                state.finished = true;
                                state.adapter.finish()
                            } else {
                                serde_json::from_str::<Value>(&event)
                                    .map_err(|_| "Invalid JSON in model stream".to_string())
                                    .and_then(|chunk| state.adapter.push(&chunk))
                            };
                            match result {
                                Ok(events) => state.pending.extend(events.into_iter().map(encode)),
                                Err(error) => state.fail(&error),
                            }
                            if state.finished {
                                break;
                            }
                        }
                    }
                    Err(error) => state.fail(&error),
                },
                Some(Err(_)) => state.fail("Model stream connection failed"),
                None => state.fail("Model stream ended without its explicit [DONE] marker"),
            }
        }
    });
    Response::builder()
        .header(header::CONTENT_TYPE, "text/event-stream")
        .header(header::CACHE_CONTROL, "no-cache")
        .body(Body::from_stream(stream))
        .expect("static valid response headers")
}

struct StreamState {
    upstream: Pin<Box<dyn Stream<Item = Result<Bytes, reqwest::Error>> + Send>>,
    decoder: SseDecoder,
    adapter: ChatStream,
    pending: VecDeque<Bytes>,
    finished: bool,
}
impl StreamState {
    fn fail(&mut self, message: &str) {
        log::warn!("Model compatibility stream failed: {message}");
        self.pending.push_back(encode(self.adapter.fail(message)));
        self.finished = true;
    }
}
fn encode(event: Value) -> Bytes {
    let kind = event.get("type").and_then(Value::as_str).unwrap_or("error");
    format!("event: {kind}\ndata: {event}\n\n").into()
}

#[cfg(test)]
mod http_tests;
