mod support;
use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use std::time::Duration;
use support::*;
use tokio::sync::oneshot;
use tokio::time::timeout;
use tokio_tungstenite::tungstenite::Message;
use vha_codex_agent::*;

#[tokio::test]
async fn handshake_and_models_use_real_frames() {
    let (listener, agent) = fixture().await;
    let server = tokio::spawn(async move {
        let mut ws = accept(&listener).await;
        handshake(&mut ws).await;
        let req = receive(&mut ws).await;
        assert_eq!(req["method"], "model/list");
        assert_eq!(req["params"]["cursor"], "cursor-1");
        send(&mut ws, json!({"id":req["id"],"result":{"data":[{"id":"test","model":"test-model","displayName":"Test model","inputModalities":["text"],"defaultReasoningEffort":"low","supportedReasoningEfforts":[{"reasoningEffort":"low","description":"fast"}]}],"nextCursor":"cursor-2"}})).await;
        assert!(matches!(
            ws.next().await,
            Some(Ok(Message::Close(_))) | None
        ));
    });
    agent.connect().await.unwrap();
    assert_eq!(agent.status().phase, ConnectionPhase::Ready);
    let models = agent.list_models(Some("cursor-1")).await.unwrap();
    assert_eq!(models.data[0].model, "test-model");
    assert_eq!(models.data[0].input_modalities, vec!["text"]);
    assert_eq!(models.next_cursor.as_deref(), Some("cursor-2"));
    agent.disconnect().await.unwrap();
    server.await.unwrap();
    assert_eq!(agent.status().phase, ConnectionPhase::Disconnected);
}

#[tokio::test]
async fn concurrent_responses_can_arrive_in_reverse_order() {
    let (listener, agent) = fixture().await;
    let server = tokio::spawn(async move {
        let mut ws = accept(&listener).await;
        handshake(&mut ws).await;
        let first = receive(&mut ws).await;
        let second = receive(&mut ws).await;
        assert_ne!(first["id"], second["id"]);
        for req in [second, first] {
            send(
                &mut ws,
                json!({"id":req["id"],"result":{"cwd":req["params"]["cwds"][0]}}),
            )
            .await;
        }
    });
    agent.connect().await.unwrap();
    let (a, b) = tokio::join!(agent.list_skills("/a"), agent.list_skills("/b"));
    assert_eq!(a.unwrap(), json!({"cwd":"/a"}));
    assert_eq!(b.unwrap(), json!({"cwd":"/b"}));
    server.await.unwrap();
    agent.disconnect().await.unwrap();
}

#[tokio::test]
async fn pending_rpc_does_not_block_notifications_or_user_approval() {
    let (listener, agent) = fixture().await;
    let mut events = agent.subscribe_events();
    let server = tokio::spawn(async move {
        let mut ws = accept(&listener).await;
        handshake(&mut ws).await;
        let req = receive(&mut ws).await;
        send(&mut ws,json!({"method":"item/agentMessage/delta","params":{"threadId":"t","turnId":"u","itemId":"i","delta":"hello"}})).await;
        send(&mut ws,json!({"id":"approval-A","method":"item/commandExecution/requestApproval","params":{"threadId":"t","turnId":"u"}})).await;
        assert_eq!(
            receive(&mut ws).await,
            json!({"id":"approval-A","result":{"decision":"decline"}})
        );
        send(&mut ws, json!({"id":req["id"],"result":{"data":[]}})).await;
    });
    agent.connect().await.unwrap();
    let task_agent = agent.clone();
    let call = tokio::spawn(async move { task_agent.list_models(None).await });
    let mut saw_delta = false;
    let request = timeout(DEADLINE, async {
        loop {
            match events.recv().await.unwrap() {
                AgentEvent::Notification { method, params }
                    if method == "item/agentMessage/delta" =>
                {
                    assert_eq!(params["delta"], "hello");
                    saw_delta = true;
                }
                AgentEvent::ServerRequest { request } => break request,
                _ => {}
            }
        }
    })
    .await
    .unwrap();
    assert!(saw_delta);
    agent
        .approve(&request, ApprovalDecision::Decline)
        .await
        .unwrap();
    assert!(call.await.unwrap().unwrap().data.is_empty());
    server.await.unwrap();
    agent.disconnect().await.unwrap();
}

