//! Same-origin browser gateway. Only whitelisted application operations cross into Codex.
use std::collections::{HashMap, HashSet};
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        DefaultBodyLimit, Multipart, Path, Request, State,
    },
    http::{header::ORIGIN, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{delete, get, post},
    Json, Router,
};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use serde_json::{json, Value};
use tokio::sync::{broadcast, mpsc, watch, Semaphore};

use crate::{
    agent_service::{AgentService, ServiceError},
    attachments::MAX_ATTACHMENT_BYTES,
    shutdown::ShutdownSignal,
};

#[derive(Clone)]
struct ApiState {
    service: Arc<AgentService>,
    shutdown: ShutdownSignal,
    origins: Arc<HashSet<String>>,
    connections: Arc<Semaphore>,
    calls: Arc<Semaphore>,
    uploads: Arc<Semaphore>,
}

pub fn routes(service: Arc<AgentService>, shutdown: ShutdownSignal, addr: SocketAddr) -> Router {
    let origins = Arc::new(HashSet::from([
        format!("http://127.0.0.1:{}", addr.port()),
        format!("http://localhost:{}", addr.port()),
        "http://127.0.0.1:5173".into(),
        "http://localhost:5173".into(),
    ]));
    let state = ApiState {
        service,
        shutdown,
        origins,
        connections: Arc::new(Semaphore::new(32)),
        calls: Arc::new(Semaphore::new(32)),
        uploads: Arc::new(Semaphore::new(2)),
    };
    Router::new()
        .route("/ws", get(upgrade))
        .route("/api/agent/status", get(status))
        .route("/api/agent/attachments", post(upload))
        .route("/api/agent/attachments/{id}", delete(remove_upload))
        .layer(DefaultBodyLimit::max(MAX_ATTACHMENT_BYTES + 64 * 1024))
        .route_layer(middleware::from_fn_with_state(state.clone(), same_origin))
        .with_state(state)
}

async fn same_origin(State(state): State<ApiState>, request: Request, next: Next) -> Response {
    if let Some(origin) = request.headers().get(ORIGIN) {
        if origin
            .to_str()
            .ok()
            .is_none_or(|origin| !state.origins.contains(origin))
        {
            return (
                StatusCode::FORBIDDEN,
                Json(json!({"error":{"code":"origin","message":"不允许来自该网页的请求"}})),
            )
                .into_response();
        }
    }
    next.run(request).await
}

async fn status(State(state): State<ApiState>) -> Json<Value> {
    Json(state.service.snapshot().await)
}

fn http_error(status: StatusCode, code: &'static str, message: impl Into<String>) -> Response {
    (
        status,
        Json(json!({"error":ServiceError::new(code,message)})),
    )
        .into_response()
}

async fn upload(State(state): State<ApiState>, mut multipart: Multipart) -> Response {
    if state
        .service
        .closing
        .load(std::sync::atomic::Ordering::Acquire)
    {
        return http_error(StatusCode::SERVICE_UNAVAILABLE, "closing", "程序正在退出");
    }
    let Ok(_permit) = state.uploads.try_acquire() else {
        return http_error(
            StatusCode::TOO_MANY_REQUESTS,
            "busy",
            "上传任务过多，请稍后重试",
        );
    };
    let Some(store) = &state.service.uploads else {
        return http_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "configuration",
            "附件存储尚未就绪",
        );
    };
    let operation = async {
        let field = multipart
            .next_field()
            .await
            .map_err(|_| "无法读取附件表单")?
            .ok_or("缺少附件")?;
        if field.name() != Some("file") {
            return Err("附件字段必须命名为 file");
        }
        let name = field.file_name().unwrap_or("attachment").to_owned();
        let bytes = field
            .bytes()
            .await
            .map_err(|_| "附件读取失败或超过大小限制")?;
        if multipart
            .next_field()
            .await
            .map_err(|_| "附件表单格式错误")?
            .is_some()
        {
            return Err("每次上传只能包含一个文件");
        }
        Ok((name, bytes))
    };
    match tokio::time::timeout(Duration::from_secs(30), operation).await {
        Ok(Ok((name, bytes))) => match store.store(&name, &bytes).await {
            Ok(attachment) => Json(json!({"attachment":attachment})).into_response(),
            Err(error) => http_error(StatusCode::BAD_REQUEST, "attachment", error),
        },
        Ok(Err(error)) => http_error(StatusCode::BAD_REQUEST, "attachment", error),
        Err(_) => http_error(StatusCode::REQUEST_TIMEOUT, "timeout", "附件上传超时"),
    }
}

async fn remove_upload(State(state): State<ApiState>, Path(id): Path<String>) -> Response {
    let Some(store) = &state.service.uploads else {
        return http_error(
            StatusCode::SERVICE_UNAVAILABLE,
            "configuration",
            "附件存储尚未就绪",
        );
    };
    match store.remove(&id).await {
        Ok(()) => Json(json!({})).into_response(),
        Err(error) => http_error(StatusCode::CONFLICT, "attachment", error),
    }
}

