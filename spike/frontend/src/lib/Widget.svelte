<script lang="ts">
  import { onMount } from "svelte";
  import { fade } from "svelte/transition";
  import {
    tasks as tasksStore,
    cat,
    openTaskRemote,
    openApp,
    askAssistant,
    widgetAction,
  } from "./data.svelte";

  const tasks = $derived(tasksStore());

  // reloj local: la única "consulta" es el propio reloj del frontend
  // (sin polling a la BD; los datos llegan por tasks:changed)
  let now = $state(Date.now());
  onMount(() => {
    const iv = setInterval(() => (now = Date.now()), 30_000);
    return () => clearInterval(iv);
  });

  const pending = $derived(
    tasks.filter((t) => t.status !== "completada").sort((a, b) => a.start.getTime() - b.start.getTime()),
  );

  // AHORA: actividad en curso (ventana [start, end] contiene a `now`) o,
  // si no, la única tarea marcada en-curso.
  const current = $derived(
    (() => {
      const inWindow = pending.find((t) => t.start.getTime() <= now && t.end.getTime() >= now);
      if (inWindow) return inWindow;
      const started = pending.filter((t) => t.status === "en-curso").sort((a, b) => a.start.getTime() - b.start.getTime())[0];
      return started ?? null;
    })(),
  );

  // SIGUIENTE: primera actividad que empieza en el futuro.
  const next = $derived(
    (() => {
      const candidates = pending.filter((t) => t.start.getTime() > now && t.id !== current?.id);
      const first = candidates[0];
      return first ?? null;
    })(),
  );

  // IMPORTANTE: vencimiento cercano — prioridad alta o todo-el-día o que
  // termina pronto; se elige el más próximo, sin repetir NOW/NEXT.
  const important = $derived(
    (() => {
      const used = new Set([current?.id, next?.id].filter((x) => x != null));
      const future = pending.filter((t) => !used.has(t.id) && t.end.getTime() >= now);
      const prioritized = future.filter((t) => t.priority === "alta" || t.allDay || t.end.getTime() - now < 48 * 3_600_000);
      const pool = prioritized.length > 0 ? prioritized : future;
      return pool.sort((a, b) => a.end.getTime() - b.end.getTime())[0] ?? null;
    })(),
  );

  // PENDIENTE RELEVANTE: aparece cuando no hay actividad en curso — la
  // primera tarea por hacer (atrasada primero).
  const relevant = $derived(
    (() => {
      if (current) return null;
      const used = new Set([next?.id, important?.id].filter((x) => x != null));
      const overdue = pending.filter((t) => !used.has(t.id) && t.end.getTime() < now && !t.allDay);
      const rest = pending.filter((t) => !used.has(t.id) && !overdue.includes(t));
      return overdue[0] ?? rest[0] ?? null;
    })(),
  );

  const doneToday = $derived(
    tasks.filter((t) => {
      const s = t.start;
      const today = new Date(now);
      return (
        s.getDate() === today.getDate() &&
        s.getMonth() === today.getMonth() &&
        s.getFullYear() === today.getFullYear() &&
        t.status === "completada"
      );
    }).length,
  );

  function remainLabel(t: { end: Date }): string {
    const min = Math.max(0, Math.round((t.end.getTime() - now) / 60_000));
    if (min <= 1) return "termina ya";
    if (min < 60) return `${min} min restantes`;
    if (min < 24 * 60) {
      const h = Math.floor(min / 60);
      const m = min % 60;
      return m === 0 ? `${h} h restantes` : `${h} h ${m} min restantes`;
    }
    const d = Math.floor(min / (24 * 60));
    const h = Math.floor((min % (24 * 60)) / 60);
    return h === 0 ? `${d} d restantes` : `${d} d ${h} h restantes`;
  }

  function timeLabel(t: { allDay: boolean; start: Date }): string {
    if (t.allDay) return "todo el día";
    return t.start.toLocaleTimeString("es-ES", { hour: "2-digit", minute: "2-digit" });
  }

  function dueLabel(d: Date): string {
    const diff = d.getTime() - now;
    const days = Math.floor(diff / 86_400_000);
    if (days <= 0) return "hoy";
    if (days === 1) return "mañana";
    return d.toLocaleDateString("es-ES", { day: "numeric", month: "short" });
  }

  function onAction(e: MouseEvent, id: number, action: "complete" | "postpone" | "start") {
    e.stopPropagation();
    widgetAction(id, action);
  }
</script>

