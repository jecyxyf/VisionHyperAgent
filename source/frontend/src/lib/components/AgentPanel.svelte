<script lang="ts">
  import Icon from "./Icon.svelte";
  import IconButton from "./IconButton.svelte";
  import GlassPanel from "./GlassPanel.svelte";

  type ChatMessage = {
    id: number;
    body: string;
    tone: "user" | "system";
  };

  let draft = $state("");
  let nextId = 1;
  let messages = $state<ChatMessage[]>([]);

  function send() {
    const body = draft.trim();
    if (!body) return;

    messages = [
      ...messages,
      { id: nextId++, body, tone: "user" },
      { id: nextId++, body: "Agent 未连接，消息未发送。", tone: "system" },
    ];
    draft = "";
  }

  function clearHistory() {
    messages = [];
  }
</script>

<GlassPanel class="agent-panel">
  <header>
    <div class="title-row">
      <Icon name="spark" size={18} />
      <h2>Agent</h2>
      <IconButton label="清除聊天记录" icon="trash" onclick={clearHistory} />
    </div>
    <div class="separator"></div>
  </header>

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

  <div class="composer">
    <textarea
      bind:value={draft}
      placeholder="输入消息…"
      aria-label="消息输入"
      onkeydown={(event) => {
        if (event.key === "Enter" && !event.shiftKey) {
          event.preventDefault();
          send();
        }
      }}
    ></textarea>
    <div class="composer-actions">
      <IconButton label="发送消息" icon="send" primary onclick={send} />
    </div>
  </div>
</GlassPanel>

<style>
  :global(.agent-panel) {
    position: relative;
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
    gap: 8px;
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
  }

  article span {
    display: block;
    margin-top: 10px;
    color: var(--vha-muted);
    font-size: 10.5px;
  }

  .composer {
    flex: 0 0 132px;
    display: flex;
    flex-direction: column;
    gap: 10px;
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

  .composer-actions {
    display: flex;
    justify-content: flex-end;
  }
</style>
