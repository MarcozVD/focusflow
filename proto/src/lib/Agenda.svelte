<script lang="ts">
  import { tasks } from "./data.svelte";
  import TaskCard from "./TaskCard.svelte";

  const groups = $derived(() => {
    const now = new Date();
    const byDay = new Map<string, typeof tasks>();
    const sorted = [...tasks].sort((a, b) => a.start.getTime() - b.start.getTime());
    for (const t of sorted) {
      if (t.status === "completada") continue;
      const key = t.start.toLocaleDateString("es-ES", { weekday: "long", day: "numeric", month: "short" });
      if (!byDay.has(key)) byDay.set(key, []);
      byDay.get(key)!.push(t);
    }
    return Array.from(byDay.entries()).slice(0, 5);
  });
</script>

<div class="agenda">
  {#each groups() as [day, list]}
    <div class="group">
      <div class="day-label">{day}</div>
      {#each list as t}
        <TaskCard task={t} />
      {/each}
    </div>
  {/each}
</div>

<style>
  .agenda {
    display: flex;
    flex-direction: column;
    gap: var(--s-6);
    padding: var(--s-2) var(--s-8) var(--s-8);
    max-width: 720px;
    margin: 0 auto;
  }
  .group {
    display: flex;
    flex-direction: column;
    gap: var(--s-3);
  }
  .day-label {
    font-size: 11px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.1em;
    color: var(--text-3);
    margin-left: var(--s-2);
    text-transform: capitalize;
  }
</style>
