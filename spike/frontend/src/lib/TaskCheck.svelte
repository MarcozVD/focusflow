<script lang="ts">
  // Check rápido de completar/reabrir. La visibilidad la controla el PADRE
  // (wrapper .check-slot): aquí solo el botón y sus estados.
  import { completeTask, type Task } from "./data.svelte";

  let { task, size = 16 }: { task: Task; size?: number } = $props();

  const done = $derived(task.status === "completada");
  const label = $derived(done ? "Reabrir tarea" : "Marcar como completada");
</script>

<button
  type="button"
  class="tcheck"
  class:done={done}
  style="width: {size}px; height: {size}px"
  aria-label={label}
  aria-pressed={done}
  title={label}
  onpointerdown={(e) => e.stopPropagation()}
  onkeydown={(e) => e.stopPropagation()}
  onclick={(e) => {
    e.stopPropagation();
    completeTask(task.id);
  }}
>
  <svg
    width={Math.round(size * 0.6)}
    height={Math.round(size * 0.6)}
    viewBox="0 0 24 24"
    fill="none"
    aria-hidden="true"
  >
    <path
      d="M4 12.5l5.5 5.5L20 6.5"
      stroke="currentColor"
      stroke-width="3"
      stroke-linecap="round"
      stroke-linejoin="round"
    />
  </svg>
</button>

<style>
  .tcheck {
    display: inline-grid;
    place-items: center;
    padding: 0;
    border-radius: 50%;
    border: 1.5px solid var(--text-3);
    background: var(--surface);
    box-shadow: var(--shadow-inset-sm);
    color: var(--primary);
    cursor: pointer;
    transition:
      background var(--dur-fast) var(--ease-out),
      border-color var(--dur-fast) var(--ease-out),
      color var(--dur-fast) var(--ease-out),
      transform var(--dur-fast) var(--ease-out);
  }
  .tcheck svg {
    opacity: 0;
    transition: opacity var(--dur-fast) var(--ease-out);
  }
  /* Hover = preview de la acción: borde primario y ✓ tenue */
  .tcheck:hover {
    border-color: var(--primary);
  }
  .tcheck:hover svg {
    opacity: 0.6;
  }
  .tcheck:active {
    transform: scale(0.92);
  }
  .tcheck.done {
    background: var(--text-3);
    border-color: var(--text-3);
    color: var(--surface);
  }
  .tcheck.done svg {
    opacity: 1;
  }
</style>
