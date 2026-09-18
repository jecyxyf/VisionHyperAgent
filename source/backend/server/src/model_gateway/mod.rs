//! Internal model gateway. Codex still owns all tools and browser clients cannot reach it.
mod protocol;

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
use vha_codex_agent::{
    config::{redact_value, ConfigManager},
    PreparedAgent, ProviderWireApi, ResolvedAgentConfig,
};

#[derive(Clone)]
struct Gateway {
    agent: Arc<PreparedAgent>,
    client: reqwest::Client,
}

#[derive(Clone)]
struct RuntimeConfig {
    resolved: Arc<ResolvedAgentConfig>,
    manager: Option<&'static ConfigManager>,
}

impl RuntimeConfig {
    fn new(fallback: &Arc<PreparedAgent>) -> Self {
        Self {
            resolved: vha_codex_agent::config::current().unwrap_or_else(|| fallback.config.clone()),
            manager: vha_codex_agent::config::global(),
        }
    }

    fn redact(&self, value: &mut Value) {
        if let Some(manager) = self.manager {
            manager.redact(value);
        }
        let secrets: Vec<_> = self
            .resolved
            .models
            .values()
            .map(|model| model.api_key.as_str())
            .collect();
        redact_value(value, &secrets);
    }
}

pub(crate) fn routes(agent: Option<Arc<PreparedAgent>>) -> Result<Router, String> {
    let Some(agent) = agent.filter(|agent| !agent.config.models.is_empty()) else {
        return Ok(Router::new());
    };
    let client = reqwest::Client::builder()
        .redirect(reqwest::redirect::Policy::none())
        .connect_timeout(Duration::from_secs(10))
        .timeout(Duration::from_secs(180))
        .build()
        .map_err(|_| "无法初始化模型网关")?;
    let gateway = Gateway { agent, client };
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
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .unwrap_or("");
    // Equal-length random tokens; no provider credential is exposed to the browser or Codex.
    let token = gateway.agent.gateway_token();
    let equal = supplied.len() == token.len()
        && supplied
            .bytes()
            .zip(token.bytes())
            .fold(0_u8, |difference, (left, right)| {
                difference | (left ^ right)
            })
            == 0;
    if !equal {
        return StatusCode::UNAUTHORIZED.into_response();
    }
    next.run(request).await
}

fn failure(status: StatusCode, message: &str) -> Response {
    (
        status,
        Json(json!({"error":{"type":"invalid_request_error","code":"model_gateway_error","message":message}})),
    )
        .into_response()
}

async fn responses(State(gateway): State<Gateway>, Json(mut request): Json<Value>) -> Response {
    let config = RuntimeConfig::new(&gateway.agent);
    let Some(model_id) = request.get("model").and_then(Value::as_str) else {
        return failure(StatusCode::BAD_REQUEST, "model is required");
    };
    let Some(model) = config.resolved.model(model_id) else {
        return failure(
            StatusCode::BAD_REQUEST,
            "Model is not enabled in backend configuration",
        );
    };
    request["model"] = Value::String(model.model_name.clone());
    let (url, chat) = match model.wire_api {
        ProviderWireApi::ChatCompletions => (
            format!("{}/chat/completions", model.base_url.trim_end_matches('/')),
            true,
        ),
        ProviderWireApi::Responses => (
            format!("{}/responses", model.base_url.trim_end_matches('/')),
            false,
        ),
    };
    if chat {
        let converted = match protocol::convert_request(&request) {
            Ok(converted) => converted,
            Err(message) => {
                log::warn!("Model compatibility request rejected: {message}");
                return failure(StatusCode::BAD_REQUEST, &message);
            }
        };
        request = converted.body;
        let upstream = match send_upstream(&gateway, &url, &model.api_key, request).await {
            Ok(upstream) => upstream,
            Err(response) => return response,
        };
        let mut adapter = ChatStream::new(model.model_id.clone(), converted.custom_tools)
            .with_tool_names(converted.tool_names);
        let pending = adapter.begin().into_iter().map(encode).collect();
        let state = ChatStreamState {
            config,
            upstream: Box::pin(upstream.bytes_stream()),
            decoder: SseDecoder::default(),
            adapter,
            pending,
            finished: false,
        };
        return sse_response(stream_chat(state));
    }

    let upstream = match send_upstream(&gateway, &url, &model.api_key, request).await {
        Ok(upstream) => upstream,
        Err(response) => return response,
    };
    let state = ResponsesStreamState {
        config,
        upstream: Box::pin(upstream.bytes_stream()),
        decoder: SseDecoder::default(),
        pending: VecDeque::new(),
        finished: false,
    };
    sse_response(stream_responses(state))
}

