<script lang="ts">
  import { cat, openTaskDetail, type Task } from "./data.svelte";

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
    onPointerDown,
    onClick,
  }: {
    task: Task;
    seg: Seg;
    top: number;
    height: number;
    left: number;
    width: number;
    onPointerDown?: (t: Task, mode: "move" | "resize-start" | "resize-end", e: PointerEvent) => void;
    onClick?: (t: Task) => void;
  } = $props();

  const c = $derived(cat(task.categoryId));
  const compact = $derived(height < 36);
  const tall = $derived(height >= 62);

  function fmt(d: Date): string {
    return `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
  }

  const label = $derived(
    seg.kind === "inicio" ? `Inicio · ${task.title}` : seg.kind === "fin" ? `Fin · ${task.title}` : task.title,
  );

  const tooltip = $derived(
    `${task.title} · ${fmt(seg.start)} – ${fmt(seg.end)}${task.description ? "\n" + task.description : ""}${task.priority === "alta" ? "\nPrioridad alta" : ""}${task.status === "vencida" ? "\nVencida" : ""}`,
  );

  function onMove(e: PointerEvent) {
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
  class="evt {seg.kind} {task.status === 'vencida' ? 'overdue' : ''} {compact ? 'compact' : ''} {task.status === 'completada' ? 'done' : ''}"
  style="top: {top}px; height: {height}px; left: {left}%; width: {width}%; --c: {c.color}"
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
      {fmt(seg.start)}{tall ? ` – ${fmt(seg.end)}` : ""}
      {#if task.priority === "alta"}
        <span class="prio-dot" title="Prioridad alta"></span>
      {/if}
    </span>
    <span class="evt-title">{label}</span>
    {#if tall && task.description}
      <span class="evt-desc">{task.description}</span>
    {/if}
    {#if !tall && task.priority === "alta"}
      <span class="evt-title" aria-hidden="true">
        <span class="prio-bar"></span>
      </span>
    {/if}
  {:else}
    <span class="evt-inline">
      <span class="evt-time-mini">{fmt(seg.start)}</span>
      <span class="evt-title">{label}</span>
    </span>
  {/if}
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
</div>

<style>
  .evt {
    position: absolute;
    background: color-mix(in srgb, var(--c) 13%, var(--surface));
    border-left: 3px solid var(--c);
    border-radius: var(--r-sm);
    padding: 4px 8px;
    display: flex;
    flex-direction: column;
    gap: 2px;
    overflow: hidden;
    z-index: 1;
    box-shadow: var(--shadow-inset-sm);
    transition: transform var(--dur-fast) var(--ease-out), box-shadow var(--dur-fast) var(--ease-out);
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
  .evt.done {
    opacity: 0.5;
  }
  .evt.done .evt-title {
    text-decoration: line-through;
  }
  .evt.inicio,
  .evt.fin {
    background: color-mix(in srgb, var(--c) 18%, var(--surface));
  }
  .evt-time {
    font-size: 10px;
    font-weight: 700;
    color: var(--text-2);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
    display: inline-flex;
    align-items: center;
    gap: 4px;
  }
  .evt-title {
    font-size: 11.5px;
    font-weight: 600;
    color: var(--text-1);
    line-height: 1.25;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    word-break: break-word;
    min-width: 0;
  }
  .evt-desc {
    font-size: 10px;
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
    gap: 5px;
    min-width: 0;
    height: 100%;
  }
  .evt-time-mini {
    font-size: 10px;
    font-weight: 700;
    color: var(--text-2);
    font-variant-numeric: tabular-nums;
    flex-shrink: 0;
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
