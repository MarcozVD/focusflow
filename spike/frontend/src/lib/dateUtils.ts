/** Fecha LOCAL en formato AAAA-MM-DD (para <input type="date">, claves de
 *  semana y nombres de archivo). `toISOString().slice(0, 10)` da el día UTC:
 *  en UTC-5, desde las 19:00 devolvía el día siguiente y, al recomponerlo como
 *  fecha local, guardar movía el evento +1 día. */
export function localIsoDate(d: Date | number): string {
  const x = typeof d === "number" ? new Date(d) : d;
  return `${x.getFullYear()}-${String(x.getMonth() + 1).padStart(2, "0")}-${String(x.getDate()).padStart(2, "0")}`;
}
