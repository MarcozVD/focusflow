<script lang="ts">
  import { contextualNotif, closeContextualNotif, notifRespond } from "./data.svelte";

  let { onplan }: { onplan: (n: NonNullable<ReturnType<typeof contextualNotif>>) => void } = $props();

  const KIND_LABEL: Record<string, string> = {
    deadline: "Vence pronto",
    missed: "Tarea atrasada",
    conflict: "Conflicto de horario",
    free_time: "Tiempo disponible",
    important: "Compromiso importante",
    reschedule: "Sugerencia",
  };

  function plan() {
    const n = contextualNotif();
    if (!n) return;
    onplan(n);
    notifRespond(n.log_id, "planned");
    closeContextualNotif();
  }

  function later() {
    const n = contextualNotif();
    if (!n) return;
    notifRespond(n.log_id, "later");
    closeContextualNotif();
  }

  function dismiss() {
    const n = contextualNotif();
    if (!n) return;
    notifRespond(n.log_id, "dismissed");
    closeContextualNotif();
  }
</script>

{#if contextualNotif()}
  <div class="toast" role="alert">
    <div class="head">
      <span class="badge">{KIND_LABEL[contextualNotif()!.kind] ?? contextualNotif()!.kind}</span>
      <button class="x" onclick={dismiss} title="Descartar">×</button>
    </div>
    <p class="body">{contextualNotif()!.body}</p>
    <div class="actions">
      <button class="btn primary" onclick={plan}>Plan</button>
      <button class="btn" onclick={later}>Más tarde</button>
      <button class="btn ghost" onclick={dismiss}>Descartar</button>
    </div>
  </div>
{/if}

<style>
  .toast {
    position: fixed;
    right: 20px;
    bottom: 20px;
    width: 340px;
    z-index: 60;
    background: var(--surface-2);
    border: 1px solid var(--border);
    border-radius: var(--r-card);
    padding: var(--s-4);
    box-shadow: var(--e2);
    animation: rise 0.25s var(--ease-out);
  }
  @keyframes rise {
    from {
      opacity: 0;
      transform: translateY(12px);
    }
    to {
      opacity: 1;
      transform: translateY(0);
    }
  }
  .head {
    display: flex;
    justify-content: space-between;
    align-items: center;
    margin-bottom: var(--s-2);
  }
  .badge {
    font-size: var(--fs-xs);
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--primary);
    background: var(--primary-soft);
    padding: var(--s-1) var(--s-2);
    border-radius: var(--r-chip);
  }
  .x {
    border: none;
    background: none;
    color: var(--text-3);
    font-size: var(--fs-lg);
    cursor: pointer;
    padding: var(--s-0_5) var(--s-1_5);
    border-radius: var(--r-icon);
  }
  .x:hover {
    background: var(--surface-3);
    color: var(--text-1);
  }
  .body {
    margin: 0 0 var(--s-3);
    font-size: var(--fs-md);
    line-height: 1.5;
    color: var(--text-1);
  }
  .actions {
    display: flex;
    gap: var(--s-2);
  }
  .btn {
    padding: var(--s-2) var(--s-3);
    border-radius: var(--r-control);
    border: 1px solid var(--border);
    background: var(--surface);
    color: var(--text-1);
    font-size: var(--fs-base);
    cursor: pointer;
  }
  .btn.primary {
    background: var(--primary);
    border-color: var(--primary);
    color: #fff;
  }
  .btn.ghost {
    border-color: transparent;
    background: none;
    color: var(--text-3);
  }
</style>
