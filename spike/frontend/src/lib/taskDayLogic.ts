// Lógica pura de presencia/segmentos de tareas por día.
// ÚNICA fuente de verdad para Calendario (mes/día/semana/popup) y Agenda:
// una tarea "está en un día" si su intervalo [start, end) cubre ese día
// (`coversDay`), con independencia de dónde se renderice luego.
//
// Sin importaciones de Svelte ni DOM: testeable con vitest (env node).

export interface TaskLike {
  id: number;
  title: string;
  start: Date;
  end: Date;
  /** Ausente = no es de día completo (los `Task` reales lo traen opcional). */
  allDay?: boolean;
  status: string;
}

export const DAY_MS = 86_400_000;
export const HOUR_MS = 3_600_000;

/** Inicio (medianoche local, ms) del día que contiene `d`. */
export function dayStartMs(d: Date): number {
  return new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
}

/** Inicio del día como Date (medianoche local). */
export function startOfDay(d: Date): Date {
  return new Date(dayStartMs(d));
}

export function sameDay(a: Date, b: Date): boolean {
  return a.toDateString() === b.toDateString();
}

/** ¿La tarea cubre el día `d`? Semántica [dayStart, dayEnd) unificada. */
export function coversDay(t: TaskLike, d: Date): boolean {
  const ds = dayStartMs(d);
  const de = ds + DAY_MS;
  return t.start.getTime() < de && t.end.getTime() > ds;
}

/** Último día (medianoche local, ms) en que la tarea está presente.
 *  Un fin exactamente a medianoche NO cubre ese día: el último es el anterior. */
export function lastCoveredDayMs(t: TaskLike): number {
  const e = t.end.getTime();
  return e === dayStartMs(t.end) ? e - DAY_MS : e;
}

/** Nº de días que la tarea ocupa (>= 1). Fin a medianoche → se descuenta. */
export function daySpanDays(t: TaskLike): number {
  const s = dayStartMs(t.start);
  const e = lastCoveredDayMs(t);
  // floor: el último día cubierto puede quedar a mitad de día (p. ej. fin a las
  // 14:00 = 2 días + 14 h → 2 días completos + 1 = 3, nunca redondear a 4)
  return Math.max(1, Math.floor((e - s) / DAY_MS) + 1);
}

export function isMultiDay(t: TaskLike): boolean {
  return daySpanDays(t) > 1;
}

/** Tareas activas (no completadas) que cubren el día. Predicado único. */
export function tasksOnDay(tasks: TaskLike[], d: Date): TaskLike[] {
  return tasks.filter((t) => t.status !== "completada" && coversDay(t, d));
}

// ---------------------------------------------------------------------------
// Segmentos del área de tiempo (día/semana)
// ---------------------------------------------------------------------------

export type SegmentKind = "full" | "inicio" | "fin";

export interface Segment {
  start: Date;
  end: Date;
  kind: SegmentKind;
}

/** Segmento horario de la tarea para el día `d` (área de tiempo).
 *  - Mismo día → bloque real [start, end).
 *  - Multi-día, día de inicio → stub de 2 h desde el inicio.
 *  - Multi-día, último día cubierto → stub de 2 h hasta el final.
 *  - Días intermedios → `null` (chip en la fila superior vía `multiDayChipsOn`). */
export function segmentFor(t: TaskLike, d: Date): Segment | null {
  const dayStart = startOfDay(d);
  const dayEnd = dayStart.getTime() + DAY_MS;
  if (!isMultiDay(t)) {
    if (t.end.getTime() <= dayStart.getTime() || t.start.getTime() >= dayEnd) return null;
    return { start: t.start, end: t.end, kind: "full" };
  }
  if (sameDay(t.start, d)) {
    const end = new Date(Math.min(t.start.getTime() + 2 * HOUR_MS, t.end.getTime()));
    return { start: t.start, end, kind: "inicio" };
  }
  const lastDay = new Date(lastCoveredDayMs(t));
  if (sameDay(lastDay, d)) {
    const end = new Date(Math.min(t.end.getTime(), lastDay.getTime() + DAY_MS));
    const start = new Date(Math.max(end.getTime() - 2 * HOUR_MS, t.start.getTime()));
    return { start, end, kind: "fin" };
  }
  return null;
}

// ---------------------------------------------------------------------------
// Chips (fila "Todo el día" y mes)
// ---------------------------------------------------------------------------

/** Chips "Todo el día": tareas de día completo que cubren el día. */
export function allDayChipsOn(tasks: TaskLike[], d: Date): TaskLike[] {
  return tasksOnDay(tasks, d).filter((t) => t.allDay);
}

