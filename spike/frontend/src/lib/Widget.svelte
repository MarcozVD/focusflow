<script lang="ts">
  import { fade } from "svelte/transition";
  import { tasks as tasksStore, completeTask, cat, openTaskRemote } from "./data.svelte";

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
</script>

  <div class="widget">
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
        {#each todayTasks as t (t.id)}
          <button class="task" type="button" transition:fade={{ duration: 140 }} onclick={() => openTaskRemote(t.id)}>
            <span class="dot" style="--c: {cat(t.categoryId).color}"></span>
            <span class="ttl">{t.title}</span>
            <span class="due" class:allday={t.allDay}>{timeLabel(t)}</span>
            <span class="check" onclick={(e) => { e.stopPropagation(); completeTask(t.id); }} title="Completar">✓</span>
          </button>
        {/each}

        {#if upcoming.length > 0}
          <div class="sec-label">Próximas</div>
          {#each upcoming as t (t.id)}
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
    justify-content: flex-end;
    gap: 5px;
    font-size: 10px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-3);
    /* Ancho estable: al cambiar "1 hecha" → "12 hechas" el texto no empuja
       ni se corre a la derecha (el contador crece hacia la izquierda). */
    min-width: 96px;
    white-space: nowrap;
    text-align: right;
    font-variant-numeric: tabular-nums;
    overflow: hidden;
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
    /* La ventana es de tamaño fijo (sin resizing dinámico: causaba que el
       contenido se desplazara). Si la lista crece, hace scroll interno. */
    max-height: 340px;
    overflow-y: auto;
    overscroll-behavior: contain;
    scrollbar-width: thin;
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
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-width: 56px;
    font-size: 10.5px;
    font-weight: 600;
    color: var(--text-2);
    background: var(--surface-3);
    border-radius: var(--r-full);
    padding: 2px 9px;
    flex-shrink: 0;
    white-space: nowrap;
    /* Dígitos de ancho fijo: "09:00" y "10:30" miden igual → sin reflow. */
    font-variant-numeric: tabular-nums;
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
</style>
