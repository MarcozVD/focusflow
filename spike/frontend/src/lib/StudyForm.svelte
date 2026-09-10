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

      <div class="grid2">
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
    background: rgba(15, 23, 42, 0.38);
    display: grid; place-items: center;
    padding: var(--s-4);
  }
  .modal {
    width: min(440px, 100%);
    background: var(--surface);
    border-radius: var(--r-lg);
    box-shadow: var(--shadow-raised-lg);
    padding: var(--s-5);
    display: flex; flex-direction: column; gap: var(--s-3);
  }
  header { display: flex; align-items: center; justify-content: space-between; }
  h3 { margin: 0; font-size: 16px; font-weight: 700; }
  .x {
    border: none; background: none; color: var(--text-3);
    font-size: 14px; cursor: pointer; padding: 4px 6px; border-radius: var(--r-sm);
  }
  .x:hover { background: var(--surface-2); color: var(--text-1); }

  form { display: flex; flex-direction: column; gap: var(--s-3); }
  .grid2 { display: grid; grid-template-columns: 1fr 1fr; gap: var(--s-3); }
  .row { display: flex; flex-direction: column; gap: 4px; min-width: 0; }
  .lab { font-size: 11px; font-weight: 700; text-transform: uppercase; letter-spacing: 0.06em; color: var(--text-3); }
  .inp {
    font: inherit; font-size: 13px; color: var(--text-1);
    background: var(--surface-2); border: 1px solid var(--border);
    border-radius: var(--r-md); padding: 7px 9px; width: 100%; min-width: 0;
  }
  .inp:focus { outline: 2px solid color-mix(in srgb, var(--study) 45%, transparent); border-color: var(--study); }
  .timerange { display: flex; align-items: center; gap: 6px; }
  .to { font-size: 12px; color: var(--text-3); }
  .hint { margin: 0; font-size: 11.5px; color: var(--text-3); }
  .hint strong { color: var(--study); font-weight: 700; font-variant-numeric: tabular-nums; }
  .note {
    margin: 0; font-size: 11.5px; color: var(--text-2);
    background: color-mix(in srgb, var(--study) 8%, transparent);
    border-left: 3px solid var(--study);
    border-radius: var(--r-md); padding: 7px 10px;
  }
  .ferr {
    margin: 0; font-size: 12.5px; color: var(--danger);
    background: var(--danger-bg); border-radius: var(--r-md); padding: 8px 10px;
  }
  footer { display: flex; align-items: center; gap: var(--s-2); margin-top: var(--s-1); flex-wrap: wrap; }
  .grow { flex: 1; }
  .btn {
    font: inherit; font-size: 13px; font-weight: 600; cursor: pointer;
    border: 1px solid var(--border); background: var(--surface); color: var(--text-2);
    border-radius: var(--r-md); padding: 7px 14px;
  }
  .btn:hover { background: var(--surface-2); }
  .btn.primary { background: var(--primary); border-color: var(--primary); color: #fff; }
  .btn.primary:hover { background: var(--primary-hover); }
  .btn.primary.study { background: var(--study); border-color: var(--study); }
  .btn.primary.study:hover { background: color-mix(in srgb, var(--study) 85%, #000); }
  .btn.primary:disabled { opacity: 0.6; cursor: default; }
  .btn.danger { background: var(--danger); border-color: var(--danger); color: #fff; }
  .btn.ghost-danger { background: none; border-color: transparent; color: var(--danger); }
  .btn.ghost-danger:hover { background: var(--danger-bg); }
  .delconfirm {
    width: 100%; display: flex; flex-direction: column; gap: var(--s-2);
    background: var(--danger-bg); border-radius: var(--r-md); padding: var(--s-3);
    font-size: 12.5px; color: var(--text-1);
  }
  .delbtns { display: flex; gap: var(--s-2); }

  @media (max-width: 560px) {
    .grid2 { grid-template-columns: 1fr; }
  }
</style>