#[tokio::test]
async fn approval_can_be_answered_exactly_once() {
    let (listener, agent) = fixture().await;
    let mut events = agent.subscribe_events();
    let (done_tx, done_rx) = oneshot::channel();
    let server = tokio::spawn(async move {
        let mut ws = accept(&listener).await;
        handshake(&mut ws).await;
        send(
            &mut ws,
            json!({"id":91,"method":"item/fileChange/requestApproval","params":{}}),
        )
        .await;
        assert_eq!(
            receive(&mut ws).await,
            json!({"id":91,"result":{"decision":"accept"}})
        );
        done_rx.await.unwrap();
    });
    agent.connect().await.unwrap();
    let request = interaction(&mut events).await;
    agent
        .approve(&request, ApprovalDecision::Accept)
        .await
        .unwrap();
    assert!(matches!(
        agent.approve(&request, ApprovalDecision::Accept).await,
        Err(AgentError::StaleRequest)
    ));
    done_tx.send(()).unwrap();
    server.await.unwrap();
    agent.disconnect().await.unwrap();
}

#[tokio::test]
async fn unknown_server_requests_are_rejected_not_approved() {
    let (listener, agent) = fixture().await;
    let server = tokio::spawn(async move {
        let mut ws = accept(&listener).await;
        handshake(&mut ws).await;
        send(
            &mut ws,
            json!({"id":"unsafe","method":"unknown/writeAnything","params":{}}),
        )
        .await;
        let response = receive(&mut ws).await;
        assert_eq!(response["id"], "unsafe");
        assert_eq!(response["error"]["code"], -32601);
        assert!(response.get("result").is_none());
    });
    agent.connect().await.unwrap();
    server.await.unwrap();
    agent.disconnect().await.unwrap();
}

#[tokio::test]
async fn missing_interaction_consumer_denies_request() {
    let (listener, agent) = fixture().await;
    let server = tokio::spawn(async move {
        let mut ws = accept(&listener).await;
        handshake(&mut ws).await;
        send(
            &mut ws,
            json!({"id":1,"method":"item/fileChange/requestApproval","params":{}}),
        )
        .await;
        let response = receive(&mut ws).await;
        assert!(response.get("error").is_some());
        assert!(response.get("result").is_none());
    });
    agent.connect().await.unwrap();
    server.await.unwrap();
    agent.disconnect().await.unwrap();
}

#[tokio::test]
async fn expired_approval_is_denied_and_reported() {
    let (listener, agent) = fixture().await;
    let mut events = agent.subscribe_events();
    let server = tokio::spawn(async move {
        let mut ws = accept(&listener).await;
        handshake(&mut ws).await;
        send(&mut ws,json!({"id":"expiring","method":"item/tool/requestUserInput","params":{"questions":[]}})).await;
        let response = receive(&mut ws).await;
        assert_eq!(response["id"], "expiring");
        assert!(response.get("error").is_some());
    });
    agent.connect().await.unwrap();
    let req = interaction(&mut events).await;
    timeout(DEADLINE, async {
        loop {
            if let AgentEvent::ServerRequestExpired { id, .. } = events.recv().await.unwrap() {
                assert_eq!(id, req.id);
                break;
            }
        }
    })
    .await
    .unwrap();
    server.await.unwrap();
    agent.disconnect().await.unwrap();
}

#[tokio::test]
async fn input_answers_preserve_the_server_request_id() {
    let (listener, agent) = fixture().await;
    let mut events = agent.subscribe_events();
    let server = tokio::spawn(async move {
        let mut ws = accept(&listener).await;
        handshake(&mut ws).await;
        send(&mut ws,json!({"id":"input-1","method":"item/tool/requestUserInput","params":{"questions":[{"id":"q"}]}})).await;
        assert_eq!(
            receive(&mut ws).await,
            json!({"id":"input-1","result":{"answers":{"q":{"answers":["option-1"]}}}})
        );
    });
    agent.connect().await.unwrap();
    let req = interaction(&mut events).await;
    assert!(matches!(
        agent.approve(&req, ApprovalDecision::Accept).await,
        Err(AgentError::UnsupportedMethod)
    ));
    agent
        .respond(&req, Ok(json!({"answers":{"q":{"answers":["option-1"]}}})))
        .await
        .unwrap();
    server.await.unwrap();
    agent.disconnect().await.unwrap();
}

