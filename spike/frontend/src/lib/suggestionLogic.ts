import { localIsoDate } from "./dateUtils";

/** all_day ORIGINAL de una sugerencia, con la misma regla que usa el backend
 *  al aceptarla (sync.rs accept_suggestion): sin inicio/fin, inicio == fin sin
 *  hora explícita (00:00 / 23:59 local) o disponibilidad multi-día.
 *  Antes la edición inferia `allDay: startAt === endAt` a partir del
 *  formulario, así que un evento con hora cuyo fin igualaba al inicio pasaba
 *  a todo-el-día y uno todo-el-día con horas distintas dejaba de serlo. */
export function suggestionAllDay(s: {
  kind: string;
  start_at: number | null;
  end_at: number | null;
}): boolean {
  if (s.start_at == null || s.end_at == null) return true;
  if (s.kind === "availability") return true;
  if (s.start_at !== s.end_at) return false;
  const d = new Date(s.start_at);
  const hm = `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
  return hm === "00:00" || hm === "23:59";
}

/** Valores iniciales del formulario de edición (fecha LOCAL, no UTC). */
export function suggestionEditDate(startAt: number | null, now: number = Date.now()): string {
  return startAt ? localIsoDate(startAt) : localIsoDate(now + 86_400_000);
}
