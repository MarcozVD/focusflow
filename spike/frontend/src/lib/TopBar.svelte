<script lang="ts">
  import { MONTHS_ES } from "./data.svelte";
  import QuickAdd from "./QuickAdd.svelte";

  let {
    date,
    view,
    navigate,
    goToday,
  }: { date: Date; view: string; navigate: (d: -1 | 1) => void; goToday: () => void } = $props();

  const title = $derived(
    view === "mes"
      ? `${MONTHS_ES[date.getMonth()]} ${date.getFullYear()}`
      : view === "dia"
        ? date.toLocaleDateString("es-ES", { weekday: "long", day: "numeric", month: "long" })
        : view === "sugerencias"
          ? "Eventos detectados"
          : view === "ajustes"
            ? "Ajustes"
            : "Semana",
  );
</script>

<div class="top">
  <div class="left">
    <div class="title">{title}</div>
    {#if view !== "sugerencias" && view !== "ajustes"}
      <div class="nav">
        <button class="arrow" onclick={() => navigate(-1)} aria-label="Anterior">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none"><path d="M15 5L8 12L15 19" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"/></svg>
        </button>
        <button class="arrow" onclick={() => navigate(1)} aria-label="Siguiente">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none"><path d="M9 5L16 12L9 19" stroke="currentColor" stroke-width="2.5" stroke-linecap="round" stroke-linejoin="round"/></svg>
        </button>
        <button class="today" onclick={goToday}>Hoy</button>
      </div>
    {/if}
  </div>
  <QuickAdd />
</div>

<style>
  .top {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--s-5);
    padding: var(--s-5) var(--s-8);
  }
  .left {
    display: flex;
    align-items: center;
    gap: var(--s-5);
  }
  .title {
    font-size: 22px;
    font-weight: 700;
    letter-spacing: -0.02em;
    text-transform: capitalize;
  }
  .nav {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .arrow {
    width: 34px;
    height: 34px;
    border: none;
    background: var(--surface);
    border-radius: 12px;
    box-shadow: var(--e1);
    color: var(--text-2);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    transition: all var(--dur-fast) var(--ease-out);
  }
  .arrow:hover {
    color: var(--primary);
    transform: translateY(-1px);
    box-shadow: var(--e2);
  }
  .arrow:active {
    transform: translateY(0);
    box-shadow: var(--shadow-inset-sm);
  }
  .today {
    border: none;
    background: var(--surface);
    border-radius: 12px;
    box-shadow: var(--e1);
    padding: 8px 14px;
    font-size: 13px;
    font-weight: 600;
    color: var(--text-1);
    transition: all var(--dur-fast) var(--ease-out);
  }
  .today:hover {
    color: var(--primary);
    box-shadow: var(--e2);
  }
</style>
