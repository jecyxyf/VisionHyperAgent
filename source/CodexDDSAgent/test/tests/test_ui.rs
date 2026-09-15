use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    path::PathBuf,
    process::{Child, Command, Stdio},
    time::{Duration, Instant},
};

use serde_json::{json, Value};

#[test]
fn test_ui_supports_multiple_desktops_over_one_service() {
    let first_home = tempfile::tempdir().unwrap();
    let second_home = tempfile::tempdir().unwrap();
    let first_ui = TestUi::start(first_home.path().to_path_buf());
    let second_ui = TestUi::start(second_home.path().to_path_buf());
    let service_name = format!("ui-auto-{}", std::process::id());
    let service_port = free_port();

    let service = post_json(
        first_ui.port,
        "/api/service/start",
        json!({
            "service_name": service_name,
            "agent_name": "initial-agent",
            "fixed_port": service_port,
            "discovery_enabled": false
        }),
    );
    let endpoint = service["endpoint"].as_str().unwrap().to_string();

    let mut invalid_model = model_payload("missing-model");
    invalid_model["default_provider"] = json!("main");
    let invalid = post_json(
        first_ui.port,
        "/api/agent/create",
        agent_request(
            &service_name,
            "invalid-model",
            Some(&endpoint),
            invalid_model,
        ),
    );
    assert_eq!(invalid["ok"], false);
    assert_eq!(invalid["error"]["code"], "invalid_model_config");
    assert!(!first_home
        .path()
        .join("data/codex/invalid-model/config.toml")
        .exists());

    let first_agent = post_json(
        first_ui.port,
        "/api/agent/create",
        agent_request(
            &service_name,
            "desktop-a",
            Some(&endpoint),
            model_payload("model-a"),
        ),
    );
    assert_eq!(first_agent["state"], "running");

    let opened = post_json(
        second_ui.port,
        "/api/session/open",
        json!({"endpoint": endpoint}),
    );
    assert_eq!(opened["ok"], true);
    let second_agent = post_json(
        second_ui.port,
        "/api/agent/create",
        agent_request(
            &service_name,
            "desktop-b",
            Some(&endpoint),
            model_payload("model-b"),
        ),
    );
    assert_eq!(second_agent["state"], "running");

    for (ui, agent_name) in [(first_ui.port, "desktop-a"), (second_ui.port, "desktop-b")] {
        let heartbeat = post_json(
            ui,
            "/api/agent/heartbeat",
            agent_request(&service_name, agent_name, Some(&endpoint), Value::Null),
        );
        assert_eq!(heartbeat["ok"], true);

        let status = post_json(
            ui,
            "/api/agent/status",
            agent_request(&service_name, agent_name, Some(&endpoint), Value::Null),
        );
        assert_eq!(status["state"], "running");
        assert_eq!(status["heartbeat_active"], true);
        assert_eq!(status["websocket_state"], "connected");

        let rpc = post_json(
            ui,
            "/api/rpc",
            json!({
                "endpoint": endpoint,
                "service_name": service_name,
                "agent_name": agent_name,
                "method": "account/usage/read",
                "params": {"client": agent_name}
            }),
        );
        assert_eq!(rpc["ok"], true);
        assert_eq!(rpc["result"]["method"], "account/usage/read");
        assert_eq!(rpc["result"]["params"]["client"], agent_name);

        let deadline = Instant::now() + Duration::from_secs(5);
        let logs = loop {
            let logs = post_json(
                ui,
                "/api/logs",
                agent_request(&service_name, agent_name, Some(&endpoint), Value::Null),
            );
            if !logs["records"].as_array().unwrap().is_empty() || Instant::now() > deadline {
                break logs;
            }
            std::thread::sleep(Duration::from_millis(100));
        };
        assert!(
            !logs["records"].as_array().unwrap().is_empty(),
            "agent {agent_name} did not receive status feed"
        );

        let reverse = wait_for_reverse(ui, &service_name, agent_name, &endpoint);
        let reverse_id = reverse["reverse_id"].as_str().unwrap().to_string();
        assert_eq!(reverse["method"], "currentTime/read");
        let accepted = post_json(
            ui,
            "/api/reverse/response",
            json!({
                "endpoint": endpoint,
                "service_name": service_name,
                "agent_name": agent_name,
                "reverse_id": reverse_id,
                "result": {"approved": true, "value": agent_name},
                "error": null
            }),
        );
        assert_eq!(accepted["ok"], true, "reverse response failed: {accepted}");
        wait_for_reverse_ack(ui, &service_name, agent_name, &reverse_id);

        let detached = post_json(
            ui,
            "/api/agent/detach",
            agent_request(&service_name, agent_name, Some(&endpoint), Value::Null),
        );
        assert_eq!(detached["state"], "stopped");
    }

    let shutdown = post_json(
        first_ui.port,
        "/api/service/shutdown",
        json!({"service_name": service_name}),
    );
    assert_eq!(shutdown["ok"], true);
}

