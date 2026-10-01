<script lang="ts">
  import {
    DAYS_ES,
    tasks as tasksStore,
    cat,
    openTaskDetail,
    openStudyDetail,
    moveTask,
    moveStudy,
    guardClassConflict,
    guardStudyConflicts,
    classList,
    studySessions,
    type Task,
  } from "./data.svelte";
  import { classConflictsIn, DAY_MS } from "./classLogic";
  import { capitalizeFirst } from "./dateUtils";
  import EventBlock from "./EventBlock.svelte";
  import {
    sameDay,
    startOfDay,
    dayStartMs,
    segmentFor,
    chipTextFor,
    topChipsOn,
    monthChipsOn,
    layoutMetrics,
    pendingFirst,
    type Segment,
  } from "./taskDayLogic";
  import { studiesOnDay, studySegment, type StudySession } from "./studyLogic";
  import TaskCheck from "./TaskCheck.svelte";

  let {
    view,
    date,
    onSelectDate,
  }: { view: "mes" | "semana" | "dia"; date: Date; onSelectDate: (d: Date) => void } = $props();

  const tasks = $derived(tasksStore());
  const studies = $derived(studySessions());

  /**
   * Las sesiones de estudio comparten el MISMO sistema de layout que las
   * tareas (segmentFor + layoutMetrics) vía el adapter studyTaskLike, pero
   * viven en la lista `studies` (entidad propia, no son tareas).
   */
  interface StudyPlaced {
    s: StudySession;
    seg: Segment;
    top: number;
    height: number;
    left: number;
    width: number;
  }
  function layoutStudies(d: Date): StudyPlaced[] {
    const items: { s: StudySession; seg: Segment; sMin: number; eMin: number }[] = [];
    for (const s of studiesOnDay(studies, d)) {
      const seg = studySegment(s, d);
      if (!seg) continue;
      const ref = dayStartMs(seg.start);
      items.push({
        s,
        seg,
        sMin: (seg.start.getTime() - ref) / 60_000,
        eMin: (seg.end.getTime() - ref) / 60_000,
      });
    }
    items.sort((a, b) => a.sMin - b.sMin || b.eMin - a.eMin);
    // Columnas: mismo algoritmo de clústeres que las tareas para que una
    // sesión y una tarea solapadas se repartan lado a lado.
    const clusters: typeof items[] = [];
    let cur: typeof items = [];
    let curMaxEnd = -1;
    for (const it of items) {
      if (cur.length === 0 || it.sMin < curMaxEnd) {
        cur.push(it);
        curMaxEnd = Math.max(curMaxEnd, it.eMin);
      } else {
        clusters.push(cur);
        cur = [it];
        curMaxEnd = it.eMin;
      }
    }
    if (cur.length) clusters.push(cur);
    const placed: StudyPlaced[] = [];
    for (const group of clusters) {
      const n = group.length;
      const cols: { e: number }[] = [];
      for (const it of group) {
        let ci = cols.findIndex((c) => it.sMin >= c.e);
        if (ci === -1) {
          ci = cols.length;
          cols.push({ e: it.eMin });
        } else {
          cols[ci].e = Math.max(cols[ci].e, it.eMin);
        }
        const { top, height } = layoutMetrics(it.seg, grid.lo, grid.hi, pxH);
        placed.push({ s: it.s, seg: it.seg, top, height, left: (ci / n) * 100, width: 100 / n });
      }
    }
    return placed;
  }
  const studyLayouts = $derived.by(() => {
    const m = new Map<string, StudyPlaced[]>();
    if (view === "mes") return m; // regla 17: NO se representan en el mes
    for (const d of days) m.set(d.toDateString(), layoutStudies(d));
    return m;
  });

  /** ¿La sesión solapa una clase activa en el día d? (borde de conflicto) */
  function studyClashesWithClass(s: StudySession, d: Date): boolean {
    if (view === "mes") return false;
    const midnight = new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
    const st = Math.max(midnight, s.start.getTime());
    const en = Math.min(midnight + DAY_MS - 1, s.end.getTime());
    return en > st && classInstancesOf(d).some((i) => st < i.end_at && en > i.start_at);
  }

  // ---- utilidades ----
  const isToday = $derived((d: Date) => sameDay(d, new Date()));

  function weekDays(anchor: Date): Date[] {
    const s = startOfDay(anchor);
    const dow = (s.getDay() + 6) % 7; // lunes = 0
    s.setDate(s.getDate() - dow);
    return Array.from({ length: 7 }, (_, i) => {
      const d = new Date(s);
      d.setDate(s.getDate() + i);
      return d;
    });
  }

  const days = $derived(view === "semana" ? weekDays(date) : [startOfDay(date)]);

  const DEFAULT_START = 6;
  const DEFAULT_END = 22;

  /**
   * Altura real del área horaria medida con ResizeObserver → px por hora dinámico.
   * La cuadrícula siempre llega al borde inferior de la ventana, sin huecos;
   * en ventanas bajas el área nunca cae por debajo de su mínimo (28 px/hora)
   * y el scroll aparece solo en `.week-body`.
   */
  let timeAreaH = $state(0);
  $effect(() => {
    const el = dayEls[0];
    if (!el) return;
    const ro = new ResizeObserver(() => {
      timeAreaH = el.clientHeight;
    });
    ro.observe(el);
    timeAreaH = el.clientHeight;
    return () => ro.disconnect();
  });

  /**
   * Franja horaria visible: por defecto 6:00–22:00.
   * Si alguna tarea ocurre fuera, se expande dinámicamente (solo ese día).
   */
  const grid = $derived.by(() => {
    let lo = DEFAULT_START;
    let hi = DEFAULT_END;
    for (const d of days) {
      for (const t of tasks) {
        if (t.allDay) continue;
        const seg = segmentFor(t, d);
        if (!seg) continue;
        lo = Math.min(lo, seg.start.getHours());
        // minutos RELATIVOS al día del inicio del segmento (igual que layoutMetrics):
        // un fin a medianoche = 1440 min, no 0 (bug de expansión de cuadrícula)
        const ref = dayStartMs(seg.start);
        const endMin = (seg.end.getTime() - ref) / 60_000;
        hi = Math.max(hi, Math.min(24, Math.ceil(endMin / 60)));
      }
    }
    lo = Math.max(0, lo);
    hi = Math.min(24, hi);
    if (hi <= lo) hi = lo + 1;
    return { lo, hi };
  });

  const hours = $derived(Array.from({ length: grid.hi - grid.lo + 1 }, (_, i) => grid.lo + i));
  // declarados tras grid/hours (antes se usaban antes de su declaración: TDZ)
  const pxH = $derived(grid.hi > grid.lo && timeAreaH > 0 ? timeAreaH / (grid.hi - grid.lo) : 56);
  const minTimeAreaH = $derived(hours.length * 28);

  const nowInRange = $derived.by(() => {
    const n = new Date();
    const mins = n.getHours() * 60 + n.getMinutes();
    return mins >= grid.lo * 60 && mins <= grid.hi * 60;
  });

  function dayStartOf(d: Date): number {
    return new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
  }
  function nowTop(): number {
    const n = new Date();
    const mins = n.getHours() * 60 + n.getMinutes();
    return (mins - grid.lo * 60) * (pxH / 60);
  }

  interface Placed {
    t: Task;
    seg: Segment;
    top: number;
    height: number;
    left: number;
    width: number;
  }

  /** Algoritmo de columnas: las tareas que se solapan se reparten lado a lado. */
  function layoutDay(d: Date, maxCount = 999): Placed[] {
    const items: { t: Task; seg: Segment; s: number; e: number }[] = [];
    for (const t of tasks) {
      if (t.allDay) continue;
      const seg = segmentFor(t, d);
      if (!seg) continue;
      // minutos relativos al día del inicio del segmento (mismo criterio que
      // layoutMetrics): el fin a medianoche = 1440 min y agrupa en su día real
      const ref = dayStartMs(seg.start);
      const s = (seg.start.getTime() - ref) / 60_000;
      const e = (seg.end.getTime() - ref) / 60_000;
      items.push({ t, seg, s, e });
    }
    items.sort((a, b) => a.s - b.s || b.e - a.e);

    // clústeres: eventos encadenados en el tiempo (la columna del grupo es ancho del grupo)
    const clusters: typeof items[] = [];
    let cur: typeof items = [];
    let curMaxEnd = -1;
    for (const it of items) {
      if (cur.length === 0 || it.s < curMaxEnd) {
        cur.push(it);
        curMaxEnd = Math.max(curMaxEnd, it.e);
      } else {
        clusters.push(cur);
        cur = [it];
        curMaxEnd = it.e;
      }
    }
    if (cur.length) clusters.push(cur);

    const placed: Placed[] = [];
    for (const group of clusters) {
      const n = group.length;
      const cols: { e: number }[] = [];
      for (const it of group) {
        let ci = cols.findIndex((c) => it.s >= c.e);
        if (ci === -1) {
          ci = cols.length;
          cols.push({ e: it.e });
        } else {
          cols[ci].e = Math.max(cols[ci].e, it.e);
        }
        const { top, height } = layoutMetrics(it.seg, grid.lo, grid.hi, pxH);
        placed.push({
          t: it.t,
          seg: it.seg,
          top,
          height,
          left: (ci / n) * 100,
          width: 100 / n,
        });
      }
    }
    return placed.slice(0, maxCount);
  }

  /** Layouts por día cacheados: se calculan una vez por cambio de estado, no por render. */
  const fullLayouts = $derived.by(() => {
    const m = new Map<string, Placed[]>();
    if (view === "mes") return m;
    for (const d of days) m.set(d.toDateString(), layoutDay(d));
    return m;
  });

  /** Recorte a 8 visibles en semana: pendientes primero, conservando el
   *  orden y las posiciones (top/left/width) ya calculados en layoutDay. */
  function visibleWeekPlaced(all: Placed[]): Placed[] {
    return [...all].sort((a, b) => pendingFirst(a.t, b.t)).slice(0, 8);
  }

  /** Layout visible (día → todos; semana → primeros 8 pendientes-primero + botón "+N más"). */
  function placedOf(d: Date): Placed[] {
    const all = fullLayouts.get(d.toDateString()) ?? [];
    return view === "dia" ? all : visibleWeekPlaced(all);
  }

  /**
   * Reglas 8 y 22–25: las clases ACTIVAS (vigencia y día correctos) se ven
   * como franjas tenues bajo las tareas, y las tareas que las invaden
   * después de confirmar muestran un borde de conflicto. Solo en día/semana
   * (regla: las clases NO se muestran en mes).
   */
  const classInstances = $derived(
    view === "mes"
      ? []
      : classConflictsIn(
          classList(),
          days[0].getTime(),
          days[days.length - 1].getTime() + DAY_MS - 1,
        ),
  );
  function classInstancesOf(d: Date) {
    const midnight = new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
    return classInstances.filter((i) => i.date_ms === midnight);
  }
  function stripMetrics(ins: { start_at: number; end_at: number; date_ms: number }) {
    const sMin = (ins.start_at - ins.date_ms) / 60_000;
    const eMin = (ins.end_at - ins.date_ms) / 60_000;
    return {
      top: (sMin / 60 - grid.lo) * pxH,
      height: Math.max(10, ((eMin - sMin) / 60) * pxH),
    };
  }
  /** ¿La tarea p solapa una clase activa en el día d? (borde de conflicto) */
  function taskClashesWithClass(t: Task, d: Date): boolean {
    if (view === "mes" || t.allDay) return false;
    const midnight = new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
    const s = Math.max(midnight, t.start.getTime());
    const e = Math.min(midnight + DAY_MS - 1, t.end.getTime());
    return e > s && classInstancesOf(d).some((i) => s < i.end_at && e > i.start_at);
  }

  function hourLabel(h: number): string {
    if (h === 0) return "12 a";
    if (h === 12) return "12 p";
    return `${h < 12 ? h : h - 12} ${h < 12 ? "a" : "p"}`;
  }
  function fmtTime(d: Date): string {
    return `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
  }

  // ---- mes ----
  function monthCells(): { d: Date; inMonth: boolean }[] {
    const first = new Date(date.getFullYear(), date.getMonth(), 1);
    const start = startOfDay(first);
    const offset = (start.getDay() + 6) % 7;
    start.setDate(start.getDate() - offset);
    return Array.from({ length: 42 }, (_, i) => {
      const d = new Date(start);
      d.setDate(start.getDate() + i);
      return { d, inMonth: d.getMonth() === date.getMonth() };
    });
  }

  let popupDay = $state<Date | null>(null);

  /** Chips de la fila superior (semana/día): todo el día + multi-día intermedio. */
  const topChipsOf = (d: Date) => topChipsOn(tasks, d);
  const visibleTopChipsOf = (d: Date) => topChipsOf(d).slice(0, 3);
  const restTopChipsOf = (d: Date) => Math.max(0, topChipsOf(d).length - 3);

  /** Chips de mes y popup: todo lo que cubre el día. */
  const monthChipsOf = (d: Date) => monthChipsOn(tasks, d);

  /** Borde inferior del último evento visible (para el botón "+N más"). */
  function lastShownBottom(d: Date): number {
    const placed = visibleWeekPlaced(fullLayouts.get(d.toDateString()) ?? []);
    if (placed.length === 0) return (grid.hi - grid.lo) * pxH;
    const last = Math.max(...placed.map((p) => p.top + p.height));
    return Math.min(last + 4, (grid.hi - grid.lo) * pxH);
  }

  // ---- drag & drop + redimensionado (TAREAS y SESIONES comparten máquina) ----
  interface DragState {
    task: Task;
    /** Si no es null, el arrastre mueve una SESIÓN DE ESTUDIO (no la tarea). */
    study: StudySession | null;
    mode: "move" | "resize-start" | "resize-end";
    startAt: number;
    endAt: number;
    /** Desfase del agarre en PÍXELES (cursor − borde superior del evento).
     *  Se guarda en píxeles para que el punto de agarre se conserve exacto
     *  aunque pxH cambie durante el arrastre (redimensionado de ventana). */
    grabY: number;
    curStart: number;
    curEnd: number;
    moved: boolean;
    dropAllDay: boolean;
  }
  let drag = $state<DragState | null>(null);
  let dayEls: HTMLElement[] = [];
  let alldayEls: HTMLElement[] = [];
  let toastMsg = $state("");

  let bodyEl: HTMLElement | null = $state(null);
  $effect(() => {
    if (view !== "mes" && bodyEl) {
      const top = Math.max(0, nowTop() - 40);
      bodyEl.scrollTop = top;
    }
  });

  /** Columna bajo el puntero usando el área horaria (ignora la fila de todo el día). */
  function dayColAt(clientX: number, clientY: number): { el: HTMLElement; day: number } | null {
    for (const el of dayEls) {
      if (!el) continue;
      const r = el.getBoundingClientRect();
      if (clientX >= r.left && clientX < r.right) {
        return { el, day: Number(el.dataset.day) };
      }
    }
    // fuera de columnas pero dentro del cuerpo → día más cercano
    const first = dayEls.find((x) => x);
    if (first && clientY > first.getBoundingClientRect().top) {
      let best: HTMLElement | null = null;
      let bd = Infinity;
      for (const el of dayEls) {
        if (!el) continue;
        const r = el.getBoundingClientRect();
        const d = Math.abs((r.left + r.width / 2) - clientX);
        if (d < bd) {
          bd = d;
          best = el;
        }
      }
      if (best) return { el: best, day: Number(best.dataset.day) };
    }
    return null;
  }

  function alldayHitAt(day: number, clientY: number): boolean {
    const i = days.findIndex((d) => dayStartOf(d) === day);
    const el = alldayEls[i];
    if (!el) return false;
    const r = el.getBoundingClientRect();
    return clientY >= r.top && clientY < r.bottom;
  }

  function onEventPointerDown(
    t: Task,
    mode: "move" | "resize-start" | "resize-end",
    e: PointerEvent,
    study: StudySession | null = null,
  ) {
    if (e.button !== 0 || view === "mes") return;
    e.stopPropagation();
    e.preventDefault();
    const evtEl = e.currentTarget as HTMLElement;
    const rect = evtEl.getBoundingClientRect();
    // Punto de agarre en píxeles: el evento NO se mueve al iniciar, el cursor
    // conserva exactamente la misma posición relativa durante todo el arrastre.
    const grabY = e.clientY - rect.top;
    const from = study ?? t;
    drag = {
      task: t,
      study,
      mode,
      startAt: from.start.getTime(),
      endAt: from.end.getTime(),
      grabY,
      curStart: from.start.getTime(),
      curEnd: from.end.getTime(),
      moved: false,
      dropAllDay: false,
    };
    const onMove = (ev: PointerEvent) => onDragMove(ev);
    const onUp = (ev: PointerEvent) => {
      window.removeEventListener("pointermove", onMove);
      window.removeEventListener("pointerup", onUp);
      window.removeEventListener("pointercancel", onUp);
      try {
        evtEl.releasePointerCapture(ev.pointerId);
      } catch {
        // sin captura
      }
      onDragEnd();
    };
    window.addEventListener("pointermove", onMove);
    window.addEventListener("pointerup", onUp);
    window.addEventListener("pointercancel", onUp);
    try {
      evtEl.setPointerCapture(e.pointerId);
    } catch {
      // captura opcional
    }
  }

  function onDragMove(e: PointerEvent) {
    const d = drag;
    if (!d) return;
    const col = dayColAt(e.clientX, e.clientY);
    if (!col) return;
    const body = bodyEl;
    if (body) {
      const br = body.getBoundingClientRect();
      const edge = 28;
      if (e.clientY < br.top + edge) body.scrollTop -= 14;
      else if (e.clientY > br.bottom - edge) body.scrollTop += 14;
    }
    const rect = col.el.getBoundingClientRect();
    const colMs = col.day;
    // Posición del cursor dentro de la columna, en píxeles y en minutos.
    // La conversión píxel → minuto usa grid.lo como referencia (igual que el
    // render): el borde superior de la cuadrícula es grid.lo, no medianoche.
    const py = e.clientY - rect.top;
    const clampStart = colMs;

    // soltar en la fila "Todo el día" → convierte la tarea en de día completo
    if (d.mode === "move" && alldayHitAt(colMs, e.clientY)) {
      d.dropAllDay = true;
      d.curStart = colMs;
      d.curEnd = colMs;
      if (Math.abs(d.curStart - d.startAt) > 60_000 || Math.abs(d.curEnd - d.endAt) > 60_000) {
        d.moved = true;
      }
      return;
    }
    d.dropAllDay = false;
    if (d.mode === "move") {
      const dur = d.endAt - d.startAt;
      // Borde superior del evento = cursor − punto de agarre (sin redondeo:
      // el evento sigue al cursor con precisión; el snap ocurre al soltar).
      let startMin = grid.lo * 60 + ((py - d.grabY) / pxH) * 60;
      startMin = Math.max(grid.lo * 60, Math.min(startMin, grid.hi * 60 - dur / 60_000));
      d.curStart = clampStart + startMin * 60_000;
      d.curEnd = d.curStart + dur;
    } else if (d.mode === "resize-end") {
      let endMin = grid.lo * 60 + (py / pxH) * 60;
      endMin = Math.max((d.curStart - colMs) / 60_000 + 30, Math.min(endMin, grid.hi * 60));
      d.curEnd = colMs + endMin * 60_000;
    } else {
      let startMin = grid.lo * 60 + (py / pxH) * 60;
      startMin = Math.min(startMin, (d.curEnd - colMs) / 60_000 - 30);
      d.curStart = Math.max(clampStart, colMs + Math.max(grid.lo * 60, startMin) * 60_000);
    }
    if (Math.abs(d.curStart - d.startAt) > 60_000 || Math.abs(d.curEnd - d.endAt) > 60_000) {
      d.moved = true;
    }
  }

  async function onDragEnd() {
    const d = drag;
    if (!d) return;
    const moved = d.moved;
    const dropAllDay = d.dropAllDay && !d.study;
    // El snap a 5 minutos ocurre al soltar, nunca durante el arrastre:
    // así el evento sigue al cursor sin saltos y solo se redondea al persistir.
    let start = d.curStart;
    let end = d.curEnd;
    const dur = d.endAt - d.startAt;
    if (d.mode === "move") {
      start = Math.round(start / 300_000) * 300_000;
      end = start + dur;
    } else if (d.mode === "resize-end") {
      end = Math.max(start + 30 * 60_000, Math.round(end / 300_000) * 300_000);
    } else {
      start = Math.min(end - 30 * 60_000, Math.round(start / 300_000) * 300_000);
    }
    const want = { start, end };
    drag = null;
    if (moved) {
      suppressClick = true;
      setTimeout(() => (suppressClick = false), 0);
    }
    if (!moved || (want.start === d.startAt && want.end === d.endAt && !dropAllDay)) return;
    // SESIÓN DE ESTUDIO: conflicto con clases (regla 10, diálogo propio con
    // "Continuar de todas formas") y aviso de tareas (regla 11). Nunca se
    // mueve automáticamente; una sesión no puede soltarse en "Todo el día".
    if (d.study) {
      const g = await guardStudyConflicts(d.study, want.start, want.end);
      if (g.result === "aborted") return;
      const r = await moveStudy(d.study.id, g.startAt, g.endAt);
      if (!r.ok) {
        toastMsg = `No se pudo mover: ${r.error ?? "horario no válido"}`;
        setTimeout(() => (toastMsg = ""), 4000);
      }
      return;
    }
    // regla 22: las clases del horario también preguntan al soltar
    if (!dropAllDay && d.mode !== "resize-start") {
      const g = await guardClassConflict(d.task.title, want.start, want.end);
      if (g.result === "aborted") return;
      want.start = g.startAt;
      want.end = g.endAt;
    }
    const r = await moveTask(d.task.id, want.start, want.end, dropAllDay || undefined);
    if (!r.ok) {
      toastMsg = `No se pudo mover: ${r.error ?? "conflicto de horario"}`;
      setTimeout(() => (toastMsg = ""), 4000);
    } else if (r.conflict) {
      toastMsg = `Movida con aviso: se solapa con «${r.conflict}»`;
      setTimeout(() => (toastMsg = ""), 4000);
    }
  }

  function openFromCard(t: Task) {
    if (!drag && !suppressClick) openTaskDetail(t);
  }
  /** Click en una sesión: abre SU editor/visor (no el de la tarea). */
  function openFromStudyCard(s: StudySession) {
    if (!drag && !suppressClick) openStudyDetail(s.id);
  }

  let suppressClick = $state(false);

  const ghostSeg = $derived(
    drag
      ? segmentFor({ ...drag.task, start: new Date(drag.curStart), end: new Date(drag.curEnd) }, new Date(drag.curStart))
      : null,
  );
  /** Color del fantasma: el de la categoría o el de las sesiones de estudio. */
  const ghostColor = $derived(drag?.study ? "var(--study)" : drag ? cat(drag.task.categoryId).color : "#888");
  function ghostTop(): number {
    if (!drag || !ghostSeg) return 0;
    return layoutMetrics(ghostSeg, grid.lo, grid.hi, pxH).top;
  }
  function ghostHeight(): number {
    if (!drag || !ghostSeg) return 0;
    return layoutMetrics(ghostSeg, grid.lo, grid.hi, pxH).height;
  }
</script>

<div class="cal" class:week={view === "semana"} class:day={view === "dia"}>
  {#if view === "mes"}
    <div class="month-head">
      {#each Array.from({ length: 7 }, (_, i) => i) as i}
        <span>{DAYS_ES[(i + 1) % 7]}</span>
      {/each}
    </div>
    <div class="month-grid">
      {#each monthCells() as { d, inMonth } (d.toDateString())}
        <div
          class="cell {inMonth ? '' : 'outside'} {isToday(d) ? 'today' : ''}"
          role="button" tabindex="0"
          onclick={() => (popupDay = d)}
          onkeydown={(e) => { if (e.key === "Enter" || e.key === " ") { e.preventDefault(); popupDay = d; } }}
        >
          <span class="daynum">{d.getDate()}</span>
          <div class="chips">
            {#each monthChipsOf(d).slice(0, 2) as t (t.id)}
              <button
                type="button"
                class="minichip {t.status === 'completada' ? 'done' : ''}"
                style="--c: {cat(t.categoryId).color}"
                title={t.title}
                onclick={(e) => { e.stopPropagation(); openTaskDetail(t); }}
              >{chipTextFor(t, d)}</button>
            {/each}
            {#if monthChipsOf(d).length > 2}
              <button
                type="button"
                class="more"
                onclick={(e) => { e.stopPropagation(); popupDay = d; }}
              >+{monthChipsOf(d).length - 2} más</button>
            {/if}
          </div>
        </div>
      {/each}
    </div>

    {#if popupDay}
        <div class="day-popup">
        <div class="pop-head">
          <strong>{capitalizeFirst(popupDay.toLocaleDateString("es-ES", { weekday: "long", day: "numeric", month: "long" }))}</strong>
          <button class="pop-close" onclick={() => (popupDay = null)} aria-label="Cerrar">✕</button>
        </div>
        <div class="pop-list">
          {#if monthChipsOf(popupDay).length === 0}
            <p class="pop-empty">Sin tareas este día.</p>
          {/if}
          {#each monthChipsOf(popupDay) as t (t.id)}
            <div class="chip-wrap" class:done={t.status === "completada"}>
              <span class="check-slot"><TaskCheck task={t} /></span>
              <button class="pop-item" style="--c: {cat(t.categoryId).color}" onclick={() => { openTaskDetail(t); popupDay = null; }}>
                <span class="pop-dot"></span>
                <span class="pop-title {t.status === 'completada' ? 'done' : ''}"><span class="strike">{chipTextFor(t, popupDay)}</span></span>
                <span class="pop-time">
                  {t.allDay ? "Todo el día" : `${fmtTime(t.start)}–${fmtTime(t.end)}`}
                </span>
              </button>
            </div>
          {/each}
        </div>
        <button class="pop-go" onclick={() => { if (popupDay) onSelectDate(popupDay); popupDay = null; }}>
          Ver día completo →
        </button>
      </div>
    {/if}
  {:else}
    <div class="week-head">
      <span class="gutter-spacer"></span>
      {#each days as d (d.toDateString())}
        <button class="day-head {isToday(d) ? 'today' : ''}" onclick={() => onSelectDate(d)}>
          <span class="dow">{DAYS_ES[d.getDay()]}</span>
          <span class="num">{d.getDate()}</span>
        </button>
      {/each}
    </div>
    <div class="week-body" bind:this={bodyEl} class:dragging={!!drag}>
      <div class="gutter">
        <div class="allday-spacer"></div>
        <div class="hours-area">
          {#each hours as h}
            <span class="hour" style="top: {(h - grid.lo) * pxH}px">{hourLabel(h)}</span>
          {/each}
        </div>
      </div>
      {#each days as d, di (d.toDateString())}
        <div class="day-col {isToday(d) ? 'today' : ''}">
          <div class="allday-row" bind:this={alldayEls[di]}>
            {#if view !== "semana"}<span class="allday-label">Todo el día</span>{/if}
            {#each visibleTopChipsOf(d) as t (t.id)}
              <div class="chip-wrap" class:done={t.status === "completada"}>
                <button
                  type="button"
                  class="allday-chip {!t.allDay ? 'cont' : ''} {t.status === 'completada' ? 'done' : ''}"
                  style="--c: {cat(t.categoryId).color}"
                  title={sameDay(t.start, t.end)
                    ? t.title
                    : `${t.title} (del ${t.start.toLocaleDateString("es-ES", { day: "numeric", month: "short" })} al ${t.end.toLocaleDateString("es-ES", { day: "numeric", month: "short" })})`}
                  onclick={() => openTaskDetail(t)}
                ><span class="strike">{chipTextFor(t, d)}</span></button>
                <span class="check-slot"><TaskCheck task={t} size={14} /></span>
              </div>
            {/each}
            {#if restTopChipsOf(d) > 0}
              <span class="allday-more">+{restTopChipsOf(d)}</span>
            {/if}
            {#if drag && drag.dropAllDay && sameDay(d, new Date(drag.curStart))}
              <span class="allday-chip ghost" style="--c: {cat(drag.task.categoryId).color}">{drag.task.title}</span>
            {/if}
          </div>
          <div
            class="time-area"
            bind:this={dayEls[di]}
            data-day={new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime()}
            style="min-height: {minTimeAreaH}px"
          >
            <div class="slots">
              {#each hours.slice(0, -1) as h}
                <div class="slot"></div>
              {/each}
            </div>
            {#if isToday(d) && nowInRange}
              <div class="now-line" style="top: {nowTop()}px"></div>
            {/if}
            {#each classInstancesOf(d) as ci (ci.class_id + "-" + ci.date_ms)}
              <div class="class-strip" style="top: {stripMetrics(ci).top}px; height: {stripMetrics(ci).height}px">
                <span class="class-strip-label">{ci.title} · {fmtTime(new Date(ci.start_at))}–{fmtTime(new Date(ci.end_at))}</span>
              </div>
            {/each}
            {#each placedOf(d) as p (p.t.id)}
              <EventBlock
                task={p.t}
                seg={p.seg}
                top={p.top}
                height={p.height}
                left={p.left}
                width={p.width}
                conflict={taskClashesWithClass(p.t, d)}
                onPointerDown={onEventPointerDown}
                onClick={openFromCard}
              />
            {/each}
            {#each (studyLayouts.get(d.toDateString()) ?? []) as sp (sp.s.id)}
              <EventBlock
                task={sp.s.taskId != null
                  ? (tasks.find((t) => t.id === sp.s.taskId) ?? {
                      id: sp.s.taskId, title: "", start: sp.s.start, end: sp.s.end,
                      allDay: false, status: "pendiente", priority: "media",
                      categoryId: "otr", description: "",
                    })
                  : {
                      id: -1, title: "", start: sp.s.start, end: sp.s.end,
                      allDay: false, status: "pendiente", priority: "media",
                      categoryId: "otr", description: "",
                    }}
                seg={sp.seg}
                top={sp.top}
                height={sp.height}
                left={sp.left}
                width={sp.width}
                conflict={studyClashesWithClass(sp.s, d)}
                study={sp.s}
                onPointerDown={(t, mode, e) => onEventPointerDown(t, mode, e, sp.s)}
                onClick={() => openFromStudyCard(sp.s)}
              />
            {/each}
            {#if view === "semana" && (fullLayouts.get(d.toDateString())?.length ?? 0) > 8}
              <button class="more-evts" style="top: {lastShownBottom(d)}px" onclick={() => onSelectDate(d)}>
                +{(fullLayouts.get(d.toDateString())?.length ?? 0) - 8} más
              </button>
            {/if}
            {#if drag && !drag.dropAllDay && sameDay(d, new Date(drag.curStart)) && ghostSeg}
              <div
                class="evt ghost {drag.mode}"
                style="top: {ghostTop()}px; height: {ghostHeight()}px; left: 6px; right: 6px; --c: {ghostColor}"
              >
                <span class="evt-time">
                  {#if drag.study}
                    <span class="study-tag" aria-hidden="true">
                      <svg width="9" height="9" viewBox="0 0 24 24" fill="none"><path d="M4 5.5A2.5 2.5 0 0 1 6.5 3H19V19H6.5A2.5 2.5 0 0 0 4 21.5V5.5Z" stroke="currentColor" stroke-width="2.5" stroke-linejoin="round"/></svg>
                    </span>
                  {/if}
                  {fmtTime(ghostSeg.start)} – {fmtTime(ghostSeg.end)}
                </span>
                <span class="evt-title">{drag.study?.title ?? drag.task.title}</span>
              </div>
            {/if}
          </div>
        </div>
      {/each}
    </div>
    {#if toastMsg}
      <div class="drag-toast">{toastMsg}</div>
    {/if}
  {/if}
</div>

<style>
  .cal {
    background: var(--surface);
    border-radius: var(--r-xl);
    box-shadow: var(--shadow-raised-lg);
    overflow: hidden;
    min-width: 0;
    display: flex;
    flex-direction: column;
    min-height: 0;
    position: relative;
  }
  .cal.week {
    height: 100%;
  }

  /* ---------- mes ---------- */
  .month-head {
    display: grid;
    grid-template-columns: repeat(7, 1fr);
    padding: var(--s-4) var(--s-4) var(--s-2);
    font-size: var(--fs-xs);
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--text-3);
    text-align: center;
    flex-shrink: 0;
  }
  .month-grid {
    /* 6 filas de altura flexible (mínimo 80px por celda): llenan el alto de
       la tarjeta si sobra y, si la ventana no alcanza, scrollea .content
       junto con la TopBar. */
    display: grid;
    grid-template-columns: repeat(7, 1fr);
    grid-template-rows: repeat(6, minmax(80px, 1fr));
    flex: 1;
    min-height: 0;
    /* contenedor: un escalón por encima del gap base (aire) */
    gap: var(--s-2);
    padding: 0 var(--s-4) var(--s-4);
  }
  .cell {
    background: var(--surface-2);
    border: none;
    border-radius: var(--r-md);
    box-shadow: var(--shadow-inset-sm);
    padding: var(--s-1) var(--s-1);
    text-align: left;
    display: flex;
    flex-direction: column;
    gap: var(--s-0_5);
    transition: transform var(--dur-fast) var(--ease-out), box-shadow var(--dur-fast) var(--ease-out);
    overflow: hidden;
    min-width: 0;
    min-height: 0;
  }
  .cell:hover {
    transform: translateY(-1px);
    box-shadow: var(--shadow-inset-sm);
  }
  .cell.outside {
    opacity: 0.4;
  }
  .cell.today {
    /* anillo de acento, sin relleno duro */
    outline: 1.5px solid var(--primary);
    outline-offset: -1.5px;
  }
  .daynum {
    font-size: var(--fs-xs);
    font-weight: 600;
    color: var(--text-2);
    width: 18px;
    height: 18px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border-radius: var(--r-full);
    flex-shrink: 0;
  }
  .cell.today .daynum {
    background: var(--grad-accent);
    color: #fff;
    box-shadow: var(--glow-accent);
  }
  .chips {
    display: flex;
    flex-direction: column;
    gap: var(--s-0_5);
    overflow: hidden;
    min-height: 0;
  }
  .minichip {
    font-size: var(--fs-2xs);
    font-weight: 500;
    line-height: 1.25;
    color: color-mix(in srgb, var(--c) 60%, var(--text-1));
    background: color-mix(in srgb, var(--c) 13%, var(--surface));
    border: none;
    border-radius: var(--r-full);
    padding: 1px var(--s-1_5);
    box-shadow: 2px 2px 5px var(--neu-dark), -2px -2px 5px var(--neu-light);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    /* si el espacio escasea, se truncan los chips ANTES que el "+N más" */
    flex-shrink: 1;
    min-height: 0;
  }
  .minichip.done {
    background: var(--surface-2);
    color: var(--text-2);
    text-decoration: line-through;
    text-decoration-color: var(--text-3);
    box-shadow: var(--shadow-inset-sm);
  }
  .more {
    font-size: var(--fs-2xs);
    font-weight: 600;
    color: var(--primary);
    background: var(--primary-soft);
    border: none;
    border-radius: var(--r-full);
    padding: 1px var(--s-2);
    flex-shrink: 0;
    cursor: pointer;
    transition: all var(--dur-fast) var(--ease-out);
    align-self: flex-start;
    font-family: inherit;
  }
  .more:hover {
    background: var(--primary-soft-2);
  }

  /* popup del día (mes) */
  .day-popup {
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    width: min(380px, calc(100% - 48px));
    max-height: 70%;
    background: var(--surface);
    border-radius: var(--r-xl);
    box-shadow: var(--e3);
    border: 1px solid var(--border);
    /* contenedor: padding y gaps un escalón por encima (aire) */
    padding: var(--s-6);
    display: flex;
    flex-direction: column;
    gap: var(--s-4);
    z-index: 50;
    overflow: hidden;
  }
  .pop-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--s-4);
  }
  .pop-close {
    width: 30px;
    height: 30px;
    border: none;
    background: var(--surface-2);
    color: var(--text-2);
    border-radius: var(--r-sm);
    font-size: var(--fs-base);
    transition: all var(--dur-fast) var(--ease-out);
    flex-shrink: 0;
  }
  .pop-close:hover {
    color: var(--danger);
    background: var(--danger-bg);
  }
  .pop-list {
    display: flex;
    flex-direction: column;
    gap: var(--s-2);
    overflow-y: auto;
    min-height: 0;
  }
  .pop-empty {
    color: var(--text-3);
    font-size: var(--fs-base);
    text-align: center;
    margin: var(--s-3) 0;
  }
  /* Wrap del popup: check a la izquierda EN FLUJO (reserva su espacio aunque
     esté oculto) y la fila ocupa el resto */
  .pop-list .chip-wrap {
    display: flex;
    align-items: center;
    gap: var(--s-2);
    min-width: 0;
  }
  .pop-list .check-slot {
    display: inline-grid;
    place-items: center;
    flex-shrink: 0;
    opacity: 0;
    transition: opacity var(--dur-fast) var(--ease-out);
  }
  .pop-list .chip-wrap:hover .check-slot,
  .pop-list .chip-wrap:focus-within .check-slot,
  .pop-list .chip-wrap.done .check-slot {
    opacity: 1;
  }
  .pop-item {
    display: flex;
    align-items: center;
    gap: var(--s-2);
    background: var(--surface-2);
    border-radius: var(--r-full);
    box-shadow: 2px 2px 5px var(--neu-dark), -2px -2px 5px var(--neu-light);
    padding: var(--s-2) var(--s-3);
    font-size: var(--fs-base);
    border: none;
    flex: 1;
    min-width: 0;
    color: inherit;
    text-align: left;
    cursor: pointer;
    transition: background var(--dur-fast) var(--ease-out), box-shadow var(--dur-fast) var(--ease-out);
  }
  .pop-item:hover {
    background: var(--surface-3);
  }
  .pop-list .chip-wrap.done .pop-item {
    box-shadow: var(--shadow-inset-sm);
  }
  .pop-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--c);
    flex-shrink: 0;
    transition: background var(--dur-slow) var(--ease-out);
  }
  .pop-title {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    transition: color var(--dur-slow) var(--ease-out);
  }
  .pop-title.done {
    color: var(--text-2);
  }
  .pop-list .chip-wrap.done .pop-dot {
    background: var(--text-3);
  }
  .pop-list .chip-wrap.done .pop-time {
    color: var(--text-3);
  }
  .pop-time {
    font-size: var(--fs-xs);
    color: var(--text-3);
    font-variant-numeric: tabular-nums;
    flex-shrink: 0;
  }
  .pop-go {
    border: none;
    background: var(--primary);
    color: #fff;
    border-radius: var(--r-sm);
    padding: var(--s-2);
    font-size: var(--fs-base);
    font-weight: 600;
    transition: all var(--dur-fast) var(--ease-out);
  }
  .pop-go:hover {
    background: var(--primary-hover);
  }

  /* ---------- semana / día ---------- */
  .week-head {
    display: flex;
    padding: var(--s-4) var(--s-4) var(--s-2);
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
    border-radius: var(--r-md);
    padding: var(--s-1_5) 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--s-0_5);
    transition: background var(--dur-fast) var(--ease-out);
    min-width: 0;
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
    border-radius: var(--r-full);
    color: var(--text-1);
  }

  .week-body {
    flex: 1;
    display: flex;
    overflow-y: auto;
    overflow-x: hidden;
    padding: 0 var(--s-4) var(--s-4);
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
  /* Espejo de la fila "Todo el día" para que las etiquetas de hora queden
     alineadas con la cuadrícula (mismas medidas que .allday-row). */
  .allday-spacer {
    flex-shrink: 0;
    min-height: 30px;
    padding: var(--s-1) var(--s-1);
    border-bottom: 1px solid transparent;
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
    border-radius: var(--r-md);
    min-width: 0;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }
  .day-col.today {
    /* resaltado sutil dentro del pozo, sin líneas duras */
    background: color-mix(in srgb, var(--accent) 6%, transparent);
  }
  .time-area {
    position: relative;
    flex: 1;
    min-height: 0;
    /* cuadrícula hundida (pozo) */
    background: var(--surface-2);
    box-shadow: var(--shadow-inset-sm);
  }
  .allday-row {
    display: flex;
    align-items: center;
    gap: var(--s-1);
    padding: var(--s-1) var(--s-1);
    min-height: 30px;
    border-bottom: 1px solid var(--border);
    background: var(--surface-2);
    box-shadow: var(--shadow-inset-sm);
    border-radius: var(--r-sm) var(--r-sm) 0 0;
    flex-shrink: 0;
    overflow: hidden;
  }
  .week-body.dragging .allday-row {
    border-bottom-color: var(--primary-soft-2);
    background: color-mix(in srgb, var(--primary-soft) 55%, var(--surface-2));
  }
  .allday-label {
    font-size: var(--fs-2xs);
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--text-3);
    margin-right: var(--s-0_5);
    flex-shrink: 0;
  }
  .allday-chip {
    font-size: var(--fs-2xs);
    font-weight: 600;
    color: color-mix(in srgb, var(--c) 60%, var(--text-1));
    background: color-mix(in srgb, var(--c) 14%, var(--surface));
    border: none;
    border-radius: var(--r-full);
    padding: var(--s-0_5) var(--s-2);
    box-shadow: 2px 2px 5px var(--neu-dark), -2px -2px 5px var(--neu-light);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 100%;
    font-family: inherit;
    cursor: pointer;
    flex-shrink: 1;
    min-width: 0;
    transition:
      filter var(--dur-fast) var(--ease-out),
      transform var(--dur-fast) var(--ease-out),
      background var(--dur-slow) var(--ease-out),
      color var(--dur-slow) var(--ease-out),
      border-color var(--dur-slow) var(--ease-out);
  }
  .allday-chip:hover {
    filter: brightness(1.06);
    transform: translateY(-1px);
  }
  .allday-chip.ghost {
    pointer-events: none;
    opacity: 0.55;
    border-left: 2px dashed color-mix(in srgb, var(--c) 70%, transparent);
  }
  .allday-chip.cont {
    border: 1px dashed color-mix(in srgb, var(--c) 45%, transparent);
    background: color-mix(in srgb, var(--c) 8%, var(--surface));
  }
  /* Completada: hundida gris, sin hover-lift. Sin borde (el chip base no lo
     tiene): el tamaño no cambia respecto al pendiente; solo .cont lo lleva. */
  .allday-chip.done {
    background: var(--surface-2);
    color: var(--text-2);
    box-shadow: var(--shadow-inset-sm);
  }
  .allday-chip.done.cont {
    border: 1px dashed color-mix(in srgb, var(--text-3) 40%, transparent);
  }
  .allday-chip.done:hover {
    filter: none;
    transform: none;
  }
  /* En completadas el check está siempre visible y tapa el final del texto:
     reserva su espacio (en pendientes el check es solo hover) */
  .chip-wrap.done .allday-chip {
    padding-right: var(--s-5);
  }
  /* Tachado animado izq→der (mismo lenguaje que EventBlock) */
  .strike {
    background: linear-gradient(currentColor, currentColor) no-repeat 0 55% / 0% 1.5px;
    transition: background-size var(--dur-slow) var(--ease-out);
  }
  .allday-chip.done .strike,
  .pop-title.done .strike {
    background-size: 100% 1.5px;
  }
  /* Wrap del chip + check: ocupa el mismo hueco flex que ocupaba el chip */
  .allday-row .chip-wrap {
    position: relative;
    display: flex;
    align-items: center;
    min-width: 0;
    flex-shrink: 1;
    max-width: 100%;
  }
  .allday-row .check-slot {
    position: absolute;
    right: 3px;
    top: 50%;
    transform: translateY(-50%);
    z-index: 1;
    opacity: 0;
    transition: opacity var(--dur-fast) var(--ease-out);
  }
  .allday-row .chip-wrap:hover .check-slot,
  .allday-row .chip-wrap:focus-within .check-slot,
  .allday-row .chip-wrap.done .check-slot {
    opacity: 1;
  }
  .allday-more {
    font-size: var(--fs-2xs);
    font-weight: 600;
    color: var(--text-3);
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
    background: var(--primary);
    border-radius: var(--r-full);
    z-index: 2;
    pointer-events: none;
  }
  /* Regla 8: franja tenue de clase bajo las tareas (día/semana) */
  .class-strip {
    position: absolute;
    left: 2px;
    right: 2px;
    background: var(--primary-soft);
    border-left: 2px solid var(--primary);
    border-radius: var(--r-xs);
    z-index: 0;
    pointer-events: none;
    overflow: hidden;
  }
  .class-strip-label {
    display: block;
    padding: 1px var(--s-1_5);
    font-size: var(--fs-2xs);
    font-weight: 600;
    color: var(--primary);
    opacity: 0.75;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .now-line::after {
    content: "";
    position: absolute;
    left: -3px;
    top: -3px;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--primary);
  }
  .evt {
    position: absolute;
    background: color-mix(in srgb, var(--c) 13%, var(--surface));
    border-left: 3px solid var(--c);
    border-radius: var(--r-sm);
    padding: var(--s-1) var(--s-2);
    display: flex;
    flex-direction: column;
    gap: 1px;
    overflow: hidden;
    z-index: 1;
    box-shadow: var(--shadow-inset-sm);
    transition: transform var(--dur-fast) var(--ease-out), box-shadow var(--dur-fast) var(--ease-out);
    min-width: 0;
    cursor: pointer;
    touch-action: none;
  }
  .evt:hover {
    transform: translateY(-1px) scale(1.01);
    box-shadow: var(--e1);
    z-index: 3;
  }
  .evt.overdue {
    border-left-style: dashed;
    opacity: 0.75;
  }
  .evt.ghost {
    pointer-events: none;
    opacity: 0.55;
    border-left-style: dashed;
    z-index: 4;
    transition: none;
    will-change: top, height;
  }
  .drag-toast {
    position: absolute;
    left: 50%;
    bottom: 24px;
    transform: translateX(-50%);
    background: var(--danger);
    color: #fff;
    padding: var(--s-2) var(--s-4);
    border-radius: var(--r-sm);
    font-size: var(--fs-base);
    font-weight: 600;
    box-shadow: var(--e2);
    z-index: 60;
  }
  .week-body.dragging {
    cursor: grabbing;
  }
  .week-body.dragging .day-col,
  .week-body.dragging .evt {
    cursor: grabbing;
  }
  /* Durante el arrastre: sin hover transform (desfasaría el punto de agarre)
     y original atenuado para que el fantasma sea la única referencia visual. */
  :global(.week-body.dragging .evt) {
    transform: none !important;
    transition: none;
  }
  :global(.week-body.dragging .evt:not(.ghost)) {
    opacity: 0.3;
  }
  .evt.inicio,
  .evt.fin {
    background: color-mix(in srgb, var(--c) 18%, var(--surface));
  }
  .evt-time {
    font-size: var(--fs-2xs);
    font-weight: 600;
    color: var(--text-2);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  .evt-title {
    font-size: var(--fs-xs);
    font-weight: 600;
    color: var(--text-1);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    min-width: 0;
  }
  .more-evts {
    position: absolute;
    left: 6px;
    right: 6px;
    border: none;
    background: var(--surface);
    color: var(--text-2);
    font-size: var(--fs-2xs);
    font-weight: 600;
    border-radius: var(--r-full);
    box-shadow: 2px 2px 5px var(--neu-dark), -2px -2px 5px var(--neu-light);
    padding: var(--s-1) 0;
    z-index: 4;
    transition: box-shadow var(--dur-fast) var(--ease-out), color var(--dur-fast) var(--ease-out);
  }
  .more-evts:hover {
    color: var(--primary);
    box-shadow: var(--btn-shadow-hover);
  }
</style>
