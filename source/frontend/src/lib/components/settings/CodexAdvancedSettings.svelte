<script lang="ts">
  import type { CodexConfig } from "./types";

  let { codex = $bindable() }: { codex: CodexConfig } = $props();
</script>

<section class="codex" aria-label="Codex 高级设置">
  <div class="grid">
    <label>
      <span>Codex 可执行文件（留空自动查找）</span>
      <input
        value={codex.executable ?? ""}
        oninput={(event) => (codex.executable = event.currentTarget.value)}
        autocomplete="off"
        spellcheck="false"
        placeholder="自动查找"
      />
    </label>
    <label>
      <span>工作目录</span>
      <input bind:value={codex.workspace} autocomplete="off" spellcheck="false" />
    </label>
    <label>
      <span>Codex Home</span>
      <input bind:value={codex.home} autocomplete="off" spellcheck="false" />
    </label>
    <label><span>连接超时 ms</span><input bind:value={codex.connectTimeoutMs} type="number" min="1" max="3600000" /></label>
    <label><span>请求超时 ms</span><input bind:value={codex.requestTimeoutMs} type="number" min="1" max="3600000" /></label>
    <label><span>重连间隔 ms</span><input bind:value={codex.reconnectIntervalMs} type="number" min="1" max="3600000" /></label>
    <label><span>最大重连次数</span><input bind:value={codex.maxReconnectAttempts} type="number" min="0" max="100" /></label>
    <label><span>审批超时 ms</span><input bind:value={codex.approvalTimeoutMs} type="number" min="1" max="3600000" /></label>
    <label><span>事件容量</span><input bind:value={codex.eventCapacity} type="number" min="1" max="65536" /></label>
    <label class="checkbox">
      <input type="checkbox" bind:checked={codex.experimentalApi} />
      <span>启用实验 API</span>
    </label>
  </div>
  <p>Codex WebSocket 端口由后端每次启动自动分配；修改其他 Codex 参数后需要重启 VisionHyperAgent 生效。</p>
</section>

<style>
  .codex { display: grid; gap: 10px; }
  .grid { display: grid; grid-template-columns: repeat(auto-fit, minmax(175px, 1fr)); gap: 10px; }
  label { display: grid; min-width: 0; gap: 5px; }
  label span { color: var(--vha-subdued); font-size: 10.6px; }
  input { min-width: 0; height: 32px; padding: 0 9px; border: 1px solid var(--vha-border); border-radius: 9px; background: rgba(255, 255, 255, .84); color: var(--vha-text); font-size: 11.7px; }
  input:focus { border-color: var(--vha-accent); outline: 2px solid rgba(122, 105, 245, .14); }
  .checkbox { display: flex; align-items: center; gap: 7px; padding-bottom: 7px; }
  .checkbox input { width: 15px; height: 15px; min-width: 15px; padding: 0; accent-color: var(--vha-accent); }
  p { margin: 0; color: var(--vha-subdued); font-size: 10.8px; line-height: 1.5; }
</style>
