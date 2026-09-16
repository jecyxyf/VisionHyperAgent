<script lang="ts">
  import { onMount } from "svelte";
  import AgentPanel from "./lib/components/AgentPanel.svelte";
  import AppHeader from "./lib/components/AppHeader.svelte";
  import GlassPanel from "./lib/components/GlassPanel.svelte";
  import Icon from "./lib/components/Icon.svelte";
  import NavItem from "./lib/components/NavItem.svelte";
  import StatusBar from "./lib/components/StatusBar.svelte";
  import AboutPage from "./pages/About.svelte";
  import AnnotationPage from "./pages/Annotation.svelte";
  import InferencePage from "./pages/Inference.svelte";
  import ModelsPage from "./pages/Models.svelte";
  import PreannotationPage from "./pages/Preannotation.svelte";
  import PretrainingPage from "./pages/Pretraining.svelte";
  import SettingsPage from "./pages/Settings.svelte";
  import Splitter from "./lib/components/Splitter.svelte";
  import TrainingPage from "./pages/Training.svelte";

  type Page =
    | "inference"
    | "models"
    | "preannotation"
    | "annotation"
    | "pretraining"
    | "training"
    | "settings"
    | "about";

  type ModelPage = Extract<Page, "preannotation" | "annotation" | "pretraining" | "training">;

  let currentPage = $state<Page>("inference");
  let modelsExpanded = $state(true);
  let workspaceGrid = $state<HTMLElement | null>(null);
  const initialViewportWidth = typeof window === "undefined" ? 1440 : window.innerWidth;
  let navigationWidth = $state(140);
  let agentWidth = $state(Math.max(260, Math.floor((initialViewportWidth - 64 - 140) / 3)));
  let statusMessage = $state("");
  let statusTimer: ReturnType<typeof setTimeout> | null = null;

  const modelPages: Array<{ id: ModelPage; label: string }> = [
    { id: "preannotation", label: "预标注" },
    { id: "annotation", label: "标注" },
    { id: "pretraining", label: "预训练" },
    { id: "training", label: "训练" },
  ];

  function showNotice(message: string) {
    statusMessage = message;
    if (statusTimer) clearTimeout(statusTimer);
    statusTimer = setTimeout(() => (statusMessage = ""), 5000);
  }

  function resizeNavigation(nextWidth: number) {
    navigationWidth = nextWidth;
  }

  function resizeAgent(nextWidth: number) {
    agentWidth = nextWidth;
  }

  function clampPanelWidths() {
    const availableWidth = workspaceGrid?.clientWidth ?? initialViewportWidth - 32;
    navigationWidth = Math.min(
      Math.max(140, navigationWidth),
      Math.max(140, availableWidth - 32 - 540 - 260),
    );
    agentWidth = Math.min(
      Math.max(260, agentWidth),
      Math.max(260, availableWidth - 32 - 540 - navigationWidth),
    );
  }

  onMount(() => {
    clampPanelWidths();
    window.addEventListener("resize", clampPanelWidths);

    return () => window.removeEventListener("resize", clampPanelWidths);
  });
</script>

