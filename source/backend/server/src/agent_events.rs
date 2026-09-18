use super::*;

impl AgentService {
    pub(crate) async fn pump(self: Arc<Self>, mut receiver: broadcast::Receiver<AgentEvent>) {
        loop {
            match receiver.recv().await {
                Ok(event) => self.on_event(event).await,
                Err(broadcast::error::RecvError::Lagged(_)) => {
                    self.emit(json!({"type":"resync_required"}));
                    for slot in self.state.lock().await.active.values_mut() {
                        slot.uncertain = true;
                    }
                    self.schedule_resync();
                }
                Err(broadcast::error::RecvError::Closed) => break,
            }
        }
    }

    async fn on_event(self: &Arc<Self>, event: AgentEvent) {
        match &event {
            AgentEvent::Status { state: connection } => {
                let mut state = self.state.lock().await;
                let changed = connection.phase == ConnectionPhase::Ready
                    && state.last_connection != connection.connection_id;
                if connection.phase == ConnectionPhase::Ready {
                    state.last_connection = connection.connection_id;
                }
                if connection.phase != ConnectionPhase::Ready {
                    for slot in state.active.values_mut() {
                        slot.uncertain = true;
                    }
                }
                drop(state);
                if changed {
                    self.schedule_resync();
                    self.emit(json!({"type":"resync_required"}));
                }
            }
            AgentEvent::ServerRequest { request } => {
                self.state
                    .lock()
                    .await
                    .interactions
                    .insert(interaction_key(request), request.clone());
            }
            AgentEvent::ServerRequestExpired { id, connection_id } => {
                let key = format!(
                    "{}:{}",
                    connection_id,
                    serde_json::to_string(id).unwrap_or_default()
                );
                self.state.lock().await.interactions.remove(&key);
            }
            AgentEvent::Notification { method, params } => {
                if matches!(method.as_str(), "item/started" | "item/completed") {
                    if let (Some(thread), Some(turn), Some(item)) = (
                        params.get("threadId").and_then(Value::as_str),
                        params.get("turnId").and_then(Value::as_str),
                        params.get("item"),
                    ) {
                        if item.get("type").and_then(Value::as_str) == Some("commandExecution") {
                            if let Some(id) = item.get("id").and_then(Value::as_str) {
                                let mut state = self.state.lock().await;
                                if state.command_items.len() >= 256 {
                                    let stale = state
                                        .command_items
                                        .keys()
                                        .find(|(thread, turn)| {
                                            !state.active.get(thread).is_some_and(|slot| {
                                                slot.turn.as_ref().is_some_and(|t| &t.id == turn)
                                            })
                                        })
                                        .cloned();
                                    if let Some(key) = stale {
                                        state.command_items.remove(&key);
                                    }
                                }
                                state
                                    .command_items
                                    .entry((thread.into(), turn.into()))
                                    .or_default()
                                    .insert(id.into());
                            }
                        }
                    }
                }
                if matches!(method.as_str(), "turn/started" | "turn/completed") {
                    if let (Some(thread), Some(turn)) = (
                        params.get("threadId").and_then(Value::as_str),
                        params.get("turn"),
                    ) {
                        if let Ok(turn) = serde_json::from_value::<Turn>(turn.clone()) {
                            let mut state = self.state.lock().await;
                            state.empty_threads.remove(thread);
                            if method == "turn/started" {
                                let slot =
                                    state.active.entry(thread.into()).or_insert(ActiveTurn {
                                        ticket: Uuid::new_v4(),
                                        turn: None,
                                        uncertain: false,
                                        stop_requested: false,
                                    });
                                slot.turn = Some(turn.clone());
                                slot.uncertain = false;
                                let stop = slot.stop_requested;
                                drop(state);
                                if stop {
                                    self.queue_interrupt(thread.into(), turn.id.clone());
                                }
                            } else {
                                if state.active.get(thread).is_some_and(|slot| {
                                    slot.turn.as_ref().is_some_and(|t| t.id == turn.id)
                                }) {
                                    state.active.remove(thread);
                                }
                                state.interactions.retain(|_, request| {
                                    request.params.get("turnId").and_then(Value::as_str)
                                        != Some(&turn.id)
                                });
                            }
                        }
                    }
                }
            }
        }
        self.emit(json!({"type":"codex","event":event}));
        // Text deltas need not serialize and broadcast the entire application snapshot.
        if !matches!(&event,AgentEvent::Notification {method,..} if method=="item/agentMessage/delta"||method.ends_with("Delta")||method.ends_with("/delta"))
        {
            self.publish_status().await;
        }
    }

    pub(crate) async fn observe_thread(&self, thread: &Thread) {
        let mut state = self.state.lock().await;
        for turn in thread.turns.iter().rev().take(2) {
            let items: HashSet<_> = turn
                .items
                .iter()
                .filter(|item| item.get("type").and_then(Value::as_str) == Some("commandExecution"))
                .filter_map(|item| item.get("id").and_then(Value::as_str).map(str::to_owned))
                .collect();
            if !items.is_empty() {
                state
                    .command_items
                    .entry((thread.id.clone(), turn.id.clone()))
                    .or_default()
                    .extend(items);
            }
        }
        if let Some(turn) = thread.turns.iter().rev().find(|t| !is_finished(&t.status)) {
            let slot = state.active.entry(thread.id.clone()).or_insert(ActiveTurn {
                ticket: Uuid::new_v4(),
                turn: None,
                uncertain: false,
                stop_requested: false,
            });
            slot.turn = Some(turn.clone());
            slot.uncertain = false;
        } else if state
            .active
            .get(&thread.id)
            .is_some_and(|slot| slot.uncertain)
        {
            state.active.remove(&thread.id);
        }
    }

    fn schedule_resync(self: &Arc<Self>) {
        if self.closing.load(Ordering::Acquire) || self.resyncing.swap(true, Ordering::AcqRel) {
            return;
        }
        let service = self.clone();
        tokio::spawn(async move {
            let ids: Vec<_> = service.state.lock().await.active.keys().cloned().collect();
            for id in ids {
                if service.closing.load(Ordering::Acquire) {
                    break;
                }
                if let Ok(thread) = service.client.read_thread(&id).await {
                    service.observe_thread(&thread).await;
                    let state = service.state.lock().await;
                    let stop = state
                        .active
                        .get(&id)
                        .filter(|s| s.stop_requested)
                        .and_then(|s| s.turn.as_ref())
                        .map(|t| t.id.clone());
                    drop(state);
                    if let Some(turn) = stop {
                        service.queue_interrupt(id, turn);
                    }
                }
            }
            service.resyncing.store(false, Ordering::Release);
            service.publish_status().await;
        });
    }

    pub(crate) async fn refresh_models(&self) {
        let Some(settings) = &self.settings else {
            return;
        };
        if let Ok(Page { data, .. }) = self.client.list_models(None).await {
            if let Some(mut model) = data.into_iter().find(|m| m.model == settings.model) {
                model.is_default = true;
                // Generic Codex fallback metadata is not evidence of an external model's vision support.
                model.input_modalities = if settings.supports_images {
                    vec!["text".into(), "image".into()]
                } else {
                    vec!["text".into()]
                };
                self.state.lock().await.model = Some(model);
            }
        }
        self.publish_status().await;
    }
}