#[tokio::test]
async fn request_timeout_ignores_late_response_without_corrupting_next_request() {
    let (listener, agent) = fixture().await;
    let server = tokio::spawn(async move {
        let mut ws = accept(&listener).await;
        handshake(&mut ws).await;
        let stale = receive(&mut ws).await;
        let next = receive(&mut ws).await;
        send(&mut ws, json!({"id":stale["id"],"result":{"value":"late"}})).await;
        send(&mut ws, json!({"id":next["id"],"result":{"value":"fresh"}})).await;
    });
    agent.connect().await.unwrap();
    assert!(matches!(
        agent.list_skills("/late").await,
        Err(AgentError::Timeout)
    ));
    assert_eq!(
        agent.list_skills("/fresh").await.unwrap(),
        json!({"value":"fresh"})
    );
    server.await.unwrap();
    agent.disconnect().await.unwrap();
}

#[tokio::test]
async fn disconnect_fails_pending_rpc_without_waiting_for_request_timeout() {
    let (listener, agent) = fixture().await;
    let (sent, received) = oneshot::channel();
    let server = tokio::spawn(async move {
        let mut ws = accept(&listener).await;
        handshake(&mut ws).await;
        receive(&mut ws).await;
        sent.send(()).unwrap();
        ws.close(None).await.unwrap();
    });
    agent.connect().await.unwrap();
    let copy = agent.clone();
    let pending = tokio::spawn(async move { copy.list_models(None).await });
    received.await.unwrap();
    assert!(matches!(
        timeout(Duration::from_millis(400), pending)
            .await
            .unwrap()
            .unwrap(),
        Err(AgentError::Disconnected)
    ));
    server.await.unwrap();
    agent.disconnect().await.unwrap();
}

#[tokio::test]
async fn reconnect_reinitializes_and_never_replays_a_turn() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mut cfg = config(&listener);
    cfg.max_reconnect_attempts = 3;
    let agent = CodexAgent::new(cfg);
    let (second_tx, second_rx) = oneshot::channel();
    let server = tokio::spawn(async move {
        let mut first = accept(&listener).await;
        handshake(&mut first).await;
        assert_eq!(receive(&mut first).await["method"], "turn/start");
        first.close(None).await.unwrap();
        drop(first);
        let mut second = accept(&listener).await;
        handshake(&mut second).await;
        second_tx.send(()).unwrap();
        let req = receive(&mut second).await;
        assert_eq!(
            req["method"], "model/list",
            "turn/start must not be replayed"
        );
        send(&mut second, json!({"id":req["id"],"result":{"data":[]}})).await;
    });
    agent.connect().await.unwrap();
    let epoch = agent.status().connection_id;
    let input = TurnInput {
        client_user_message_id: None,
        thread_id: "t".into(),
        input: vec![UserInput::Text {
            text: "one operation".into(),
        }],
        model: None,
        effort: None,
    };
    assert!(matches!(
        agent.start_turn(&input).await,
        Err(AgentError::Disconnected)
    ));
    second_rx.await.unwrap();
    phase(&agent, ConnectionPhase::Ready).await;
    assert!(agent.status().connection_id > epoch);
    agent.list_models(None).await.unwrap();
    agent.disconnect().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn closing_during_handshake_cancels_connect_promptly() {
    let (listener, agent) = fixture().await;
    let (init_tx, init_rx) = oneshot::channel();
    let server = tokio::spawn(async move {
        let mut ws = accept(&listener).await;
        receive(&mut ws).await;
        init_tx.send(()).unwrap();
        let _ = timeout(DEADLINE, ws.next()).await.unwrap();
    });
    let copy = agent.clone();
    let connecting = tokio::spawn(async move { copy.connect().await });
    init_rx.await.unwrap();
    agent.disconnect().await.unwrap();
    assert!(matches!(
        timeout(Duration::from_millis(400), connecting)
            .await
            .unwrap()
            .unwrap(),
        Err(AgentError::Disconnected)
    ));
    server.await.unwrap();
    assert_eq!(agent.status().phase, ConnectionPhase::Disconnected);
}