async fn send_upstream(
    gateway: &Gateway,
    url: &str,
    api_key: &str,
    body: Value,
) -> Result<reqwest::Response, Response> {
    match gateway
        .client
        .post(url)
        .bearer_auth(api_key)
        .json(&body)
        .send()
        .await
    {
        Ok(response) if response.status().is_success() => Ok(response),
        Ok(response) => {
            let status =
                StatusCode::from_u16(response.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
            log::warn!("Model provider rejected request, HTTP status={status}");
            Err(upstream_failure(status))
        }
        Err(_) => {
            log::error!("Model provider connection failed");
            Err(failure(
                StatusCode::BAD_GATEWAY,
                "Model provider connection failed",
            ))
        }
    }
}

fn sse_response<S>(stream: S) -> Response
where
    S: Stream<Item = Result<Bytes, Infallible>> + Send + 'static,
{
    Response::builder()
        .header(header::CONTENT_TYPE, "text/event-stream")
        .header(header::CACHE_CONTROL, "no-cache")
        .body(Body::from_stream(stream))
        .expect("static valid response headers")
}

fn upstream_failure(status: StatusCode) -> Response {
    let status = if status.is_redirection() {
        StatusCode::BAD_GATEWAY
    } else {
        status
    };
    failure(
        status,
        "Model provider rejected the request; check authentication, model parameters or rate limits",
    )
}

struct ChatStreamState {
    config: RuntimeConfig,
    upstream: Pin<Box<dyn Stream<Item = Result<Bytes, reqwest::Error>> + Send>>,
    decoder: SseDecoder,
    adapter: ChatStream,
    pending: VecDeque<Bytes>,
    finished: bool,
}

struct ResponsesStreamState {
    config: RuntimeConfig,
    upstream: Pin<Box<dyn Stream<Item = Result<Bytes, reqwest::Error>> + Send>>,
    decoder: SseDecoder,
    pending: VecDeque<Bytes>,
    finished: bool,
}

fn stream_chat(state: ChatStreamState) -> impl Stream<Item = Result<Bytes, Infallible>> {
    futures_util::stream::unfold(state, |mut state| async move {
        loop {
            if let Some(bytes) = state.pending.pop_front() {
                return Some((Ok(bytes), state));
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
    })
}

impl ChatStreamState {
    fn fail(&mut self, message: &str) {
        log::warn!("Model compatibility stream failed: {message}");
        let mut event = self.adapter.fail(message);
        self.config.redact(&mut event);
        self.pending.push_back(encode(event));
        self.finished = true;
    }
}

fn stream_responses(state: ResponsesStreamState) -> impl Stream<Item = Result<Bytes, Infallible>> {
    futures_util::stream::unfold(state, |mut state| async move {
        loop {
            if let Some(bytes) = state.pending.pop_front() {
                return Some((Ok(bytes), state));
            }
            if state.finished {
                return None;
            }
            match state.upstream.next().await {
                Some(Ok(bytes)) => match state.decoder.push(&bytes) {
                    Ok(events) => {
                        for event in events {
                            if event.trim() == "[DONE]" {
                                state.pending.push_back(Bytes::from("data: [DONE]\n\n"));
                                state.finished = true;
                                break;
                            }
                            match serde_json::from_str::<Value>(&event) {
                                Ok(mut event) => {
                                    state.config.redact(&mut event);
                                    state.pending.push_back(encode(event));
                                }
                                Err(_) => {
                                    log::warn!("Model provider returned invalid SSE JSON");
                                    state.pending.push_back(Bytes::from(
                                        "data: {\"type\":\"response.failed\",\"response\":{\"status\":\"failed\",\"error\":{\"code\":\"provider_stream_error\",\"message\":\"Invalid provider stream event\"}}}\n\n",
                                    ));
                                    state.finished = true;
                                    break;
                                }
                            }
                        }
                    }
                    Err(error) => {
                        log::warn!("Model provider stream framing failed: {error}");
                        state.pending.push_back(Bytes::from(
                            "data: {\"type\":\"response.failed\",\"response\":{\"status\":\"failed\",\"error\":{\"code\":\"provider_stream_error\",\"message\":\"Provider stream framing failed\"}}}\n\n",
                        ));
                        state.finished = true;
                    }
                },
                Some(Err(_)) => {
                    log::warn!("Model provider stream connection failed");
                    state.pending.push_back(Bytes::from(
                        "data: {\"type\":\"response.failed\",\"response\":{\"status\":\"failed\",\"error\":{\"code\":\"provider_stream_error\",\"message\":\"Provider stream connection failed\"}}}\n\n",
                    ));
                    state.finished = true;
                }
                None => {
                    log::warn!("Model provider stream ended without DONE");
                    state.pending.push_back(Bytes::from(
                        "data: {\"type\":\"response.failed\",\"response\":{\"status\":\"failed\",\"error\":{\"code\":\"provider_stream_error\",\"message\":\"Provider stream ended without DONE\"}}}\n\n",
                    ));
                    state.finished = true;
                }
            }
        }
    })
}

fn encode(event: Value) -> Bytes {
    let kind = event.get("type").and_then(Value::as_str).unwrap_or("error");
    format!("event: {kind}\ndata: {event}\n\n").into()
}

#[cfg(test)]
#[path = "http_tests.rs"]
mod http_tests;
