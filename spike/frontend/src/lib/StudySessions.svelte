<script lang="ts">
  // VISTA "Sesiones de estudio" — MISMA rejilla semana/día que Horario
  // (Schedule.svelte): cabecera de días clicables, carriles de solape, línea
  // de "ahora" y click en hueco para crear. La diferencia: los bloques son
  // SESIONES (tiempo reservado, teal) y el click las abre para editar/mover.
  import {
    studySessions,
    studyAdd,
    resetStudyAdd,
    studyDetail,
    openStudyDetail,
    closeStudyDetail,
    createStudy,
    updateStudy,
    deleteStudy,
    studyConflicts,
    tasks as tasksStore,
    classList as classCatalog,
  } from "./data.svelte";
  import {
    dayStartMs,
    dowMonFirst,
    formatMinutes,
    DAY_MS,
    classConflictsIn as classInstancesInRange,
    type ClassInstance,
  } from "./classLogic";
  import { sortStudies, type StudySession } from "./studyLogic";
  import StudyForm from "./StudyForm.svelte";

  // Mismo modelo que Schedule/Calendar: la fecha visible es estado GLOBAL de
  // App (compartida con el TopBar: título, ‹ › y Hoy idénticos). smode es el
  // conmutador semana/día, gestionado desde el TopBar.
  let {
    date,
    smode,
    setDate,
    setSmode,
  }: {
    date: Date;
    smode: "semana" | "dia";
    setDate: (d: Date) => void;
    setSmode: (m: "semana" | "dia") => void;
  } = $props();

  const studies = $derived(sortStudies(studySessions()));
  const tasks = $derived(tasksStore());

  let formOpen = $state(false);
  let editing = $state<StudySession | null>(null);
  let prefill = $state<{ start: Date; end: Date } | null>(null);
  // Cada apertura incrementa la clave: StudyForm captura initial/prefill solo
  // al montar; reabrirlo ya abierto con otra sesión guardaba sobre el id
  // equivocado. {#key formKey} lo remonta.
  let formKey = $state(0);
  let formError = $state("");
  let busy = $state(false);

  function mondayOf(d: Date): number {
    return dayStartMs(d) - dowMonFirst(new Date(dayStartMs(d))) * DAY_MS;
  }

  const weekStart = $derived(mondayOf(date));

  /** Los 7 días visibles (semana) o 1 (día), anclados a `date`. */
  const days = $derived(
    smode === "semana"
      ? Array.from({ length: 7 }, (_, i) => new Date(weekStart + i * DAY_MS))
      : [new Date(dayStartMs(date))],
  );

  function sessionsOfDay(dayMs: number) {
    return studies.filter((s) => s.start.getTime() < dayMs + DAY_MS && s.end.getTime() > dayMs);
  }

  /** ¿La sesión solapa una clase activa ese día? (borde de conflicto) */
  function clashesWithClass(s: StudySession, dayMs: number): boolean {
    return classListFor(dayMs).some((i) => s.start.getTime() < i.end_at && s.end.getTime() > i.start_at);
  }

  // Instancias de clase del horario en el rango visible: solo para el AVISO
  // visual de conflicto (borde ámbar); nunca bloquean la sesión.
  const classInstances = $derived.by(() => {
    const from = days[0].getTime();
    const to = days[days.length - 1].getTime() + DAY_MS - 1;
    const list = classInstancesInRange(classCatalog(), from, to);
    const out = new Map<number, ClassInstance[]>();
    for (const dayMs of days.map((d) => d.getTime())) {
      out.set(dayMs, list.filter((i) => i.date_ms === dayMs));
    }
    return out;
  });
  function classListFor(dayMs: number): ClassInstance[] {
    return classInstances.get(dayMs) ?? [];
  }

  /** Rejilla horaria: 6–22 por defecto, expandida si algo cae fuera. */
  const grid = $derived.by(() => {
    let lo = 6;
    let hi = 22;
    for (const d of days) {
      const dayMs = d.getTime();
      for (const s of sessionsOfDay(dayMs)) {
        const sMin = (s.start.getTime() - dayMs) / 60_000;
        const eMin = (s.end.getTime() - dayMs) / 60_000;
        lo = Math.min(lo, Math.floor(Math.max(0, sMin) / 60));
        hi = Math.max(hi, Math.ceil(Math.min(1440, eMin) / 60));
      }
    }
    lo = Math.max(0, lo);
    hi = Math.min(24, hi);
    if (hi <= lo) hi = lo + 1;
    return { lo, hi };
  });
  const hours = $derived(
    Array.from({ length: grid.hi - grid.lo + 1 }, (_, i) => grid.lo + i),
  );

  // pxH dinámico igual que Schedule: las horas estiran para llenar la pantalla.
  let timeAreaH = $state(0);
  const pxH = $derived(
    grid.hi > grid.lo && timeAreaH > 0 ? timeAreaH / (grid.hi - grid.lo) : 56,
  );
  const minTimeAreaH = $derived(hours.length * 28);

  /** Métricas de un bloque y su carril (solapes lado a lado, regla 9). */
  type LaneStudy = { s: StudySession; dayMs: number; lane: number; lanes: number };
  const laidOut = $derived.by(() => {
    const perDay = new Map<number, LaneStudy[]>();
    for (const d of days) {
      const dayMs = d.getTime();
      const items = [...sessionsOfDay(dayMs)].sort(
        (a, b) => a.start.getTime() - b.start.getTime() || a.end.getTime() - b.end.getTime(),
      );
      const laneEnds: number[] = [];
      const withLane = items.map((s) => {
        const st = Math.max(s.start.getTime(), dayMs);
        const en = Math.min(s.end.getTime(), dayMs + DAY_MS - 1);
        let lane = laneEnds.findIndex((end) => end <= st);
        if (lane === -1) {
          laneEnds.push(en);
          lane = laneEnds.length - 1;
        } else {
          laneEnds[lane] = Math.max(laneEnds[lane], en);
        }
        return { s, dayMs, lane, lanes: 1 };
      });
      const lanes = Math.max(1, laneEnds.length);
      perDay.set(dayMs, withLane.map((w) => ({ ...w, lanes })));
    }
    return perDay;
  });

  function blockMetrics(s: StudySession, dayMs: number) {
    const startMin = Math.max(0, (s.start.getTime() - dayMs) / 60_000);
    const endMin = Math.min(1440, (s.end.getTime() - dayMs) / 60_000);
    const top = ((startMin - grid.lo * 60) / 60) * pxH;
    const height = Math.max(22, ((endMin - startMin) / 60) * pxH);
    return { top, height };
  }

  function openNew(day?: Date, hour?: number) {
    editing = null;
    formError = "";
    prefill = day
      ? {
          start: new Date(day.getTime() + (hour ?? 8) * 3_600_000),
          end: new Date(day.getTime() + (hour ?? 8) * 3_600_000 + 2 * 3_600_000),
        }
      : null;
    formKey++;
    formOpen = true;
  }

  // El sidebar ("Añadir sesión de estudio") pide abrir el formulario: una vez.
  $effect(() => {
    if (studyAdd() > 0) {
      resetStudyAdd();
      openNew();
    }
  });

  // El calendario (click en un bloque) pide abrir EL editor de esa sesión.
  $effect(() => {
    const id = studyDetail();
    if (id != null) {
      const s = studies.find((x) => x.id === id);
      if (s) {
        editing = s;
        prefill = null;
        formError = "";
        formKey++;
        formOpen = true;
      }
      closeStudyDetail();
    }
  });

  async function onSubmit(v: {
    title: string;
    start: Date;
    end: Date;
    taskId: number | null;
    notes: string;
  }) {
    formError = "";
    // Conflicto con clases (regla 10): advertir con Editar horario / Cancelar /
    // Continuar de todas formas. Nunca se mueve nada automáticamente.
    const c = await studyConflicts(v.start.getTime(), v.end.getTime());
    if (c.classes.length > 0) {
      const go = await askConflict(v.title, v.start, v.end, c.classes, c.tasks);
      if (!go) return;
    }
    busy = true;
    const r = editing
      ? await updateStudy(editing.id, v)
      : await createStudy(v);
    busy = false;
    if (!r.ok) {
      formError = r.error ?? "No se pudo guardar la sesión.";
      return;
    }
    formOpen = false;
    editing = null;
  }

  async function removeCurrent() {
    if (!editing) return;
    busy = true;
    const r = await deleteStudy(editing.id);
    busy = false;
    if (r.ok) {
      formOpen = false;
      editing = null;
    } else {
      formError = r.error ?? "No se pudo eliminar.";
    }
  }

  function slotClick(d: Date, e: MouseEvent) {
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
    const y = e.clientY - rect.top;
    const hour = grid.lo + Math.floor(y / pxH);
    openNew(d, Math.min(23, Math.max(0, hour)));
  }

  // ---- diálogo de conflicto (mismo flujo que la vista lista) ----
  let conflictInfo = $state<null | {
    title: string;
    window: string;
    classes: ClassInstance[];
    tasks: { id: number; title: string }[];
  }>(null);
  let pendingResolve: ((go: boolean) => void) | null = null;
  function askConflict(
    title: string,
    start: Date,
    end: Date,
    classes: ClassInstance[],
    tasksC: { id: number; title: string }[],
  ): Promise<boolean> {
    return new Promise((resolve) => {
      pendingResolve = resolve;
      conflictInfo = { title, window: `${fmtHM(start.getTime())} – ${fmtHM(end.getTime())}`, classes, tasks: tasksC };
    });
  }
  function resolveConflict(go: boolean) {
    conflictInfo = null;
    pendingResolve?.(go);
    pendingResolve = null;
  }

  function taskTitle(id: number | null): string | null {
    if (id == null) return null;
    return tasks.find((t) => t.id === id)?.title ?? null;
  }

  const DAYS_ES = ["lun", "mar", "mié", "jue", "vie", "sáb", "dom"];
  function isToday(d: Date) {
    return d.getTime() === dayStartMs(new Date());
  }
  function fmtHM(ms: number): string {
    const d = new Date(ms);
    return `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
  }
  let nowMs = $state(Date.now());
  $effect(() => {
    const t = setInterval(() => (nowMs = Date.now()), 60_000);
    return () => clearInterval(t);
  });
  function nowTop() {
    const d = new Date(nowMs);
    return ((d.getHours() * 60 + d.getMinutes() - grid.lo * 60) / 60) * pxH;
  }
  const nowInRange = $derived(
    nowTop() >= 0 && nowTop() <= (grid.hi - grid.lo) * pxH,
  );
  function hourLabel(h: number): string {
    if (h === 0) return "12 a";
    if (h === 12) return "12 p";
    return `${h < 12 ? h : h - 12} ${h < 12 ? "a" : "p"}`;
  }
</script>

<div class="cal" class:week={smode === "semana"} class:day={smode === "dia"}>
  {#if studies.length === 0 && !formOpen}
    <div class="empty">
      <p><strong>No hay sesiones de estudio programadas.</strong></p>
      <p>Reserva un bloque para estudiar o trabajar: «Estudiar cálculo, 10:00 – 12:00». No son tareas: son tiempo planificado que puedes mover y editar.</p>
      <button class="add" onclick={() => openNew()}>Añadir mi primera sesión</button>
    </div>
  {:else}
    <div class="week-head">
      <span class="gutter-spacer"></span>
      {#each days as d (d.toDateString())}
        <button class="day-head {isToday(d) ? 'today' : ''}" onclick={() => { setDate(d); setSmode("dia"); }}>
          <span class="dow">{DAYS_ES[dowMonFirst(d)]}</span>
          <span class="num">{d.getDate()}</span>
        </button>
      {/each}
    </div>
    <div class="week-body">
      <div class="gutter">
        <div class="hours-area">
          {#each hours as h}
            <span class="hour" style="top: {(h - grid.lo) * pxH}px">{hourLabel(h)}</span>
          {/each}
        </div>
      </div>
      {#each days as d (d.toDateString())}
        {@const dayMs = d.getTime()}
        <div class="day-col {isToday(d) ? 'today' : ''}">
          <div
            class="time-area"
            bind:clientHeight={timeAreaH}
            style="min-height: {minTimeAreaH}px"
            onclick={(e) => slotClick(d, e)}
          >
            <div class="slots">
              {#each hours.slice(0, -1) as h}
                <div class="slot"></div>
              {/each}
            </div>
            {#if isToday(d) && nowInRange}
              <div class="now-line" style="top: {nowTop()}px"></div>
            {/if}
            {#each laidOut.get(dayMs) ?? [] as it (it.s.id + "-" + it.dayMs)}
              {@const m = blockMetrics(it.s, it.dayMs)}
              {@const clash = clashesWithClass(it.s, it.dayMs)}
              <button
                type="button"
                class="stu-block {clash ? 'clash' : ''} {m.height < 36 ? 'compact' : ''}"
                data-testid="study-block"
                style="top: {m.top}px; height: {m.height}px; left: calc({(it.lane * 100) / it.lanes}% + 3px); width: calc({100 / it.lanes}% - 6px);"
                title="Sesión de estudio · clic para editar/mover{clash ? '\n⚠ Coincide con una clase de tu horario' : ''}"
                onclick={(e) => { e.stopPropagation(); openStudyDetail(it.s.id); }}
              >
                {#if m.height < 36}
                  <span class="sb-inline">
                    <span class="sb-time">{fmtHM(it.s.start.getTime())}</span>
                    <span class="sb-title">{it.s.title}</span>
                  </span>
                {:else}
                  <span class="sb-title">{it.s.title}</span>
                  <span class="sb-time">{fmtHM(it.s.start.getTime())} – {fmtHM(it.s.end.getTime())}</span>
                  {#if m.height >= 62 && taskTitle(it.s.taskId)}
                    <span class="sb-task">↳ {taskTitle(it.s.taskId)}</span>
                  {/if}
                {/if}
              </button>
            {/each}
          </div>
        </div>
      {/each}
    </div>
  {/if}
</div>

{#if formOpen}
  {#key formKey}
  <StudyForm
    initial={editing}
    prefill={prefill}
    error={formError}
    oncancel={() => { formOpen = false; editing = null; prefill = null; }}
    {onSubmit}
    ondelete={removeCurrent}
  />
  {/key}
{/if}

{#if conflictInfo}
  <div class="overlay" role="presentation" onclick={(e) => { if (e.target === e.currentTarget) resolveConflict(false); }}>
    <div class="modal" role="alertdialog" aria-modal="true" aria-labelledby="sc-title">
      <header>
        <span class="badge" aria-hidden="true">⚠</span>
        <h3 id="sc-title">Esta sesión coincide con una clase</h3>
      </header>
      <p class="lead">«{conflictInfo.title}» ({conflictInfo.window}) se superpone con:</p>
      <ul class="cl">
        {#each conflictInfo.classes as c (c.class_id + "-" + c.date_ms)}
          <li>
            <strong>{c.title}</strong>
            <span class="tm">{formatMinutes((c.start_at - c.date_ms) / 60_000)} – {formatMinutes((c.end_at - c.date_ms) / 60_000)}</span>
          </li>
        {/each}
      </ul>
      {#if conflictInfo.tasks.length > 0}
        <p class="lead tasks-note">
          También coincide con la{conflictInfo.tasks.length > 1 ? "s" : ""} tarea{conflictInfo.tasks.length > 1 ? "s" : ""}:
          {conflictInfo.tasks.map((t) => `«${t.title}»`).join(", ")}. Ambas pueden coexistir.
        </p>
      {/if}
      <footer>
        <button type="button" class="btn" onclick={() => resolveConflict(false)}>Editar horario</button>
        <button type="button" class="btn primary study" onclick={() => resolveConflict(true)}>Continuar de todas formas</button>
      </footer>
      <p class="hint">No se mueve nada automáticamente: la decisión es tuya.</p>
    </div>
  </div>
{/if}

<style>
  /* Cromo idéntico a Schedule.svelte (misma rejilla semana/día): mismas
     clases y mismos valores para que sesiones se vea igual que horario. */
  .cal {
    background: var(--surface);
    border-radius: var(--r-card);
    box-shadow: var(--shadow-raised-lg);
    overflow: hidden;
    min-width: 0;
    display: flex;
    flex-direction: column;
    min-height: 0;
    height: 100%;
    position: relative;
  }

  .week-head {
    display: flex;
    padding: var(--s-4) var(--s-4) var(--s-2);
    /* cabecera de la vista: gap un escalón por encima (aire) */
    gap: var(--s-2);
    flex-shrink: 0;
  }
  .gutter-spacer {
    width: 56px;
    flex-shrink: 0;
  }
  .day-head {
    flex: 1;
    border: none;
    background: transparent;
    border-radius: var(--r-control);
    padding: var(--s-1_5) 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--s-0_5);
    transition: background var(--dur-fast) var(--ease-out);
    min-width: 0;
    cursor: pointer;
    font-family: inherit;
  }
  .day-head:hover {
    background: var(--surface-2);
  }
  .day-head.today .num {
    background: var(--grad-accent);
    box-shadow: var(--glow-accent);
    color: #fff;
  }
  .dow {
    font-size: var(--fs-xs);
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--text-3);
  }
  .num {
    font-size: var(--fs-lg);
    font-weight: 600;
    font-variant-numeric: tabular-nums;
    width: 30px;
    height: 30px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border-radius: var(--r-icon);
    color: var(--text-1);
  }

  .week-body {
    flex: 1;
    display: flex;
    overflow-y: auto;
    overflow-x: hidden;
    padding: var(--s-1_5) var(--s-4) var(--s-4);
    gap: var(--s-2);
    min-height: 0;
  }
  .gutter {
    width: 56px;
    flex-shrink: 0;
    position: relative;
    display: flex;
    flex-direction: column;
  }
  .hours-area {
    position: relative;
    flex: 1;
    min-height: 0;
  }
  .hour {
    position: absolute;
    right: 10px;
    font-size: var(--fs-xs);
    font-weight: 600;
    color: var(--text-3);
    transform: translateY(-6px);
    font-variant-numeric: tabular-nums;
    text-align: right;
    white-space: nowrap;
  }
  .day-col {
    flex: 1;
    position: relative;
    border-radius: var(--r-well);
    min-width: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  .day-col.today {
    /* resaltado sutil dentro del pozo, sin líneas duras */
    background: color-mix(in srgb, var(--study) 6%, transparent);
  }
  .time-area {
    position: relative;
    flex: 1;
    min-height: 0;
    cursor: copy;
    /* cuadrícula hundida (pozo) */
    background: var(--surface-2);
    box-shadow: var(--shadow-inset-sm);
  }
  .slots {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
  }
  .slot {
    flex: 1 1 0;
    min-height: 28px;
    border-top: 1px solid var(--border);
    margin-left: var(--s-0_5);
    margin-right: var(--s-0_5);
  }
  .now-line {
    position: absolute;
    left: 2px;
    right: 2px;
    height: 2px;
    background: var(--study);
    border-radius: var(--r-chip);
    z-index: 2;
    pointer-events: none;
  }

  /* Bloque de sesión: misma silueta que .cls-block/.evt, en color propio.
     Contenido arriba: si no cabe, solo se recorta por abajo. */
  .stu-block {
    position: absolute;
    display: flex;
    flex-direction: column;
    justify-content: flex-start;
    align-items: stretch;
    gap: var(--s-0_5);
    background: color-mix(in srgb, var(--study) 13%, var(--surface));
    border: none;
    border-left: 3px solid var(--study);
    border-radius: var(--r-sm);
    padding: var(--s-1) var(--s-2);
    text-align: left;
    overflow: hidden;
    z-index: 1;
    /* sesión: relieve mínimo elevado sobre su tinte teal */
    box-shadow: 2px 2px 5px var(--neu-dark), -2px -2px 5px var(--neu-light);
    transition: transform var(--dur-fast) var(--ease-out), box-shadow var(--dur-fast) var(--ease-out);
    min-width: 0;
    cursor: pointer;
    font: inherit;
  }
  .stu-block:hover {
    transform: translateY(-1px) scale(1.01);
    box-shadow: 4px 4px 10px var(--neu-dark), -4px -4px 10px var(--neu-light);
    z-index: 3;
  }
  /* Conflicto con clase (regla 10): borde ámbar; el usuario decidió seguir. */
  .stu-block.clash {
    outline: 1.5px solid var(--warning);
    outline-offset: -1.5px;
  }
  .sb-inline {
    display: flex;
    align-items: center;
    gap: var(--s-1);
    min-width: 0;
  }
  .sb-inline .sb-time {
    flex-shrink: 0;
  }
  .sb-inline .sb-title {
    min-width: 0;
  }
  .sb-title {
    font-size: var(--fs-sm);
    font-weight: 600;
    color: var(--text-1);
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    overflow: hidden;
    word-break: break-word;
  }
  /* Compacto (< 36px): hora + título en UNA línea */
  .stu-block.compact .sb-title {
    -webkit-line-clamp: 1;
    line-clamp: 1;
  }
  .sb-time {
    font-size: var(--fs-xs);
    color: var(--text-3);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  .sb-task {
    font-size: var(--fs-2xs);
    color: var(--text-3);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: var(--s-2);
    padding: var(--s-8);
    text-align: center;
    color: var(--text-2);
    flex: 1;
  }
  .empty p { margin: 0; max-width: 420px; font-size: var(--fs-base); }
  .empty strong { font-size: var(--fs-lg); color: var(--text-1); }
  .add {
    font: inherit;
    font-size: var(--fs-base);
    font-weight: 600;
    margin-top: var(--s-2);
    background: var(--study);
    color: #fff;
    border: none;
    border-radius: var(--r-control);
    padding: var(--s-2) var(--s-3);
    cursor: pointer;
    box-shadow: var(--e1);
    transition: background var(--dur-fast) var(--ease-out), transform var(--dur-fast) var(--ease-out);
  }
  .add:hover { background: color-mix(in srgb, var(--study) 85%, #000); transform: translateY(-1px); }

  /* Diálogo de conflicto (mismo lenguaje visual que el resto). */
  .overlay {
    position: fixed; inset: 0; z-index: 70;
    background: var(--overlay);
    -webkit-backdrop-filter: blur(var(--overlay-blur));
    backdrop-filter: blur(var(--overlay-blur));
    display: grid; place-items: center; padding: var(--s-4);
  }
  .modal {
    width: min(420px, 100%);
    background: var(--surface);
    border-radius: var(--r-card);
    box-shadow: var(--shadow-raised-lg);
    /* contenedor: padding y gap un escalón por encima (aire) */
    padding: var(--s-6);
    display: flex; flex-direction: column; gap: var(--s-3);
  }
  .modal header { display: flex; align-items: center; gap: var(--s-2); }
  .badge {
    width: 30px; height: 30px; border-radius: var(--r-icon);
    background: var(--warning-bg); color: var(--warning);
    display: grid; place-items: center; font-size: var(--fs-lg); font-weight: 600;
  }
  h3 { margin: 0; font-size: var(--fs-lg); font-weight: 600; }
  .lead { margin: 0; font-size: var(--fs-base); color: var(--text-2); }
  .tasks-note { font-size: var(--fs-sm); }
  .cl { margin: 0; padding: 0; list-style: none; display: flex; flex-direction: column; gap: var(--s-1); }
  .cl li {
    display: flex; justify-content: space-between; gap: var(--s-3);
    font-size: var(--fs-base);
    background: color-mix(in srgb, var(--study) 10%, transparent);
    border-left: 3px solid var(--study);
    border-radius: var(--r-well); padding: var(--s-1_5) var(--s-2);
  }
  .cl .tm { font-variant-numeric: tabular-nums; color: var(--text-3); font-size: var(--fs-sm); }
  footer { display: flex; gap: var(--s-2); margin-top: var(--s-2); }
  .btn {
    flex: 1; font: inherit; font-size: var(--fs-base); font-weight: 600; cursor: pointer;
    border: 1px solid var(--border); background: var(--surface); color: var(--text-2);
    border-radius: var(--r-control); padding: var(--s-2) var(--s-2);
  }
  .btn:hover { background: var(--surface-2); }
  .btn.primary.study { background: var(--study); border-color: var(--study); color: #fff; }
  .btn.primary.study:hover { background: color-mix(in srgb, var(--study) 85%, #000); }
  .hint { margin: 0; font-size: var(--fs-xs); color: var(--text-3); }

  @media (max-width: 860px) {
    .hour { font-size: var(--fs-2xs); }
    .stu-block { padding: var(--s-0_5) var(--s-1); }
    .sb-time, .sb-task { display: none; }
  }
</style>
