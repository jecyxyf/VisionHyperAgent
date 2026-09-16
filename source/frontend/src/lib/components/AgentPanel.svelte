<script lang="ts">
  import { onDestroy } from "svelte";
  import Icon from "./Icon.svelte";
  import IconButton from "./IconButton.svelte";
  import GlassPanel from "./GlassPanel.svelte";
  import AgentAttachmentList from "./agent/AgentAttachmentList.svelte";
  import AgentHistoryPanel from "./agent/AgentHistoryPanel.svelte";
  import type {
    AgentAttachment,
    AgentChatMessage,
    AgentHistorySession,
  } from "./agent/types";

  let draft = $state("");
  let nextId = 1;
  let messages = $state<AgentChatMessage[]>([]);
  let historyOpen = $state(false);
  let selectedModel = $state("codex-default");
  let selectedEffort = $state("medium");
  let attachments = $state<AgentAttachment[]>([]);
  let dragging = $state(false);
  let generating = $state(false);
  let nextAttachmentId = 1;
  let turnTimer: ReturnType<typeof setTimeout> | null = null;

  const initialSessions: AgentHistorySession[] = [
    {
      id: "short-circuit-rule",
      title: "短路焊点识别规则",
      updatedAt: "今天 09:24",
      summary: "讨论锡堆过多、短路焊点与正常焊点的区分条件。",
      messages: [
        {
          id: 1,
          body: "识别电路板上的短路焊点，外观是锡堆过多，与正常焊点的区别是连接了相邻焊盘。",
          tone: "user",
        },
        {
          id: 2,
          body: "Agent 未连接，消息未发送。",
          tone: "system",
        },
      ],
    },
    {
      id: "surface-scratch",
      title: "表面划痕标注",
      updatedAt: "昨天 16:40",
      summary: "整理金属表面划痕的走向、长度和干扰项描述。",
      messages: [
        {
          id: 1,
          body: "帮我描述金属表面划痕的标注规则。",
          tone: "user",
        },
      ],
    },
  ];

  let sessions = $state<AgentHistorySession[]>(initialSessions);
  let activeSessionId = $state(initialSessions[0]?.id ?? "");
  messages = [...initialSessions[0].messages];
  nextId = initialSessions[0].messages.length + 1;

  const canSend = $derived(!generating && draft.trim().length > 0);

  function syncActiveSession() {
    sessions = sessions.map((session) =>
      session.id === activeSessionId ? { ...session, messages: [...messages] } : session,
    );
  }

  function appendMessage(message: Omit<AgentChatMessage, "id">) {
    messages = [...messages, { ...message, id: nextId++ }];
    syncActiveSession();
  }

  function send() {
    const body = draft.trim();
    if (!body) return;

    const attachmentNames = attachments.map((attachment) => attachment.file.name);
    appendMessage({
      body:
        attachmentNames.length > 0
          ? body + "\n附件：" + attachmentNames.join("、")
          : body,
      tone: "user",
    });

    draft = "";
    releaseAttachments(attachments);
    attachments = [];
    generating = true;

    turnTimer = setTimeout(() => {
      appendMessage({ body: "Agent 未连接，消息未发送。", tone: "system" });
      generating = false;
      turnTimer = null;
    }, 1000);
  }

  function toggleTurn() {
    if (generating) {
      stopTurn();
      return;
    }

    send();
  }

  function stopTurn() {
    if (!generating) return;

    if (turnTimer) {
      clearTimeout(turnTimer);
      turnTimer = null;
    }

    appendMessage({ body: "已停止本轮回复。", tone: "system" });
    generating = false;
  }

  function clearHistory() {
    messages = [];
    nextId = 1;
    syncActiveSession();
  }

  function createSession() {
    const id = "session-" + Date.now();
    const session: AgentHistorySession = {
      id,
      title: "新会话",
      updatedAt: "刚刚",
      summary: "等待输入第一条消息。",
      messages: [],
    };

    sessions = [session, ...sessions];
    activeSessionId = id;
    messages = [];
    nextId = 1;
    historyOpen = false;
  }

  function selectSession(id: string) {
    const session = sessions.find((item) => item.id === id);
    if (!session) return;

    activeSessionId = id;
    messages = [...session.messages];
    nextId = session.messages.reduce((max, message) => Math.max(max, message.id), 0) + 1;
    historyOpen = false;
  }

  function deleteSession(id: string) {
    const remainingSessions = sessions.filter((session) => session.id !== id);
    sessions = remainingSessions;

    if (id === activeSessionId) {
      if (remainingSessions[0]) {
        selectSession(remainingSessions[0].id);
      } else {
        activeSessionId = "";
        messages = [];
        nextId = 1;
      }
    }
  }

  function handleDragOver(event: DragEvent) {
    if (!event.dataTransfer?.types.includes("Files")) return;

    event.preventDefault();
    event.dataTransfer.dropEffect = "copy";
    dragging = true;
  }

  function handleDragLeave(event: DragEvent) {
    if (
      event.relatedTarget instanceof Node &&
      event.currentTarget instanceof Node &&
      event.currentTarget.contains(event.relatedTarget)
    ) {
      return;
    }

    dragging = false;
  }

  function handleDrop(event: DragEvent) {
    event.preventDefault();
    dragging = false;
    addAttachments(event.dataTransfer?.files ?? []);
  }

  function addAttachments(fileList: FileList | File[]) {
    const files = Array.from(fileList).filter((file) => file.size > 0);
    if (files.length === 0) return;

    const nextAttachments = files.map((file) => ({
      id: "attachment-" + nextAttachmentId++,
      file,
      previewUrl: file.type.startsWith("image/") ? URL.createObjectURL(file) : undefined,
    }));

    attachments = [...attachments, ...nextAttachments];
  }

  function removeAttachment(id: string) {
    const target = attachments.find((attachment) => attachment.id === id);
    if (target?.previewUrl) URL.revokeObjectURL(target.previewUrl);
    attachments = attachments.filter((attachment) => attachment.id !== id);
  }

  function releaseAttachments(list: AgentAttachment[]) {
    list.forEach((attachment) => {
      if (attachment.previewUrl) URL.revokeObjectURL(attachment.previewUrl);
    });
  }

  onDestroy(() => {
    if (turnTimer) clearTimeout(turnTimer);
    releaseAttachments(attachments);
  });