/** Multi-día con horario (no all-day) en días INTERMEDIOS → chip "cont".
 *  Los días de inicio/fin se representan con su stub en el área de tiempo. */
export function multiDayChipsOn(tasks: TaskLike[], d: Date): TaskLike[] {
  return tasksOnDay(tasks, d).filter(
    (t) => !t.allDay && isMultiDay(t) && !sameDay(t.start, d) && !sameDay(new Date(lastCoveredDayMs(t)), d),
  );
}

/** Chips de la fila superior (semana/día): todo el día + multi-día intermedio. */
export function topChipsOn(tasks: TaskLike[], d: Date): TaskLike[] {
  return [...allDayChipsOn(tasks, d), ...multiDayChipsOn(tasks, d)];
}

/** Tareas de un día para el MES y el POPUP: todo lo que cubre el día. */
export function monthChipsOn(tasks: TaskLike[], d: Date): TaskLike[] {
  return tasksOnDay(tasks, d);
}

/** Texto del chip según el rol del día (único, inicio, fin o medio). */
export function chipTextFor(t: TaskLike, d: Date): string {
  if (!isMultiDay(t)) return t.title;
  const ds = dayStartMs(d);
  const ts = t.start.getTime();
  if (ts >= ds && ts < ds + DAY_MS) return `Inicio · ${t.title}`;
  const lastDay = lastCoveredDayMs(t);
  if (lastDay >= ds && lastDay < ds + DAY_MS) return `Fin · ${t.title}`;
  return t.title;
}

// ---------------------------------------------------------------------------
// Métricas de layout (área de tiempo)
// ---------------------------------------------------------------------------

export interface LayoutMetrics {
  top: number;
  height: number;
}

/** top/height en px dentro del área horaria [gridLo, gridHi). Los minutos se
 *  calculan RELATIVOS al día del inicio del segmento, así un fin a medianoche
 *  (00:00 del día siguiente) = 1440 min y no colapsa a 0 (bug histórico).
 *  La porción visible se recorta a [gridLo, gridHi]: los stubs que se salen
 *  del área dibujan 20 px mínimos en el borde en vez de una barra completa. */
export function layoutMetrics(
  seg: Segment,
  gridLo: number,
  gridHi: number,
  pxPerHour: number,
): LayoutMetrics {
  const ref = dayStartMs(seg.start);
  const sMin = (seg.start.getTime() - ref) / 60_000;
  const eMin = (seg.end.getTime() - ref) / 60_000;
  const lo = gridLo * 60;
  const hi = gridHi * 60;
  const vs = Math.max(sMin, lo);
  const ve = Math.min(eMin, hi);
  const top = (vs - lo) * (pxPerHour / 60);
  const height = Math.max(20, (ve - vs) * (pxPerHour / 60));
  return { top, height };
}

// ---------------------------------------------------------------------------
// Agenda (hoy + futuro)
// ---------------------------------------------------------------------------

export interface AgendaGroup {
  /** Medianoche local del día (ms) — la clave agrupadora. */
  dayMs: number;
  tasks: TaskLike[];
}

/** Días (medianoche local, ms) en que la tarea aparece en Agenda: desde
 *  `fromDayStart` (hoy) hasta su último día cubierto, ambos inclusive. */
export function agendaDays(t: TaskLike, fromDayStart: number): number[] {
  if (t.status === "completada") return [];
  const end = t.end.getTime();
  if (end <= fromDayStart) return [];
  const first = Math.max(dayStartMs(t.start), fromDayStart);
  const out: number[] = [];
  for (let d = first; d < end; d += DAY_MS) out.push(d);
  return out;
}

function orderInDay(a: TaskLike, b: TaskLike): number {
  const aAll = a.allDay ? 1 : 0;
  const bAll = b.allDay ? 1 : 0;
  if (aAll !== bAll) return aAll - bAll; // con horario primero, "Todo el día" después
  return a.start.getTime() - b.start.getTime();
}

/** Agrupación de Agenda: solo días con presencia a partir de hoy, hoy primero,
 *  luego cronológico; dentro del día, horario → todo el día. */
export function groupAgenda(tasks: TaskLike[], now: Date): AgendaGroup[] {
  const todayStart = dayStartMs(now);
  const groups = new Map<number, AgendaGroup>();
  for (const t of tasks) {
    for (const d of agendaDays(t, todayStart)) {
      const g = groups.get(d) ?? { dayMs: d, tasks: [] };
      g.tasks.push(t);
      groups.set(d, g);
    }
  }
  const out = [...groups.values()].sort((a, b) => a.dayMs - b.dayMs);
  for (const g of out) g.tasks.sort(orderInDay);
  return out;
}
