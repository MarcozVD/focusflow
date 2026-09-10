<script lang="ts">
  import { MONTHS_ES } from "./data.svelte";
  import QuickAdd from "./QuickAdd.svelte";

  let {
    date,
    view,
    navigate,
    goToday,
    hmode = "semana",
    setHmode = () => {},
  }: {
    date: Date;
    view: string;
    navigate: (d: -1 | 1) => void;
    goToday: () => void;
    hmode?: "semana" | "dia";
    setHmode?: (m: "semana" | "dia") => void;
  } = $props();

  const title = $derived(
    view === "mes"
      ? `${MONTHS_ES[date.getMonth()]} ${date.getFullYear()}`
      : view === "dia"
        ? date.toLocaleDateString("es-ES", { weekday: "long", day: "numeric", month: "long" })
        : view === "horario"
          ? hmode === "dia"
            ? date.toLocaleDateString("es-ES", { weekday: "long", day: "numeric", month: "long" })
            : "Mi horario"
          : view === "sesiones"
            ? hmode === "dia"
              ? date.toLocaleDateString("es-ES", { weekday: "long", day: "numeric", month: "long" })
              : "Sesiones de estudio"
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
    {#if view === "horario" || view === "sesiones"}
      <div class="switcher">
        <button class="sw {hmode === 'semana' ? 'on' : ''}" onclick={() => setHmode("semana")}>Semana</button>
        <button class="sw {hmode === 'dia' ? 'on' : ''}" onclick={() => setHmode("dia")}>Día</button>
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
  .switcher {
    display: inline-flex;
    background: var(--surface);
    border-radius: 12px;
    box-shadow: var(--shadow-inset-sm);
    padding: 3px;
    gap: 2px;
  }
  .sw {
    border: none;
    background: transparent;
    border-radius: 9px;
    padding: 6px 12px;
    font-size: 12.5px;
    font-weight: 600;
    color: var(--text-3);
    font-family: inherit;
    transition: all var(--dur-fast) var(--ease-out);
  }
  .sw:hover {
    color: var(--text-1);
  }
  .sw.on {
    background: var(--primary);
    color: #fff;
    box-shadow: var(--e1);
  }
</style>
