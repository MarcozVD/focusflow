// Lógica pura del dominio "Sesiones de estudio". ÚNICA fuente de verdad en el
// frontend para validación, duración, presencia por día y solapes.
//
// Una sesión de estudio es tiempo RESERVADO por el usuario (planificado,
// movible y editable), NO trabajo pendiente: por eso este módulo no mira
// `status`, no calcula vencimientos y no cuenta pendientes.
//
// Sin Svelte ni DOM: testeable con vitest (igual que classLogic/taskDayLogic).

import { DAY_MS, dayStartMs, parseDateToMs, parseTimeToMinutes, type ClassInstance } from "./classLogic";
import { segmentFor, type Segment, type TaskLike } from "./taskDayLogic";

export { DAY_MS, dayStartMs, parseDateToMs, parseTimeToMinutes };
export type { ClassInstance };

/** Fila tal como la devuelve `study_list_range` (ms ÉPOCA local). */
export interface StudyRow {
  id: number;
  title: string;
  start_at: number;
  end_at: number;
  task_id: number | null;
  notes: string;
  created_at: number;
  updated_at: number;
}

/** Sesión de estudio en el modelo del frontend (fechas reales). */
export interface StudySession {
  id: number;
  title: string;
  start: Date;
  end: Date;
  /** Tarea relacionada (opcional): la tarea vive por su cuenta. */
  taskId: number | null;
  notes: string;
}

/**
 * Color propio de las sesiones de estudio: no son tareas, así que no usan el
 * color de una categoría. Diferencia visual sin romper la estética.
 */
export const STUDY_COLOR = "#0D9488";

export function toStudy(r: StudyRow): StudySession {
  return {
    id: r.id,
    title: r.title,
    start: new Date(r.start_at),
    end: new Date(r.end_at),
    taskId: r.task_id,
    notes: r.notes,
  };
}

/** Duración en minutos (siempre > 0 para sesiones válidas). */
export function studyDurationMin(s: Pick<StudySession, "start" | "end">): number {
  return Math.round((s.end.getTime() - s.start.getTime()) / 60_000);
}

/** "2 h", "1 h 30 min", "45 min" — etiqueta legible de duración. */
export function fmtDuration(min: number): string {
  if (!Number.isFinite(min) || min <= 0) return "—";
  const h = Math.floor(min / 60);
  const m = Math.round(min % 60);
  if (h === 0) return `${m} min`;
  if (m === 0) return `${h} h`;
  return `${h} h ${m} min`;
}

/** Solape real semiabierto: [9,10) y [10,11) NO se solapan. */
export function overlaps(aStart: number, aEnd: number, bStart: number, bEnd: number): boolean {
  return aStart < bEnd && bStart < aEnd;
}

/** Adapter al modelo de segmentos del calendario (reutiliza su geometría). */
export function studyTaskLike(s: StudySession): TaskLike {
  return {
    id: s.id,
    title: s.title,
    start: s.start,
    end: s.end,
    allDay: false,
    status: "pendiente",
  };
}

/** Segmento horario de la sesión para el día `d` (null si no cae ese día). */
export function studySegment(s: StudySession, d: Date): Segment | null {
  return segmentFor(studyTaskLike(s), d);
}

/** Sesiones que cubren el día `d` (semiabierto [dayStart, dayEnd)). */
export function studiesOnDay(list: StudySession[], d: Date): StudySession[] {
  const ds = dayStartMs(d);
  const de = ds + DAY_MS;
  return list.filter((s) => s.start.getTime() < de && s.end.getTime() > ds);
}

/** Sesiones ordenadas por inicio (orden visual estable). */
export function sortStudies(list: StudySession[]): StudySession[] {
  return [...list].sort((a, b) => a.start.getTime() - b.start.getTime() || a.id - b.id);
}

/** Etiqueta corta "10:00 – 12:00" (24 h). */
export function fmtRange(s: Pick<StudySession, "start" | "end">): string {
  const p = (n: number) => String(n).padStart(2, "0");
  const t = (d: Date) => `${p(d.getHours())}:${p(d.getMinutes())}`;
  return `${t(s.start)} – ${t(s.end)}`;
}

// ---------------------------------------------------------------------------
// Validación del formulario (regla 6)
// ---------------------------------------------------------------------------

export interface StudyFormValues {
  title: string;
  /** ms ÉPOCA local de inicio (fecha + hora). */
  startMs: number;
  /** ms ÉPOCA local de fin. */
  endMs: number;
}

/**
 * Validaciones exigidas: campos obligatorios, fecha válida, hora final
 * posterior a la inicial y duración > 0. Devuelve el mensaje o null.
 */
export function validateStudyForm(f: StudyFormValues): string | null {
  if (!f.title.trim()) return "El título de la sesión es obligatorio";
  if (Number.isNaN(f.startMs) || Number.isNaN(f.endMs)) return "La fecha u hora de la sesión no es válida";
  if (f.startMs <= 0 || f.endMs <= 0) return "La fecha u hora de la sesión no es válida";
  if (f.endMs <= f.startMs) return "La hora de finalización debe ser posterior a la de inicio";
  if (studyDurationMin({ start: new Date(f.startMs), end: new Date(f.endMs) }) <= 0) {
    return "La duración de la sesión debe ser mayor que 0";
  }
  return null;
}

/** Combina "YYYY-MM-DD" + "HH:MM" en ms ÉPOCA local (NaN si algo no es válido). */
export function combineDateAndTime(dateV: string, timeV: string): number {
  const dayMs = parseDateToMs(dateV);
  const min = parseTimeToMinutes(timeV);
  if (Number.isNaN(dayMs) || Number.isNaN(min)) return NaN;
  return dayMs + min * 60_000;
}

/** Suma minutos a un ms conservando la fecha (para autocompletar el fin). */
export function plusMinutes(ms: number, minutes: number): number {
  return ms + minutes * 60_000;
}

// ---------------------------------------------------------------------------
// Vínculo con tareas (regla 7 y 11)
// ---------------------------------------------------------------------------

export interface TaskLikeRef {
  id: number;
  title: string;
  start: Date;
  end: Date;
}

/** Tareas que se solapan con la ventana: AVISO, nunca bloqueo (regla 11). */
export function tasksOverlapping(tasks: TaskLikeRef[], startMs: number, endMs: number): TaskLikeRef[] {
  return tasks.filter((t) => overlaps(startMs, endMs, t.start.getTime(), t.end.getTime()));
}
