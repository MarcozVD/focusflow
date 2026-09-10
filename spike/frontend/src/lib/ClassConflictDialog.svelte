<script lang="ts">
  // Diálogo de conflicto con el horario. Sirve para TAREAS (regla 22: Editar
  // mueve la tarea al siguiente hueco libre) y para SESIONES DE ESTUDIO
  // (regla 10: Editar horario / Cancelar / Continuar de todas formas, sin
  // mover nada automáticamente). El copy depende de `req.kind`.
  import { classConflictRequest, resolveClassConflict } from "./data.svelte";
  import { formatMinutes } from "./classLogic";

  const req = $derived(classConflictRequest());
  const isStudy = $derived(req?.kind === "study");

  function timeOf(ms: number) {
    const d = new Date(ms);
    return d.toLocaleDateString("es-ES", { weekday: "short", day: "numeric" }) +
      " " + d.toLocaleTimeString("es-ES", { hour: "2-digit", minute: "2-digit" });
  }
  function onKey(e: KeyboardEvent) {
    if (!req) return;
    if (e.key === "Escape") { e.preventDefault(); resolveClassConflict("cancelar"); }
  }
</script>

<svelte:window onkeydown={onKey} />

{#if req}
  <div class="overlay" role="presentation" onclick={(e) => { if (e.target === e.currentTarget) resolveClassConflict("cancelar"); }}>
    <div class="modal" role="alertdialog" aria-modal="true" aria-labelledby="cc-title">
      <header>
        <span class="badge" aria-hidden="true">⚠</span>
        <h3 id="cc-title">{isStudy ? "Esta sesión coincide con una clase" : "Conflicto con tu horario"}</h3>
      </header>
      <p class="lead">
        «{req.taskTitle}» en {timeOf(req.startAt)}–{timeOf(req.endAt)} coincide
        con {req.conflicts.length === 1 ? "una clase" : "varias clases"} de tu horario:
      </p>
      <ul class="cl">
        {#each req.conflicts as c (c.class_id + "-" + c.date_ms)}
          <li>
            <strong>{c.title}</strong>
            <span class="tm"
              >{formatMinutes((c.start_at - c.date_ms) / 60_000)} – {formatMinutes((c.end_at - c.date_ms) / 60_000)}</span
            >
          </li>
        {/each}
      </ul>
      <footer>
        <button type="button" class="btn primary" onclick={() => resolveClassConflict("editar")}>
          {isStudy ? "Editar horario" : "Editar"}
        </button>
        <button type="button" class="btn" onclick={() => resolveClassConflict("continuar")}>
          {isStudy ? "Continuar de todas formas" : "Continuar"}
        </button>
        <button type="button" class="btn ghost" onclick={() => resolveClassConflict("cancelar")}>
          Cancelar
        </button>
      </footer>
      <p class="hint">
        {isStudy
          ? "Editar horario cierra este aviso para que corrijas la hora a mano. Continuar deja la sesión superpuesta a la clase: no se mueve nada automáticamente."
          : "Editar mueve la tarea al siguiente hueco libre. Continuar la deja encima de la clase."}
      </p>
    </div>
  </div>
{/if}

<style>
  .overlay {
    position: fixed; inset: 0; z-index: 70;
    background: rgba(15, 23, 42, 0.42);
    display: grid; place-items: center; padding: var(--s-4);
  }
  .modal {
    width: min(400px, 100%);
    background: var(--surface);
    border-radius: var(--r-lg);
    box-shadow: var(--shadow-raised-lg);
    padding: var(--s-5);
    display: flex; flex-direction: column; gap: var(--s-2);
  }
  header { display: flex; align-items: center; gap: var(--s-2); }
  .badge {
    width: 30px; height: 30px; border-radius: 999px;
    background: var(--danger-bg); color: var(--danger);
    display: grid; place-items: center; font-size: 15px; font-weight: 700;
  }
  h3 { margin: 0; font-size: 15.5px; font-weight: 700; }
  .lead { margin: 0; font-size: 13px; color: var(--text-2); }
  .cl { margin: 0; padding: 0; list-style: none; display: flex; flex-direction: column; gap: 5px; }
  .cl li {
    display: flex; justify-content: space-between; gap: var(--s-3);
    font-size: 13px;
    background: var(--primary-soft);
    border-left: 3px solid var(--primary);
    border-radius: var(--r-md); padding: 6px 10px;
  }
  .cl .tm { font-variant-numeric: tabular-nums; color: var(--text-3); font-size: 12px; }
  footer { display: flex; gap: var(--s-2); margin-top: var(--s-2); }
  .btn {
    flex: 1; font: inherit; font-size: 13px; font-weight: 600; cursor: pointer;
    border: 1px solid var(--border); background: var(--surface); color: var(--text-2);
    border-radius: var(--r-md); padding: 8px 10px;
  }
  .btn:hover { background: var(--surface-2); }
  .btn.primary { background: var(--primary); border-color: var(--primary); color: #fff; }
  .btn.primary:hover { background: var(--primary-hover); }
  .btn.ghost { border-color: transparent; background: none; color: var(--text-3); }
  .hint { margin: 0; font-size: 11.5px; color: var(--text-3); }
</style>
