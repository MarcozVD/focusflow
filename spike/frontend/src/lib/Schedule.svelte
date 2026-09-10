<script lang="ts">
  import { classAdd, classList, createClass, deleteClass, resetClassAdd, updateClass } from "./data.svelte";
  import {
    DAY_MS,
    classConflictsIn,
    dayStartMs,
    dowMonFirst,
    formatMinutes,
    type ClassRow,
  } from "./classLogic";
  import ClassForm, { type ClassFormValue } from "./ClassForm.svelte";

  // Mismo modelo que el Calendario de tareas: la fecha visible es el estado
  // GLOBAL de App (compartido con el TopBar: título, ‹ › y Hoy idénticos).
  // hmode es el conmutador semana/día, gestionado desde el TopBar.
  let {
    date,
    hmode,
    setDate,
    setHmode,
  }: {
    date: Date;
    hmode: "semana" | "dia";
    setDate: (d: Date) => void;
    setHmode: (m: "semana" | "dia") => void;
  } = $props();

  let formOpen = $state(false);
  let editing: ClassRow | null = $state(null);
  let prefill: { day_of_week: number; start_min: number } | null = null;
  let formError = $state("");

  function mondayOf(d: Date): number {
    return dayStartMs(d) - dowMonFirst(new Date(dayStartMs(d))) * DAY_MS;
  }

  const weekStart = $derived(mondayOf(date));

  /** Los 7 días visibles (semana) o 1 (día), anclados a `date`. */
  const days = $derived(
    hmode === "semana"
      ? Array.from({ length: 7 }, (_, i) => new Date(weekStart + i * DAY_MS))
      : [new Date(dayStartMs(date))],
  );

  /** Instancias de clase materializadas en el rango visible. */
  const instances = $derived(
    classConflictsIn(classList(), days[0].getTime(), days[days.length - 1].getTime() + DAY_MS - 1),
  );

  function instancesOfDay(dayMs: number) {
    return instances.filter((i) => i.date_ms === dayMs);
  }

  /**
   * Rejilla horaria: 6–22 por defecto, expandida si alguna clase cae fuera
   * (mismo criterio que el calendario de tareas).
   */
  const grid = $derived.by(() => {
    let lo = 6;
    let hi = 22;
    for (const ins of instances) {
      const sMin = (ins.start_at - ins.date_ms) / 60_000;
      const eMin = (ins.end_at - ins.date_ms) / 60_000;
      lo = Math.min(lo, Math.floor(sMin / 60));
      hi = Math.max(hi, Math.ceil(eMin / 60));
    }
    lo = Math.max(0, lo);
    hi = Math.min(24, hi);
    if (hi <= lo) hi = lo + 1;
    return { lo, hi };
  });
  const hours = $derived(
    Array.from({ length: grid.hi - grid.lo + 1 }, (_, i) => grid.lo + i),
  );

  // pxH dinámico igual que Calendar: las horas estiran para llenar la pantalla.
  let timeAreaH = $state(0);
  const pxH = $derived(
    grid.hi > grid.lo && timeAreaH > 0 ? timeAreaH / (grid.hi - grid.lo) : 56,
  );
  const minTimeAreaH = $derived(hours.length * 28);

  /** Métricas de un bloque y su carril (solapes lado a lado, regla 9). */
  type LaneInstance = (typeof instances)[number] & { lane: number; lanes: number };
  const laidOut = $derived.by(() => {
    const perDay = new Map<number, LaneInstance[]>();
    for (const d of days) {
      const dayMs = d.getTime();
      const items = instancesOfDay(dayMs).sort(
        (a, b) => a.start_at - b.start_at || a.end_at - b.end_at,
      );
      // asignación voraz de carriles: cada clase va al primer carril libre
      const laneEnds: number[] = [];
      const withLane = items.map((it) => {
        let lane = laneEnds.findIndex((end) => end <= it.start_at);
        if (lane === -1) {
          laneEnds.push(it.end_at);
          lane = laneEnds.length - 1;
        } else {
          laneEnds[lane] = it.end_at;
        }
        return { ...it, lane, lanes: 1 };
      });
      const lanes = Math.max(1, laneEnds.length);
      perDay.set(dayMs, withLane.map((w) => ({ ...w, lanes })));
    }
    return perDay;
  });

  function blockMetrics(ins: { start_at: number; end_at: number; date_ms: number }) {
    const startMin = (ins.start_at - ins.date_ms) / 60_000;
    const endMin = (ins.end_at - ins.date_ms) / 60_000;
    const top = ((startMin - grid.lo * 60) / 60) * pxH;
    const height = Math.max(22, ((endMin - startMin) / 60) * pxH);
    return { top, height };
  }

  function openNew(day?: Date, hour?: number) {
    editing = null;
    formError = "";
    prefill = day
      ? { day_of_week: dowMonFirst(day), start_min: (hour ?? 8) * 60 }
      : null;
    formOpen = true;
  }

  // El sidebar ("Añadir horario") pide abrir el formulario: se consume una vez.
  $effect(() => {
    if (classAdd() > 0) {
      resetClassAdd();
      openNew();
    }
  });

  function openEdit(classId: number) {
    const c = classList().find((x) => x.id === classId);
    if (!c) return;
    editing = c;
    formError = "";
    formOpen = true;
  }

  async function onSubmit(v: ClassFormValue) {
    formError = "";
    const draft = {
      title: v.title,
      day_of_week: v.day_of_week,
      start_min: v.start_min,
      end_min: v.end_min,
      start_date: v.start_date,
      end_date: v.end_date,
    };
    const r = editing
      ? await updateClass(editing.id, draft)
      : await createClass(draft);
    if (!r.ok) {
      formError = r.error ?? "No se pudo guardar la clase.";
      return;
    }
    formOpen = false;
    editing = null;
  }

  function slotClick(d: Date, e: MouseEvent) {
    const rect = (e.currentTarget as HTMLElement).getBoundingClientRect();
    const y = e.clientY - rect.top;
    const hour = grid.lo + Math.floor(y / pxH);
    openNew(d, Math.min(23, Math.max(0, hour)));
  }

  const DAYS_ES = ["lun", "mar", "mié", "jue", "vie", "sáb", "dom"];
  function isToday(d: Date) {
    return d.getTime() === dayStartMs(new Date());
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

<div class="cal" class:week={hmode === "semana"} class:day={hmode === "dia"}>
  {#if classList().length === 0 && !formOpen}
    <div class="empty">
      <p><strong>Aún no tienes clases en tu horario.</strong></p>
      <p>Añade tus asignaturas con su día, hora y periodo de validez. El planificador las respetará automáticamente.</p>
      <button class="add" onclick={() => openNew()}>Añadir mi primera clase</button>
    </div>
  {:else}
    <div class="week-head">
      <span class="gutter-spacer"></span>
      {#each days as d (d.toDateString())}
        <button class="day-head {isToday(d) ? 'today' : ''}" onclick={() => { setDate(d); setHmode("dia"); }}>
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
            {#each laidOut.get(dayMs) ?? [] as ins (ins.class_id + "-" + ins.date_ms)}
              {@const m = blockMetrics(ins)}
              <button
                type="button"
                class="cls-block"
                style="top: {m.top}px; height: {m.height}px; left: calc({(ins.lane * 100) / ins.lanes}% + 3px); width: calc({100 / ins.lanes}% - 6px);"
                title="{formatMinutes((ins.start_at - ins.date_ms) / 60_000)}–{formatMinutes((ins.end_at - ins.date_ms) / 60_000)} · clic para editar"
                onclick={(e) => { e.stopPropagation(); openEdit(ins.class_id); }}
              >
                <span class="cb-title">{ins.title}</span>
                <span class="cb-time"
                  >{formatMinutes((ins.start_at - ins.date_ms) / 60_000)} – {formatMinutes((ins.end_at - ins.date_ms) / 60_000)}</span
                >
              </button>
            {/each}
          </div>
        </div>
      {/each}
    </div>
  {/if}
</div>

{#if formOpen}
  <ClassForm
    initial={editing}
    {prefill}
    error={formError}
    weekStart={weekStart}
    oncancel={() => { formOpen = false; editing = null; }}
    {onSubmit}
    ondelete={async (id) => {
      const r = await deleteClass(id);
      if (!r.ok) formError = r.error ?? "No se pudo eliminar.";
      else { formOpen = false; editing = null; }
    }}
  />
{/if}

<style>
  /* Cromo idéntico a Calendar.svelte (vista semana/día): mismas clases y
     mismos valores para que horario se vea exactamente igual. */
  .cal {
    background: var(--surface);
    border-radius: var(--r-lg);
    box-shadow: var(--shadow-raised);
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
    gap: 6px;
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
    border-radius: var(--r-md);
    padding: 6px 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
    transition: background var(--dur-fast) var(--ease-out);
    min-width: 0;
  }
  .day-head:hover {
    background: var(--surface-2);
  }
  .day-head.today .num {
    background: var(--primary);
    color: #fff;
  }
  .dow {
    font-size: 11px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--text-3);
  }
  .num {
    font-size: 15px;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
    width: 30px;
    height: 30px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border-radius: var(--r-full);
    color: var(--text-1);
  }

  .week-body {
    flex: 1;
    display: flex;
    overflow-y: auto;
    overflow-x: hidden;
    padding: 0 var(--s-4) var(--s-4);
    gap: 6px;
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
    font-size: 11px;
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
    border-radius: var(--r-md);
    min-width: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  .day-col.today {
    box-shadow: inset 0 0 0 2px var(--primary-soft-2);
  }
  .time-area {
    position: relative;
    flex: 1;
    min-height: 0;
    cursor: copy;
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
    margin-left: 2px;
    margin-right: 2px;
  }
  .now-line {
    position: absolute;
    left: 2px;
    right: 2px;
    height: 2px;
    background: var(--primary);
    border-radius: var(--r-full);
    z-index: 2;
    pointer-events: none;
  }

  /* Bloque de clase: misma silueta que .evt (EventBlock) en color primario. */
  .cls-block {
    position: absolute;
    display: flex;
    flex-direction: column;
    gap: 2px;
    background: color-mix(in srgb, var(--primary) 13%, var(--surface));
    border: none;
    border-left: 3px solid var(--primary);
    border-radius: var(--r-sm);
    padding: 4px 8px;
    text-align: left;
    overflow: hidden;
    z-index: 1;
    box-shadow: var(--shadow-inset-sm);
    transition: transform var(--dur-fast) var(--ease-out), box-shadow var(--dur-fast) var(--ease-out);
    min-width: 0;
    cursor: pointer;
    font: inherit;
  }
  .cls-block:hover {
    transform: translateY(-1px) scale(1.01);
    box-shadow: var(--e1);
    z-index: 3;
  }
  .cb-title {
    font-size: 12px;
    font-weight: 600;
    color: var(--text-1);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .cb-time {
    font-size: 11px;
    color: var(--text-3);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
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
  .empty p { margin: 0; max-width: 420px; font-size: 13px; }
  .empty strong { font-size: 15px; color: var(--text-1); }
  .add {
    font: inherit;
    font-size: 13px;
    font-weight: 600;
    margin-top: var(--s-2);
    background: var(--primary);
    color: #fff;
    border: none;
    border-radius: var(--r-md);
    padding: 8px 14px;
    cursor: pointer;
    box-shadow: var(--e1);
    transition: background var(--dur-fast) var(--ease-out), transform var(--dur-fast) var(--ease-out);
  }
  .add:hover { background: var(--primary-hover); transform: translateY(-1px); }

  @media (max-width: 860px) {
    .hour { font-size: 10px; }
    .cls-block { padding: 3px 5px; }
    .cb-time { display: none; }
  }
</style>
