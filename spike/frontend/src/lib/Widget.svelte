<script lang="ts">
  import { fade } from "svelte/transition";
  import { tasks as tasksStore, completeTask, cat, openTaskRemote, openAgenda, widgetHeight } from "./data.svelte";

  const tasks = $derived(tasksStore());

  const today = $derived(new Date());

  const todayTasks = $derived(
    tasks.filter((t) => {
      const s = t.start;
      return (
        s.getDate() === today.getDate() &&
        s.getMonth() === today.getMonth() &&
        s.getFullYear() === today.getFullYear() &&
        t.status !== "completada"
      );
    }),
  );

  const upcoming = $derived(
    tasks
      .filter((t) => t.status !== "completada" && !todayTasks.includes(t))
      .sort((a, b) => a.start.getTime() - b.start.getTime()),
  );

  /** Altura máxima configurable del widget (px). Al alcanzarla, se corta con "+N tareas más". */
  const MAX_H = 560;
  const ROW_H = 30;
  const FIXED_H = 148;

  /** Filas que caben dentro de la altura máxima. */
  const capacity = $derived(Math.max(3, Math.floor((MAX_H - FIXED_H) / ROW_H)));

  const todayShown = $derived(todayTasks.slice(0, capacity));
  const upcomingShown = $derived(upcoming.slice(0, Math.max(0, capacity - todayTasks.length)));

  const hiddenCount = $derived(
    Math.max(0, tasks.filter((t) => t.status !== "completada").length - todayShown.length - upcomingShown.length),
  );

  const doneToday = $derived(
    tasks.filter((t) => {
      const s = t.start;
      return (
        s.getDate() === today.getDate() &&
        s.getMonth() === today.getMonth() &&
        s.getFullYear() === today.getFullYear() &&
        t.status === "completada"
      );
    }).length,
  );

  function timeLabel(t: { allDay: boolean; start: Date }): string {
    if (t.allDay) return "Todo el día";
    return t.start.toLocaleTimeString("es-ES", { hour: "2-digit", minute: "2-digit" });
  }

  function dueLabel(d: Date): string {
    const diff = d.getTime() - Date.now();
    const days = Math.floor(diff / 86400000);
    if (days <= 0) return "hoy";
    if (days === 1) return "mañana";
    return d.toLocaleDateString("es-ES", { day: "numeric", month: "short" });
  }

  let rootEl = $state<HTMLElement | null>(null);
  $effect(() => {
    if (!rootEl) return;
    const ro = new ResizeObserver(() => widgetHeight(rootEl.offsetHeight));
    ro.observe(rootEl);
    widgetHeight(rootEl.offsetHeight);
    return () => ro.disconnect();
  });
</script>

<div class="widget" bind:this={rootEl}>
  <header class="head" data-tauri-drag-region>
    <span class="brand" data-tauri-drag-region>
      <span class="logo" data-tauri-drag-region>F</span>
      <span data-tauri-drag-region>FocusFlow</span>
    </span>
    <span class="status" title="Sincronizado">
      <span class="pulse"></span>
      hoy · {doneToday} hecha{doneToday === 1 ? "" : "s"}
    </span>
  </header>

  <div class="body">
    {#if todayTasks.length === 0 && upcoming.length === 0}
      <div class="empty">Sin tareas pendientes. Descansa ✨</div>
    {:else}
      <div class="sec-label">Hoy</div>
      {#each todayShown as t (t.id)}
        <button class="task" type="button" transition:fade={{ duration: 140 }} onclick={() => openTaskRemote(t.id)}>
          <span class="dot" style="--c: {cat(t.categoryId).color}"></span>
          <span class="ttl">{t.title}</span>
          <span class="due" class:allday={t.allDay}>{timeLabel(t)}</span>
          <span class="check" onclick={(e) => { e.stopPropagation(); completeTask(t.id); }} title="Completar">✓</span>
        </button>
      {/each}

      {#if upcoming.length > 0}
        <div class="sec-label">Próximas</div>
        {#each upcomingShown as t (t.id)}
          <button class="task" type="button" transition:fade={{ duration: 140 }} onclick={() => openTaskRemote(t.id)}>
            <span class="dot" style="--c: {cat(t.categoryId).color}"></span>
            <span class="ttl">{t.title}</span>
            <span class="due">{dueLabel(t.start)}</span>
            <span class="check" onclick={(e) => { e.stopPropagation(); completeTask(t.id); }} title="Completar">✓</span>
          </button>
        {/each}
      {/if}
    {/if}
  </div>

  {#if hiddenCount > 0}
    <button class="more" type="button" onclick={openAgenda}>+{hiddenCount} tareas más</button>
  {/if}
</div>

<style>
  .widget {
    background: var(--surface);
    border-radius: var(--r-xl);
    box-shadow: var(--shadow-raised-lg);
    border: 1px solid var(--border);
    padding: var(--s-4);
    display: flex;
    flex-direction: column;
    gap: 8px;
    font-size: 13px;
  }
  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
    cursor: grab;
    user-select: none;
  }
  .head:active {
    cursor: grabbing;
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 8px;
    font-weight: 700;
    font-size: 13.5px;
  }
  .logo {
    width: 20px;
    height: 20px;
    border-radius: 7px;
    background: var(--primary);
    color: #fff;
    font-size: 11px;
    font-weight: 700;
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }
  .status {
    display: flex;
    align-items: center;
    gap: 5px;
    font-size: 10px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-3);
  }
  .pulse {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: #22c55e;
    box-shadow: 0 0 0 3px color-mix(in srgb, #22c55e 18%, transparent);
  }
  .body {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .sec-label {
    font-size: 9.5px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.1em;
    color: var(--text-3);
    margin-top: 2px;
  }
  .task {
    display: flex;
    align-items: center;
    gap: 8px;
    border: none;
    background: transparent;
    border-radius: var(--r-sm);
    padding: 6px 8px;
    font-size: 12.5px;
    font-family: inherit;
    color: var(--text-1);
    text-align: left;
    transition: background var(--dur-fast) var(--ease-out);
    cursor: pointer;
  }
  .task:hover {
    background: var(--surface-2);
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--c);
    flex-shrink: 0;
  }
  .ttl {
    flex: 1;
    min-width: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .due {
    font-size: 10.5px;
    font-weight: 600;
    color: var(--text-2);
    background: var(--surface-3);
    border-radius: var(--r-full);
    padding: 2px 9px;
    flex-shrink: 0;
    white-space: nowrap;
  }
  .due.allday {
    color: var(--primary);
    background: var(--primary-soft);
  }
  .check {
    width: 20px;
    height: 20px;
    border-radius: 50%;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    font-size: 11px;
    color: var(--text-3);
    border: 1px solid var(--border);
    flex-shrink: 0;
    opacity: 0;
    transition: all var(--dur-fast) var(--ease-out);
  }
  .task:hover .check {
    opacity: 1;
  }
  .check:hover {
    background: var(--primary);
    border-color: var(--primary);
    color: #fff;
    transform: scale(1.08);
  }
  .empty {
    font-size: 12.5px;
    color: var(--text-3);
    padding: 10px 8px;
    text-align: center;
  }
  .more {
    border: 1px dashed var(--border);
    background: transparent;
    color: var(--text-2);
    font-size: 12px;
    font-weight: 600;
    border-radius: var(--r-sm);
    padding: 6px;
    cursor: pointer;
    transition: all var(--dur-fast) var(--ease-out);
  }
  .more:hover {
    color: var(--primary);
    border-color: var(--primary);
    background: var(--primary-soft);
  }
</style>
