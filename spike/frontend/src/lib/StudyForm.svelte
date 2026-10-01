<script lang="ts">
  // Formulario de SESIÓN DE ESTUDIO (regla 6): título, fecha, hora de inicio y
  // de fin obligatorios; tarea relacionada y notas opcionales. La duración se
  // calcula sola a partir de inicio-fin; las validaciones viven en studyLogic
  // (igual que las clases en classLogic).
  import {
    STUDY_COLOR,
    combineDateAndTime,
    fmtDuration,
    plusMinutes,
    validateStudyForm,
    type StudySession,
  } from "./studyLogic";
  import { tasks as tasksStore } from "./data.svelte";

  export interface StudyFormValue {
    title: string;
    start: Date;
    end: Date;
    taskId: number | null;
    notes: string;
  }

  let {
    initial = null,
    prefill = null,
    error = "",
    oncancel,
    onSubmit,
    ondelete,
  }: {
    initial?: StudySession | null;
    prefill?: { start: Date; end: Date } | null;
    error?: string;
    oncancel: () => void;
    onSubmit: (v: StudyFormValue) => void | Promise<void>;
    ondelete?: (id: number) => void | Promise<void>;
  } = $props();

  const tasks = $derived(
    tasksStore().filter((t) => t.status !== "completada").slice(0, 200),
  );

  function dateInput(ms: number) {
    const d = new Date(ms);
    const p = (n: number) => String(n).padStart(2, "0");
    return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`;
  }
  function timeInput(ms: number) {
    const d = new Date(ms);
    const p = (n: number) => String(n).padStart(2, "0");
    return `${p(d.getHours())}:${p(d.getMinutes())}`;
  }

  const baseStart = initial?.start.getTime() ?? prefill?.start.getTime() ?? defaultStartMs();
  function defaultStartMs(): number {
    const n = new Date();
    n.setSeconds(0, 0);
    n.setMinutes(n.getMinutes() < 30 ? 30 - n.getMinutes() : 60 - n.getMinutes());
    n.setHours(n.getHours() + 1, 0, 0, 0);
    return n.getTime() + 60_000;
  }
  const baseEnd = initial?.end.getTime() ?? prefill?.end.getTime() ?? baseStart + 120 * 60_000;

  let title = $state(initial?.title ?? "");
  let dateV = $state(dateInput(baseStart));
  let startV = $state(timeInput(baseStart));
  let endV = $state(timeInput(baseEnd));
  let taskId = $state<number | "">(initial?.taskId ?? "");
  let notes = $state(initial?.notes ?? "");
  let localError = $state("");
  let busy = $state(false);
  let confirmDel = $state(false);

  const shownError = $derived(localError || error);

  // La duración se calcula sola a partir de inicio y fin.
  const startMs = $derived(combineDateAndTime(dateV, startV));
  const endMs = $derived(combineDateAndTime(dateV, endV));
  const duration = $derived(
    Number.isNaN(startMs) || Number.isNaN(endMs) ? NaN : Math.round((endMs - startMs) / 60_000),
  );

  // Al cambiar la hora de inicio, el fin se corre en solido para conservar la
  // duración (si el usuario no lo ha tocado después).
  let lastTouched: "start" | "end" = "start";
  function onStartInput() {
    lastTouched = "start";
    if (!Number.isNaN(startMs)) {
      const keep = duration > 0 ? duration : 120;
      endV = timeInput(plusMinutes(startMs, keep));
    }
  }
  function onEndInput() {
    lastTouched = "end";
  }

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    localError = "";
    const msg = validateStudyForm({ title, startMs, endMs });
    if (msg) {
      localError = msg;
      return;
    }
    busy = true;
    try {
      await onSubmit({
        title: title.trim(),
        start: new Date(startMs),
        end: new Date(endMs),
        taskId: taskId === "" ? null : Number(taskId),
        notes: notes.trim(),
      });
    } finally {
      busy = false;
    }
  }

  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape") {
      if (confirmDel) confirmDel = false;
      else oncancel();
    }
    if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
      (document.getElementById("study-form") as HTMLFormElement | null)?.requestSubmit();
    }
  }
</script>

<svelte:window onkeydown={onKey} />

<div class="overlay" role="presentation" onclick={(e) => { if (e.target === e.currentTarget) oncancel(); }}>
  <div class="modal" role="dialog" aria-modal="true" aria-label={initial ? "Editar sesión de estudio" : "Nueva sesión de estudio"}>
    <header>
      <h3>{initial ? "Editar sesión de estudio" : "Nueva sesión de estudio"}</h3>
      <button class="x" onclick={oncancel} aria-label="Cerrar">✕</button>
    </header>

    <form id="study-form" onsubmit={submit}>
      <label class="row">
        <span class="lab">Título de la sesión *</span>
        <input class="inp" type="text" maxlength="120" placeholder="Ej. Estudiar cálculo" bind:value={title} required />
      </label>

      <div class="grid2 date-time">
        <label class="row">
          <span class="lab">Fecha *</span>
          <input class="inp" type="date" bind:value={dateV} required />
        </label>
        <div class="row">
          <span class="lab">Horario *</span>
          <div class="timerange">
            <input class="inp" type="time" bind:value={startV} required oninput={onStartInput} />
            <span class="to">a</span>
            <input class="inp" type="time" bind:value={endV} required oninput={onEndInput} />
          </div>
        </div>
      </div>

      <p class="hint">
        Duración: <strong data-testid="study-duration">{fmtDuration(duration)}</strong>
        — se calcula sola con la hora de inicio y la de fin.
      </p>

      <div class="grid2">
        <label class="row">
          <span class="lab">Tarea relacionada (opcional)</span>
          <select class="inp" bind:value={taskId}>
            <option value="">— sin tarea —</option>
            {#each tasks as t (t.id)}
              <option value={t.id}>{t.title}</option>
            {/each}
          </select>
        </label>
        <label class="row">
          <span class="lab">Notas (opcional)</span>
          <input class="inp" type="text" maxlength="300" placeholder="Ej. Capítulo 4, ejercicios 1–10" bind:value={notes} />
        </label>
      </div>

      <p class="note">La sesión reserva tiempo: no es una tarea y no aparece en tus pendientes.</p>

      {#if shownError}
        <p class="ferr" role="alert">{shownError}</p>
      {/if}

      <footer>
        {#if initial && ondelete}
          {#if confirmDel}
            <div class="delconfirm">
              <span>¿Eliminar «{initial.title}»? El bloque desaparecerá del calendario.</span>
              <div class="delbtns">
                <button type="button" class="btn danger" onclick={() => ondelete(initial.id)}>Eliminar</button>
                <button type="button" class="btn" onclick={() => (confirmDel = false)}>Cancelar</button>
              </div>
            </div>
          {:else}
            <button type="button" class="btn ghost-danger" onclick={() => (confirmDel = true)}>Eliminar</button>
          {/if}
        {/if}
        <div class="grow"></div>
        <button type="button" class="btn" onclick={oncancel}>Cancelar</button>
        <button type="submit" class="btn primary study" disabled={busy}>
          {busy ? "Guardando…" : initial ? "Guardar cambios" : "Añadir sesión"}
        </button>
      </footer>
    </form>
  </div>
</div>

<style>
  .overlay {
    position: fixed; inset: 0; z-index: 60;
    background: var(--overlay);
    -webkit-backdrop-filter: blur(var(--overlay-blur));
    backdrop-filter: blur(var(--overlay-blur));
    display: grid; place-items: center;
    padding: var(--s-4);
  }
  .modal {
    width: min(480px, 100%);
    background: var(--surface);
    border-radius: var(--r-xl);
    box-shadow: var(--shadow-raised-lg);
    /* contenedor: padding y gap un escalón por encima (aire) */
    padding: var(--s-6);
    display: flex; flex-direction: column; gap: var(--s-4);
  }
  header { display: flex; align-items: center; justify-content: space-between; }
  h3 { margin: 0; font-size: var(--fs-lg); font-weight: 600; }
  .x {
    border: none;
    background: var(--surface);
    color: var(--text-3);
    font-size: var(--fs-md);
    cursor: pointer;
    padding: var(--s-1) var(--s-1_5);
    border-radius: 50%;
    box-shadow: var(--btn-shadow);
    transition: box-shadow var(--dur-fast) var(--ease-out), color var(--dur-fast) var(--ease-out);
  }
  .x:hover { box-shadow: var(--btn-shadow-hover); color: var(--text-1); }
  .x:active { box-shadow: var(--btn-shadow-active); }

  form { display: flex; flex-direction: column; gap: var(--s-3); }
  /* 2ª columna más ancha: los inputs de hora necesitan el texto completo
     ("10:01 a. m.") más el icono de reloj; align-items:end alinea los
     controles aunque una etiqueta ocupe 2 líneas (Tarea relacionada). */
  .grid2 { display: grid; grid-template-columns: 1fr 1.6fr; gap: var(--s-3); align-items: end; }
  /* Fila fecha+horario: la columna del horario toma su ancho natural (dos
     campos de hora + "a") y la fecha el resto; nunca desborda el modal. */
  .grid2.date-time { grid-template-columns: minmax(0, 1fr) auto; }
  .row { display: flex; flex-direction: column; gap: var(--s-1); min-width: 0; }
  .lab { font-size: var(--fs-xs); font-weight: 600; text-transform: uppercase; letter-spacing: 0.06em; color: var(--text-3); }
  .inp {
    font: inherit; font-size: var(--fs-base); color: var(--text-1);
    background: var(--input-bg); border: var(--input-border);
    box-shadow: var(--input-shadow);
    border-radius: var(--r-full); padding: var(--s-2) var(--s-2); width: 100%; min-width: 0;
    transition: border-color var(--dur-fast) var(--ease-out);
  }
  .inp:focus { border-color: var(--study); }
  .timerange { display: flex; align-items: center; gap: var(--s-1_5); }
  /* Ancho de control (no espaciado): cabe "12:59 p. m." + icono de reloj */
  .timerange .inp { min-width: 124px; }
  .to { font-size: var(--fs-sm); color: var(--text-3); }
  .hint { margin: 0; font-size: var(--fs-xs); color: var(--text-3); }
  .hint strong { color: var(--study); font-weight: 600; font-variant-numeric: tabular-nums; }
  .note {
    margin: 0; font-size: var(--fs-xs); color: var(--text-2);
    background: color-mix(in srgb, var(--study) 8%, transparent);
    border-left: 3px solid var(--study);
    border-radius: var(--r-md); padding: var(--s-2) var(--s-2);
  }
  .ferr {
    margin: 0; font-size: var(--fs-sm); color: var(--danger);
    background: var(--danger-bg); border-radius: var(--r-md); padding: var(--s-2) var(--s-2);
  }
  footer { display: flex; align-items: center; gap: var(--s-2); margin-top: var(--s-1); flex-wrap: wrap; }
  .grow { flex: 1; }
  .btn {
    font: inherit; font-size: var(--fs-base); font-weight: 600; cursor: pointer;
    border: none; background: var(--surface); color: var(--text-1);
    border-radius: var(--r-full); padding: var(--s-2) var(--s-3);
    box-shadow: var(--btn-shadow);
    transition: box-shadow var(--dur-fast) var(--ease-out), color var(--dur-fast) var(--ease-out);
  }
  .btn:hover { box-shadow: var(--btn-shadow-hover); }
  .btn:active { box-shadow: var(--btn-shadow-active); }
  .btn.primary {
    background: var(--grad-accent);
    color: #fff;
    box-shadow: var(--btn-primary-shadow);
  }
  .btn.primary:active { box-shadow: var(--btn-shadow-active); }
  /* Identidad teal de las sesiones de estudio: degradado derivado de --study */
  .btn.primary.study {
    background: linear-gradient(135deg, color-mix(in srgb, var(--study) 78%, #fff), var(--study));
    box-shadow: var(--btn-shadow), 0 6px 16px -4px color-mix(in srgb, var(--study) 45%, transparent);
  }
  .btn.primary.study:active { box-shadow: var(--btn-shadow-active); }
  .btn.primary:disabled { opacity: 0.6; cursor: default; }
  .btn.danger { color: var(--danger); }
  .btn.ghost-danger { color: var(--danger); }
  .btn.ghost-danger:hover { box-shadow: var(--btn-shadow-hover); }
  .delconfirm {
    width: 100%; display: flex; flex-direction: column; gap: var(--s-2);
    background: var(--danger-bg); border-radius: var(--r-md); padding: var(--s-3);
    font-size: var(--fs-sm); color: var(--text-1);
  }
  .delbtns { display: flex; gap: var(--s-2); }

  @media (max-width: 560px) {
    .grid2 { grid-template-columns: 1fr; }
  }
</style>
