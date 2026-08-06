<script lang="ts">
  import { DAYS_ES, MONTHS_ES, tasks as tasksStore, cat, openTaskDetail, moveTask, type Task } from "./data.svelte";
  import EventBlock from "./EventBlock.svelte";

  let {
    view,
    date,
    onSelectDate,
  }: { view: "mes" | "semana" | "dia"; date: Date; onSelectDate: (d: Date) => void } = $props();

  const tasks = $derived(tasksStore());

  // ---- utilidades ----
  function startOfDay(d: Date): Date {
    const x = new Date(d);
    x.setHours(0, 0, 0, 0);
    return x;
  }
  function sameDay(a: Date, b: Date): boolean {
    return a.toDateString() === b.toDateString();
  }
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
  const pxH = $derived(grid.hi > grid.lo && timeAreaH > 0 ? timeAreaH / (grid.hi - grid.lo) : 56);
  const minTimeAreaH = $derived(hours.length * 28);

  /**
   * Franja horaria visible: por defecto 6:00–22:00.
   * Si alguna tarea ocurre fuera, se expande dinámicamente (solo ese día).
   */
  const grid = $derived.by(() => {
    let lo = DEFAULT_START;
    let hi = DEFAULT_END;
    for (const d of days) {
      for (const t of tasks) {
        if (t.allDay || t.status === "completada") continue;
        const seg = segmentFor(t, d);
        if (!seg) continue;
        lo = Math.min(lo, seg.start.getHours());
        const endMin = seg.end.getHours() * 60 + seg.end.getMinutes();
        hi = Math.max(hi, Math.min(24, Math.ceil(endMin / 60)));
      }
    }
    lo = Math.max(0, lo);
    hi = Math.min(24, hi);
    if (hi <= lo) hi = lo + 1;
    return { lo, hi };
  });

  const hours = $derived(Array.from({ length: grid.hi - grid.lo + 1 }, (_, i) => grid.lo + i));

  const nowInRange = $derived.by(() => {
    const n = new Date();
    const mins = n.getHours() * 60 + n.getMinutes();
    return mins >= grid.lo * 60 && mins <= grid.hi * 60;
  });

  function dayStartOf(d: Date): number {
    return new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
  }
  function topOf(t: Task): number {
    const mins = t.start.getHours() * 60 + t.start.getMinutes();
    return (mins - grid.lo * 60) * (pxH / 60);
  }
  function nowTop(): number {
    const n = new Date();
    const mins = n.getHours() * 60 + n.getMinutes();
    return (mins - grid.lo * 60) * (pxH / 60);
  }

  /** Segmento de una tarea para un día concreto (multi-día → solo inicio/fin, ~2 h). */
  function segmentFor(t: Task, d: Date): { start: Date; end: Date; kind: "full" | "inicio" | "fin" } | null {
    const dayStart = startOfDay(d);
    const dayEnd = new Date(dayStart.getTime() + 86_400_000);
    const multi = !sameDay(t.start, t.end);
    if (!multi) {
      if (t.end.getTime() <= dayStart.getTime() || t.start.getTime() >= dayEnd.getTime()) return null;
      return { start: t.start, end: t.end, kind: "full" };
    }
    if (sameDay(t.start, d)) {
      const end = new Date(Math.min(t.start.getTime() + 2 * 3_600_000, t.end.getTime()));
      return { start: t.start, end, kind: "inicio" };
    }
    if (sameDay(t.end, d)) {
      const start = new Date(Math.max(t.end.getTime() - 2 * 3_600_000, t.start.getTime()));
      return { start, end: t.end, kind: "fin" };
    }
    return null;
  }

  function dayTasks(d: Date): Task[] {
    return tasks.filter((t) => {
      const s = t.start;
      return s.getFullYear() === d.getFullYear() && s.getMonth() === d.getMonth() && s.getDate() === d.getDate();
    });
  }

  interface Placed {
    t: Task;
    seg: { start: Date; end: Date; kind: "full" | "inicio" | "fin" };
    top: number;
    height: number;
    left: number;
    width: number;
  }

  /** Algoritmo de columnas: las tareas que se solapan se reparten lado a lado. */
  function layoutDay(d: Date, maxCount = 999): Placed[] {
    const items: { t: Task; seg: { start: Date; end: Date; kind: "full" | "inicio" | "fin" }; s: number; e: number }[] = [];
    for (const t of tasks) {
      if (t.status === "completada" || t.allDay) continue;
      const seg = segmentFor(t, d);
      if (!seg) continue;
      const s = seg.start.getHours() * 60 + seg.start.getMinutes();
      const e = seg.end.getHours() * 60 + seg.end.getMinutes();
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
        placed.push({
          t: it.t,
          seg: it.seg,
          top: Math.max(0, (it.s - grid.lo * 60) * (pxH / 60)),
          height: Math.max(
            20,
            Math.min((it.e - grid.lo * 60) * (pxH / 60), (grid.hi - grid.lo) * pxH) -
              Math.max(0, (it.s - grid.lo * 60) * (pxH / 60)),
          ),
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

  /** Layout visible (día → todos; semana → primeros 8 + botón "+N más"). */
  function placedOf(d: Date): Placed[] {
    const all = fullLayouts.get(d.toDateString()) ?? [];
    return view === "dia" ? all : all.slice(0, 8);
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

  const allDayOf = (d: Date) =>
    tasks.filter((t) => t.allDay && sameDay(t.start, d) && t.status !== "completada");
  /** Multi-día en día intermedio (ni inicio ni fin): ocupa todo el día. */
  const continuaOf = (d: Date) => {
    const ds = startOfDay(d).getTime();
    const de = ds + 86_400_000;
    return tasks.filter(
      (t) => t.status !== "completada" && !t.allDay && t.start.getTime() < ds && t.end.getTime() > de,
    );
  };
  const topChipsOf = (d: Date) => [...allDayOf(d), ...continuaOf(d)];
  const visibleTopChipsOf = (d: Date) => topChipsOf(d).slice(0, 3);
  const restTopChipsOf = (d: Date) => Math.max(0, topChipsOf(d).length - 3);

  /** Chips de mes: todo el día + multi-día en curso + tareas del día (sin duplicados). */
  const monthChipsOf = (d: Date) => {
    const seen = new Map<number, Task>();
    for (const t of [...allDayOf(d), ...continuaOf(d), ...dayTasks(d)]) {
      if (t.status !== "completada" && !seen.has(t.id)) seen.set(t.id, t);
    }
    return [...seen.values()];
  };

  /** Borde inferior del último evento visible (para el botón "+N más"). */
  function lastShownBottom(d: Date): number {
    const placed = (fullLayouts.get(d.toDateString()) ?? []).slice(0, 8);
    if (placed.length === 0) return (grid.hi - grid.lo) * pxH;
    const last = Math.max(...placed.map((p) => p.top + p.height));
    return Math.min(last + 4, (grid.hi - grid.lo) * pxH);
  }

  // ---- drag & drop + redimensionado ----
  interface DragState {
    task: Task;
    mode: "move" | "resize-start" | "resize-end";
    startAt: number;
    endAt: number;
    grabMin: number;
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

  function onEventPointerDown(t: Task, mode: "move" | "resize-start" | "resize-end", e: PointerEvent) {
    if (e.button !== 0 || view === "mes") return;
    e.stopPropagation();
    e.preventDefault();
    const evtEl = e.currentTarget as HTMLElement;
    const rect = evtEl.getBoundingClientRect();
    const grabMin = ((e.clientY - rect.top) / pxH) * 60;
    drag = {
      task: t,
      mode,
      startAt: t.start.getTime(),
      endAt: t.end.getTime(),
      grabMin,
      curStart: t.start.getTime(),
      curEnd: t.end.getTime(),
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
    const rect = col.el.getBoundingClientRect();
    const colMs = col.day;
    const spanMin = (grid.hi - grid.lo) * 60;
    let mins = ((e.clientY - rect.top) / pxH) * 60;
    mins = Math.round(mins / 15) * 15;
    mins = Math.max(0, Math.min(mins, spanMin));
    const clampStart = colMs;
    const clampEnd = colMs + spanMin * 60_000;

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
      let ns = colMs + mins * 60_000 - d.grabMin * 60_000;
      ns = Math.max(clampStart, Math.min(ns, clampEnd - dur));
      d.curStart = ns;
      d.curEnd = ns + dur;
    } else if (d.mode === "resize-end") {
      let ne = colMs + mins * 60_000;
      ne = Math.max(d.curStart + 30 * 60_000, Math.min(ne, clampEnd));
      d.curEnd = ne;
    } else {
      let ns = colMs + mins * 60_000;
      ns = Math.min(ns, d.curEnd - 30 * 60_000);
      d.curStart = Math.max(clampStart, ns);
    }
    if (Math.abs(d.curStart - d.startAt) > 60_000 || Math.abs(d.curEnd - d.endAt) > 60_000) {
      d.moved = true;
    }
  }

  async function onDragEnd() {
    const d = drag;
    if (!d) return;
    const moved = d.moved;
    const dropAllDay = d.dropAllDay;
    const want = { start: d.curStart, end: d.curEnd };
    drag = null;
    if (moved) {
      suppressClick = true;
      setTimeout(() => (suppressClick = false), 0);
    }
    if (!moved || (want.start === d.startAt && want.end === d.endAt && !dropAllDay)) return;
    const r = await moveTask(d.task.id, want.start, want.end, dropAllDay || undefined);
    if (!r.ok) {
      toastMsg = `No se pudo mover: ${r.error ?? "conflicto de horario"}`;
      setTimeout(() => (toastMsg = ""), 4000);
    }
  }

  function openFromCard(t: Task) {
    if (!drag && !suppressClick) openTaskDetail(t);
  }

  let suppressClick = $state(false);

  const ghostSeg = $derived(
    drag
      ? segmentFor({ ...drag.task, start: new Date(drag.curStart), end: new Date(drag.curEnd) }, new Date(drag.curStart))
      : null,
  );
  function ghostTop(): number {
    if (!drag || !ghostSeg) return 0;
    return ((ghostSeg.start.getHours() * 60 + ghostSeg.start.getMinutes() - grid.lo * 60) / 60) * pxH;
  }
  function ghostHeight(): number {
    if (!drag || !ghostSeg) return 0;
    return Math.max(26, (ghostSeg.end.getTime() - ghostSeg.start.getTime()) / 3_600_000 * pxH);
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
            {#each monthChipsOf(d).slice(0, 3) as t (t.id)}
              <button
                type="button"
                class="minichip {t.status === 'completada' ? 'done' : ''}"
                style="--c: {cat(t.categoryId).color}"
                title={t.title}
                onclick={(e) => { e.stopPropagation(); openTaskDetail(t); }}
              >{t.title}</button>
            {/each}
            {#if monthChipsOf(d).length > 3}
              <button
                type="button"
                class="more"
                onclick={(e) => { e.stopPropagation(); popupDay = d; }}
              >+{monthChipsOf(d).length - 3} más</button>
            {/if}
          </div>
        </div>
      {/each}
    </div>

    {#if popupDay}
      <div class="day-popup">
        <div class="pop-head">
          <strong>{popupDay.toLocaleDateString("es-ES", { weekday: "long", day: "numeric", month: "long" })}</strong>
          <button class="pop-close" onclick={() => (popupDay = null)} aria-label="Cerrar">✕</button>
        </div>
        <div class="pop-list">
          {#if dayTasks(popupDay).length === 0 && continuaOf(popupDay).length === 0}
            <p class="pop-empty">Sin tareas este día.</p>
          {/if}
          {#each monthChipsOf(popupDay) as t (t.id)}
            <button class="pop-item" style="--c: {cat(t.categoryId).color}" onclick={() => { openTaskDetail(t); popupDay = null; }}>
              <span class="pop-dot"></span>
              <span class="pop-title {t.status === 'completada' ? 'done' : ''}">{t.title}</span>
              <span class="pop-time">
                {t.allDay ? "Todo el día" : `${fmtTime(t.start)}–${fmtTime(t.end)}`}
              </span>
            </button>
          {/each}
        </div>
        <button class="pop-go" onclick={() => { onSelectDate(popupDay); popupDay = null; }}>
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
        {#each hours as h}
          <span class="hour" style="top: {(h - grid.lo) * pxH}px">{hourLabel(h)}</span>
        {/each}
      </div>
      {#each days as d, di (d.toDateString())}
        <div class="day-col {isToday(d) ? 'today' : ''}">
          <div class="allday-row" bind:this={alldayEls[di]}>
            <span class="allday-label">Todo el día</span>
            {#each visibleTopChipsOf(d) as t (t.id)}
              {#if t.allDay}
                <button
                  type="button"
                  class="allday-chip"
                  style="--c: {cat(t.categoryId).color}"
                  title={t.title}
                  onclick={() => openTaskDetail(t)}
                >{t.title}</button>
              {:else}
                <button
                  type="button"
                  class="allday-chip cont"
                  style="--c: {cat(t.categoryId).color}"
                  title={`Continúa · ${t.title} (del ${t.start.toLocaleDateString("es-ES", { day: "numeric", month: "short" })} al ${t.end.toLocaleDateString("es-ES", { day: "numeric", month: "short" })})`}
                  onclick={() => openTaskDetail(t)}
                >⟳ {t.title}</button>
              {/if}
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
            {#each placedOf(d) as p (p.t.id)}
              <EventBlock
                task={p.t}
                seg={p.seg}
                top={p.top}
                height={p.height}
                left={p.left}
                width={p.width}
                onPointerDown={onEventPointerDown}
                onClick={openFromCard}
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
                style="top: {ghostTop()}px; height: {ghostHeight()}px; left: 6px; right: 6px; --c: {cat(drag.task.categoryId).color}"
              >
                <span class="evt-time">
                  {fmtTime(ghostSeg.start)} – {fmtTime(ghostSeg.end)}
                </span>
                <span class="evt-title">{drag.task.title}</span>
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
    border-radius: var(--r-lg);
    box-shadow: var(--shadow-raised);
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
    font-size: 11px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--text-3);
    text-align: center;
    flex-shrink: 0;
  }
  .month-grid {
    flex: 1;
    display: grid;
    grid-template-columns: repeat(7, 1fr);
    grid-template-rows: repeat(6, 1fr);
    min-height: 0;
    gap: 6px;
    padding: 0 var(--s-4) var(--s-4);
  }
  .cell {
    background: var(--surface-2);
    border: none;
    border-radius: var(--r-md);
    padding: 6px 7px;
    text-align: left;
    display: flex;
    flex-direction: column;
    gap: 3px;
    transition: transform var(--dur-fast) var(--ease-out), box-shadow var(--dur-fast) var(--ease-out);
    overflow: hidden;
    min-width: 0;
    min-height: 0;
  }
  .cell:hover {
    transform: translateY(-1px);
    box-shadow: var(--e1);
  }
  .cell.outside {
    opacity: 0.4;
  }
  .cell.today {
    box-shadow: inset 0 0 0 2px var(--primary-soft-2);
  }
  .daynum {
    font-size: 12px;
    font-weight: 600;
    color: var(--text-2);
    width: 24px;
    height: 24px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border-radius: var(--r-full);
    flex-shrink: 0;
  }
  .cell.today .daynum {
    background: var(--primary);
    color: #fff;
  }
  .chips {
    display: flex;
    flex-direction: column;
    gap: 2px;
    overflow: hidden;
    min-height: 0;
  }
  .minichip {
    font-size: 10.5px;
    font-weight: 500;
    color: color-mix(in srgb, var(--c) 60%, var(--text-1));
    background: color-mix(in srgb, var(--c) 13%, var(--surface));
    border-radius: 7px;
    padding: 1.5px 7px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    flex-shrink: 0;
  }
  .minichip.done {
    text-decoration: line-through;
    opacity: 0.55;
  }
  .more {
    font-size: 10px;
    font-weight: 700;
    color: var(--primary);
    background: var(--primary-soft);
    border: none;
    border-radius: var(--r-full);
    padding: 2px 8px;
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
    border-radius: var(--r-lg);
    box-shadow: var(--e3);
    border: 1px solid var(--border);
    padding: var(--s-5);
    display: flex;
    flex-direction: column;
    gap: var(--s-3);
    z-index: 50;
    overflow: hidden;
  }
  .pop-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--s-3);
    text-transform: capitalize;
  }
  .pop-close {
    width: 30px;
    height: 30px;
    border: none;
    background: var(--surface-2);
    color: var(--text-2);
    border-radius: 10px;
    font-size: 13px;
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
    gap: 6px;
    overflow-y: auto;
    min-height: 0;
  }
  .pop-empty {
    color: var(--text-3);
    font-size: 13px;
    text-align: center;
    margin: var(--s-3) 0;
  }
  .pop-item {
    display: flex;
    align-items: center;
    gap: 9px;
    background: var(--surface-2);
    border-radius: 12px;
    padding: 9px 12px;
    font-size: 13px;
    border: none;
    width: 100%;
    color: inherit;
    text-align: left;
    cursor: pointer;
    transition: background var(--dur-fast) var(--ease-out);
  }
  .pop-item:hover {
    background: var(--surface-3);
  }
  .pop-dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--c);
    flex-shrink: 0;
  }
  .pop-title {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .pop-title.done {
    text-decoration: line-through;
    opacity: 0.55;
  }
  .pop-time {
    font-size: 11px;
    color: var(--text-3);
    font-variant-numeric: tabular-nums;
    flex-shrink: 0;
  }
  .pop-go {
    border: none;
    background: var(--primary);
    color: #fff;
    border-radius: 12px;
    padding: 10px;
    font-size: 13px;
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
  }
  .allday-row {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 5px 4px;
    min-height: 30px;
    border-bottom: 1px solid var(--border);
    background: color-mix(in srgb, var(--surface-2) 55%, transparent);
    border-radius: var(--r-sm) var(--r-sm) 0 0;
    flex-shrink: 0;
    overflow: hidden;
  }
  .week-body.dragging .allday-row {
    border-bottom-color: var(--primary-soft-2);
    background: color-mix(in srgb, var(--primary-soft) 55%, var(--surface-2));
  }
  .allday-label {
    font-size: 9px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--text-3);
    margin-right: 2px;
    flex-shrink: 0;
  }
  .allday-chip {
    font-size: 10px;
    font-weight: 600;
    color: color-mix(in srgb, var(--c) 60%, var(--text-1));
    background: color-mix(in srgb, var(--c) 14%, var(--surface));
    border: none;
    border-radius: 7px;
    padding: 2px 7px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 100%;
    font-family: inherit;
    cursor: pointer;
    flex-shrink: 1;
    min-width: 0;
    transition: filter var(--dur-fast) var(--ease-out), transform var(--dur-fast) var(--ease-out);
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
  .allday-more {
    font-size: 10px;
    font-weight: 700;
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
    padding: 3px 7px;
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
  }
  .drag-toast {
    position: absolute;
    left: 50%;
    bottom: 24px;
    transform: translateX(-50%);
    background: var(--danger);
    color: #fff;
    padding: 9px 16px;
    border-radius: 12px;
    font-size: 13px;
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
  .evt.inicio,
  .evt.fin {
    background: color-mix(in srgb, var(--c) 18%, var(--surface));
  }
  .evt-time {
    font-size: 10px;
    font-weight: 700;
    color: var(--text-2);
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }
  .evt-title {
    font-size: 11.5px;
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
    background: var(--surface-3);
    color: var(--text-2);
    font-size: 10.5px;
    font-weight: 700;
    border-radius: 8px;
    padding: 3px 0;
    z-index: 4;
    transition: all var(--dur-fast) var(--ease-out);
  }
  .more-evts:hover {
    color: var(--primary);
    background: var(--primary-soft);
  }
</style>