async fn upgrade(State(state): State<ApiState>, upgrade: WebSocketUpgrade) -> Response {
    let Ok(permit) = state.connections.clone().try_acquire_owned() else {
        return http_error(StatusCode::TOO_MANY_REQUESTS, "busy", "连接数量已达上限");
    };
    upgrade
        .max_message_size(128 * 1024)
        .max_frame_size(128 * 1024)
        .on_upgrade(move |socket| async move {
            let _permit = permit;
            serve_socket(socket, state).await;
        })
        .into_response()
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Call {
    id: String,
    method: String,
    #[serde(default)]
    params: Value,
}

async fn send(
    sink: &mut futures_util::stream::SplitSink<WebSocket, Message>,
    value: Value,
) -> bool {
    matches!(
        tokio::time::timeout(
            Duration::from_secs(2),
            sink.send(Message::Text(value.to_string().into()))
        )
        .await,
        Ok(Ok(()))
    )
}

async fn serve_socket(socket: WebSocket, state: ApiState) {
    let (mut sink, mut stream) = socket.split();
    let mut events = state.service.subscribe();
    let mut shutdown = state.shutdown.clone();
    let (replies, mut completed) = mpsc::channel::<(String, Value)>(32);
    let (overloaded, mut close_signal) = watch::channel(false);
    let mut inflight: HashMap<String, Instant> = HashMap::new();
    let mut pulse = tokio::time::interval(Duration::from_secs(10));
    let mut last_seen = Instant::now();
    if !send(
        &mut sink,
        json!({"type":"status","data":state.service.snapshot().await}),
    )
    .await
    {
        return;
    }
    loop {
        tokio::select! {
            _=shutdown.wait()=>{let _=send(&mut sink,json!({"type":"closing"})).await;break;}
            _=close_signal.changed()=>break,
            _=pulse.tick()=>{
                if last_seen.elapsed()>Duration::from_secs(40){break;}
                if !matches!(tokio::time::timeout(Duration::from_secs(2),sink.send(Message::Ping(vec![].into()))).await,Ok(Ok(()))){break;}
            }
            event=events.recv()=>{
                let event=match event {
                    Ok(event)=>event,
                    Err(broadcast::error::RecvError::Lagged(_))=>json!({"type":"resync_required"}),
                    Err(broadcast::error::RecvError::Closed)=>break,
                };
                if !send(&mut sink,event).await{break;}
            }
            Some((id,response))=completed.recv()=>{
                inflight.remove(&id);
                // Deliver notifications already queued before a read response, reducing snapshot races.
                for _ in 0..32 {
                    match events.try_recv() {
                        Ok(event)=>{if !send(&mut sink,event).await{return;}},
                        Err(broadcast::error::TryRecvError::Lagged(_))=>{if !send(&mut sink,json!({"type":"resync_required"})).await{return;}},
                        _=>break,
                    }
                }
                if !send(&mut sink,response).await{break;}
            }
            frame=stream.next()=>{
                last_seen=Instant::now();
                match frame {
                    Some(Ok(Message::Text(text)))=>{
                        let value:Value=match serde_json::from_str(&text){Ok(value)=>value,Err(_)=>break};
                        if value.get("type").and_then(Value::as_str)==Some("ping") {
                            if !send(&mut sink,json!({"type":"pong"})).await{break;}continue;
                        }
                        let call:Call=match serde_json::from_value(value){Ok(call)=>call,Err(_)=>break};
                        if call.id.is_empty()||call.id.len()>128||call.method.len()>64{break;}
                        if inflight.contains_key(&call.id)||inflight.len()>=16 {
                            if !send(&mut sink,json!({"id":call.id,"error":ServiceError::new("busy","重复请求编号或请求数量过多")})).await{break;}continue;
                        }
                        let permit=match state.calls.clone().try_acquire_owned(){
                            Ok(permit)=>permit,Err(_)=>{
                                if !send(&mut sink,json!({"id":call.id,"error":ServiceError::new("busy","后端正在处理较多请求")})).await{break;}continue;
                            }
                        };
                        inflight.insert(call.id.clone(),Instant::now());
                        let service=state.service.clone();let replies=replies.clone();let overloaded=overloaded.clone();
                        // Deliberately detached from this browser connection: closing a tab must
                        // not cancel an accepted business operation or interrupt a Codex turn.
                        tokio::spawn(async move {
                            let _permit=permit;
                            let result=service.call(&call.method,call.params).await;
                            let response=match result {Ok(result)=>json!({"id":call.id,"result":result}),Err(error)=>json!({"id":call.id,"error":error})};
                            if let Err(mpsc::error::TrySendError::Full(_))=replies.try_send((call.id,response)) {overloaded.send_replace(true);}
                        });
                    }
                    Some(Ok(Message::Pong(_)))=>{},
                    Some(Ok(Message::Ping(bytes)))=>{
                        if !matches!(tokio::time::timeout(Duration::from_secs(2),sink.send(Message::Pong(bytes))).await,Ok(Ok(()))){break;}
                    }
                    _=>break,
                }
            }
        }
    }
    let _ = tokio::time::timeout(Duration::from_millis(300), sink.send(Message::Close(None))).await;
}

#[cfg(test)]
#[path = "agent_api_tests.rs"]
mod tests;
