<script lang="ts">
  import { onMount, tick } from "svelte";
  import Icon from "./Icon.svelte";
  import IconButton from "./IconButton.svelte";
  import GlassPanel from "./GlassPanel.svelte";
  import AgentAttachmentList from "./agent/AgentAttachmentList.svelte";
  import AgentHistoryPanel from "./agent/AgentHistoryPanel.svelte";
  import AgentInteractionCard from "./agent/AgentInteractionCard.svelte";
  import { AgentSocket, AgentRequestError } from "../api/websocket";
  import { errorText, historySession, itemMessage, mergeHydrated, threadMessages } from "./agent/messages";
  import type { AgentAttachment, AgentChatMessage, AgentHistorySession, AgentSnapshot, CodexItem, CodexThread, CodexTurn, ThreadPage } from "./agent/types";

  const socket = new AgentSocket();
  const storageKey = "vha.agent.activeThread";
  let draft = $state("");
  let messageCache = $state<Record<string, AgentChatMessage[]>>({});
  let sessions = $state<AgentHistorySession[]>([]);
  let historyOpen = $state(false);
  let historyCursor = $state<string | null>(null);
  let historyLoading = $state(false);
  let selectedModel = $state("");
  let selectedEffort = $state("medium");
  let activeSessionId = $state("");
  let attachments = $state<AgentAttachment[]>([]);
  let dragging = $state(false);
  let connected = $state(false);
  let notice = $state("");
  let snapshot = $state<AgentSnapshot>({ phase: "connecting", models: [], activeTurns: {}, interactions: [] });
  let sendPhase = $state<"preparing" | "uploading" | "submitting" | null>(null);
  let switching = $state(false);
  let chatBody = $state<HTMLElement | null>(null);
  let initialized = $state(false);
  let initializing = $state(false);
  let disposed = false;
  let operation = 0;
  let hydration = 0;
  let followBottom = true;
  let uploadController: AbortController | null = null;
  let stopAfterSend: string | null = null;
  let historyRefresh: ReturnType<typeof setTimeout> | null = null;
  const messages = $derived(messageCache[activeSessionId] ?? []);
  const activeTurn = $derived(snapshot.activeTurns[activeSessionId]);
  const generating = $derived(Boolean(sendPhase || activeTurn));
  const ready = $derived(connected && snapshot.phase === "ready");
  const selectedModelData = $derived(snapshot.models.find((model) => model.id === selectedModel));
  const modelEfforts = $derived(selectedModelData?.supportedReasoningEfforts.map((effort) => effort.reasoningEffort) ?? []);
  const supportsImages = $derived(Boolean(selectedModelData?.inputModalities.includes("image")));
  const effortSupported = $derived(!modelEfforts.length || modelEfforts.includes(selectedEffort));
  const inputTooLong = $derived(new TextEncoder().encode(draft).byteLength > 64 * 1024);
  // Agent readiness is intentionally informational here. A send attempt is always allowed to
  // reach the backend when the local draft is valid, and the backend error becomes UI feedback.
  const canSend = $derived(!inputTooLong && !generating && !switching && selectedModel !== "" && effortSupported && (draft.trim().length > 0 || attachments.length > 0));
  const statusText = $derived(!connected ? "后端未连接" : ({ starting: "Codex 启动中", connecting: "正在连接 Codex", reconnecting: "Codex 重连中", ready: initializing ? "正在同步会话" : activeTurn?.uncertain ? "任务状态待同步" : generating ? "任务进行中" : "Agent 已就绪", error: "Agent 不可用", stopping: "后端正在退出", stopped: "后端已停止" }[snapshot.phase] ?? snapshot.phase));
  const interactions = $derived(snapshot.interactions.filter((item) => !item.request.params.threadId || item.request.params.threadId === activeSessionId));

  async function scroll() { await tick(); if (followBottom && chatBody) chatBody.scrollTop = chatBody.scrollHeight; }
  function report(reason: unknown) { if (!disposed) notice = reason instanceof Error ? reason.message : "操作失败，请检查后端状态。"; }
  function remember(id: string) { try { localStorage.setItem(storageKey, id); } catch { /* private browsing may disable storage */ } }
  function mergeSession(thread: CodexThread) { const session = historySession(thread); sessions = [session, ...sessions.filter((entry) => entry.id !== thread.id)]; }
  function upsert(thread: string, message: AgentChatMessage) {
    const list = messageCache[thread] ?? [];
    let index = list.findIndex((entry) => entry.id === message.id);
    if (index < 0 && message.clientId) index = list.findIndex((entry) => entry.clientId === message.clientId);
    messageCache[thread] = index < 0 ? [...list, message] : list.map((entry, i) => i === index ? { ...entry, ...message } : entry);
    if (thread === activeSessionId) void scroll();
  }
  function system(thread: string, body: string, id: string = crypto.randomUUID()) { upsert(thread, { id, body, tone: "system" }); }
  function applyStatus(data: unknown) {
    if (!data || typeof data !== "object") return;
    const candidate = data as AgentSnapshot;
    if (typeof candidate.phase !== "string" || !Array.isArray(candidate.models) || !candidate.activeTurns || typeof candidate.activeTurns !== "object" || !Array.isArray(candidate.interactions)) return;
    if (candidate.models.some((model) => !model || typeof model.id !== "string" || typeof model.model !== "string" || !Array.isArray(model.supportedReasoningEfforts))) return;
    snapshot = candidate;
    if (!snapshot.models.some((model) => model.id === selectedModel)) selectedModel = snapshot.models[0]?.id ?? "";
    if (ready && !initialized) void initialize();
    if (stopAfterSend && snapshot.activeTurns[stopAfterSend]) {
      const threadId = stopAfterSend; stopAfterSend = null;
      void socket.request("turn.interrupt", { threadId }).catch(report);
    }
  }

  async function loadHistory(more = false) {
    if (historyLoading || !ready) return;
    historyLoading = true;
    try {
      const page = await socket.request<ThreadPage>("thread.list", { cursor: more ? historyCursor : null });
      const next = page.data.map(historySession);
      sessions = more ? [...sessions, ...next.filter((entry) => !sessions.some((old) => old.id === entry.id))] : next;
      historyCursor = page.nextCursor ?? null;
    } finally { historyLoading = false; }
  }
  function refreshHistorySoon() {
    if (historyRefresh) clearTimeout(historyRefresh);
    historyRefresh = setTimeout(() => { historyRefresh = null; void loadHistory().catch(report); }, 250);
  }
  async function hydrate(id: string) {
    const version = ++hydration;
    const { thread } = await socket.request<{ thread: CodexThread }>("thread.resume", { threadId: id });
    if (disposed || version !== hydration || id !== activeSessionId) return;
    const busy = Boolean(snapshot.activeTurns[id]);
    messageCache[id] = mergeHydrated(threadMessages(thread), messageCache[id] ?? [], busy);
    mergeSession(thread); void scroll();
  }
  async function newSession() {
    const { thread } = await socket.request<{ thread: CodexThread }>("thread.create");
    if (disposed) return;
    ++hydration; activeSessionId = thread.id; messageCache[thread.id] = []; mergeSession(thread);
    remember(thread.id); followBottom = true; historyOpen = false;
  }
  async function initialize() {
    if (initializing || initialized || !ready || disposed) return;
    initializing = true;
    try {
      await loadHistory();
      if (activeSessionId) {
        try { await hydrate(activeSessionId); }
        catch { activeSessionId = ""; }
      }
      if (!activeSessionId && sessions[0]) { activeSessionId = sessions[0].id; await hydrate(activeSessionId); remember(activeSessionId); }
      if (!activeSessionId) await newSession();
      initialized = true;
    } catch (reason) { report(reason); }
    finally { initializing = false; }
  }
  async function synchronize() {
    try {
      applyStatus(await socket.request<AgentSnapshot>("status"));
      if (!ready) return;
      if (!initialized) { await initialize(); return; }
      await loadHistory(); if (activeSessionId) await hydrate(activeSessionId);
    } catch (reason) { report(reason); }
  }
  async function createSession() {
    if (!ready || initializing || sendPhase || switching) return;
    switching = true; notice = "";
    try { await newSession(); } catch (reason) { report(reason); } finally { switching = false; }
  }
  async function selectSession(id: string) {
    if (sendPhase || switching) return;
    const previous = activeSessionId;
    activeSessionId = id; switching = true; followBottom = true; notice = "";
    try { await hydrate(id); remember(id); historyOpen = false; }
    catch (reason) { activeSessionId = previous; report(reason); }
    finally { switching = false; }
  }
  async function archiveSession(id: string) {
    if (!ready) return;
    try {
      await socket.request("thread.archive", { threadId: id });
      sessions = sessions.filter((session) => session.id !== id); delete messageCache[id];
      if (activeSessionId === id) { activeSessionId = ""; if (sessions[0]) await selectSession(sessions[0].id); else await newSession(); }
      notice = "会话已归档。";
    } catch (reason) { report(reason); }
  }
  async function upload(attachment: AgentAttachment, signal: AbortSignal) {
    if (attachment.uploadedId) return attachment.uploadedId;
    const form = new FormData(); form.append("file", attachment.file);
    const response = await fetch("/api/agent/attachments", { method: "POST", body: form, signal });
    let result: { attachment?: { id: string }; error?: { message: string } };
    try { result = await response.json(); } catch { throw new Error("附件上传失败或文件过大"); }
    if (!response.ok || !result.attachment) throw new Error(result.error?.message ?? "附件上传失败");
    attachment.uploadedId = result.attachment.id; return result.attachment.id;
  }
  async function send() {
    if (!canSend || sendPhase) return;
    notice = ""; sendPhase = "preparing"; const version = ++operation;
    const text = draft.trim(); const files = [...attachments];
    let thread = activeSessionId; let pendingId: string | null = null;
    try {
      if (!thread) { await newSession(); thread = activeSessionId; }
      if (version !== operation || disposed) return;
      sendPhase = "uploading"; uploadController = new AbortController();
      const ids: string[] = [];
      for (const file of files) ids.push(await upload(file, uploadController.signal));
      if (version !== operation || disposed) return;
      sendPhase = "submitting";
      pendingId = "pending-" + crypto.randomUUID();
      upsert(thread, { id: pendingId, clientId: pendingId, tone: "user", body: [text, ...files.map((item) => "附件：" + item.file.name)].filter(Boolean).join("\n"), status: "pending" });
      await socket.request<{ turn: CodexTurn }>("turn.start", { threadId: thread, text, modelId: selectedModel, effort: selectedEffort, attachments: ids, clientMessageId: pendingId });
      if (disposed) return;
      const list = messageCache[thread] ?? [];
      messageCache[thread] = list.map((message) => message.id === pendingId ? { ...message, status: "sent" } : message);
      draft = ""; releaseAttachments(files); attachments = [];
      if (stopAfterSend === thread) { stopAfterSend = null; await socket.request("turn.interrupt", { threadId: thread }); }
      refreshHistorySoon();
    } catch (reason) {
      if (reason instanceof DOMException && reason.name === "AbortError") notice = "已取消上传，消息尚未发送。";
      else report(reason);
      if (!(reason instanceof AgentRequestError && reason.code === "outcome_unknown") && stopAfterSend === thread) stopAfterSend = null;
      if (pendingId) messageCache[thread] = (messageCache[thread] ?? []).map((message) => message.id === pendingId ? { ...message, status: "failed" } : message);
      if (reason instanceof AgentRequestError && reason.code === "outcome_unknown") void synchronize();
    } finally { if (!disposed) sendPhase = null; uploadController = null; }
  }
  async function stopTurn() {
    const thread = activeSessionId;
    if (sendPhase === "preparing" || sendPhase === "uploading") { ++operation; uploadController?.abort(); notice = "正在取消发送…"; }
    if (sendPhase === "submitting") stopAfterSend = thread;
    if (snapshot.activeTurns[thread] || sendPhase === "submitting") {
      try { await socket.request("turn.interrupt", { threadId: thread }); }
      catch (reason) { report(reason); }
    }
  }

  function codexEvent(payload: unknown) {
    const event = (payload as { event?: { type: string; method?: string; params?: Record<string, unknown> } })?.event;
    if (!event || event.type !== "notification" || !event.params) return;
    const params = event.params;
    const thread = typeof params.threadId === "string" ? params.threadId : "";
    const turnId = typeof params.turnId === "string" ? params.turnId : "";
    if (!thread) return;
    if (event.method === "item/agentMessage/delta" && typeof params.itemId === "string" && typeof params.delta === "string") {
      const current = (messageCache[thread] ?? []).find((message) => message.id === params.itemId);
      upsert(thread, { id: params.itemId, turnId, body: (current?.body ?? "") + params.delta, tone: "assistant", status: "streaming" });
    } else if ((event.method === "item/started" || event.method === "item/completed") && params.item && typeof params.item === "object") {
      const item = params.item as CodexItem;
      const message = itemMessage(item, turnId, event.method === "item/started");
      if (message) upsert(thread, message);
    } else if (event.method === "turn/completed" && params.turn && typeof params.turn === "object") {
      const turn = params.turn as CodexTurn;
      messageCache[thread] = (messageCache[thread] ?? []).map((message) => message.turnId === turn.id && message.status === "streaming" ? { ...message, status: turn.status === "interrupted" ? "interrupted" : "completed" } : message);
      if (turn.status === "interrupted") system(thread, "本轮任务已停止。", `${turn.id}-interrupted`);
      if (turn.status === "failed") system(thread, errorText(turn.error), `${turn.id}-failed`);
      refreshHistorySoon();
    } else if (event.method === "error") {
      system(thread, errorText(params.error));
    }
  }
  function handleDrop(event: DragEvent) { event.preventDefault(); dragging = false; addAttachments(event.dataTransfer?.files ?? []); }
  function handleDragOver(event: DragEvent) { if (!event.dataTransfer?.types.includes("Files") || sendPhase) return; event.preventDefault(); event.dataTransfer.dropEffect = "copy"; dragging = true; }
  function handleDragLeave(event: DragEvent) { if (event.relatedTarget instanceof Node && event.currentTarget instanceof Node && event.currentTarget.contains(event.relatedTarget)) return; dragging = false; }
  function addAttachments(list: FileList | File[]) {
    if (sendPhase) return;
    for (const file of Array.from(list)) {
      if (file.size === 0 || file.size > 16 * 1024 * 1024) { notice = "附件不能为空，且每个文件不得超过 16 MiB。"; continue; }
      if (file.type.startsWith("image/") && !supportsImages) { notice = "当前模型不支持图片附件。"; continue; }
      if (attachments.length >= 16) { notice = "每条消息最多携带 16 个附件。"; break; }
      attachments = [...attachments, { id: crypto.randomUUID(), file, previewUrl: file.type.startsWith("image/") ? URL.createObjectURL(file) : undefined }];
    }
  }
  function removeAttachment(id: string) {
    if (sendPhase) return;
    const item = attachments.find((item) => item.id === id);
    if (item?.previewUrl) URL.revokeObjectURL(item.previewUrl);
    if (item?.uploadedId) void fetch(`/api/agent/attachments/${encodeURIComponent(item.uploadedId)}`, { method: "DELETE" }).catch(() => {});
    attachments = attachments.filter((item) => item.id !== id);
  }
  function releaseAttachments(items: AgentAttachment[]) { for (const item of items) if (item.previewUrl) URL.revokeObjectURL(item.previewUrl); }

  onMount(() => {
    try { activeSessionId = localStorage.getItem(storageKey) ?? ""; } catch { /* no storage */ }
    const off = [
      socket.on("open", () => { connected = true; initialized = false; }),
      socket.on("close", () => { connected = false; initialized = false; }),
      socket.on("status", applyStatus), socket.on("codex", codexEvent),
      socket.on("process_lost", (data) => {
        const threads = (data as { threads?: string[] }).threads ?? [];
        for (const id of threads) {
          messageCache[id] = (messageCache[id] ?? []).map((message) => message.status === "streaming" ? { ...message, status: "failed" } : message);
          system(id, "Codex 进程已退出，本轮任务未确认完成。请重新启动应用后检查会话。");
        }
      }),
      socket.on("resync_required", () => { if (initialized) void synchronize(); }),
      socket.on("closing", () => { snapshot = { ...snapshot, phase: "stopping" }; notice = "主程序正在退出，Codex 将由后端关闭。"; }),
    ];
    socket.connect();
    return () => { disposed = true; ++operation; ++hydration; off.forEach((fn) => fn()); socket.disconnect(); uploadController?.abort(); releaseAttachments(attachments); if (historyRefresh) clearTimeout(historyRefresh); };
  });

  $effect(() => {
    if (modelEfforts.length && !modelEfforts.includes(selectedEffort)) {
      selectedEffort = selectedModelData?.defaultReasoningEffort ?? modelEfforts[0];
    }
  });