#[tokio::test]
async fn failed_handshake_never_becomes_ready() {
    let (listener, agent) = fixture().await;
    let server = tokio::spawn(async move {
        let mut ws = accept(&listener).await;
        let init = receive(&mut ws).await;
        send(
            &mut ws,
            json!({"id":init["id"],"error":{"code":-32600,"message":"initialization rejected"}}),
        )
        .await;
    });
    assert!(matches!(
        agent.connect().await,
        Err(AgentError::ConnectionFailed)
    ));
    assert_eq!(agent.status().phase, ConnectionPhase::Error);
    agent.disconnect().await.unwrap();
    server.await.unwrap();
}

#[tokio::test]
async fn malformed_and_binary_frames_fail_pending_calls() {
    for frame in [
        Message::Text("not-json".into()),
        Message::Binary(vec![1, 2, 3].into()),
    ] {
        let (listener, agent) = fixture().await;
        let server = tokio::spawn(async move {
            let mut ws = accept(&listener).await;
            handshake(&mut ws).await;
            receive(&mut ws).await;
            ws.send(frame).await.unwrap();
            let _ = timeout(DEADLINE, ws.next()).await;
        });
        agent.connect().await.unwrap();
        assert!(matches!(
            agent.list_models(None).await,
            Err(AgentError::Disconnected)
        ));
        server.await.unwrap();
        agent.disconnect().await.unwrap();
    }
}

#[tokio::test]
async fn dropping_last_client_closes_socket_and_actor() {
    let (listener, agent) = fixture().await;
    let server = tokio::spawn(async move {
        let mut ws = accept(&listener).await;
        handshake(&mut ws).await;
        let frame = timeout(DEADLINE, ws.next()).await.unwrap();
        assert!(!matches!(frame, Some(Ok(Message::Text(_)))));
    });
    agent.connect().await.unwrap();
    drop(agent);
    server.await.unwrap();
}

#[tokio::test]
async fn heartbeat_ping_is_answered_without_application_requests() {
    let (listener, agent) = fixture().await;
    let server = tokio::spawn(async move {
        let mut ws = accept(&listener).await;
        handshake(&mut ws).await;
        ws.send(Message::Ping(vec![5, 9].into())).await.unwrap();
        assert_eq!(
            timeout(DEADLINE, ws.next())
                .await
                .unwrap()
                .unwrap()
                .unwrap(),
            Message::Pong(vec![5, 9].into())
        );
    });
    agent.connect().await.unwrap();
    server.await.unwrap();
    agent.disconnect().await.unwrap();
}

#[tokio::test]
async fn lagging_subscribers_are_told_to_resynchronize() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mut cfg = config(&listener);
    cfg.event_capacity = 2;
    let agent = CodexAgent::new(cfg);
    let mut events = agent.subscribe_events();
    let (sent, received) = oneshot::channel();
    let (done, wait) = oneshot::channel();
    let server = tokio::spawn(async move {
        let mut ws = accept(&listener).await;
        handshake(&mut ws).await;
        for n in 0..20 {
            send(
                &mut ws,
                json!({"method":"item/agentMessage/delta","params":{"delta":n.to_string()}}),
            )
            .await;
        }
        let req = receive(&mut ws).await;
        send(&mut ws, json!({"id":req["id"],"result":{"data":[]}})).await;
        sent.send(()).unwrap();
        wait.await.unwrap();
    });
    agent.connect().await.unwrap();
    agent.list_models(None).await.unwrap();
    received.await.unwrap();
    assert!(matches!(
        events.recv().await,
        Err(tokio::sync::broadcast::error::RecvError::Lagged(_))
    ));
    done.send(()).unwrap();
    server.await.unwrap();
    agent.disconnect().await.unwrap();
}

