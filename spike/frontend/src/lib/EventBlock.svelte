<script lang="ts">
  // Bloque de evento del calendario día/semana. Sirve para TAREAS y, mediante
  // la prop `study`, para SESIONES DE ESTUDIO: mismas interacciones (mover,
  // redimensionar, click), color propio (--study) y rótulo distinto.
  import { cat, openTaskDetail, type Task } from "./data.svelte";
  import type { StudySession } from "./studyLogic";
  import TaskCheck from "./TaskCheck.svelte";

  interface Seg {
    start: Date;
    end: Date;
    kind: "full" | "inicio" | "fin";
  }

  let {
    task,
    seg,
    top,
    height,
    left,
    width,
    conflict = false,
    study = null,
    onPointerDown,
    onClick,
  }: {
    task: Task;
    seg: Seg;
    top: number;
    height: number;
    left: number;
    width: number;
    /** La tarea coincide con una clase del horario (regla 8). */
    conflict?: boolean;
    /** Si viene, el bloque representa una SESIÓN DE ESTUDIO (no una tarea). */
    study?: StudySession | null;
    onPointerDown?: (t: Task, mode: "move" | "resize-start" | "resize-end", e: PointerEvent) => void;
    onClick?: (t: Task) => void;
  } = $props();

  const c = $derived(study ? "var(--study)" : cat(task.categoryId).color);
  const compact = $derived(height < 36);
  const tall = $derived(height >= 62);
  const isStudy = $derived(study != null);
  const done = $derived(!isStudy && task.status === "completada");

  function fmt(d: Date): string {
    return `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
  }

  const label = $derived(
    isStudy
      ? study!.title
      : seg.kind === "inicio"
        ? `Inicio · ${task.title}`
        : seg.kind === "fin"
          ? `Fin · ${task.title}`
          : task.title,
  );

  const tooltip = $derived(
    isStudy
      ? `Sesión de estudio: ${study!.title} · ${fmt(seg.start)} – ${fmt(seg.end)}${study!.notes ? "\n" + study!.notes : ""}${conflict ? "\n⚠ Coincide con una clase de tu horario" : ""}`
      : `${task.title} · ${fmt(seg.start)} – ${fmt(seg.end)}${task.description ? "\n" + task.description : ""}${conflict ? "\n⚠ Coincide con una clase de tu horario" : ""}${task.priority === "alta" ? "\nPrioridad alta" : ""}${task.status === "vencida" ? "\nVencida" : ""}`,
  );

  function onMove(e: PointerEvent) {
    // Completadas: sin arrastre (el click sigue abriendo el drawer)
    if (done) return;
    onPointerDown?.(task, "move", e);
  }
  function onResizeStart(e: PointerEvent) {
    onPointerDown?.(task, "resize-start", e);
  }
  function onResizeEnd(e: PointerEvent) {
    onPointerDown?.(task, "resize-end", e);
  }
</script>

<div
  class="evt {seg.kind} {task.status === 'vencida' ? 'overdue' : ''} {compact ? 'compact' : ''} {done ? 'done' : ''} {conflict ? 'class-conflict' : ''} {isStudy ? 'study' : ''}"
  style="top: {top}px; height: {height}px; left: {left}%; width: {width}%; --c: {c}"
  title={tooltip}
  role="button"
  tabindex="0"
  onpointerdown={onMove}
  onclick={() => (onClick ? onClick(task) : openTaskDetail(task))}
  onkeydown={(e) => {
    if (e.key === "Enter" || e.key === " ") {
      e.preventDefault();
      onClick ? onClick(task) : openTaskDetail(task);
    }
  }}
>
  {#if !compact}
    <span class="evt-time">
      {#if isStudy}
        <span class="study-tag" aria-hidden="true">
          <svg width="9" height="9" viewBox="0 0 24 24" fill="none"><path d="M4 5.5A2.5 2.5 0 0 1 6.5 3H19V19H6.5A2.5 2.5 0 0 0 4 21.5V5.5Z" stroke="currentColor" stroke-width="2.5" stroke-linejoin="round"/></svg>
        </span>
      {/if}
      {fmt(seg.start)}{tall ? ` – ${fmt(seg.end)}` : ""}
      {#if !isStudy && task.priority === "alta"}
        <span class="prio-dot" title="Prioridad alta"></span>
      {/if}
    </span>
    <span class="evt-title"><span class="strike">{label}</span></span>
    {#if tall && !isStudy && task.description}
      <span class="evt-desc">{task.description}</span>
    {/if}
    {#if tall && isStudy && study!.notes}
      <span class="evt-desc">{study!.notes}</span>
    {/if}
    {#if !tall && !isStudy && task.priority === "alta"}
      <span class="evt-title" aria-hidden="true">
        <span class="prio-bar"></span>
      </span>
    {/if}
  {:else}
    <span class="evt-inline">
      {#if isStudy}
        <span class="study-tag" aria-hidden="true">
          <svg width="9" height="9" viewBox="0 0 24 24" fill="none"><path d="M4 5.5A2.5 2.5 0 0 1 6.5 3H19V19H6.5A2.5 2.5 0 0 0 4 21.5V5.5Z" stroke="currentColor" stroke-width="2.5" stroke-linejoin="round"/></svg>
        </span>
      {/if}
      {#if done}
        <span class="done-check" aria-hidden="true">
          <svg width="9" height="9" viewBox="0 0 24 24" fill="none"><path d="M4 12.5l5.5 5.5L20 6.5" stroke="currentColor" stroke-width="3" stroke-linecap="round" stroke-linejoin="round"/></svg>
        </span>
      {/if}
      <span class="evt-time-mini">{fmt(seg.start)}</span>
      <span class="evt-title"><span class="strike">{label}</span></span>
    </span>
  {/if}
  {#if !compact && !isStudy}
    <span class="check-slot"><TaskCheck {task} /></span>
  {/if}
  {#if !done}
    <span
      class="resize top"
      role="separator"
      aria-orientation="vertical"
      aria-label="Arrastrar para cambiar el inicio"
      onpointerdown={(e) => { e.stopPropagation(); onResizeStart(e); }}
      title="Arrastrar para cambiar inicio"
    ></span>
    <span
      class="resize bottom"
      role="separator"
      aria-orientation="vertical"
      aria-label="Arrastrar para cambiar el fin"
      onpointerdown={(e) => { e.stopPropagation(); onResizeEnd(e); }}
      title="Arrastrar para cambiar fin"
    ></span>
  {/if}
</div>

<style>
  .evt {
    position: absolute;
    background: color-mix(in srgb, var(--c) 13%, var(--surface));
    border-left: 3px solid var(--c);
    border-radius: var(--r-sm);
    padding: var(--s-1) var(--s-2);
    display: flex;
    flex-direction: column;
    gap: var(--s-0_5);
    overflow: hidden;
    z-index: 1;
    box-shadow: var(--shadow-inset-sm);
    transition:
      transform var(--dur-fast) var(--ease-out),
      box-shadow var(--dur-fast) var(--ease-out),
      background var(--dur-slow) var(--ease-out),
      border-color var(--dur-slow) var(--ease-out),
      color var(--dur-slow) var(--ease-out);
    min-width: 0;
    cursor: pointer;
    touch-action: none;
    user-select: none;
  }
  .evt:hover {
    transform: translateY(-1px) scale(1.01);
    box-shadow: var(--e1);
    z-index: 3;
  }
  .evt:active {
    cursor: grabbing;
  }
  .evt.overdue {
    border-left-style: dashed;
    opacity: 0.8;
  }
  /* Regla 8: la tarea coincide con una clase (el usuario confirmó) */
  .evt.class-conflict {
    outline: 1.5px solid var(--warning);
    outline-offset: -1.5px;
  }
  .evt.inicio,
  .evt.fin {
    background: color-mix(in srgb, var(--c) 18%, var(--surface));
  }
  /* Completada: hundida gris, sin hover-lift, por debajo de las pendientes.
     Va DESPUÉS de .inicio/.fin (misma especificidad): el gris debe ganar. */
  .evt.done {
    background: var(--surface-2);
    border-left-color: var(--text-3);
    box-shadow: var(--shadow-inset);
    z-index: 0;
    cursor: pointer;
  }
  .evt.done:hover {
    transform: none;
    box-shadow: var(--shadow-inset);
    z-index: 0;
  }
  .evt.done:active {
    cursor: pointer;
  }
  .evt.done.class-conflict {
    outline: none;
  }
  /* Título = contenido esencial: --text-2 (≈4.2:1 sobre --surface-2).
     --text-3 solo para meta (ACCESSIBILITY.md §5). */
  .evt.done .evt-title {
    color: var(--text-2);
  }
  .evt.done .evt-time,
  .evt.done .evt-time-mini,
  .evt.done .evt-desc {
    color: var(--text-3);
  }
  .evt.done .prio-dot,
  .evt.done .prio-bar {
    display: none;
  }
  /* Tachado animado izq→der al completar; al reabrir se deshace */
  .strike {
    background: linear-gradient(currentColor, currentColor) no-repeat 0 55% / 0% 1.5px;
    transition: background-size var(--dur-slow) var(--ease-out);
  }
  .evt.done .strike {
    background-size: 100% 1.5px;
  }
  /* Sesión de estudio: rótulo mini con icono de libro para distinguirla */
  .study-tag {
    display: inline-grid;
    place-items: center;
    width: 13px;
    height: 13px;
    border-radius: var(--r-xs);
    background: color-mix(in srgb, var(--c) 20%, transparent);
    color: var(--c);
    flex-shrink: 0;
  }
  .evt-time {
    font-size: var(--fs-2xs);
    font-weight: 600;
    color: var(--text-2);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
    display: inline-flex;
    align-items: center;
    gap: var(--s-1);
    transition: color var(--dur-slow) var(--ease-out);
  }
  .evt-title {
    font-size: var(--fs-xs);
    font-weight: 600;
    color: var(--text-1);
    line-height: 1.25;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    word-break: break-word;
    min-width: 0;
    transition: color var(--dur-slow) var(--ease-out);
  }
  .evt-desc {
    font-size: var(--fs-2xs);
    color: var(--text-3);
    line-height: 1.2;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    word-break: break-word;
  }
  .prio-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: var(--danger);
    flex-shrink: 0;
  }
  .prio-bar {
    display: inline-block;
    width: 100%;
    height: 2px;
    border-radius: var(--r-full);
    background: var(--danger);
  }
  .evt-inline {
    display: flex;
    align-items: center;
    gap: var(--s-1);
    min-width: 0;
    height: 100%;
  }
  /* Compacto: el título va a UNA línea (el ✓ y la hora restan ancho) */
  .evt-inline .evt-title {
    -webkit-line-clamp: 1;
    line-clamp: 1;
  }
  .evt-time-mini {
    font-size: var(--fs-2xs);
    font-weight: 600;
    color: var(--text-2);
    font-variant-numeric: tabular-nums;
    flex-shrink: 0;
    transition: color var(--dur-slow) var(--ease-out);
  }
  .done-check {
    display: inline-grid;
    place-items: center;
    color: var(--text-3);
    flex-shrink: 0;
  }
  /* Check rápido: solo en bloques no compactos de tareas (oculto hasta
     hover/focus; siempre visible en completadas) */
  .check-slot {
    position: absolute;
    top: 4px;
    right: 4px;
    z-index: 3;
    opacity: 0;
    transition: opacity var(--dur-fast) var(--ease-out);
  }
  .evt:hover .check-slot,
  .evt:focus-within .check-slot {
    opacity: 1;
  }
  .evt.done .check-slot {
    opacity: 1;
  }
  .resize {
    position: absolute;
    left: 4px;
    right: 4px;
    height: 10px;
    cursor: ns-resize;
    opacity: 0;
    transition: opacity var(--dur-fast) var(--ease-out);
    z-index: 2;
    touch-action: none;
  }
  .resize.top {
    top: 0;
  }
  .resize.bottom {
    bottom: 0;
  }
  .evt:hover .resize {
    opacity: 1;
  }
  .resize:hover {
    background: color-mix(in srgb, var(--c) 12%, transparent);
  }
  .resize::after {
    content: "";
    position: absolute;
    left: 50%;
    transform: translateX(-50%);
    width: 28px;
    height: 3px;
    border-radius: var(--r-full);
    background: color-mix(in srgb, var(--c) 70%, var(--text-1));
    box-shadow: var(--shadow-inset-sm);
  }
  .resize.top::after {
    top: 2px;
  }
  .resize.bottom::after {
    bottom: 2px;
  }
</style>