<div class="widget">
  <header class="head" data-tauri-drag-region>
    <span class="brand" data-tauri-drag-region>
      <span class="logo" data-tauri-drag-region>F</span>
      <span data-tauri-drag-region>FocusFlow</span>
    </span>
    <span class="status">
      <span class="pulse"></span>
      hoy · {doneToday} hecha{doneToday === 1 ? "" : "s"}
    </span>
  </header>

  <div class="body">
    {#if current}
      <div class="sec now" transition:fade={{ duration: 140 }}>
        <span class="sec-label">Ahora</span>
        <button class="task" type="button" onclick={() => openTaskRemote(current.id)}>
          <span class="dot" style="--c: {cat(current.categoryId).color}"></span>
          <span class="ttl">{current.title}</span>
          <span class="remaining">{remainLabel(current)}</span>
        </button>
        <div class="qa">
          <button class="qa-btn" onclick={(e) => onAction(e, current.id, "complete")} title="Completar">✓</button>
          <button class="qa-btn" onclick={(e) => onAction(e, current.id, "postpone")} title="Posponer 1 hora">⟳</button>
          {#if current.status !== "en-curso"}
            <button class="qa-btn" onclick={(e) => onAction(e, current.id, "start")} title="Empezar ahora">▶</button>
          {/if}
        </div>
      </div>
    {:else if relevant}
      <div class="sec" transition:fade={{ duration: 140 }}>
        <span class="sec-label">Por hacer</span>
        <button class="task" type="button" onclick={() => openTaskRemote(relevant.id)}>
          <span class="dot" style="--c: {cat(relevant.categoryId).color}"></span>
          <span class="ttl">{relevant.title}</span>
          <span class="due">{relevant.allDay ? "hoy" : dueLabel(relevant.start)}</span>
        </button>
        <div class="qa">
          <button class="qa-btn" onclick={(e) => onAction(e, relevant.id, "complete")} title="Completar">✓</button>
          <button class="qa-btn" onclick={(e) => onAction(e, relevant.id, "start")} title="Empezar ahora">▶</button>
        </div>
      </div>
    {/if}

    {#if next}
      <div class="sec" transition:fade={{ duration: 140 }}>
        <span class="sec-label">Siguiente</span>
        <button class="task" type="button" onclick={() => openTaskRemote(next.id)}>
          <span class="dot" style="--c: {cat(next.categoryId).color}"></span>
          <span class="ttl">{next.title}</span>
          <span class="due">{timeLabel(next)}</span>
        </button>
        <div class="qa">
          <button class="qa-btn" onclick={(e) => onAction(e, next.id, "complete")} title="Completar">✓</button>
          <button class="qa-btn" onclick={(e) => onAction(e, next.id, "postpone")} title="Posponer 1 hora">⟳</button>
        </div>
      </div>
    {/if}

    {#if important}
      <div class="sec" transition:fade={{ duration: 140 }}>
        <span class="sec-label">Importante</span>
        <button class="task" type="button" onclick={() => openTaskRemote(important.id)}>
          <span class="dot imp" style="--c: {cat(important.categoryId).color}"></span>
          <span class="ttl">{important.title}</span>
          <span class="due imp">{important.allDay ? "todo el día" : dueLabel(important.end)}</span>
        </button>
        <div class="qa">
          <button class="qa-btn" onclick={(e) => onAction(e, important.id, "complete")} title="Completar">✓</button>
          <button class="qa-btn" onclick={(e) => onAction(e, important.id, "postpone")} title="Posponer 1 hora">⟳</button>
        </div>
      </div>
    {/if}

    {#if !current && !next && !important && !relevant}
      <div class="empty" transition:fade={{ duration: 140 }}>Todo claro por ahora</div>
    {/if}
  </div>

  <footer class="foot">
    <button class="foot-btn" onclick={() => openApp()} title="Abrir FocusFlow">
      ⤢ <span>Abrir</span>
    </button>
    <button class="foot-btn" onclick={() => askAssistant()} title="Preguntar a FocusFlow">
      <span>Preguntar</span>
    </button>
  </footer>
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
    gap: 6px;
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
    gap: 6px;
    max-height: 320px;
    overflow-y: auto;
    overscroll-behavior: contain;
    scrollbar-width: thin;
  }
  .sec {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }
  .sec-label {
    font-size: 9.5px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.1em;
    color: var(--text-3);
  }
  .sec.now .sec-label {
    color: var(--primary);
  }
  .task {
    display: flex;
    align-items: center;
    gap: 8px;
    border: none;
    background: transparent;
    border-radius: var(--r-sm);
    padding: 5px 8px;
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
  .dot.imp {
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--c) 20%, transparent);
  }
  .ttl {
    flex: 1;
    min-width: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .remaining {
    font-size: 10.5px;
    font-weight: 700;
    color: var(--primary);
    background: var(--primary-soft);
    border-radius: var(--r-full);
    padding: 2px 9px;
    flex-shrink: 0;
    white-space: nowrap;
    font-variant-numeric: tabular-nums;
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
    font-variant-numeric: tabular-nums;
  }
  .due.imp {
    color: var(--danger);
    background: color-mix(in srgb, var(--danger) 12%, var(--surface));
  }
  .qa {
    display: flex;
    gap: 6px;
    padding-left: 8px;
  }
  .qa-btn {
    width: 22px;
    height: 20px;
    border: 1px solid var(--border);
    background: var(--surface-3);
    color: var(--text-2);
    border-radius: var(--r-full);
    font-size: 11px;
    line-height: 1;
    cursor: pointer;
    transition: all var(--dur-fast) var(--ease-out);
    opacity: 0;
  }
  .sec:hover .qa-btn,
  .qa-btn:focus-visible {
    opacity: 1;
  }
  .qa-btn:hover {
    background: var(--primary);
    border-color: var(--primary);
    color: #fff;
  }
  .qa-btn[title="Completar"]:hover {
    background: var(--success);
    border-color: var(--success);
  }
  .empty {
    font-size: 12.5px;
    color: var(--text-3);
    padding: 10px 8px;
    text-align: center;
  }
  .foot {
    display: flex;
    gap: 6px;
    justify-content: flex-end;
    border-top: 1px solid var(--border);
    padding-top: 6px;
  }
  .foot-btn {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    border: none;
    background: transparent;
    color: var(--text-3);
    font-size: 11px;
    font-weight: 600;
    cursor: pointer;
    padding: 3px 8px;
    border-radius: var(--r-full);
    transition: all var(--dur-fast) var(--ease-out);
  }
  .foot-btn:hover {
    color: var(--primary);
    background: var(--primary-soft);
  }
</style>
