<script lang="ts">
  import { toggleTheme, categories, tasks as tasksStore, DAYS_ES, suggestionsPending } from "./data.svelte";

  let {
    view,
    setView,
    navigate,
  }: {
    view: string;
    setView: (v: string) => void;
    navigate: (dir: -1 | 1) => void;
  } = $props();

  const today = $derived(new Date());
  const tasks = $derived(tasksStore());
  const pending = $derived(tasks.filter((t) => t.status !== "completada").length);
  const pendingSug = $derived(suggestionsPending());
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
      { id: "asistente", label: "Asistente", icon: "sparkle" },
      { id: "sugerencias", label: "Sugerencias", icon: "inbox" },
      { id: "ajustes", label: "Ajustes", icon: "gear" },
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
          {:else if item.icon === "list"}
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none"><path d="M8 6H21M8 12H21M8 18H21M3 6H3.01M3 12H3.01M3 18H3.01" stroke="currentColor" stroke-width="2.5" stroke-linecap="round"/></svg>
          {:else if item.icon === "inbox"}
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none"><path d="M22 12H16L14 15H10L8 12H2M5 4H19L22 12V18A2 2 0 0 1 20 20H4A2 2 0 0 1 2 18V12L5 4Z" stroke="currentColor" stroke-width="2" stroke-linejoin="round"/></svg>
          {:else if item.icon === "sparkle"}
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none"><path d="M12 3L13.8 9.2L20 11L13.8 12.8L12 19L10.2 12.8L4 11L10.2 9.2L12 3Z" stroke="currentColor" stroke-width="2" stroke-linejoin="round"/><path d="M19 16L19.9 18.9L22 19.8L19.9 20.7L19 23.6L18.1 20.7L16 19.8L18.1 18.9L19 16Z" stroke="currentColor" stroke-width="1.5" stroke-linejoin="round"/></svg>
          {:else}
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none"><circle cx="12" cy="12" r="3" stroke="currentColor" stroke-width="2"/><path d="M19.4 15A1.65 1.65 0 0 0 21 12.36V11.64A1.65 1.65 0 0 0 19.4 9 1.65 1.65 0 0 0 19.4 6.6 1.65 1.65 0 0 0 17 5.6 1.65 1.65 0 0 0 15 4 1.65 1.65 0 0 0 12.36 5.6H11.64A1.65 1.65 0 0 0 9 4a1.65 1.65 0 0 0-1.4 1.6A1.65 1.65 0 0 0 5.6 6.6 1.65 1.65 0 0 0 4 9a1.65 1.65 0 0 0 1.6 1.4v1.2A1.65 1.65 0 0 0 4 12.36v1.28A1.65 1.65 0 0 0 5.6 15 1.65 1.65 0 0 0 4.6 17.4 1.65 1.65 0 0 0 7 18.4 1.65 1.65 0 0 0 9 20a1.65 1.65 0 0 0 2.64-1.6h1.28A1.65 1.65 0 0 0 15 20a1.65 1.65 0 0 0 1.4-1.6A1.65 1.65 0 0 0 19 17.4a1.65 1.65 0 0 0 1-2Z" stroke="currentColor" stroke-width="2"/></svg>
          {/if}
        </span>
        {item.label}
        {#if item.id === "sugerencias" && pendingSug > 0}
          <span class="badge">{pendingSug}</span>
        {/if}
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
  .badge {
    margin-left: auto;
    background: var(--primary);
    color: #fff;
    font-size: 11px;
    font-weight: 700;
    min-width: 20px;
    height: 20px;
    border-radius: var(--r-full);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    padding: 0 6px;
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
  .today-box {
    margin-top: auto;
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
