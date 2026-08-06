<script lang="ts">
  import { tasks, completeTask, cat } from "./data.svelte";

  let { compact = false }: { compact?: boolean } = $props();

  const nextDue = $derived(
    [...tasks]
      .filter((t) => t.status !== "completada")
      .sort((a, b) => a.start.getTime() - b.start.getTime())[0],
  );

  const upcoming = $derived(
    [...tasks]
      .filter((t) => t.status !== "completada")
      .sort((a, b) => a.start.getTime() - b.start.getTime())
      .slice(0, compact ? 3 : 8),
  );

  const todayTasks = $derived(
    tasks.filter((t) => {
      const s = t.start;
      const n = new Date();
      return (
        s.getDate() === n.getDate() && s.getMonth() === n.getMonth() && t.status !== "completada"
      );
    }),
  );

  function countdown(t: Date): string {
    const diff = t.getTime() - Date.now();
    const d = Math.floor(diff / 86400000);
    const h = Math.floor((diff % 86400000) / 3600000);
    return `${d}d ${h}h`;
  }

  function dueLabel(t: Date): string {
    const diff = t.getTime() - Date.now();
    const d = Math.floor(diff / 86400000);
    if (d <= 0) return "hoy";
    if (d === 1) return "mañana";
    return t.toLocaleDateString("es-ES", { day: "numeric", month: "short" });
  }
</script>

<div class="widget" class:compact>
  <div class="head">
    <span class="brand"><span class="logo">F</span>FocusFlow</span>
    <span class="mode">{compact ? "compacto" : "expandido"}</span>
  </div>

  {#if nextDue}
    <div class="countdown">
      <span class="label">Próxima entrega</span>
      <span class="num">{countdown(nextDue.start)}</span>
      <span class="title">{nextDue.title}</span>
    </div>
  {/if}

  {#if todayTasks.length > 0}
    <div class="sec-label">Hoy</div>
    {#each todayTasks as t}
      <button class="task" onclick={() => completeTask(t.id)}>
        <span class="dot" style="--c: {cat(t.categoryId).color}"></span>
        <span class="ttl">{t.title}</span>
        <span class="due">{dueLabel(t.start)}</span>
      </button>
    {/each}
  {/if}

  {#if !compact}
    <div class="sec-label">Próximos</div>
    {#each upcoming.filter((t) => !todayTasks.includes(t)).slice(0, 5) as t}
      <button class="task" onclick={() => completeTask(t.id)}>
        <span class="dot" style="--c: {cat(t.categoryId).color}"></span>
        <span class="ttl">{t.title}</span>
        <span class="due">{dueLabel(t.start)}</span>
      </button>
    {/each}
  {/if}
</div>

<style>
  .widget {
    background: color-mix(in srgb, var(--surface) 94%, transparent);
    border: 1px solid rgba(255, 255, 255, 0.6);
    border-radius: var(--r-xl);
    box-shadow: var(--shadow-raised-lg);
    padding: var(--s-5);
    display: flex;
    flex-direction: column;
    gap: 6px;
    width: 320px;
    font-size: 13px;
  }
  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    margin-bottom: 4px;
  }
  .brand {
    display: flex;
    align-items: center;
    gap: 8px;
    font-weight: 700;
    font-size: 14px;
  }
  .logo {
    width: 22px;
    height: 22px;
    border-radius: 8px;
    background: var(--primary);
    color: #fff;
    font-size: 12px;
    font-weight: 700;
    display: inline-flex;
    align-items: center;
    justify-content: center;
  }
  .mode {
    font-size: 10px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--text-3);
  }
  .countdown {
    background: var(--surface-2);
    border-radius: var(--r-md);
    padding: var(--s-3) var(--s-4);
    margin: 2px 0 var(--s-2);
    display: flex;
    flex-direction: column;
  }
  .label {
    font-size: 10px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.1em;
    color: var(--text-3);
  }
  .num {
    font-size: 34px;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
    line-height: 1.1;
    color: var(--text-1);
  }
  .title {
    font-size: 12px;
    color: var(--text-2);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .sec-label {
    font-size: 10px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.1em;
    color: var(--text-3);
    margin-top: 4px;
  }
  .task {
    display: flex;
    align-items: center;
    gap: 8px;
    border: none;
    background: transparent;
    border-radius: var(--r-sm);
    padding: 7px 8px;
    font-size: 13px;
    font-family: inherit;
    color: var(--text-1);
    text-align: left;
    transition: background var(--dur-fast) var(--ease-out);
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
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .due {
    font-size: 11px;
    font-weight: 600;
    color: var(--warning);
    background: var(--warning-bg);
    border-radius: var(--r-full);
    padding: 1px 9px;
    flex-shrink: 0;
  }
</style>