#[tokio::test]
async fn typed_business_methods_send_protocol_fields_without_hidden_defaults() {
    let (listener, agent) = fixture().await;
    let server = tokio::spawn(async move {
        let mut ws = accept(&listener).await;
        handshake(&mut ws).await;
        let methods = [
            "thread/start",
            "thread/resume",
            "thread/read",
            "thread/list",
            "turn/start",
            "turn/interrupt",
            "thread/archive",
        ];
        for method in methods {
            let req = receive(&mut ws).await;
            assert_eq!(req["method"], method);
            let params = &req["params"];
            let result = match method {
                "thread/start" | "thread/resume" => {
                    assert_eq!(params["approvalPolicy"], "on-request");
                    assert_eq!(params["sandbox"], "workspace-write");
                    assert_eq!(params["cwd"], "/workspace");
                    json!({"thread":{"id":"t"}})
                }
                "thread/read" => {
                    assert_eq!(params["includeTurns"], true);
                    json!({"thread":{"id":"t","turns":[]}})
                }
                "thread/list" => {
                    assert_eq!(params["cursor"], "next");
                    assert_eq!(params["archived"], false);
                    json!({"data":[{"id":"t"}],"nextCursor":"last"})
                }
                "turn/start" => {
                    assert_eq!(params["model"], "MiniMax-M3");
                    assert_eq!(params["effort"], "ultra");
                    assert_eq!(
                        params["input"],
                        json!([{"type":"text","text":"prompt"},{"type":"localImage","path":"/workspace/a.png"}])
                    );
                    json!({"turn":{"id":"u","status":"inProgress","items":[]}})
                }
                "turn/interrupt" => {
                    assert_eq!(params, &json!({"threadId":"t","turnId":"u"}));
                    json!({})
                }
                _ => json!({}),
            };
            send(&mut ws, json!({"id":req["id"],"result":result})).await;
        }
    });
    agent.connect().await.unwrap();
    let options = ThreadOptions {
        model: Some("MiniMax-M3".into()),
        cwd: "/workspace".into(),
        approval_policy: ApprovalPolicy::OnRequest,
        sandbox: SandboxMode::WorkspaceWrite,
    };
    assert_eq!(agent.create_thread(&options).await.unwrap().id, "t");
    agent.resume_thread("t", &options).await.unwrap();
    agent.read_thread("t").await.unwrap();
    assert_eq!(
        agent
            .list_threads(Some("next"), "/workspace")
            .await
            .unwrap()
            .next_cursor
            .as_deref(),
        Some("last")
    );
    agent
        .start_turn(&TurnInput {
            client_user_message_id: None,
            thread_id: "t".into(),
            model: Some("MiniMax-M3".into()),
            effort: Some("ultra".into()),
            input: vec![
                UserInput::Text {
                    text: "prompt".into(),
                },
                UserInput::LocalImage {
                    path: "/workspace/a.png".into(),
                },
            ],
        })
        .await
        .unwrap();
    agent.interrupt_turn("t", "u").await.unwrap();
    agent.archive_thread("t").await.unwrap();
    server.await.unwrap();
    agent.disconnect().await.unwrap();
}

#[tokio::test]
async fn rpc_errors_are_typed_but_display_does_not_leak_upstream_message() {
    let (listener, agent) = fixture().await;
    let server = tokio::spawn(async move {
        let mut ws = accept(&listener).await;
        handshake(&mut ws).await;
        let req = receive(&mut ws).await;
        send(
            &mut ws,
            json!({"id":req["id"],"error":{"code":-32001,"message":"CREDENTIAL_MARKER"}}),
        )
        .await;
    });
    agent.connect().await.unwrap();
    let error = agent.list_models(None).await.unwrap_err();
    assert!(!error.to_string().contains("CREDENTIAL_MARKER"));
    assert!(matches!(error, AgentError::Rpc { code: -32001, .. }));
    server.await.unwrap();
    agent.disconnect().await.unwrap();
}

#[tokio::test]
async fn zero_timeout_is_rejected_and_idle_calls_fail_immediately() {
    let agent = CodexAgent::new(AgentConfig {
        request_timeout: Duration::ZERO,
        ..Default::default()
    });
    assert!(matches!(
        agent.connect().await,
        Err(AgentError::Configuration(_))
    ));
    assert!(matches!(
        agent.list_models(None).await,
        Err(AgentError::Disconnected)
    ));
    agent.disconnect().await.unwrap();
    agent.disconnect().await.unwrap();
}

