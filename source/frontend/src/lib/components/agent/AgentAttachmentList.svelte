<script lang="ts">
  import Icon from "../Icon.svelte";
  import type { AgentAttachment } from "./types";

  let {
    attachments,
    onRemove,
  }: {
    attachments: AgentAttachment[];
    onRemove?: (id: string) => void;
  } = $props();

  function formatFileSize(size: number) {
    if (size < 1024) return size + " B";
    if (size < 1024 * 1024) return (size / 1024).toFixed(1) + " KB";
    return (size / 1024 / 1024).toFixed(1) + " MB";
  }
</script>

{#if attachments.length > 0}
  <ul class="attachments" aria-label="待发送附件">
    {#each attachments as attachment (attachment.id)}
      <li>
        {#if attachment.previewUrl}
          <img src={attachment.previewUrl} alt={attachment.file.name} />
        {:else}
          <span class="file-icon">
            <Icon name="folder" size={15} />
          </span>
        {/if}

        <span class="meta">
          <strong>{attachment.file.name}</strong>
          <small>{formatFileSize(attachment.file.size)}</small>
        </span>

        <button
          type="button"
          aria-label={"删除附件 " + attachment.file.name}
          title={"删除附件 " + attachment.file.name}
          onclick={() => onRemove?.(attachment.id)}
        >
          <Icon name="close" size={12} />
        </button>
      </li>
    {/each}
  </ul>
{/if}

<style>
  .attachments {
    display: flex;
    flex: 0 0 auto;
    gap: 8px;
    max-height: 94px;
    margin: 0;
    padding: 2px;
    overflow-y: auto;
    list-style: none;
  }

  li {
    display: flex;
    min-width: 176px;
    max-width: 100%;
    align-items: center;
    gap: 8px;
    flex: 0 0 auto;
    padding: 6px;
    border: 1px solid var(--vha-border);
    border-radius: 12px;
    background: var(--vha-elevated);
  }

  img,
  .file-icon {
    display: grid;
    place-items: center;
    width: 34px;
    height: 34px;
    flex: 0 0 auto;
    overflow: hidden;
    border: 1px solid var(--vha-glass-edge);
    border-radius: 9px;
    background: var(--vha-surface);
    color: var(--vha-muted);
    object-fit: cover;
  }

  .meta {
    display: grid;
    min-width: 0;
    flex: 1;
    gap: 2px;
  }

  strong {
    overflow: hidden;
    color: var(--vha-text);
    font-size: 10.8px;
    font-weight: 600;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  small {
    color: var(--vha-subdued);
    font-size: 10px;
  }

  button {
    display: grid;
    place-items: center;
    width: 21px;
    height: 21px;
    flex: 0 0 auto;
    border-radius: 7px;
    color: var(--vha-subdued);
    transition: background-color 120ms ease, color 120ms ease;
  }

  button:hover {
    background: var(--vha-selection-gradient);
    color: var(--vha-accent);
  }
</style>
