<script lang="ts">
  import { tasks as tasksStore } from "./data.svelte";
  import TaskCard from "./TaskCard.svelte";
  import { groupAgenda } from "./taskDayLogic";

  const tasks = $derived(tasksStore());

  /** Primeros 5 días con presencia a partir de hoy (hoy primero, luego cronológico).
   *  Los multi-día aparecen en cada día que cubren; las vencidas de hoy, hoy. */
  const groups = $derived(groupAgenda(tasks, new Date()).slice(0, 5));
</script>

<div class="agenda">
  {#each groups as g}
    <div class="group">
      <div class="day-label">
        {new Date(g.dayMs).toLocaleDateString("es-ES", { weekday: "long", day: "numeric", month: "short" })}
      </div>
      {#each g.tasks as t}
        <TaskCard task={t} />
      {/each}
    </div>
  {/each}
</div>

<style>
.agenda {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: var(--s-6);
    padding: var(--s-2) var(--s-8) var(--s-8);
    max-width: 760px;
    margin: 0 auto;
    width: 100%;
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
