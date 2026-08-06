<script lang="ts">
  import { MONTHS_ES, DAYS_ES, tasks, completeTask, cat, type Task } from "./data.svelte";

  let { task }: { task: Task } = $props();
  let justCompleted = $state(false);

  const c = $derived(cat(task.categoryId));
  const chipStyle = $derived({
    background: `color-mix(in srgb, ${c.color} 13%, var(--surface))`,
    color: `color-mix(in srgb, ${c.color} 55%, var(--text-1))`,
  });

  function fmt(t: Date): string {
    return `${t.getHours().toString().padStart(2, "0")}:${t.getMinutes().toString().padStart(2, "0")}`;
  }
  const isCompleted = $derived(task.status === "completada");
  const isOverdue = $derived(task.status === "vencida");

  function onComplete() {
    justCompleted = true;
    setTimeout(() => (justCompleted = false), 250);
    completeTask(task.id);
  }
</script>

<div
  class="card {isCompleted ? 'done' : ''} {isOverdue ? 'overdue' : ''} {justCompleted ? 'pop' : ''}"
  style="--accent: {c.color}"
>
  <button class="check" class:checked={isCompleted} onclick={onComplete} aria-label="Completar tarea">
    {#if isCompleted}
      <svg width="11" height="11" viewBox="0 0 12 12" fill="none">
        <path d="M2 6.5L4.5 9L10 3" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" />
      </svg>
    {/if}
  </button>

  <div class="body">
    <div class="title-row">
      <span class="title">{task.title}</span>
      {#if task.priority === "alta"}
        <span class="prio high">Alta</span>
      {:else if task.priority === "media"}
        <span class="prio mid">Media</span>
      {/if}
    </div>
    <div class="meta">
      <span class="time">
        {#if task.allDay}
          Todo el día
        {:else}
          {fmt(task.start)} – {fmt(task.end)}
        {/if}
      </span>
      <span class="chip" style={chipStyle}>{c.name}</span>
      {#if isOverdue}
        <span class="overdue-tag">Vencida</span>
      {/if}
    </div>
  </div>
</div>

<style>
  .card {
    display: flex;
    gap: var(--s-3);
    align-items: flex-start;
    background: var(--surface);
    border-radius: var(--r-md);
    box-shadow: var(--shadow-raised);
    padding: var(--s-3) var(--s-4);
    border-left: 4px solid var(--accent);
    border-left-color: var(--accent);
    transition: transform var(--dur-fast) var(--ease-out), box-shadow var(--dur-fast) var(--ease-out),
      opacity var(--dur-base) var(--ease-out);
  }
  .card:hover {
    transform: translateY(-1px);
    box-shadow: var(--e1);
  }
  .card.done {
    opacity: 0.55;
  }
  .card.done .title {
    text-decoration: line-through;
    color: var(--text-2);
  }
  .card.overdue {
    background: var(--danger-bg);
    border-left-color: var(--danger);
  }
  .card.pop {
    transform: scale(0.98);
  }

  .check {
    width: 22px;
    height: 22px;
    flex-shrink: 0;
    border-radius: var(--r-full);
    border: 2px solid var(--text-3);
    background: transparent;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    margin-top: 1px;
    transition: all var(--dur-base) var(--ease-spring);
    padding: 0;
  }
  .check:hover {
    border-color: var(--success);
    box-shadow: 0 0 0 4px color-mix(in srgb, var(--success) 15%, transparent);
  }
  .check.checked {
    background: var(--success);
    border-color: var(--success);
  }

  .body {
    min-width: 0;
    flex: 1;
  }
  .title-row {
    display: flex;
    align-items: center;
    gap: var(--s-2);
  }
  .title {
    font-weight: 500;
    font-size: 14px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .meta {
    display: flex;
    align-items: center;
    gap: var(--s-2);
    margin-top: 2px;
  }
  .time {
    font-size: 12px;
    color: var(--text-2);
    font-variant-numeric: tabular-nums;
    font-weight: 600;
  }
  .chip {
    font-size: 11px;
    font-weight: 600;
    border-radius: var(--r-full);
    padding: 1px 9px;
  }
  .prio {
    font-size: 10px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    border-radius: var(--r-full);
    padding: 2px 8px;
  }
  .prio.high {
    background: var(--danger-bg);
    color: var(--danger);
  }
  .prio.mid {
    background: var(--primary-soft);
    color: var(--primary);
  }
  .overdue-tag {
    font-size: 10px;
    font-weight: 700;
    color: var(--danger);
    background: color-mix(in srgb, var(--danger) 12%, var(--surface));
    border-radius: var(--r-full);
    padding: 2px 8px;
  }
</style>
