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
    border: 1px solid rgba(255, 255, 255, 0.91);
    border-radius: 19px;
    background: var(--vha-header-silver);
    box-shadow: 0 6px 22px rgba(104, 67, 143, 0.18);
  }

  .header-plate::after {
    content: "";
    position: absolute;
    inset: 1px;
    border-radius: 18px;
    pointer-events: none;
    background:
      linear-gradient(180deg, rgba(255, 255, 255, 0.8), rgba(255, 255, 255, 0.05) 50%, rgba(255, 255, 255, 0.58)),
      linear-gradient(
        108deg,
        transparent 40%,
        rgba(229, 151, 240, 0.24) 54%,
        rgba(255, 198, 209, 0.37) 65%,
        rgba(250, 244, 255, 0.68) 77%,
        rgba(120, 230, 241, 0.26) 91%
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
    border: 1px solid rgba(230, 203, 255, 0.863);
    border-radius: 13px;
    background: var(--vha-header-badge);
    box-shadow: 0 5px 18px rgba(130, 81, 206, 0.36);
    color: white;
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
    color: var(--vha-header-accent);
  }
</style>