fn wait_for_reverse(ui_port: u16, service_name: &str, agent_name: &str, endpoint: &str) -> Value {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let logs = post_json(
            ui_port,
            "/api/logs",
            agent_request(service_name, agent_name, Some(endpoint), Value::Null),
        );
        if let Some(item) = logs["reverse"]
            .as_array()
            .into_iter()
            .flatten()
            .find(|item| item["method"] == "currentTime/read")
        {
            return item.clone();
        }
        assert!(
            Instant::now() < deadline,
            "agent {agent_name} did not receive reverse request: {logs}"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}

fn wait_for_reverse_ack(ui_port: u16, service_name: &str, agent_name: &str, reverse_id: &str) {
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        let logs = post_json(
            ui_port,
            "/api/logs",
            json!({
                "service_name": service_name,
                "agent_name": agent_name
            }),
        );
        let acknowledged = logs["records"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|item| {
                item["method"] == "turn/started"
                    && item["params"]["reason"] == "reverse-response"
                    && item["params"]["result"]["value"] == agent_name
            });
        let marked = logs["reverse"]
            .as_array()
            .into_iter()
            .flatten()
            .any(|item| item["reverse_id"] == reverse_id && item["answered"] == true);
        if acknowledged && marked {
            return;
        }
        assert!(
            Instant::now() < deadline,
            "reverse response was not acknowledged by mock Codex: {logs}"
        );
        std::thread::sleep(Duration::from_millis(100));
    }
}

struct TestUi {
    port: u16,
    child: Child,
}

impl TestUi {
    fn start(home: PathBuf) -> Self {
        let port = free_port();
        let child = Command::new(env!("CARGO_BIN_EXE_codex-dds-testui"))
            .arg("--http-port")
            .arg(port.to_string())
            .arg("--home")
            .arg(home)
            .arg("--no-discovery")
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()
            .expect("spawn CodexDDSAgent Test UI");

        let mut ui = Self { port, child };
        let deadline = Instant::now() + Duration::from_secs(20);
        loop {
            if let Ok(Some(status)) = ui.child.try_wait() {
                panic!("Test UI exited during startup: {status}");
            }
            if get_text(ui.port, "/").is_ok() {
                break;
            }
            assert!(Instant::now() < deadline, "Test UI did not become ready");
            std::thread::sleep(Duration::from_millis(100));
        }
        ui
    }
}

impl Drop for TestUi {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

fn agent_request(
    service_name: &str,
    agent_name: &str,
    endpoint: Option<&str>,
    model: Value,
) -> Value {
    json!({
        "endpoint": endpoint,
        "service_name": service_name,
        "agent_name": agent_name,
        "model": model
    })
}

fn model_payload(default_model: &str) -> Value {
    json!({
        "default_provider": "main",
        "providers": [{
            "id": "main",
            "base_url": "https://api.example.com/v1",
            "api_key": "test-key",
            "default_model": default_model,
            "models": ["model-a", "model-b"]
        }]
    })
}

fn post_json(port: u16, path: &str, body: Value) -> Value {
    let (status, body) = http_request(port, path, Some(body.to_string())).unwrap();
    assert_eq!(status, 200, "HTTP body: {body}");
    serde_json::from_str(&body).unwrap()
}

fn get_text(port: u16, path: &str) -> Result<String, String> {
    http_request(port, path, None).map(|(_, body)| body)
}

fn http_request(port: u16, path: &str, body: Option<String>) -> Result<(u16, String), String> {
    let address = SocketAddr::from(([127, 0, 0, 1], port));
    let mut stream = TcpStream::connect_timeout(&address, Duration::from_secs(2))
        .map_err(|error| error.to_string())?;
    stream
        .set_read_timeout(Some(Duration::from_secs(5)))
        .map_err(|error| error.to_string())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(5)))
        .map_err(|error| error.to_string())?;

    let body = body.unwrap_or_default();
    let request = format!(
        "POST {path} HTTP/1.1\r\nHost: 127.0.0.1:{port}\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}",
        body.len()
    );
    stream
        .write_all(request.as_bytes())
        .map_err(|error| error.to_string())?;

    let mut response = Vec::new();
    stream
        .read_to_end(&mut response)
        .map_err(|error| error.to_string())?;
    let response = String::from_utf8_lossy(&response).to_string();
    let status = response
        .split_whitespace()
        .nth(1)
        .and_then(|value| value.parse::<u16>().ok())
        .ok_or_else(|| format!("invalid HTTP response: {response}"))?;
    let body = response
        .split_once("\r\n\r\n")
        .map(|(_, body)| body.to_string())
        .unwrap_or_default();
    Ok((status, body))
}

fn free_port() -> u16 {
    TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port()
}
