<script lang="ts"></script>

<header class="app-header">
  <div class="header-plate">
    <div class="agent-badge">Agent</div>
    <h1>驱动的视觉模型<span>自动训练</span>及部署平台</h1>
  </div>
</header>

<style>
  .app-header {
    position: relative;
    z-index: 2;
    flex: 0 0 104px;
  }

  .header-plate {
    position: relative;
    height: 78px;
    margin: 12px 16px 0;
    overflow: hidden;
    display: flex;
    align-items: center;
    gap: 18px;
    padding: 0 24px 0 18px;
    border: 1px solid transparent;
    border-radius: 19px;
    background:
      var(--vha-white-prism) padding-box,
      linear-gradient(
        110deg,
        rgba(255, 255, 255, 0.98) 0%,
        rgba(252, 251, 255, 0.95) 40%,
        rgba(250, 253, 255, 0.96) 72%,
        rgba(255, 253, 249, 0.95) 100%
      ) padding-box,
      var(--vha-spectrum-line) border-box;
    backdrop-filter: blur(18px) saturate(150%);
    box-shadow:
      0 8px 26px rgba(42, 48, 88, 0.1),
      inset 0 1px 0 rgba(255, 255, 255, 0.88);
  }

  .header-plate::after {
    content: "";
    position: absolute;
    inset: 1px;
    border-radius: 18px;
    pointer-events: none;
    background:
      linear-gradient(180deg, rgba(255, 255, 255, 0.76), rgba(255, 255, 255, 0.04) 50%, rgba(255, 255, 255, 0.54)),
      linear-gradient(
        112deg,
        transparent 34%,
        rgba(124, 77, 255, 0.13) 47%,
        rgba(244, 95, 168, 0.12) 58%,
        rgba(255, 255, 255, 0.66) 72%,
        rgba(40, 200, 216, 0.14) 88%
      );
  }

  .agent-badge {
    position: relative;
    z-index: 1;
    display: grid;
    place-items: center;
    width: 122px;
    height: 50px;
    flex: 0 0 auto;
    border: 1px solid transparent;
    border-radius: 13px;
    background:
      linear-gradient(145deg, #ffffff, #f5f2ff) padding-box,
      var(--vha-spectrum-line) border-box;
    box-shadow:
      inset 0 1px 0 rgba(255, 255, 255, 0.95),
      0 5px 17px rgba(105, 77, 197, 0.15);
    color: var(--vha-accent);
    font-family: "Nimbus Sans", Inter, sans-serif;
    font-size: 28.7px;
    font-style: italic;
    font-weight: 700;
    letter-spacing: -0.03em;
  }

  .agent-badge::before {
    content: "";
    position: absolute;
    inset: 1px 1px auto;
    height: 22px;
    border-radius: 12px 12px 0 0;
    background: linear-gradient(180deg, rgba(255, 255, 255, 0.196), rgba(255, 255, 255, 0));
  }

  h1 {
    position: relative;
    z-index: 1;
    margin: 0;
    color: var(--vha-header-ink);
    font-size: clamp(20px, 1.85vw, 28px);
    font-weight: 700;
    letter-spacing: 0.3px;
    white-space: nowrap;
  }

  span {
    background: var(--vha-action-gradient);
    -webkit-background-clip: text;
    background-clip: text;
    color: transparent;
  }
</style>
