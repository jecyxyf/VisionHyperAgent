use std::time::Duration;

use serde_json::{json, Value};
use tokio::sync::oneshot;
use vha_codex_dds_agent::runtime::{AgentRuntime, RuntimeCommand, RuntimeEvent};

fn shell_quote(value: &str) -> String {
    format!("'{}'", value.replace('\'', "'\\''"))
}

fn make_mock_binary(root: &tempfile::TempDir, arguments: &[&str]) -> std::path::PathBuf {
    let mock = env!("CARGO_BIN_EXE_mock-codex-server");
    let mut command = format!("#!/bin/sh\nexec {}", shell_quote(mock));
    for argument in arguments {
        command.push(' ');
        command.push_str(&shell_quote(argument));
    }
    command.push_str(" \"$@\"\n");

    let path = root.path().join("codex-app-server");
    std::fs::write(&path, command).unwrap();
    make_executable(&path);
    path
}

#[cfg(unix)]
fn make_executable(path: &std::path::Path) {
    use std::os::unix::fs::PermissionsExt;
    std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755)).unwrap();
}

#[cfg(not(unix))]
fn make_executable(_path: &std::path::Path) {}

async fn start_mock(
    arguments: &[&str],
    root: &tempfile::TempDir,
) -> (AgentRuntime, tokio::sync::mpsc::Receiver<RuntimeEvent>) {
    AgentRuntime::start(
        &make_mock_binary(root, arguments),
        root.path(),
        &root.path().join("codex-app-server.log"),
    )
    .await
    .unwrap()
}

async fn next_reverse(events: &mut tokio::sync::mpsc::Receiver<RuntimeEvent>) -> String {
    loop {
        let event = tokio::time::timeout(Duration::from_secs(2), events.recv())
            .await
            .unwrap()
            .expect("runtime event channel closed");
        if let RuntimeEvent::ReverseRequest { reverse_id, .. } = event {
            return reverse_id;
        }
    }
}

#[tokio::test]
async fn rpc_requests_are_processed_in_fifo_order() {
    let root = tempfile::tempdir().unwrap();
    let (runtime, _events) = start_mock(&["--delay-ms", "80"], &root).await;
    let sender = runtime.command_sender();
    let (first_tx, first_rx) = oneshot::channel();
    let (second_tx, second_rx) = oneshot::channel();

    for (id, method, reply) in [
        ("first", "account/usage/read", first_tx),
        ("second", "account/rateLimits/read", second_tx),
    ] {
        sender
            .send(RuntimeCommand::Rpc {
                request_id: id.into(),
                method: method.into(),
                params: Value::Null,
                reply,
            })
            .unwrap();
    }

    let first = tokio::time::timeout(Duration::from_secs(2), first_rx)
        .await
        .unwrap()
        .unwrap();
    let second = tokio::time::timeout(Duration::from_secs(2), second_rx)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(first.request_id, "first");
    assert_eq!(second.request_id, "second");
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn reverse_requests_are_published_one_at_a_time() {
    let root = tempfile::tempdir().unwrap();
    let (runtime, mut events) = start_mock(&["--reverse-count", "2"], &root).await;
    let first_id = next_reverse(&mut events).await;

    let started = std::time::Instant::now();
    while started.elapsed() < Duration::from_millis(150) {
        if let Ok(Some(RuntimeEvent::ReverseRequest { .. })) =
            tokio::time::timeout(Duration::from_millis(20), events.recv()).await
        {
            panic!("queued reverse request was published before the active one completed");
        }
    }

    let (reply_tx, reply_rx) = oneshot::channel();
    runtime
        .command_sender()
        .send(RuntimeCommand::ReverseResponse {
            reverse_id: first_id,
            result: Some(json!({"ok": true})),
            error: None,
            reply: reply_tx,
        })
        .unwrap();
    tokio::time::timeout(Duration::from_secs(2), reply_rx)
        .await
        .unwrap()
        .unwrap()
        .unwrap();

    next_reverse(&mut events).await;
    runtime.stop().await.unwrap();
}

#[tokio::test]
async fn stop_fails_active_and_queued_requests() {
    let root = tempfile::tempdir().unwrap();
    let (runtime, _events) = start_mock(&["--delay-ms", "5000"], &root).await;
    let sender = runtime.command_sender();
    let (first_tx, first_rx) = oneshot::channel();
    let (second_tx, second_rx) = oneshot::channel();

    for (id, reply) in [("first", first_tx), ("second", second_tx)] {
        sender
            .send(RuntimeCommand::Rpc {
                request_id: id.into(),
                method: "account/usage/read".into(),
                params: Value::Null,
                reply,
            })
            .unwrap();
    }
    tokio::time::sleep(Duration::from_millis(100)).await;
    runtime.stop().await.unwrap();

    for response in [first_rx, second_rx] {
        let response = tokio::time::timeout(Duration::from_secs(2), response)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(response.error.unwrap().code, "agent_stopped");
    }
}

#[tokio::test]
async fn rpc_queue_has_no_capacity_limit_and_stop_waits_for_cleanup() {
    let root = tempfile::tempdir().unwrap();
    let (runtime, _events) = start_mock(&["--delay-ms", "5000"], &root).await;
    let sender = runtime.command_sender();
    let mut replies = Vec::new();

    for index in 0..1100 {
        let (reply_tx, reply_rx) = oneshot::channel();
        sender
            .send(RuntimeCommand::Rpc {
                request_id: format!("request-{index}"),
                method: "account/usage/read".into(),
                params: Value::Null,
                reply: reply_tx,
            })
            .unwrap();
        replies.push(reply_rx);
    }
    runtime.stop().await.unwrap();
    for reply in replies {
        let response = tokio::time::timeout(Duration::from_secs(2), reply)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(response.error.unwrap().code, "agent_stopped");
    }

    let (status_tx, status_rx) = oneshot::channel();
    let send_result = sender.send(RuntimeCommand::Status { reply: status_tx });
    assert!(send_result.is_err(), "worker must be closed after stop");
    drop(send_result);
    assert!(status_rx.await.is_err());
}

#[tokio::test]
async fn crashed_process_is_restarted_and_handshakes_again() {
    let root = tempfile::tempdir().unwrap();
    let crash_marker = root.path().join("crash-once");
    let arguments = vec![
        "--crash-once-file".to_string(),
        crash_marker.to_string_lossy().into_owned(),
    ];
    let argument_refs = arguments.iter().map(String::as_str).collect::<Vec<_>>();
    let (runtime, mut events) = start_mock(&argument_refs, &root).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(crash_marker.exists(), "crash marker was not created");

    let (reply_tx, reply_rx) = oneshot::channel();
    runtime
        .command_sender()
        .send(RuntimeCommand::Rpc {
            request_id: "after-recovery".into(),
            method: "account/usage/read".into(),
            params: Value::Null,
            reply: reply_tx,
        })
        .unwrap();

    let mut saw_failure = false;
    loop {
        let event = tokio::time::timeout(Duration::from_secs(3), events.recv())
            .await
            .unwrap()
            .expect("runtime event channel closed");
        if let RuntimeEvent::State {
            state,
            websocket_state,
            codex_process_state,
            ..
        } = event
        {
            saw_failure |= websocket_state == "disconnected" && codex_process_state == "stopped";
            if saw_failure && state == "running" && websocket_state == "connected" {
                break;
            }
        }
    }

    let response = tokio::time::timeout(Duration::from_secs(2), reply_rx)
        .await
        .unwrap()
        .unwrap();
    assert!(response.ok);
    assert!(crash_marker.exists());
    runtime.stop().await.unwrap();
}