<div class="app-shell">
  <div class="ambient" aria-hidden="true"></div>
  <AppHeader />

  <main class="workspace-grid" bind:this={workspaceGrid} style={`grid-template-columns: ${navigationWidth}px 16px minmax(540px, 1fr) 16px ${agentWidth}px;`}>
    <GlassPanel class="navigation">
      <NavItem text="运行" icon="play" selected={currentPage === "inference"} onclick={() => (currentPage = "inference")} />
      <div class="nav-gap"></div>

      <div class="expandable" class:selected={currentPage === "models" || (!modelsExpanded && modelPages.some((page) => page.id === currentPage))}>
        <button type="button" class="model-main" onclick={() => (currentPage = "models")}>
          <Icon name="train" size={17} />
          <span>模型</span>
        </button>
        <button
          type="button"
          class="disclosure"
          aria-label={modelsExpanded ? "折叠模型子页面" : "展开模型子页面"}
          aria-expanded={modelsExpanded}
          onclick={() => (modelsExpanded = !modelsExpanded)}
        >
          <Icon name={modelsExpanded ? "chevron" : "next"} size={14} />
        </button>
      </div>

      {#if modelsExpanded}
        {#each modelPages as page (page.id)}
          <NavItem text={page.label} nested selected={currentPage === page.id} onclick={() => (currentPage = page.id)} />
        {/each}
      {/if}

      <div class="nav-bottom"></div>
      <NavItem text="设置" icon="settings" selected={currentPage === "settings"} onclick={() => (currentPage = "settings")} />
      <NavItem text="关于" icon="about" selected={currentPage === "about"} onclick={() => (currentPage = "about")} />
    </GlassPanel>

    <Splitter
      value={navigationWidth}
      minimum={140}
      maximum={Math.max(140, (workspaceGrid?.clientWidth ?? initialViewportWidth - 32) - 32 - 540 - 260)}
      label="调整导航面板宽度"
      onResize={resizeNavigation}
    />

    <GlassPanel class="workspace">
      <div class="page-container">
        {#if currentPage === "inference"}
          <InferencePage onAction={showNotice} />
        {:else if currentPage === "models"}
          <ModelsPage onAction={showNotice} />
        {:else if currentPage === "preannotation"}
          <PreannotationPage onAction={showNotice} />
        {:else if currentPage === "annotation"}
          <AnnotationPage onAction={showNotice} />
        {:else if currentPage === "pretraining"}
          <PretrainingPage onAction={showNotice} />
        {:else if currentPage === "training"}
          <TrainingPage onAction={showNotice} />
        {:else if currentPage === "settings"}
          <SettingsPage onAction={showNotice} />
        {:else}
          <AboutPage />
        {/if}
      </div>
    </GlassPanel>

    <Splitter
      value={agentWidth}
      minimum={260}
      maximum={Math.max(260, (workspaceGrid?.clientWidth ?? initialViewportWidth - 32) - 32 - 540 - navigationWidth)}
      label="调整 Agent 面板宽度"
      reverse={true}
      onResize={resizeAgent}
    />
    <AgentPanel />
  </main>

  <StatusBar message={statusMessage} onDismiss={() => (statusMessage = "")} />
</div>

<style>
  .app-shell {
    position: relative;
    width: 100%;
    height: 100vh;
    min-width: 1120px;
    min-height: 720px;
    overflow: hidden;
    display: flex;
    flex-direction: column;
    padding-bottom: 16px;
    background: var(--vha-background);
  }

  .ambient {
    position: absolute;
    inset: 0;
    pointer-events: none;
    background-color: #fefeff;
    background:
      radial-gradient(circle at 7.5% 10%, rgba(124, 77, 255, 0.16), rgba(124, 77, 255, 0.055) 25%, transparent 47%),
      radial-gradient(circle at 89.4% 15%, rgba(244, 95, 168, 0.125), rgba(244, 95, 168, 0.045) 26%, transparent 49%),
      radial-gradient(circle at 1.9% 99%, rgba(40, 200, 216, 0.125), rgba(40, 200, 216, 0.045) 26%, transparent 49%),
      radial-gradient(circle at 95.5% 105%, rgba(255, 182, 92, 0.125), rgba(255, 182, 92, 0.045) 25%, transparent 49%),
      radial-gradient(circle at 58% 88%, rgba(53, 214, 168, 0.085), rgba(53, 214, 168, 0.03) 23%, transparent 45%),
      radial-gradient(circle at 49% 46%, rgba(255, 255, 255, 0.96), transparent 37%),
      linear-gradient(135deg, #ffffff 0%, #fbfbff 47%, #fbfdff 100%);
    animation: prismatic-drift 18s ease-in-out infinite alternate;
  }

  .workspace-grid {
    position: relative;
    z-index: 1;
    flex: 1;
    min-height: 0;
    margin: 0 16px;
    display: grid;
    grid-template-columns: 140px 16px minmax(540px, 2fr) 16px minmax(260px, 1fr);
  }

  :global(.navigation) {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 16px 10px 10px;
  }

  .nav-gap { height: 14px; }

  .expandable {
    display: flex;
    align-items: center;
    height: 40px;
    border: 1px solid transparent;
    border-radius: 12px;
    color: var(--vha-muted);
  }

  .expandable:hover { background: var(--vha-elevated); }

  .expandable.selected {
    background:
      linear-gradient(145deg, rgba(255, 255, 255, 0.96), rgba(248, 247, 255, 0.9)) padding-box,
      var(--vha-spectrum-line) border-box;
    color: var(--vha-accent);
    box-shadow:
      inset 0 1px 0 rgba(255, 255, 255, 0.9),
      0 5px 18px rgba(105, 77, 197, 0.13);
  }

  .expandable.selected:hover {
    background:
      linear-gradient(145deg, #ffffff, #f7fbff) padding-box,
      var(--vha-spectrum-line) border-box;
  }

  .model-main {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 10px;
    height: 100%;
    padding-left: 12px;
    color: inherit;
    text-align: left;
  }

  .model-main span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .disclosure {
    display: grid;
    place-items: center;
    width: 30px;
    height: 100%;
    color: inherit;
  }

  .nav-bottom { flex: 1; min-height: 8px; }
  :global(.workspace) { overflow: hidden; }

  .page-container {
    position: absolute;
    inset: 18px;
    overflow: auto;
  }

  @keyframes prismatic-drift {
    from {
      transform: translate3d(-0.8%, -0.5%, 0) scale(1.01);
    }
    to {
      transform: translate3d(0.8%, 0.5%, 0) scale(1.025);
    }
  }

</style>
