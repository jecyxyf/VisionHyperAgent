use super::*;
use crate::agent_service::tests::Fixture;
use crate::shutdown::ShutdownController;
use axum::body::Body;
use http_body_util::BodyExt;
use tower::ServiceExt;

fn router(fixture: &Fixture) -> Router {
    let shutdown = ShutdownController::new();
    // Non-websocket tests don't need a live supervisor. Origin/HTTP handling is still real.
    routes(
        fixture.service.clone(),
        ShutdownSignal::from_controller(&shutdown),
        "127.0.0.1:8420".parse().unwrap(),
    )
}
async fn value(response: Response) -> Value {
    serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap()
}
fn form(content: &str) -> String {
    format!("--vha-boundary\r\nContent-Disposition: form-data; name=\"file\"; filename=\"../../说明.txt\"\r\nContent-Type: text/plain\r\n\r\n{content}\r\n--vha-boundary--\r\n")
}

#[tokio::test]
async fn remote_null_and_spoofed_origins_are_rejected_on_http_and_websocket() {
    let fixture = Fixture::new().await;
    for origin in [
        "https://evil.invalid",
        "null",
        "http://127.0.0.1:8420.evil.invalid",
        "http://localhost:9999",
    ] {
        for endpoint in ["/api/agent/status", "/ws"] {
            let request = Request::builder()
                .uri(endpoint)
                .header(ORIGIN, origin)
                .body(Body::empty())
                .unwrap();
            assert_eq!(
                router(&fixture).oneshot(request).await.unwrap().status(),
                StatusCode::FORBIDDEN
            );
        }
    }
}

#[tokio::test]
async fn local_status_is_real_and_contains_no_provider_secret() {
    let fixture = Fixture::new().await;
    let request = Request::builder()
        .uri("/api/agent/status")
        .header(ORIGIN, "http://127.0.0.1:8420")
        .body(Body::empty())
        .unwrap();
    let response = router(&fixture).oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let status = value(response).await;
    assert_eq!(status["phase"], "ready");
    assert_eq!(status["models"][0]["model"], "fixture-model");
    assert!(!status.to_string().contains("FIXTURE_PRIVATE_TOKEN"));
}

#[tokio::test]
async fn uploads_store_bytes_with_opaque_ids_and_can_be_removed_before_use() {
    let fixture = Fixture::new().await;
    let request = Request::builder()
        .method("POST")
        .uri("/api/agent/attachments")
        .header(ORIGIN, "http://localhost:8420")
        .header("content-type", "multipart/form-data; boundary=vha-boundary")
        .body(Body::from(form("UTF8 文件测试")))
        .unwrap();
    let response = router(&fixture).oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let response = value(response).await;
    let file = &response["attachment"];
    assert_eq!(file["name"], "说明.txt");
    let id = file["id"].as_str().unwrap();
    assert!(uuid::Uuid::parse_str(id).is_ok());
    let path = fixture
        .service
        .settings
        .as_ref()
        .unwrap()
        .workspace
        .join(".vha-attachments")
        .join(format!("{id}.txt"));
    assert_eq!(std::fs::read_to_string(&path).unwrap(), "UTF8 文件测试");
    let request = Request::builder()
        .method("DELETE")
        .uri(format!("/api/agent/attachments/{id}"))
        .body(Body::empty())
        .unwrap();
    assert_eq!(
        router(&fixture).oneshot(request).await.unwrap().status(),
        StatusCode::OK
    );
    assert!(!path.exists());
}

#[tokio::test]
async fn invalid_empty_and_cross_origin_uploads_do_not_create_files() {
    let fixture = Fixture::new().await;
    let foreign = Request::builder()
        .method("POST")
        .uri("/api/agent/attachments")
        .header(ORIGIN, "https://evil.invalid")
        .body(Body::from("not multipart"))
        .unwrap();
    assert_eq!(
        router(&fixture).oneshot(foreign).await.unwrap().status(),
        StatusCode::FORBIDDEN
    );
    let empty = Request::builder()
        .method("POST")
        .uri("/api/agent/attachments")
        .header("content-type", "multipart/form-data; boundary=vha-boundary")
        .body(Body::from(form("")))
        .unwrap();
    assert_eq!(
        router(&fixture).oneshot(empty).await.unwrap().status(),
        StatusCode::BAD_REQUEST
    );
    let directory = fixture
        .service
        .settings
        .as_ref()
        .unwrap()
        .workspace
        .join(".vha-attachments");
    assert_eq!(std::fs::read_dir(directory).unwrap().count(), 0);
}
