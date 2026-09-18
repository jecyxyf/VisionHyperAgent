use super::*;
use axum::http::Request as HttpRequest;
use http_body_util::BodyExt;
use tokio::sync::Mutex;
use tower::ServiceExt;

struct Fixture {
    settings: Arc<CodexSettings>,
    requests: Arc<Mutex<Vec<(String, Value)>>>,
    task: tokio::task::JoinHandle<()>,
    root: std::path::PathBuf,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.task.abort();
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
impl Fixture {
    async fn new(payload: &str, status: StatusCode) -> Self {
        let requests = Arc::new(Mutex::new(Vec::new()));
        let capture = requests.clone();
        let payload = payload.to_owned();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let addr = listener.local_addr().unwrap();
        let app = Router::new().route(
            "/v1/chat/completions",
            post(
                move |headers: axum::http::HeaderMap, Json(body): Json<Value>| {
                    let capture = capture.clone();
                    let payload = payload.clone();
                    async move {
                        capture.lock().await.push((
                            headers
                                .get(header::AUTHORIZATION)
                                .unwrap()
                                .to_str()
                                .unwrap()
                                .to_owned(),
                            body,
                        ));
                        (
                            status,
                            [(header::CONTENT_TYPE, "text/event-stream")],
                            payload,
                        )
                    }
                },
            ),
        );
        let task = tokio::spawn(async move {
            axum::serve(listener, app).await.unwrap();
        });
        let root = std::env::temp_dir().join(format!("vha-gateway-{}", uuid::Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        let text=toml::to_string(&json!({"codex_path":std::env::current_exe().unwrap(),"model":"fixture-model","base_url":format!("http://{addr}/v1"),"api_key":"FIXTURE_PROVIDER_KEY","wire_api":"chat_completions"})).unwrap();
        let settings = Arc::new(CodexSettings::from_toml(&root, &text).unwrap());
        Self {
            settings,
            requests,
            task,
            root,
        }
    }
    async fn request(&self, origin: Option<&str>, authorized: bool) -> Response {
        let mut builder = HttpRequest::builder()
            .method("POST")
            .uri("/internal/model/v1/responses")
            .header(header::CONTENT_TYPE, "application/json");
        if let Some(origin) = origin {
            builder = builder.header(header::ORIGIN, origin);
        }
        if authorized {
            builder = builder.header(
                header::AUTHORIZATION,
                format!("Bearer {}", self.settings.gateway_token()),
            );
        }
        let body =
            json!({"model":"fixture-model","input":"fixture prompt","stream":true}).to_string();
        routes(Some(self.settings.clone()))
            .unwrap()
            .oneshot(builder.body(Body::from(body)).unwrap())
            .await
            .unwrap()
    }
}
fn frame(value: Value) -> String {
    format!("data: {value}\n\n")
}
async fn body(response: Response) -> String {
    String::from_utf8(
        response
            .into_body()
            .collect()
            .await
            .unwrap()
            .to_bytes()
            .to_vec(),
    )
    .unwrap()
}

#[tokio::test]
async fn gateway_rejects_browser_origins_and_missing_or_wrong_tokens() {
    let fixture = Fixture::new("", StatusCode::OK).await;
    assert_eq!(
        fixture.request(None, false).await.status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        fixture
            .request(Some("http://127.0.0.1:8420"), true)
            .await
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        fixture.request(Some("null"), true).await.status(),
        StatusCode::FORBIDDEN
    );
    assert!(fixture.requests.lock().await.is_empty());
}

#[tokio::test]
async fn gateway_streams_real_deltas_and_uses_provider_key_only_on_upstream() {
    let payload =
        frame(json!({"choices":[{"index":0,"delta":{"content":"hello"},"finish_reason":null}]}))
            + &frame(json!({"choices":[{"index":0,"delta":{},"finish_reason":"stop"}]}))
            + "data: [DONE]\n\n";
    let fixture = Fixture::new(&payload, StatusCode::OK).await;
    let response = fixture.request(None, true).await;
    assert_eq!(response.status(), StatusCode::OK);
    let text = body(response).await;
    assert!(text.contains("response.output_text.delta"));
    assert!(text.contains("response.completed"));
    assert!(!text.contains("FIXTURE_PROVIDER_KEY"));
    assert!(!text.contains(fixture.settings.gateway_token()));
    let requests = fixture.requests.lock().await;
    assert_eq!(requests.len(), 1);
    assert_eq!(requests[0].0, "Bearer FIXTURE_PROVIDER_KEY");
    assert_eq!(requests[0].1["model"], "fixture-model");
    assert_eq!(requests[0].1["messages"][0]["content"], "fixture prompt");
}

#[tokio::test]
async fn gateway_does_not_turn_eof_or_missing_finish_reason_into_success() {
    for payload in [
        frame(
            json!({"choices":[{"index":0,"delta":{"content":"partial"},"finish_reason":"stop"}]}),
        ),
        frame(json!({"choices":[{"index":0,"delta":{"content":"partial"},"finish_reason":null}]}))
            + "data: [DONE]\n\n",
        "data: invalid-json\n\n".into(),
    ] {
        let fixture = Fixture::new(&payload, StatusCode::OK).await;
        let text = body(fixture.request(None, true).await).await;
        assert!(text.contains("response.failed"));
        assert!(!text.contains("response.completed"));
    }
}

#[tokio::test]
async fn authentication_errors_preserve_status_without_echoing_provider_body() {
    let fixture = Fixture::new(
        "Authorization failed: FIXTURE_PROVIDER_KEY",
        StatusCode::UNAUTHORIZED,
    )
    .await;
    let response = fixture.request(None, true).await;
    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let text = body(response).await;
    assert!(!text.contains("FIXTURE_PROVIDER_KEY"));
    assert!(text.contains("Model provider rejected"));
}
