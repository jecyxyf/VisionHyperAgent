<script lang="ts">
  import GlassPanel from "../lib/components/GlassPanel.svelte";
  import IconButton from "../lib/components/IconButton.svelte";
  import PageHeader from "../lib/components/PageHeader.svelte";
  import Splitter from "../lib/components/Splitter.svelte";

  let { onAction }: { onAction: (message: string) => void } = $props();
  let epochs = $state("");
  let batchSize = $state("");
  let imageSize = $state("");
  let learningRate = $state("");
  let bodyWidth = $state(0);
  let effectsHeight = $state(0);
  let chartsHeight = $state(0);
  let parametersWidth = $state(210);
  let lossHeight = $state(180);
  let logHeight = $state(114);
  let parametersMaximum = $state(620);
  let lossMaximum = $state(260);
  let logMaximum = $state(220);

  $effect(() => {
    parametersMaximum = Math.max(180, bodyWidth - 12 - 280);
    if (parametersWidth > parametersMaximum) parametersWidth = parametersMaximum;
  });

  $effect(() => {
    logMaximum = Math.max(96, effectsHeight - 12 - 352);
    if (logHeight > logMaximum) logHeight = logMaximum;
  });

  $effect(() => {
    lossMaximum = Math.max(130, chartsHeight - 12 - 130);
    if (lossHeight > lossMaximum) lossHeight = lossMaximum;
  });
</script>

<div class="training-page">
  <PageHeader title="训练" icon="train">
    <IconButton label="Agent 建议参数" icon="spark" onclick={() => onAction("Agent 未连接，无法生成参数建议。")} />
    <i class="toolbar-divider" aria-hidden="true"></i>
    <IconButton label="停止训练" icon="stop" onclick={() => onAction("没有正在执行的训练任务。")} />
    <IconButton label="开始训练" icon="play" primary onclick={() => onAction("训练功能暂不可用。")} />
  </PageHeader>

  <div
    class="body"
    bind:clientWidth={bodyWidth}
    style={`grid-template-columns: ${parametersWidth}px 12px minmax(280px, 1fr);`}
  >
    <GlassPanel inset class="parameters">
      <h3>训练参数</h3>
      <p class="dataset">未选择数据集</p>
      <label><span>训练轮数 / Epochs</span><input bind:value={epochs} placeholder="自动" /></label>
      <label><span>批次大小 / Batch</span><input bind:value={batchSize} placeholder="自动" /></label>
      <label><span>图像尺寸 / Image size</span><input bind:value={imageSize} placeholder="自动" /></label>
      <label><span>学习率 / Learning rate</span><input bind:value={learningRate} placeholder="自动" /></label>
    </GlassPanel>

    <Splitter
      value={parametersWidth}
      minimum={180}
      maximum={parametersMaximum}
      label="训练参数宽度"
      onResize={(width) => (parametersWidth = width)}
    />

    <div
      class="effects-column"
      bind:clientHeight={effectsHeight}
      style={`grid-template-rows: minmax(352px, 1fr) 12px ${logHeight}px;`}
    >
      <GlassPanel inset class="effects">
        <div class="effects-head">
          <h3>训练效果</h3>
          <span>未开始</span>
        </div>

        <div
          class="charts"
          bind:clientHeight={chartsHeight}
          style={`grid-template-rows: ${lossHeight}px 12px minmax(130px, 1fr);`}
        >
          <section class="chart">
            <div class="chart-head">
              <strong>损失 / Loss</strong>
              <span>暂无数据</span>
            </div>
          </section>

          <Splitter
            value={lossHeight}
            minimum={130}
            maximum={lossMaximum}
            label="损失曲线高度"
            orientation="horizontal"
            onResize={(height) => (lossHeight = height)}
          />

          <section class="chart">
            <div class="chart-head">
              <strong>评估指标</strong>
              <span>暂无数据</span>
            </div>
          </section>
        </div>
      </GlassPanel>

      <Splitter
        value={logHeight}
        minimum={96}
        maximum={logMaximum}
        label="训练任务日志高度"
        orientation="horizontal"
        reverse={true}
        onResize={(height) => (logHeight = height)}
      />

      <GlassPanel inset class="logs">
        <h3>任务日志</h3>
        <p>暂无日志</p>
      </GlassPanel>
    </div>
  </div>
</div>

<style>
  .training-page {
    height: 100%;
    display: flex;
    flex-direction: column;
    gap: 16px;
    min-width: 0;
    min-height: 0;
  }

  .toolbar-divider {
    width: 1px;
    height: 18px;
    background: var(--vha-border);
  }

  .body {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-columns: 210px 12px minmax(280px, 1fr);
  }

  :global(.parameters),
  :global(.effects),
  :global(.logs) {
    min-width: 0;
    min-height: 0;
    padding: 16px;
  }

  :global(.parameters) {
    display: flex;
    flex-direction: column;
    gap: 14px;
    overflow-y: auto;
  }

  .dataset {
    margin: 0;
    color: var(--vha-subdued);
    font-size: 11.2px;
  }

  .effects-column {
    min-width: 0;
    min-height: 0;
    display: grid;
    grid-template-rows: minmax(352px, 1fr) 12px 114px;
  }

  :global(.effects),
  :global(.logs) {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  h3 {
    margin: 0;
    color: var(--vha-text);
    font-size: 14px;
    font-weight: 600;
  }

  .effects-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }

  .effects-head span {
    color: var(--vha-subdued);
    font-size: 11px;
  }

  .charts {
    flex: 1;
    min-height: 0;
    display: grid;
    grid-template-rows: 180px 12px minmax(130px, 1fr);
  }

  .chart {
    position: relative;
    min-width: 0;
    min-height: 0;
    padding: 12px;
    overflow: hidden;
    border: 1px solid var(--vha-border);
    border-radius: 14px;
    background:
      var(--vha-canvas-gradient),
      linear-gradient(rgba(102, 81, 167, 0.066) 1px, transparent 1px),
      linear-gradient(90deg, rgba(102, 81, 167, 0.066) 1px, transparent 1px);
    background-size: auto, 100% 28px, 28px 100%;
  }

  .chart::after {
    position: absolute;
    inset: auto 12px 12px;
    height: 1px;
    background: linear-gradient(90deg, transparent, var(--vha-border), transparent);
    content: "";
  }

  .chart-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 12px;
  }

  .chart-head strong {
    color: var(--vha-text);
    font-size: 12.4px;
    font-weight: 600;
  }

  .chart-head span {
    color: var(--vha-subdued);
    font-size: 10.8px;
  }

  label {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  label span {
    color: var(--vha-muted);
    font-size: 11.5px;
  }

  input {
    height: 34px;
    padding: 0 10px;
    border: 1px solid var(--vha-border);
    border-radius: 10px;
    background: var(--vha-field-gradient);
    color: var(--vha-text);
    font-size: 12.3px;
  }

  input:focus {
    outline: none;
    border-color: var(--vha-accent);
  }

  :global(.logs) p {
    margin: 0;
    color: var(--vha-subdued);
    font-size: 11.5px;
  }
</style>