</script>

<div class="agent-shell" role="region" aria-label="Agent 面板" ondragenter={handleDragOver} ondragover={handleDragOver} ondragleave={handleDragLeave} ondrop={handleDrop}>
  <GlassPanel class="agent-panel">
    <header>
      <div class="title-row">
        <Icon name="spark" size={18} /><h2>Agent</h2>
        <IconButton label="历史会话" icon="history" selected={historyOpen} onclick={() => { historyOpen = !historyOpen; if (historyOpen) void loadHistory().catch(report); }} />
        <IconButton label="清空并开始新会话" icon="trash" disabled={!ready || initializing || generating || switching} onclick={createSession} />
      </div>
      <div class="separator"></div>
    </header>
    {#if historyOpen}
      <AgentHistoryPanel {sessions} {activeSessionId} loading={historyLoading} hasMore={Boolean(historyCursor)} onLoadMore={() => void loadHistory(true).catch(report)} onClose={() => (historyOpen = false)} onSelect={selectSession} onCreate={createSession} onDelete={archiveSession} />
    {/if}
    <div class="chat-body" aria-live="polite" bind:this={chatBody} onscroll={() => { if (chatBody) followBottom = chatBody.scrollHeight - chatBody.scrollTop - chatBody.clientHeight < 72; }}>
      <div class="agent-status" data-testid="agent-status"><span class:online={ready}>{statusText}</span><button onclick={synchronize} disabled={!connected}>同步</button></div>
      {#if snapshot.message}<p class="quiet" role="status">{snapshot.message}</p>{/if}
      {#if notice}<p class="notice" role="alert">{notice}</p>{/if}
      {#if messages.length === 0}<p class="quiet">{ready ? "输入消息开始对话，或拖入附件。" : "Agent 正在准备。可以先输入内容，发送结果会在这里提示。"}</p>{/if}
      <div class="messages">
        {#each messages as message (message.id)}
          <article class:message={message.tone === "user"} class:assistant={message.tone === "assistant"} class:tool={message.tone === "tool"} data-tone={message.tone}>
            <p>{message.body}</p>
            {#if message.status === "pending"}<span>发送中…</span>{:else if message.status === "failed"}<span>未确认发送 / 失败</span>{:else if message.status === "streaming"}<span>进行中…</span>{/if}
          </article>
        {/each}
      </div>
      {#each interactions as interaction (interaction.key)}
        <AgentInteractionCard {interaction} context={messages.find((message) => message.id === interaction.request.params.itemId)?.body ?? ""} onReply={async (params) => { await socket.request("interaction.reply", params); }} />
      {/each}
    </div>
    <AgentAttachmentList {attachments} onRemove={removeAttachment} />
    <div class="composer">
      <textarea bind:value={draft} placeholder="输入消息…" aria-label="消息输入" disabled={Boolean(sendPhase)} onkeydown={(event) => { if (event.key === "Enter" && !event.shiftKey && !event.isComposing) { event.preventDefault(); if (canSend) void send(); } }}></textarea>
      <div class="composer-footer">
        <label class="setting model"><span>模型</span><span class="select-shell">
          <select bind:value={selectedModel} aria-label="选择 Agent 模型" disabled={!snapshot.models.length || generating}>
            {#if !snapshot.models.length}<option value="">等待模型…</option>{/if}
            {#each snapshot.models as model (model.id)}<option value={model.id}>{model.displayName}</option>{/each}
          </select><Icon name="chevron" size={12} />
        </span></label>
        <label class="setting effort"><span>Effort</span><span class="select-shell">
          <select bind:value={selectedEffort} aria-label="选择 Agent Effort" disabled={generating}>
            {#each modelEfforts as effort (effort)}<option value={effort}>{effort}</option>{/each}
          </select><Icon name="chevron" size={12} />
        </span></label>
        <span class="footer-spacer" aria-hidden="true"></span>
        <IconButton label={generating ? "停止回复" : "发送消息"} icon={generating ? "stop" : "send"} primary disabled={!generating && !canSend} onclick={() => generating ? void stopTurn() : void send()} />
      </div>
      {#if !effortSupported}<small class="effort-warning">当前模型不支持所选 Effort，请重新选择。</small>{/if}
      {#if inputTooLong}<small class="effort-warning" role="alert">单条消息不能超过 64 KiB，较长内容请作为附件发送。</small>{/if}
    </div>
  </GlassPanel>
  {#if dragging}<div class="drop-overlay" role="status"><span>松开添加附件</span></div>{/if}
</div>

<style>
  .agent-shell {
    position: relative;
    display: flex;
    min-width: 0;
    min-height: 0;
  }

  :global(.agent-panel) {
    position: relative;
    flex: 1;
    min-width: 0;
    min-height: 0;
    padding: 18px;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  header {
    flex: 0 0 51px;
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .title-row {
    display: flex;
    align-items: center;
    gap: 7px;
    height: 34px;
    color: var(--vha-accent);
  }

  h2 {
    flex: 1;
    margin: 0;
    color: var(--vha-text);
    font-size: 15.4px;
    font-weight: 600;
  }

  .separator {
    height: 1px;
    background: var(--vha-border);
  }

  .chat-body {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
  }

  .quiet {
    padding: 18px 4px;
    color: var(--vha-subdued);
    font-size: 12px;
  }

  .messages {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  article {
    padding: 12px;
    border: 1px solid var(--vha-glass-edge);
    border-radius: 14px;
    background: var(--vha-selection-gradient);
  }

  article p {
    margin: 0;
    overflow-wrap: anywhere;
    font-size: 12px;
    white-space: pre-wrap;
  }

  article span {
    display: block;
    margin-top: 10px;
    color: var(--vha-muted);
    font-size: 10.5px;
  }

  .composer {
    flex: 0 0 176px;
    display: flex;
    flex-direction: column;
    gap: 9px;
    padding: 12px;
    border: 1px solid var(--vha-border);
    border-radius: 16px;
    background: var(--vha-field-gradient);
  }

  .composer:focus-within {
    border-color: var(--vha-accent);
  }

  textarea {
    flex: 1;
    resize: none;
    border: 0;
    outline: 0;
    background: transparent;
    font-size: 12.2px;
  }

  textarea::placeholder {
    color: var(--vha-subdued);
  }

  .composer-footer {
    display: grid;
    grid-template-columns: 128px 78px minmax(0, 1fr) 34px;
    gap: 7px;
    align-items: end;
  }

  .footer-spacer {
    min-width: 0;
  }

  .setting {
    display: grid;
    min-width: 0;
    gap: 4px;
  }

  .setting > span:first-child {
    color: var(--vha-subdued);
    font-size: 9.5px;
    line-height: 1;
  }

  .select-shell {
    position: relative;
    display: block;
    height: 30px;
  }

  .select-shell :global(svg) {
    position: absolute;
    top: 9px;
    right: 6px;
    pointer-events: none;
    color: var(--vha-subdued);
  }

  select {
    width: 100%;
    height: 100%;
    padding: 0 20px 0 7px;
    border: 1px solid var(--vha-border);
    border-radius: 10px;
    outline: 0;
    appearance: none;
    background: var(--vha-field-gradient);
    color: var(--vha-text);
    font-size: 10.5px;
  }

  select:focus {
    border-color: var(--vha-accent);
  }

  .drop-overlay {
    position: absolute;
    inset: 6px;
    z-index: 8;
    display: grid;
    place-items: center;
    pointer-events: none;
    border: 1px dashed rgba(101, 69, 206, 0.48);
    border-radius: 18px;
    background: rgba(255, 255, 255, 0.78);
    backdrop-filter: blur(7px);
    color: var(--vha-accent);
    font-size: 13px;
    font-weight: 650;
  }

  .agent-status { display: flex; align-items: center; justify-content: space-between; gap: 8px; padding: 0 3px 10px; font-size: 10.5px; color: var(--vha-subdued); }
  .agent-status .online { color: var(--vha-accent); }
  .agent-status button { padding: 3px 7px; border-radius: 6px; background: var(--vha-surface); color: var(--vha-accent); }
  .agent-status button:disabled { opacity: .4; }
  .notice, .effort-warning { margin: 0 0 10px; padding: 9px; border-radius: 9px; background: var(--vha-elevated); color: #a53249; font-size: 11px; overflow-wrap: anywhere; white-space: pre-wrap; }
  article.assistant { background: var(--vha-elevated); }
  article.tool { background: var(--vha-surface); }
  article.tool p { max-height: 210px; overflow: auto; font-family: ui-monospace, monospace; font-size: 11px; }
  .effort-warning { margin: 0; padding: 0; }
</style>
