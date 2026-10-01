<script lang="ts">
  import {
    DAY_MS,
    isActiveOn,
    parseDateToMs,
    parseTimeToMinutes,
    validateClassForm,
    dayStartMs,
    type ClassRow,
  } from "./classLogic";

  export interface ClassFormValue {
    title: string;
    day_of_week: number;
    start_min: number;
    end_min: number;
    start_date: number;
    end_date: number;
  }

  let {
    initial = null,
    prefill = null,
    error = "",
    weekStart = dayStartMs(new Date()),
    oncancel,
    onSubmit,
    ondelete,
  }: {
    initial?: ClassRow | null;
    prefill?: { day_of_week: number; start_min: number } | null;
    error?: string;
    weekStart?: number;
    oncancel: () => void;
    onSubmit: (v: ClassFormValue) => void | Promise<void>;
    ondelete?: (id: number) => void | Promise<void>;
  } = $props();

  const DAYS_ES = ["Lunes", "Martes", "Miércoles", "Jueves", "Viernes", "Sábado", "Domingo"];

  function toInput(ms: number) {
    const d = new Date(ms);
    const p = (n: number) => String(n).padStart(2, "0");
    return `${d.getFullYear()}-${p(d.getMonth() + 1)}-${p(d.getDate())}`;
  }
  function minToInput(min: number) {
    const p = (n: number) => String(n).padStart(2, "0");
    return `${p(Math.floor(min / 60))}:${p(min % 60)}`;
  }

  const defaultStart = weekStart;
  const defaultEnd = weekStart + 84 * DAY_MS; // ~cuatrimestre, editable

  let title = $state(initial?.title ?? "");
  let day = $state(
    initial?.day_of_week ?? prefill?.day_of_week ?? 0,
  );
  let startV = $state(minToInput(initial?.start_min ?? prefill?.start_min ?? 600));
  let endV = $state(minToInput(initial?.end_min ?? (prefill?.start_min ?? 600) + 90));
  let startVd = $state(toInput(initial?.start_date ?? defaultStart));
  let endVd = $state(toInput(initial?.end_date ?? defaultEnd));
  let localError = $state("");
  let busy = $state(false);
  let confirmDel = $state(false);

  const shownError = $derived(localError || error);

  async function submit(e: SubmitEvent) {
    e.preventDefault();
    localError = "";
    const v = {
      title,
      day,
      startMin: parseTimeToMinutes(startV),
      endMin: parseTimeToMinutes(endV),
      startDateMs: parseDateToMs(startVd),
      endDateMs: parseDateToMs(endVd),
    };
    const msg = validateClassForm(v);
    if (msg) {
      localError = msg;
      return;
    }
    busy = true;
    await onSubmit({
      title: v.title.trim(),
      day_of_week: v.day,
      start_min: v.startMin,
      end_min: v.endMin,
      start_date: v.startDateMs,
      end_date: v.endDateMs,
    });
    busy = false;
  }

  /** Vista previa de vigencia: ¿la clase cae esta semana? */
  const preview = $derived.by(() => {
    const s = parseDateToMs(startVd);
    const en = parseDateToMs(endVd);
    if (Number.isNaN(s) || Number.isNaN(en) || en < s) return null;
    const daysShown = Array.from({ length: 7 }, (_, i) => weekStart + i * DAY_MS);
    const active = daysShown.some((d) =>
      isActiveOn(
        { id: 0, title: "x", day_of_week: day, start_min: 0, end_min: 1, start_date: s, end_date: en },
        d,
      ),
    );
    return active;
  });

  function onKey(e: KeyboardEvent) {
    if (e.key === "Escape") {
      if (confirmDel) confirmDel = false;
      else oncancel();
    }
    if (e.key === "Enter" && (e.metaKey || e.ctrlKey)) {
      (document.getElementById("cls-form") as HTMLFormElement | null)?.requestSubmit();
    }
  }
</script>

<svelte:window onkeydown={onKey} />

<div class="overlay" role="presentation" onclick={(e) => { if (e.target === e.currentTarget) oncancel(); }}>
  <div class="modal" role="dialog" aria-modal="true" aria-label={initial ? "Editar clase" : "Nueva clase"}>
    <header>
      <h3>{initial ? "Editar clase" : "Nueva clase"}</h3>
      <button class="x" onclick={oncancel} aria-label="Cerrar">✕</button>
    </header>

    <form id="cls-form" onsubmit={submit}>
      <label class="row">
        <span class="lab">Nombre de la clase *</span>
        <input class="inp" type="text" maxlength="120" placeholder="Ej. Matemáticas" bind:value={title} required />
      </label>

      <div class="grid2 date-time">
        <label class="row">
          <span class="lab">Día de la semana *</span>
          <select class="inp" bind:value={day}>
            {#each DAYS_ES as name, i}
              <option value={i}>{name}</option>
            {/each}
          </select>
        </label>
        <div class="row">
          <span class="lab">Horario *</span>
          <div class="timerange">
            <input class="inp" type="time" bind:value={startV} required />
            <span class="to">a</span>
            <input class="inp" type="time" bind:value={endV} required />
          </div>
        </div>
      </div>

      <div class="grid2">
        <label class="row">
          <span class="lab">Fecha de inicio *</span>
          <input class="inp" type="date" bind:value={startVd} required />
        </label>
        <label class="row">
          <span class="lab">Fecha de finalización *</span>
          <input class="inp" type="date" bind:value={endVd} required />
        </label>
      </div>
      <p class="hint">
        La clase solo aparece, genera conflictos y bloquea huecos entre el {" "}
        <strong>{startVd || "—"}</strong> y el <strong>{endVd || "—"}</strong>.
        {#if preview === false}
          <span class="warnmini">No cae ninguna clase en la semana visible.</span>
        {/if}
      </p>

      {#if shownError}
        <p class="ferr" role="alert">{shownError}</p>
      {/if}

      <footer>
        {#if initial && ondelete}
          {#if confirmDel}
            <div class="delconfirm">
              <span>¿Eliminar «{initial.title}»? Sus bloques desaparecerán del horario.</span>
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
        <button type="submit" class="btn primary" disabled={busy}>
          {busy ? "Guardando…" : initial ? "Guardar cambios" : "Añadir clase"}
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
  .grid2 { display: grid; grid-template-columns: 1fr 1fr; gap: var(--s-3); align-items: end; }
  /* Fila día+horario: la columna del horario toma su ancho natural (dos
     campos de hora + "a") y el día el resto; nunca desborda el modal. */
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
  .inp:focus { border-color: var(--primary); }
  .timerange { display: flex; align-items: center; gap: var(--s-1_5); }
  /* Ancho de control (no espaciado): cabe "12:59 p. m." + icono de reloj */
  .timerange .inp { min-width: 124px; }
  .to { font-size: var(--fs-sm); color: var(--text-3); }
  .hint { margin: 0; font-size: var(--fs-xs); color: var(--text-3); }
  .hint strong { color: var(--text-2); font-weight: 600; }
  .warnmini { color: var(--danger); font-weight: 600; }
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
