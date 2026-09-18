//! Domain tests backed by a real mock WebSocket, not a mock CodexAgent implementation.
use super::*;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tokio::task::JoinHandle;
use vha_codex_agent::AgentConfig;

pub(crate) struct Fixture {
    pub service: Arc<AgentService>,
    pub requests: mpsc::Receiver<Value>,
    pub sender: mpsc::Sender<Value>,
    pub root: std::path::PathBuf,
    server: JoinHandle<()>,
    pump: JoinHandle<()>,
}
impl Drop for Fixture {
    fn drop(&mut self) {
        self.server.abort();
        self.pump.abort();
        let _ = std::fs::remove_dir_all(&self.root);
    }
}
impl Fixture {
    pub async fn new() -> Self {
        Self::with_capabilities(false).await
    }
    async fn with_capabilities(experimental: bool) -> Self {
        let root = std::env::temp_dir().join(format!("vha-service-{}", Uuid::new_v4()));
        std::fs::create_dir(&root).unwrap();
        let config=toml::to_string(&json!({"codex_path":std::env::current_exe().unwrap(),"base_url":"http://127.0.0.1:1/v1","model":"fixture-model","api_key":"FIXTURE_PRIVATE_TOKEN"})).unwrap();
        let mut settings = CodexSettings::from_toml(&root, &config).unwrap();
        let _ = settings.prepare().unwrap();
        let uploads = Arc::new(AttachmentStore::new(&settings.workspace).await.unwrap());
        let cwd = settings.workspace.to_string_lossy().into_owned();
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let (sender, mut commands) = mpsc::channel::<Value>(64);
        let (received, requests) = mpsc::channel(64);
        let server = tokio::spawn(async move {
            let (tcp, _) = listener.accept().await.unwrap();
            let mut ws = tokio_tungstenite::accept_async(tcp).await.unwrap();
            let initial = ws.next().await.unwrap().unwrap();
            let initial: Value = serde_json::from_str(initial.to_text().unwrap()).unwrap();
            ws.send(tokio_tungstenite::tungstenite::Message::Text(
                json!({"id":initial["id"],"result":{}}).to_string().into(),
            ))
            .await
            .unwrap();
            let initialized = ws.next().await.unwrap().unwrap();
            assert!(initialized.to_text().unwrap().contains("initialized"));
            loop {
                tokio::select! {
                    frame=ws.next()=>match frame {
                        Some(Ok(tokio_tungstenite::tungstenite::Message::Text(text)))=>{if received.send(serde_json::from_str(&text).unwrap()).await.is_err(){break;}},
                        _=>break,
                    },
                    Some(value)=commands.recv()=>{if ws.send(tokio_tungstenite::tungstenite::Message::Text(value.to_string().into())).await.is_err(){break;}},
                }
            }
        });
        let client = CodexAgent::new(AgentConfig {
            websocket_url: format!("ws://{address}"),
            request_timeout: Duration::from_secs(2),
            max_reconnect_attempts: 0,
            experimental_api: experimental,
            ..Default::default()
        });
        let events = client.subscribe_events();
        let service = AgentService::new(client, Some(Arc::new(settings)), Some(uploads), None);
        let pump = tokio::spawn(service.clone().pump(events));
        service.client.connect().await.unwrap();
        service.process_status("running", None, None).await;
        let thread: Thread = serde_json::from_value(
            json!({"id":"thread-1","cwd":cwd,"turns":[],"status":{"type":"idle"}}),
        )
        .unwrap();
        service
            .state
            .lock()
            .await
            .empty_threads
            .insert(thread.id.clone(), thread);
        Self {
            service,
            requests,
            sender,
            root,
            server,
            pump,
        }
    }
    pub async fn next(&mut self) -> Value {
        tokio::time::timeout(Duration::from_secs(3), self.requests.recv())
            .await
            .unwrap()
            .unwrap()
    }
    pub async fn emit(&self, value: Value) {
        self.sender.send(value).await.unwrap();
    }
}
fn send_params() -> Value {
    json!({"threadId":"thread-1","text":"hello","model":"fixture-model","effort":"low","attachments":[],"clientMessageId":"ui-message-1"})
}

