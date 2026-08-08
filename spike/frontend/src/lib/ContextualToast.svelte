<script lang="ts">
  import { contextualNotif, closeContextualNotif, notifRespond } from "./data.svelte.ts";

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
    border-radius: var(--r-xl);
    padding: 16px;
    box-shadow: var(--shadow-raised-lg);
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
    margin-bottom: 8px;
  }
  .badge {
    font-size: 11px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--primary);
    background: var(--primary-soft);
    padding: 3px 8px;
    border-radius: var(--r-full);
  }
  .x {
    border: none;
    background: none;
    color: var(--text-3);
    font-size: 16px;
    cursor: pointer;
    padding: 2px 6px;
    border-radius: var(--r-sm);
  }
  .x:hover {
    background: var(--surface-3);
    color: var(--text-1);
  }
  .body {
    margin: 0 0 14px;
    font-size: 14px;
    line-height: 1.5;
    color: var(--text-1);
  }
  .actions {
    display: flex;
    gap: 8px;
  }
  .btn {
    padding: 7px 12px;
    border-radius: var(--r-sm);
    border: 1px solid var(--border);
    background: var(--surface);
    color: var(--text-1);
    font-size: 13px;
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