#[tokio::test]
async fn old_connection_approval_cannot_answer_a_reused_request_id() {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let mut cfg = config(&listener);
    cfg.max_reconnect_attempts = 3;
    cfg.approval_timeout = Duration::from_secs(3);
    let agent = CodexAgent::new(cfg);
    let mut events = agent.subscribe_events();
    let (close, closing) = oneshot::channel();
    let (done, wait) = oneshot::channel();
    let server = tokio::spawn(async move {
        let mut first = accept(&listener).await;
        handshake(&mut first).await;
        send(
            &mut first,
            json!({"id":"reused","method":"item/fileChange/requestApproval","params":{}}),
        )
        .await;
        closing.await.unwrap();
        first.close(None).await.unwrap();
        drop(first);
        let mut second = accept(&listener).await;
        handshake(&mut second).await;
        send(
            &mut second,
            json!({"id":"reused","method":"item/fileChange/requestApproval","params":{}}),
        )
        .await;
        assert_eq!(
            receive(&mut second).await,
            json!({"id":"reused","result":{"decision":"decline"}})
        );
        wait.await.unwrap();
    });
    agent.connect().await.unwrap();
    let old = interaction(&mut events).await;
    close.send(()).unwrap();
    let current = interaction(&mut events).await;
    assert_ne!(old.connection_id, current.connection_id);
    assert!(matches!(
        agent.approve(&old, ApprovalDecision::Accept).await,
        Err(AgentError::StaleRequest)
    ));
    agent
        .approve(&current, ApprovalDecision::Decline)
        .await
        .unwrap();
    done.send(()).unwrap();
    server.await.unwrap();
    agent.disconnect().await.unwrap();
}

#[tokio::test]
async fn cancelled_request_future_does_not_poison_the_next_rpc() {
    let (listener, agent) = fixture().await;
    let (seen, received) = oneshot::channel();
    let server = tokio::spawn(async move {
        let mut ws = accept(&listener).await;
        handshake(&mut ws).await;
        let old = receive(&mut ws).await;
        seen.send(()).unwrap();
        let next = receive(&mut ws).await;
        send(
            &mut ws,
            json!({"id":old["id"],"result":{"value":"cancelled"}}),
        )
        .await;
        send(&mut ws, json!({"id":next["id"],"result":{"value":"fresh"}})).await;
    });
    agent.connect().await.unwrap();
    let copy = agent.clone();
    let cancelled = tokio::spawn(async move { copy.list_skills("/cancelled").await });
    received.await.unwrap();
    cancelled.abort();
    let _ = cancelled.await;
    assert_eq!(
        agent.list_skills("/fresh").await.unwrap(),
        json!({"value":"fresh"})
    );
    server.await.unwrap();
    agent.disconnect().await.unwrap();
}

#[tokio::test]
async fn inflight_capacity_is_bounded_and_overload_is_reported() {
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };
    let (listener, agent) = fixture().await;
    let count = Arc::new(AtomicUsize::new(0));
    let counter = count.clone();
    let (done, mut wait) = oneshot::channel();
    let server = tokio::spawn(async move {
        let mut ws = accept(&listener).await;
        handshake(&mut ws).await;
        loop {
            tokio::select! {
                _=&mut wait=>break,
                frame=ws.next()=>match frame {
                    Some(Ok(Message::Text(_)))=>{counter.fetch_add(1,Ordering::Relaxed);},
                    _=>break,
                }
            }
        }
    });
    agent.connect().await.unwrap();
    let mut calls = tokio::task::JoinSet::new();
    for _ in 0..160 {
        let agent = agent.clone();
        calls.spawn(async move { agent.list_models(None).await });
    }
    let mut busy = 0;
    let mut timed_out = 0;
    while let Some(result) = calls.join_next().await {
        match result.unwrap() {
            Err(AgentError::Busy) => busy += 1,
            Err(AgentError::Timeout) => timed_out += 1,
            other => panic!("unexpected overload result: {other:?}"),
        }
    }
    assert!(busy > 0);
    assert!(timed_out > 0);
    assert!(count.load(Ordering::Relaxed) <= 128);
    done.send(()).unwrap();
    server.await.unwrap();
    agent.disconnect().await.unwrap();
}
