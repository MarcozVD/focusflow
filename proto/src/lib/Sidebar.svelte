<script lang="ts">
  import { toggleTheme, categories, tasks, DAYS_ES } from "./data.svelte";

  let {
    view,
    setView,
    navigate,
    showWidget,
  }: {
    view: string;
    setView: (v: string) => void;
    navigate: (dir: -1 | 1) => void;
    showWidget: () => void;
  } = $props();

  const today = $derived(new Date());
  const pending = $derived(tasks.filter((t) => t.status !== "completada").length);
</script>

<div class="side">
  <div class="logo-row">
    <div class="logo">F</div>
    <span class="brand">FocusFlow</span>
  </div>

  <nav>
    {#each [
      { id: "semana", label: "Semana", icon: "calendar" },
      { id: "mes", label: "Mes", icon: "grid" },
      { id: "dia", label: "Día", icon: "sun" },
      { id: "agenda", label: "Agenda", icon: "list" },
    ] as item}
      <button
        class="nav-item {view === item.id ? 'active' : ''}"
        onclick={() => setView(item.id)}
      >
        <span class="ico">
          {#if item.icon === "calendar"}
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none"><rect x="3" y="5" width="18" height="16" rx="4" stroke="currentColor" stroke-width="2"/><path d="M3 10H21M8 3V7M16 3V7" stroke="currentColor" stroke-width="2" stroke-linecap="round"/></svg>
          {:else if item.icon === "grid"}
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none"><rect x="3" y="3" width="8" height="8" rx="3" stroke="currentColor" stroke-width="2"/><rect x="13" y="3" width="8" height="8" rx="3" stroke="currentColor" stroke-width="2"/><rect x="3" y="13" width="8" height="8" rx="3" stroke="currentColor" stroke-width="2"/><rect x="13" y="13" width="8" height="8" rx="3" stroke="currentColor" stroke-width="2"/></svg>
          {:else if item.icon === "sun"}
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none"><circle cx="12" cy="12" r="4" stroke="currentColor" stroke-width="2"/><path d="M12 2V5M12 19V22M2 12H5M19 12H22M4.9 4.9L7 7M17 17L19.1 19.1M19.1 4.9L17 7M7 17L4.9 19.1" stroke="currentColor" stroke-width="2" stroke-linecap="round"/></svg>
          {:else}
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none"><path d="M8 6H21M8 12H21M8 18H21M3 6H3.01M3 12H3.01M3 18H3.01" stroke="currentColor" stroke-width="2.5" stroke-linecap="round"/></svg>
          {/if}
        </span>
        {item.label}
      </button>
    {/each}
  </nav>

  <div class="section-label">Categorías</div>
  <div class="cats">
    {#each categories as c}
      <button class="cat-item" style="--c: {c.color}">
        <span class="dot"></span>
        {c.name}
        <span class="count">{tasks.filter((t) => t.categoryId === c.id && t.status !== "completada").length}</span>
      </button>
    {/each}
  </div>

  <button class="widget-btn" onclick={showWidget}>
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none"><rect x="4" y="4" width="16" height="16" rx="5" stroke="currentColor" stroke-width="2"/><rect x="9" y="9" width="6" height="6" rx="2" stroke="currentColor" stroke-width="2"/></svg>
    Ver widget de escritorio
  </button>

  <div class="today-box">
    <div class="tb-day">{DAYS_ES[today.getDay()]}</div>
    <div class="tb-num">{today.getDate()}</div>
    <div class="tb-pending">{pending} pendientes</div>
  </div>

  <button class="theme-btn" onclick={toggleTheme} title="Cambiar tema">
    <svg width="16" height="16" viewBox="0 0 24 24" fill="none"><path d="M21 12.8A9 9 0 1 1 11.2 3a7 7 0 0 0 9.8 9.8Z" stroke="currentColor" stroke-width="2" stroke-linejoin="round"/></svg>
    Tema
  </button>
</div>

<style>
  .side {
    width: 232px;
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    gap: var(--s-2);
    padding: var(--s-5);
    overflow-y: auto;
  }
  .logo-row {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: var(--s-5);
  }
  .logo {
    width: 34px;
    height: 34px;
    border-radius: 12px;
    background: var(--primary);
    color: #fff;
    font-weight: 700;
    font-size: 16px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    box-shadow: 0 6px 14px -4px color-mix(in srgb, var(--primary) 60%, transparent);
  }
  .brand {
    font-size: 17px;
    font-weight: 700;
    letter-spacing: -0.01em;
  }
  nav {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .nav-item {
    display: flex;
    align-items: center;
    gap: 12px;
    border: none;
    background: transparent;
    border-radius: 14px;
    padding: 10px 12px;
    font-size: 14px;
    font-weight: 500;
    color: var(--text-2);
    transition: all var(--dur-fast) var(--ease-out);
  }
  .nav-item:hover {
    background: var(--surface-2);
    color: var(--text-1);
  }
  .nav-item.active {
    background: var(--primary-soft);
    color: var(--primary);
    font-weight: 600;
  }
  .ico {
    display: inline-flex;
  }
  .section-label {
    font-size: 10px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.1em;
    color: var(--text-3);
    margin: var(--s-5) 0 4px var(--s-3);
  }
  .cats {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .cat-item {
    display: flex;
    align-items: center;
    gap: 10px;
    border: none;
    background: transparent;
    border-radius: 12px;
    padding: 8px 12px;
    font-size: 13px;
    font-weight: 500;
    color: var(--text-2);
    transition: background var(--dur-fast) var(--ease-out);
  }
  .cat-item:hover {
    background: var(--surface-2);
  }
  .dot {
    width: 9px;
    height: 9px;
    border-radius: 50%;
    background: var(--c);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--c) 18%, transparent);
  }
  .count {
    margin-left: auto;
    font-size: 11px;
    font-weight: 600;
    color: var(--text-3);
    background: var(--surface-2);
    border-radius: var(--r-full);
    padding: 1px 8px;
  }
  .widget-btn {
    margin-top: auto;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    border: 1px dashed var(--border);
    background: var(--surface);
    border-radius: var(--r-md);
    padding: 10px;
    font-size: 13px;
    font-weight: 600;
    color: var(--text-2);
    transition: all var(--dur-fast) var(--ease-out);
  }
  .widget-btn:hover {
    color: var(--primary);
    border-color: var(--primary);
    background: var(--primary-soft);
  }
  .today-box {
    display: flex;
    align-items: center;
    gap: 10px;
    background: var(--surface);
    border-radius: var(--r-md);
    box-shadow: var(--shadow-raised);
    padding: 10px 14px;
  }
  .tb-day {
    font-size: 11px;
    font-weight: 700;
    text-transform: uppercase;
    color: var(--text-3);
  }
  .tb-num {
    font-size: 18px;
    font-weight: 700;
    color: var(--primary);
    font-variant-numeric: tabular-nums;
  }
  .tb-pending {
    margin-left: auto;
    font-size: 12px;
    color: var(--text-2);
  }
  .theme-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 8px;
    border: none;
    background: transparent;
    border-radius: 14px;
    padding: 9px;
    font-size: 13px;
    font-weight: 500;
    color: var(--text-2);
    transition: background var(--dur-fast) var(--ease-out);
  }
  .theme-btn:hover {
    background: var(--surface-2);
  }
</style>
