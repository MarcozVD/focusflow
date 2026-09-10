import { describe, expect, it } from "vitest";
import {
  classConflictsIn,
  dowMonFirst,
  formatMinutes,
  isActiveOn,
  parseDateToMs,
  parseTimeToMinutes,
  shiftToFreeSlot,
  validateClassForm,
  type ClassRow,
} from "./classLogic";

const DAY = 86_400_000;
const ms = (y: number, mo: number, d: number, h = 0, mi = 0) =>
  new Date(y, mo - 1, d, h, mi).getTime();

const cls = (over: Partial<ClassRow> & Pick<ClassRow, "id" | "title">): ClassRow => ({
  day_of_week: 0,
  start_min: 600,
  end_min: 720,
  start_date: ms(2026, 8, 4),
  end_date: ms(2026, 11, 27),
  ...over,
});

describe("dowMonFirst", () => {
  it("lunes=0, domingo=6", () => {
    expect(dowMonFirst(new Date(2026, 8, 7))).toBe(0); // lunes
    expect(dowMonFirst(new Date(2026, 8, 13))).toBe(6); // domingo
  });
});

describe("isActiveOn (vigencia, regla 21)", () => {
  const c = cls({ id: 1, title: "Matemáticas" });
  it("dentro del periodo, inclusivo en bordes", () => {
    expect(isActiveOn(c, ms(2026, 8, 4))).toBe(true);
    expect(isActiveOn(c, ms(2026, 11, 27))).toBe(true);
  });
  it("antes del inicio o después del fin, no", () => {
    expect(isActiveOn(c, ms(2026, 8, 3))).toBe(false);
    expect(isActiveOn(c, ms(2026, 11, 28))).toBe(false);
  });
});

describe("classConflictsIn (reglas 7, 24)", () => {
  // lunes 10:00–12:00 vigente en septiembre 2026
  const c = cls({ id: 1, title: "Programación", day_of_week: 0 });
  const mon = ms(2026, 9, 7); // lunes

  it("detecta la tarea 11:00–12:30 que invade la clase", () => {
    const r = classConflictsIn([c], mon + 660 * 60_000, mon + 750 * 60_000);
    expect(r).toHaveLength(1);
    expect(r[0].title).toBe("Programación");
    expect(r[0].start_at).toBe(mon + 600 * 60_000);
  });
  it("tarea contigua (12:00) no es conflicto", () => {
    const r = classConflictsIn([c], mon + 720 * 60_000, mon + 780 * 60_000);
    expect(r).toHaveLength(0);
  });
  it("otros días de la semana no chocan", () => {
    const tue = mon + DAY;
    const r = classConflictsIn([c], tue + 660 * 60_000, tue + 750 * 60_000);
    expect(r).toHaveLength(0);
  });
  it("fuera de vigencia no choca, aunque el día encaje (regla 24)", () => {
    // mismo horario 2 semanas después del end_date
    const late = ms(2026, 11, 30); // lunes tras el 27/11
    expect(dowMonFirst(new Date(late))).toBe(0);
    const r = classConflictsIn([c], late + 660 * 60_000, late + 720 * 60_000);
    expect(r).toHaveLength(0);
  });
  it("varias clases solapadas se devuelven todas", () => {
    const a = cls({ id: 2, title: "A", day_of_week: 0, start_min: 540, end_min: 660 });
    const b = cls({ id: 3, title: "B", day_of_week: 0, start_min: 600, end_min: 660 });
    const r = classConflictsIn([a, b], mon + 630 * 60_000, mon + 645 * 60_000);
    expect(r.map((x) => x.class_id).sort()).toEqual([2, 3]);
  });
});

describe("parsers y validación del formulario", () => {
  it("parseTimeToMinutes / formatMinutes roundtrip", () => {
    expect(parseTimeToMinutes("10:30")).toBe(630);
    expect(parseTimeToMinutes("25:00")).toBeNaN();
    expect(formatMinutes(630)).toBe("10:30");
    expect(formatMinutes(0)).toBe("00:00");
  });
  it("parseDateToMs es medianoche local", () => {
    expect(parseDateToMs("2026-08-04")).toBe(ms(2026, 8, 4));
    expect(parseDateToMs("2026-8-4")).toBeNaN();
  });
  it("rechaza fin<=inicio, fin<inicio de vigencia, título vacío", () => {
    const base = {
      title: "X",
      day: 0,
      startMin: 600,
      endMin: 720,
      startDateMs: ms(2026, 8, 4),
      endDateMs: ms(2026, 11, 27),
    };
    expect(validateClassForm(base)).toBeNull();
    expect(validateClassForm({ ...base, endMin: 600 })).toMatch(/finalización/);
    expect(validateClassForm({ ...base, title: "  " })).toMatch(/obligatorio/);
    expect(validateClassForm({ ...base, endDateMs: base.startDateMs - DAY })).toMatch(
      /finalización/,
    );
    expect(validateClassForm({ ...base, day: 9 })).toMatch(/inválido/);
  });
});

describe("shiftToFreeSlot (opción Editar del diálogo de conflicto)", () => {
  const c = cls({ id: 1, title: "Matemáticas", day_of_week: 0 }); // lun 10-12
  const mon = ms(2026, 9, 7);

  it("sin conflicto, devuelve la misma ventana", () => {
    const r = shiftToFreeSlot([c], mon + 780 * 60_000, mon + 840 * 60_000);
    expect(r).toEqual({ start: mon + 780 * 60_000, end: mon + 840 * 60_000, shifted: false });
  });
  it("desplaza la tarea al hueco libre tras la clase, conservando duración", () => {
    // tarea 11:00–12:30 invade la clase 10–12 → debe quedar 12:00–13:30
    const r = shiftToFreeSlot([c], mon + 660 * 60_000, mon + 750 * 60_000);
    expect(r.start).toBe(mon + 720 * 60_000);
    expect(r.end).toBe(mon + 810 * 60_000);
    expect(r.shifted).toBe(true);
  });
  it("encadena saltos si hay varias clases consecutivas", () => {
    const b = cls({ id: 2, title: "Física", day_of_week: 0, start_min: 720, end_min: 780 });
    // 11:00–12:30 choca con Matemáticas (hasta 12) y luego con Física (12–13)
    const r = shiftToFreeSlot([c, b], mon + 660 * 60_000, mon + 750 * 60_000);
    expect(r.start).toBe(mon + 780 * 60_000); // 13:00
    expect(r.end).toBe(mon + 870 * 60_000); // 14:30
  });
});
