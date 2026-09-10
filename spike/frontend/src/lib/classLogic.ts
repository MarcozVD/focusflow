// Lógica pura del dominio "Horario" (clases). ÚNICA fuente de verdad en el
// frontend para vigencia, instancias por día y solapes.
//
// Sin Svelte ni DOM: testeable con vitest (igual que taskDayLogic.ts).
// La vigencia (regla 30): una clase ocupa un día concreto si el día de la
// semana coincide Y medianoche_local(fecha) está en [start_date, end_date].

export const DAY_MS = 86_400_000;

/** Fila tal como la devuelve `class_list` (días ÉPOCA locales ms). */
export interface ClassRow {
  id: number;
  title: string;
  /** 0 = lunes … 6 = domingo (mismo criterio que `(getDay()+6)%7`). */
  day_of_week: number;
  start_min: number;
  end_min: number;
  /** Medianoche local del primer día de vigencia (inclusiva). */
  start_date: number;
  /** Medianoche local del último día de vigencia (inclusiva). */
  end_date: number;
}

/** Instancia materializada de una clase en una fecha concreta. */
export interface ClassInstance {
  class_id: number;
  title: string;
  day_of_week: number;
  /** Medianoche local del día de la instancia. */
  date_ms: number;
  start_at: number;
  end_at: number;
}

export function dayStartMs(d: Date): number {
  return new Date(d.getFullYear(), d.getMonth(), d.getDate()).getTime();
}

/** 0 = lunes … 6 = domingo, para un Date cualquiera. */
export function dowMonFirst(d: Date): number {
  return (d.getDay() + 6) % 7;
}

/** ¿La clase está vigente el día `dateMs` (medianoche local)? (regla 21/30) */
export function isActiveOn(c: ClassRow, dateMs: number): boolean {
  return dateMs >= c.start_date && dateMs <= c.end_date;
}

/**
 * Instancias activas cuya franja se solapa con [startMs, endMs) y cuyo día cae
 * dentro del rango consultado. Equivalente frontend de
 * `class_instances_in_range` + filtro de solape (reglas 7 y 24).
 */
export function classConflictsIn(
  classes: ClassRow[],
  startMs: number,
  endMs: number,
): ClassInstance[] {
  const out: ClassInstance[] = [];
  const first = dayStartMs(new Date(startMs));
  const last = dayStartMs(new Date(Math.max(endMs - 1, startMs)));
  for (let day = first; day <= last; day += DAY_MS) {
    const dow = dowMonFirst(new Date(day));
    for (const c of classes) {
      if (c.day_of_week !== dow) continue;
      if (!isActiveOn(c, day)) continue;
      const s = day + c.start_min * 60_000;
      const e = day + c.end_min * 60_000;
      if (s < endMs && e > startMs) {
        out.push({
          class_id: c.id,
          title: c.title,
          day_of_week: c.day_of_week,
          date_ms: day,
          start_at: s,
          end_at: e,
        });
      }
    }
  }
  return out;
}

/**
 * Devuelve una ventana [start, end] que NO solapa ninguna clase activa.
 * Punto de partida: la ventana pedida; si choca, se desplaza al final de la
 * última clase que la bloquea, manteniendo la duración. Itera hasta que encaje
 * (máx. 24 saltos: una clase por franja horaria del día).
 */
export function shiftToFreeSlot(
  classes: ClassRow[],
  startMs: number,
  endMs: number,
): { start: number; end: number; shifted: boolean } {
  let s = startMs;
  let e = endMs;
  let shifted = false;
  for (let i = 0; i < 24; i++) {
    const hits = classConflictsIn(classes, s, e);
    if (hits.length === 0) break;
    const latest = Math.max(...hits.map((h) => h.end_at));
    const dur = e - s;
    s = latest;
    e = s + dur;
    shifted = true;
  }
  return { start: s, end: e, shifted };
}

/** Formatea minutos-del-día como "HH:MM" (24 h). */
export function formatMinutes(min: number): string {
  const h = Math.floor(min / 60);
  const m = min % 60;
  return `${String(h).padStart(2, "0")}:${String(m).padStart(2, "0")}`;
}

/** "HH:MM" de un input[type=time] a minutos-del-día. NaN si inválido. */
export function parseTimeToMinutes(v: string): number {
  const m = /^(\d{1,2}):(\d{2})$/.exec(v.trim());
  if (!m) return NaN;
  const h = Number(m[1]);
  const mi = Number(m[2]);
  if (h > 23 || mi > 59) return NaN;
  return h * 60 + mi;
}

/** "2026-08-04" de input[type=date] a medianoche local (ms). NaN si inválido. */
export function parseDateToMs(v: string): number {
  const m = /^(\d{4})-(\d{2})-(\d{2})$/.exec(v.trim());
  if (!m) return NaN;
  return new Date(Number(m[1]), Number(m[2]) - 1, Number(m[3])).getTime();
}

/** Validación del formulario (reglas 3 y 28). Devuelve el error o null. */
export function validateClassForm(f: {
  title: string;
  day: number;
  startMin: number;
  endMin: number;
  startDateMs: number;
  endDateMs: number;
}): string | null {
  if (!f.title.trim()) return "El nombre de la clase es obligatorio";
  if (!(f.day >= 0 && f.day <= 6)) return "Día de la semana inválido";
  if (Number.isNaN(f.startMin) || Number.isNaN(f.endMin)) return "Hora de inicio o fin inválida";
  if (f.endMin <= f.startMin) return "La hora de finalización debe ser posterior a la de inicio";
  if (Number.isNaN(f.startDateMs) || Number.isNaN(f.endDateMs)) return "Fecha de inicio o fin inválida";
  if (f.endDateMs < f.startDateMs)
    return "La fecha de finalización debe ser igual o posterior a la de inicio";
  return null;
}
