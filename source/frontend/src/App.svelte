<script lang="ts">
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
</script>

<div class="app-shell">
  <div class="ambient" aria-hidden="true"></div>
  <AppHeader />

  <main class="workspace-grid">
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

    <i class="splitter" aria-hidden="true"></i>

    <GlassPanel class="workspace">
      <div class="page-container">
        {#if currentPage === "inference"}
          <InferencePage onAction={showNotice} />
        {:else if currentPage === "models"}
          <ModelsPage onAction={showNotice} />
        {:else if currentPage === "preannotation"}
          <PreannotationPage onAction={showNotice} />
        {:else if currentPage === "annotation"}
          <AnnotationPage />
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

    <i class="splitter" aria-hidden="true"></i>
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
    background:
      radial-gradient(circle at 7.5% 12%, rgba(120, 112, 245, 0.92), rgba(173, 168, 252, 0.64) 24%, transparent 46%),
      radial-gradient(circle at 89.4% 17%, rgba(243, 95, 197, 0.8), rgba(255, 173, 230, 0.54) 25%, transparent 48%),
      radial-gradient(circle at 1.9% 98%, rgba(67, 216, 233, 0.88), rgba(144, 234, 241, 0.63) 26%, transparent 49%),
      radial-gradient(circle at 95% 104%, rgba(255, 173, 120, 0.86), rgba(255, 209, 165, 0.58) 25%, transparent 49%),
      radial-gradient(circle at 48.8% 45%, rgba(255, 255, 255, 0.64), transparent 33%);
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
    background: var(--vha-surface);
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
    border-color: var(--vha-glass-edge);
    background: var(--vha-action-gradient);
    color: white;
    box-shadow: 0 2px 18px var(--vha-action-shadow);
  }

  .expandable.selected:hover { background: var(--vha-action-gradient); }

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

  .splitter { display: grid; place-items: center; }

  .splitter::before {
    width: 2px;
    height: 44px;
    border-radius: 2px;
    background: rgba(101, 82, 171, 0.16);
    content: "";
  }
</style>