#[tokio::test]
async fn invalid_parameters_do_not_reserve_a_turn_or_send_rpc() {
    let mut fixture = Fixture::new().await;
    for (field, bad) in [
        ("model", json!("other-model")),
        ("effort", json!("invalid")),
        ("clientMessageId", json!("../../bad")),
        ("text", json!("")),
    ] {
        let mut params = send_params();
        params[field] = bad;
        assert!(fixture.service.call("turn.start", params).await.is_err());
        assert!(fixture.service.state.lock().await.active.is_empty());
    }
    assert!(fixture.requests.try_recv().is_err());
}

#[tokio::test]
async fn concurrent_start_is_rejected_and_client_message_id_is_forwarded() {
    let mut fixture = Fixture::new().await;
    let service = fixture.service.clone();
    let first = tokio::spawn(async move { service.call("turn.start", send_params()).await });
    let request = fixture.next().await;
    assert_eq!(request["method"], "turn/start");
    assert_eq!(request["params"]["clientUserMessageId"], "ui-message-1");
    assert_eq!(
        fixture
            .service
            .call("turn.start", send_params())
            .await
            .unwrap_err()
            .code,
        "busy"
    );
    assert!(fixture.requests.try_recv().is_err());
    fixture.emit(json!({"id":request["id"],"result":{"turn":{"id":"turn-1","status":"inProgress","items":[]}}})).await;
    first.await.unwrap().unwrap();
}