</script>

<div
  class="agent-shell"
  role="region"
  aria-label="Agent 面板"
  ondragenter={handleDragOver}
  ondragover={handleDragOver}
  ondragleave={handleDragLeave}
  ondrop={handleDrop}
>
  <GlassPanel class="agent-panel">
    <header>
      <div class="title-row">
        <Icon name="spark" size={18} />
        <h2>Agent</h2>
        <IconButton
          label="历史会话"
          icon="history"
          selected={historyOpen}
          onclick={() => (historyOpen = !historyOpen)}
        />
        <IconButton label="清除聊天记录" icon="trash" onclick={clearHistory} />
      </div>
      <div class="separator"></div>
    </header>

    {#if historyOpen}
      <AgentHistoryPanel
        sessions={sessions}
        activeSessionId={activeSessionId}
        onClose={() => (historyOpen = false)}
        onSelect={selectSession}
        onCreate={createSession}
        onDelete={deleteSession}
      />
    {/if}

    <div class="chat-body" aria-live="polite">
      {#if messages.length === 0}
        <p class="quiet">连接 Agent 后，这里会保留当前工作区的对话。</p>
      {:else}
        <div class="messages">
          {#each messages as message (message.id)}
            <article class:message={message.tone === "user"}>
              <p>{message.body}</p>
              {#if message.tone === "user"}<span>未发送</span>{/if}
            </article>
          {/each}
        </div>
      {/if}
    </div>

    <AgentAttachmentList attachments={attachments} onRemove={removeAttachment} />

    <div class="composer">
      <textarea
        bind:value={draft}
        placeholder="输入消息…"
        aria-label="消息输入"
        onkeydown={(event) => {
          if (event.key === "Enter" && !event.shiftKey) {
            event.preventDefault();
            if (canSend) send();
          }
        }}
      ></textarea>

      <div class="composer-footer">
        <label class="setting model">
          <span>模型</span>
          <span class="select-shell">
            <select bind:value={selectedModel} aria-label="选择 Agent 模型">
              <option value="codex-default">Default</option>
              <option value="codex-fast">Fast</option>
              <option value="codex-reasoning">Reasoning</option>
            </select>
            <Icon name="chevron" size={12} />
          </span>
        </label>

        <label class="setting effort">
          <span>Effort</span>
          <span class="select-shell">
            <select bind:value={selectedEffort} aria-label="选择 Agent Effort">
              <option value="low">低</option>
              <option value="medium">中</option>
              <option value="high">高</option>
            </select>
            <Icon name="chevron" size={12} />
          </span>
        </label>

        <span class="footer-spacer" aria-hidden="true"></span>

        <IconButton
          label={generating ? "停止回复" : "发送消息"}
          icon={generating ? "stop" : "send"}
          primary
          disabled={!generating && !canSend}
          onclick={toggleTurn}
        />
      </div>
    </div>
  </GlassPanel>

  {#if dragging}
    <div class="drop-overlay" role="status">
      <span>松开添加附件</span>
    </div>
  {/if}
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
</style>
