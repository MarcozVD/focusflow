<script lang="ts">
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { onMount } from "svelte";

  let maximized = $state(false);
  const inTauri = typeof window !== "undefined" && "__TAURI_INTERNALS__" in window;

  onMount(() => {
    if (!inTauri) return;
    const win = getCurrentWindow();
    win.isMaximized().then((m) => (maximized = m));
    const unlisten = win.onResized(() => {
      win.isMaximized().then((m) => {
        if (m !== maximized) maximized = m;
      });
    });
    return () => {
      unlisten.then((f) => f());
    };
  });

  async function minimize() {
    if (!inTauri) return;
    await getCurrentWindow().minimize();
  }
  async function toggleMaximize() {
    if (!inTauri) return;
    const win = getCurrentWindow();
    if (await win.isMaximized()) {
      await win.unmaximize();
    } else {
      await win.maximize();
    }
  }
  async function close() {
    if (!inTauri) return;
    await getCurrentWindow().close();
  }
</script>

<header
  class="titlebar"
  oncontextmenu={(e) => e.preventDefault()}
  ondblclick={toggleMaximize}
>
  <div class="brand" data-tauri-drag-region>
    <span class="logo">F</span>
    <span class="name" data-tauri-drag-region>FocusFlow</span>
  </div>

  <div class="controls">
    <button class="ctl min" onclick={minimize} title="Minimizar" aria-label="Minimizar">
      <svg width="12" height="12" viewBox="0 0 12 12"><path d="M2 6h8" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/></svg>
    </button>
    <button class="ctl max" onclick={toggleMaximize} title={maximized ? "Restaurar" : "Maximizar"} aria-label="Maximizar">
      {#if maximized}
        <svg width="12" height="12" viewBox="0 0 12 12"><rect x="2.5" y="4" width="5.5" height="5.5" rx="1.5" fill="none" stroke="currentColor" stroke-width="1.4"/><path d="M4.5 4V3A1.5 1.5 0 0 1 6 1.5H9A1.5 1.5 0 0 1 10.5 3V6A1.5 1.5 0 0 1 9 7.5H8" fill="none" stroke="currentColor" stroke-width="1.4"/></svg>
      {:else}
        <svg width="12" height="12" viewBox="0 0 12 12"><rect x="2" y="2" width="8" height="8" rx="1.8" fill="none" stroke="currentColor" stroke-width="1.4"/></svg>
      {/if}
    </button>
    <button class="ctl close" onclick={close} title="Cerrar (bandeja)" aria-label="Cerrar">
      <svg width="12" height="12" viewBox="0 0 12 12"><path d="M3 3l6 6M9 3l-6 6" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/></svg>
    </button>
  </div>
</header>

<style>
  .titlebar {
    height: 44px;
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding-left: var(--s-4);
    background: color-mix(in srgb, var(--surface) 82%, transparent);
    user-select: none;
    -webkit-app-region: drag;
    z-index: 40;
    position: relative;
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 9px;
  }
  .logo {
    width: 22px;
    height: 22px;
    border-radius: 7px;
    background: var(--primary);
    color: #fff;
    font-size: 12px;
    font-weight: 700;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    box-shadow: 0 4px 10px -2px color-mix(in srgb, var(--primary) 55%, transparent);
  }
  .name {
    font-size: 13px;
    font-weight: 700;
    letter-spacing: -0.01em;
    color: var(--text-2);
  }
  .controls {
    display: flex;
    align-items: stretch;
    height: 100%;
    -webkit-app-region: no-drag;
  }
  .ctl {
    width: 46px;
    height: 100%;
    border: none;
    background: transparent;
    color: var(--text-3);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    transition: background var(--dur-fast) var(--ease-out), color var(--dur-fast) var(--ease-out);
    position: relative;
  }
  .ctl::before {
    content: "";
    position: absolute;
    inset: 8px 7px;
    border-radius: 10px;
    background: transparent;
    transition: background var(--dur-fast) var(--ease-out);
    z-index: 0;
  }
  .ctl svg {
    position: relative;
    z-index: 1;
  }
  .ctl:hover::before {
    background: var(--surface-2);
  }
  .ctl:hover {
    color: var(--text-1);
  }
  .ctl.min:hover::before {
    background: var(--surface-2);
    box-shadow: inset 2px 2px 5px rgba(31, 41, 55, 0.06), inset -2px -2px 5px rgba(255, 255, 255, 0.9);
  }
  .ctl.max:hover::before {
    background: var(--primary-soft);
    box-shadow: inset 2px 2px 5px rgba(31, 41, 55, 0.06), inset -2px -2px 5px rgba(255, 255, 255, 0.9);
  }
  .ctl.max:hover {
    color: var(--primary);
  }
  .ctl.close:hover::before {
    background: var(--danger);
  }
  .ctl.close:hover {
    color: #fff;
  }
  .ctl:active::before {
    transform: scale(0.92);
  }
</style>