#[tokio::test]
async fn completion_before_start_ack_does_not_resurrect_a_busy_turn() {
    let mut fixture = Fixture::new().await;
    let service = fixture.service.clone();
    let call = tokio::spawn(async move { service.call("turn.start", send_params()).await });
    let request = fixture.next().await;
    fixture.emit(json!({"method":"turn/started","params":{"threadId":"thread-1","turn":{"id":"turn-1","status":"inProgress","items":[]}}})).await;
    fixture.emit(json!({"method":"turn/completed","params":{"threadId":"thread-1","turn":{"id":"turn-1","status":"completed","items":[]}}})).await;
    tokio::time::timeout(Duration::from_secs(2), async {
        while !fixture.service.state.lock().await.active.is_empty() {
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    fixture.emit(json!({"id":request["id"],"result":{"turn":{"id":"turn-1","status":"inProgress","items":[]}}})).await;
    call.await.unwrap().unwrap();
    assert!(fixture.service.state.lock().await.active.is_empty());
}

#[tokio::test]
async fn stop_during_pending_start_is_sent_when_turn_id_arrives() {
    let mut fixture = Fixture::new().await;
    let service = fixture.service.clone();
    let call = tokio::spawn(async move { service.call("turn.start", send_params()).await });
    let request = fixture.next().await;
    assert_eq!(
        fixture
            .service
            .call("turn.interrupt", json!({"threadId":"thread-1"}))
            .await
            .unwrap()["requested"],
        true
    );
    fixture.emit(json!({"method":"turn/started","params":{"threadId":"thread-1","turn":{"id":"turn-1","status":"inProgress","items":[]}}})).await;
    let interrupt = fixture.next().await;
    assert_eq!(interrupt["method"], "turn/interrupt");
    assert_eq!(interrupt["params"]["turnId"], "turn-1");
    fixture
        .emit(json!({"id":interrupt["id"],"result":{}}))
        .await;
    fixture.emit(json!({"method":"turn/completed","params":{"threadId":"thread-1","turn":{"id":"turn-1","status":"interrupted","items":[]}}})).await;
    fixture.emit(json!({"id":request["id"],"result":{"turn":{"id":"turn-1","status":"interrupted","items":[]}}})).await;
    call.await.unwrap().unwrap();
}

#[tokio::test]
async fn missing_or_unsupported_attachments_restore_idle_and_keep_files() {
    let fixture = Fixture::new().await;
    let image = fixture
        .service
        .uploads
        .as_ref()
        .unwrap()
        .store("image.png", b"\x89PNG\r\n\x1a\nfixture")
        .await
        .unwrap();
    for id in ["unknown-id".to_string(), image.id.clone()] {
        let mut params = send_params();
        params["attachments"] = json!([id]);
        assert_eq!(
            fixture
                .service
                .call("turn.start", params)
                .await
                .unwrap_err()
                .code,
            "attachment"
        );
        assert!(fixture.service.state.lock().await.active.is_empty());
    }
    fixture
        .service
        .uploads
        .as_ref()
        .unwrap()
        .remove(&image.id)
        .await
        .unwrap();
}

#[tokio::test]
async fn wrong_workspace_is_rejected_without_keeping_reservation() {
    let fixture = Fixture::new().await;
    fixture
        .service
        .state
        .lock()
        .await
        .empty_threads
        .get_mut("thread-1")
        .unwrap()
        .cwd = fixture.root.to_string_lossy().into_owned();
    assert_eq!(
        fixture
            .service
            .call("turn.start", send_params())
            .await
            .unwrap_err()
            .code,
        "wrong_workspace"
    );
    assert!(fixture.service.state.lock().await.active.is_empty());
}

#[tokio::test]
async fn frontend_cannot_call_process_controls_or_arbitrary_codex_methods() {
    let mut fixture = Fixture::new().await;
    for method in [
        "process.stop",
        "process.start",
        "command/exec",
        "config/write",
        "account/login/start",
    ] {
        assert_eq!(
            fixture
                .service
                .call(method, json!({}))
                .await
                .unwrap_err()
                .code,
            "unknown_method"
        );
    }
    assert!(fixture.requests.try_recv().is_err());
}

#[tokio::test]
async fn shutdown_state_transition_is_bounded_and_blocks_new_work() {
    let fixture = Fixture::new().await;
    tokio::time::timeout(Duration::from_millis(300), fixture.service.begin_shutdown())
        .await
        .unwrap();
    assert_eq!(fixture.service.snapshot().await["phase"], "stopping");
    assert_eq!(
        fixture
            .service
            .call("turn.start", send_params())
            .await
            .unwrap_err()
            .code,
        "closing"
    );
}

#[tokio::test]
async fn busy_thread_cannot_be_archived() {
    let mut fixture = Fixture::new().await;
    let service = fixture.service.clone();
    let call = tokio::spawn(async move { service.call("turn.start", send_params()).await });
    let request = fixture.next().await;
    assert_eq!(
        fixture
            .service
            .call("thread.archive", json!({"threadId":"thread-1"}))
            .await
            .unwrap_err()
            .code,
        "busy"
    );
    fixture.emit(json!({"id":request["id"],"result":{"turn":{"id":"turn-1","status":"inProgress","items":[]}}})).await;
    call.await.unwrap().unwrap();
}

#[tokio::test]
async fn lost_process_clears_busy_turns_and_pending_interactions() {
    let mut fixture = Fixture::new().await;
    let service = fixture.service.clone();
    let call = tokio::spawn(async move { service.call("turn.start", send_params()).await });
    let request = fixture.next().await;
    fixture.emit(json!({"id":request["id"],"result":{"turn":{"id":"turn-1","status":"inProgress","items":[]}}})).await;
    call.await.unwrap().unwrap();
    fixture
        .service
        .process_status("error", None, Some("child exited".into()))
        .await;
    let state = fixture.service.snapshot().await;
    assert_eq!(state["phase"], "error");
    assert_eq!(state["activeTurns"], json!({}));
    assert_eq!(state["interactions"], json!([]));
}

#[tokio::test]
async fn approval_is_bound_to_original_request_and_cannot_expand_allowed_decisions() {
    let mut fixture = Fixture::new().await;
    fixture.emit(json!({"id":"approval-1","method":"item/commandExecution/requestApproval","params":{"threadId":"thread-1","turnId":"turn-1","command":"echo safe","availableDecisions":["decline","cancel"]}})).await;
    let key = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let snapshot = fixture.service.snapshot().await;
            if let Some(key) = snapshot["interactions"][0]["key"].as_str() {
                return key.to_owned();
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    assert_eq!(
        fixture
            .service
            .call("interaction.reply", json!({"key":key,"decision":"accept"}))
            .await
            .unwrap_err()
            .code,
        "bad_request"
    );
    assert!(fixture.requests.try_recv().is_err());
    let service = fixture.service.clone();
    let pending = tokio::spawn(async move {
        service
            .call("interaction.reply", json!({"key":key,"decision":"decline"}))
            .await
    });
    let response = fixture.next().await;
    assert_eq!(
        response,
        json!({"id":"approval-1","result":{"decision":"decline"}})
    );
    pending.await.unwrap().unwrap();
}

#[tokio::test]
async fn stopping_a_turn_terminates_only_its_own_background_commands() {
    let mut fixture = Fixture::with_capabilities(true).await;
    let service = fixture.service.clone();
    let start = tokio::spawn(async move { service.call("turn.start", send_params()).await });
    let request = fixture.next().await;
    fixture.emit(json!({"id":request["id"],"result":{"turn":{"id":"turn-1","status":"inProgress","items":[]}}})).await;
    start.await.unwrap().unwrap();
    fixture.service.state.lock().await.command_items.insert(
        ("thread-1".into(), "turn-1".into()),
        HashSet::from(["current-item".into()]),
    );
    let service = fixture.service.clone();
    let cancel = tokio::spawn(async move {
        service
            .call("turn.interrupt", json!({"threadId":"thread-1"}))
            .await
    });
    let interrupt = fixture.next().await;
    assert_eq!(interrupt["method"], "turn/interrupt");
    fixture
        .emit(json!({"id":interrupt["id"],"result":{}}))
        .await;
    fixture.emit(json!({"method":"turn/completed","params":{"threadId":"thread-1","turn":{"id":"turn-1","status":"interrupted","items":[]}}})).await;
    let list = fixture.next().await;
    assert_eq!(list["method"], "thread/backgroundTerminals/list");
    assert_eq!(
        fixture
            .service
            .call("turn.start", send_params())
            .await
            .unwrap_err()
            .code,
        "busy"
    );
    fixture.emit(json!({"id":list["id"],"result":{"data":[{"itemId":"current-item","processId":"10"},{"itemId":"old-item","processId":"11"}],"nextCursor":null}})).await;
    let terminate = fixture.next().await;
    assert_eq!(terminate["method"], "thread/backgroundTerminals/terminate");
    assert_eq!(terminate["params"]["processId"], "10");
    fixture
        .emit(json!({"id":terminate["id"],"result":{"terminated":true}}))
        .await;
    for _ in 0..2 {
        let query = fixture.next().await;
        assert_eq!(query["method"], "thread/backgroundTerminals/list");
        fixture.emit(json!({"id":query["id"],"result":{"data":[{"itemId":"old-item","processId":"11"}],"nextCursor":null}})).await;
    }
    cancel.await.unwrap().unwrap();
    assert!(fixture.service.state.lock().await.cancelling.is_empty());
    assert!(fixture.requests.try_recv().is_err());
}

#[tokio::test]
async fn an_unsubmitted_draft_is_unsubscribed_instead_of_archiving_a_missing_rollout() {
    let mut fixture = Fixture::new().await;
    let service = fixture.service.clone();
    let archive = tokio::spawn(async move {
        service
            .call("thread.archive", json!({"threadId":"thread-1"}))
            .await
    });
    let request = fixture.next().await;
    assert_eq!(request["method"], "thread/unsubscribe");
    fixture
        .emit(json!({"id":request["id"],"result":{"status":"unsubscribed"}}))
        .await;
    archive.await.unwrap().unwrap();
    assert!(!fixture
        .service
        .state
        .lock()
        .await
        .empty_threads
        .contains_key("thread-1"));
}

#[tokio::test]
async fn input_answers_are_validated_and_mapped_to_native_question_ids() {
    let mut fixture = Fixture::new().await;
    fixture.emit(json!({"id":"question-1","method":"item/tool/requestUserInput","params":{"threadId":"thread-1","questions":[{"id":"target","question":"what?"}]}})).await;
    let key = tokio::time::timeout(Duration::from_secs(2), async {
        loop {
            let s = fixture.service.snapshot().await;
            if let Some(key) = s["interactions"][0]["key"].as_str() {
                return key.to_owned();
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
    })
    .await
    .unwrap();
    assert!(fixture
        .service
        .call(
            "interaction.reply",
            json!({"key":key,"answers":{"other":"x"}})
        )
        .await
        .is_err());
    assert!(fixture.requests.try_recv().is_err());
    let service = fixture.service.clone();
    let call = tokio::spawn(async move {
        service
            .call(
                "interaction.reply",
                json!({"key":key,"answers":{"target":"crack"}}),
            )
            .await
    });
    assert_eq!(
        fixture.next().await,
        json!({"id":"question-1","result":{"answers":{"target":{"answers":["crack"]}}}})
    );
    call.await.unwrap().unwrap();
}
