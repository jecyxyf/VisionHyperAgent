use super::*;
use crate::agent_service::tests::Fixture;
use axum::body::Body;
use http_body_util::BodyExt;
use serde_json::Value;
use tower::ServiceExt;
use vha_codex_agent::{AppConfig, CodexConfig, ModelConfig, ProviderConfig, ProviderWireApi};

fn manager() -> &'static ConfigManager {
    let root = std::env::temp_dir().join(format!("vha-settings-{}", uuid::Uuid::new_v4()));
    std::fs::create_dir(&root).unwrap();
    let app = AppConfig {
        version: 1,
        agent: vha_codex_agent::AgentConfig {
            active_model_id: "fixture-model".into(),
            providers: vec![ProviderConfig {
                id: "fixture-provider".into(),
                base_url: "http://127.0.0.1:1/v1".into(),
                api_key: "SETTINGS_FIXTURE_KEY".into(),
                wire_api: ProviderWireApi::ChatCompletions,
                models: vec![ModelConfig {
                    model_id: "fixture-model".into(),
                    model_name: "upstream-fixture-model".into(),
                    ..Default::default()
                }],
            }],
            codex: CodexConfig::default(),
        },
    };
    vha_common::config::save(&root.join("app_config.json"), &app).unwrap();
    Box::leak(Box::new(ConfigManager::new(&root).unwrap()))
}

fn router(fixture: &Fixture, manager: &'static ConfigManager) -> Router {
    routes_with_manager(
        fixture.service.clone(),
        "127.0.0.1:8420".parse().unwrap(),
        Some(manager),
    )
}

async fn value(response: Response) -> Value {
    serde_json::from_slice(&response.into_body().collect().await.unwrap().to_bytes()).unwrap()
}

async fn get(fixture: &Fixture, manager: &'static ConfigManager) -> Value {
    let request = Request::builder()
        .uri("/api/settings/agent")
        .header(ORIGIN, "http://127.0.0.1:8420")
        .body(Body::empty())
        .unwrap();
    let response = router(fixture, manager).oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    value(response).await
}

#[tokio::test]
async fn settings_api_hides_and_retains_provider_keys() {
    let fixture = Fixture::new().await;
    let manager = manager();
    let mut settings = get(&fixture, manager).await;
    assert!(!settings.to_string().contains("SETTINGS_FIXTURE_KEY"));
    assert_eq!(settings["agent"]["providers"][0]["apiKey"], "");
    assert_eq!(settings["agent"]["providers"][0]["apiKeyConfigured"], true);

    settings["agent"]["providers"][0]["models"][0]["modelName"] = "updated-upstream-model".into();
    let request = Request::builder()
        .method("POST")
        .uri("/api/settings/agent")
        .header(ORIGIN, "http://127.0.0.1:8420")
        .header("content-type", "application/json")
        .header("x-vha-settings", "1")
        .body(Body::from(
            json!({"revision":settings["revision"],"agent":settings["agent"]}).to_string(),
        ))
        .unwrap();
    let response = router(&fixture, manager).oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let result = value(response).await;
    assert_eq!(result["ok"], true);
    assert_eq!(result["restartRequired"], false);
    assert!(!result.to_string().contains("SETTINGS_FIXTURE_KEY"));

    let saved = std::fs::read_to_string(manager.path()).unwrap();
    assert!(saved.contains("SETTINGS_FIXTURE_KEY"));
    assert!(saved.contains("updated-upstream-model"));
    assert_eq!(
        manager
            .current_config()
            .model("fixture-model")
            .unwrap()
            .model_name,
        "updated-upstream-model"
    );
}

#[tokio::test]
async fn settings_api_rejects_foreign_origin_missing_header_and_bad_configs() {
    let fixture = Fixture::new().await;
    let manager = manager();
    let settings = get(&fixture, manager).await;

    let foreign = Request::builder()
        .method("POST")
        .uri("/api/settings/agent")
        .header(ORIGIN, "https://evil.invalid")
        .header("content-type", "application/json")
        .header("x-vha-settings", "1")
        .body(Body::from(
            json!({"revision":settings["revision"],"agent":settings["agent"]}).to_string(),
        ))
        .unwrap();
    assert_eq!(
        router(&fixture, manager)
            .oneshot(foreign)
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );

    let missing_header = Request::builder()
        .method("POST")
        .uri("/api/settings/agent")
        .header(ORIGIN, "http://127.0.0.1:8420")
        .header("content-type", "application/json")
        .body(Body::from(
            json!({"revision":settings["revision"],"agent":settings["agent"]}).to_string(),
        ))
        .unwrap();
    assert_eq!(
        router(&fixture, manager)
            .oneshot(missing_header)
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );

    let mut invalid = settings.clone();
    invalid["agent"]["activeModelId"] = "missing-model".into();
    let invalid_request = Request::builder()
        .method("POST")
        .uri("/api/settings/agent")
        .header(ORIGIN, "http://localhost:8420")
        .header("content-type", "application/json")
        .header("x-vha-settings", "1")
        .body(Body::from(
            json!({"revision":settings["revision"],"agent":invalid["agent"]}).to_string(),
        ))
        .unwrap();
    let response = router(&fixture, manager)
        .oneshot(invalid_request)
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    assert_eq!(value(response).await["error"]["code"], "invalid_config");
}

#[tokio::test]
async fn settings_api_uses_revisions_and_reports_codex_restart() {
    let fixture = Fixture::new().await;
    let manager = manager();
    let mut settings = get(&fixture, manager).await;

    settings["agent"]["codex"]["workspace"] = "data/updated-workspace".into();
    let request = Request::builder()
        .method("POST")
        .uri("/api/settings/agent")
        .header(ORIGIN, "http://127.0.0.1:8420")
        .header("content-type", "application/json")
        .header("x-vha-settings", "1")
        .body(Body::from(
            json!({"revision":"stale-revision","agent":settings["agent"]}).to_string(),
        ))
        .unwrap();
    let response = router(&fixture, manager).oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::CONFLICT);
    assert_eq!(value(response).await["error"]["code"], "config_conflict");

    let request = Request::builder()
        .method("POST")
        .uri("/api/settings/agent")
        .header(ORIGIN, "http://127.0.0.1:8420")
        .header("content-type", "application/json")
        .header("x-vha-settings", "1")
        .body(Body::from(
            json!({"revision":settings["revision"],"agent":settings["agent"]}).to_string(),
        ))
        .unwrap();
    let response = router(&fixture, manager).oneshot(request).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(value(response).await["restartRequired"], true);
    assert_eq!(fixture.service.snapshot().await["phase"], "error");
}
