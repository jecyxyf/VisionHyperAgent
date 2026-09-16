<script lang="ts">
  import Icon from "../Icon.svelte";
  import type { AgentHistorySession } from "./types";

  let {
    sessions,
    activeSessionId,
    onClose,
    onSelect,
    onCreate,
    onDelete,
  }: {
    sessions: AgentHistorySession[];
    activeSessionId: string;
    onClose?: () => void;
    onSelect?: (id: string) => void;
    onCreate?: () => void;
    onDelete?: (id: string) => void;
  } = $props();
</script>

<div class="wrapper">
  <button type="button" class="backdrop" aria-label="关闭历史会话" onclick={onClose}></button>

  <section class="history-panel" aria-label="历史会话">
    <header>
      <span>历史会话</span>
      <button type="button" onclick={onCreate}>
        <Icon name="edit" size={13} />
        新建
      </button>
    </header>

    <div class="session-list">
      {#each sessions as session (session.id)}
        <article class:active={session.id === activeSessionId}>
          <button type="button" class="session-main" onclick={() => onSelect?.(session.id)}>
            <strong>{session.title}</strong>
            <span>{session.summary}</span>
            <small>{session.updatedAt}</small>
          </button>
          <button
            type="button"
            class="delete"
            aria-label={"删除会话 " + session.title}
            title={"删除会话 " + session.title}
            onclick={() => onDelete?.(session.id)}
          >
            <Icon name="trash" size={13} />
          </button>
        </article>
      {/each}
    </div>
  </section>
</div>

<style>
  .wrapper {
    position: absolute;
    inset: 0;
    z-index: 5;
  }

  .backdrop {
    position: absolute;
    inset: 0;
    border: 0;
    background: rgba(41, 41, 78, 0.08);
    backdrop-filter: blur(1px);
  }

  .history-panel {
    position: absolute;
    top: 67px;
    right: 14px;
    left: 14px;
    z-index: 2;
    overflow: hidden;
    border: 1px solid var(--vha-glass-edge);
    border-radius: 16px;
    background: var(--vha-glass-strong);
    box-shadow: 0 16px 38px var(--vha-shadow);
  }

  header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    height: 43px;
    padding: 0 10px 0 14px;
    border-bottom: 1px solid var(--vha-border);
    background: var(--vha-glass-gradient);
  }

  header span {
    color: var(--vha-text);
    font-size: 12.5px;
    font-weight: 650;
  }

  header button {
    display: flex;
    align-items: center;
    gap: 5px;
    height: 27px;
    padding: 0 9px;
    border: 1px solid var(--vha-border);
    border-radius: 9px;
    background: var(--vha-surface);
    color: var(--vha-muted);
    font-size: 11px;
  }

  header button:hover {
    background: var(--vha-elevated);
    color: var(--vha-accent);
  }

  .session-list {
    max-height: 308px;
    overflow-y: auto;
    padding: 8px;
  }

  article {
    display: flex;
    align-items: stretch;
    gap: 6px;
    border: 1px solid transparent;
    border-radius: 12px;
  }

  article:hover {
    background: var(--vha-surface);
  }

  article.active {
    border-color: var(--vha-border);
    background: var(--vha-selection-gradient);
  }

  .session-main {
    display: grid;
    min-width: 0;
    flex: 1;
    gap: 3px;
    padding: 10px;
    text-align: left;
  }

  strong {
    overflow: hidden;
    color: var(--vha-text);
    font-size: 11.8px;
    font-weight: 620;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .session-main span {
    display: -webkit-box;
    overflow: hidden;
    color: var(--vha-subdued);
    font-size: 10.6px;
    line-height: 1.35;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    line-clamp: 2;
  }

  .session-main small {
    color: var(--vha-muted);
    font-size: 10px;
  }

  .delete {
    display: grid;
    place-items: center;
    width: 30px;
    margin: 8px 8px 8px 0;
    border-radius: 9px;
    color: var(--vha-subdued);
  }

  .delete:hover {
    background: var(--vha-selection-gradient);
    color: var(--vha-accent);
  }
</style>
