import { describe, expect, it } from "vitest";
import {
  coversDay,
  daySpanDays,
  isMultiDay,
  lastCoveredDayMs,
  tasksOnDay,
  segmentFor,
  allDayChipsOn,
  multiDayChipsOn,
  topChipsOn,
  monthChipsOn,
  chipTextFor,
  layoutMetrics,
  agendaDays,
  groupAgenda,
  type TaskLike,
} from "./taskDayLogic";

const d = (y: number, mo: number, day: number, h = 0, mi = 0): Date =>
  new Date(y, mo - 1, day, h, mi);

function t(over: Partial<TaskLike> & Pick<TaskLike, "id" | "title" | "start" | "end">): TaskLike {
  return { allDay: false, status: "pendiente", ...over };
}

describe("coversDay", () => {
  it("cubre el día si el intervalo [start, end) lo cruza", () => {
    const task = t({ id: 1, title: "X", start: d(2026, 8, 10, 9), end: d(2026, 8, 10, 11) });
    expect(coversDay(task, d(2026, 8, 10))).toBe(true);
    expect(coversDay(task, d(2026, 8, 9))).toBe(false);
    expect(coversDay(task, d(2026, 8, 11))).toBe(false);
  });

  it("un fin exactamente a medianoche NO cubre ese día", () => {
    const task = t({ id: 2, title: "X", start: d(2026, 8, 10, 10), end: d(2026, 8, 11, 0) });
    expect(coversDay(task, d(2026, 8, 10))).toBe(true);
    expect(coversDay(task, d(2026, 8, 11))).toBe(false);
  });
});

describe("lastCoveredDayMs / daySpanDays", () => {
  it("fin a medianoche descuenta el día", () => {
    const task = t({ id: 3, title: "X", start: d(2026, 8, 10, 10), end: d(2026, 8, 12, 0) });
    expect(lastCoveredDayMs(task)).toBe(d(2026, 8, 11).getTime());
    expect(daySpanDays(task)).toBe(2);
  });

  it("multi-día normal", () => {
    const task = t({ id: 4, title: "X", start: d(2026, 8, 10, 10), end: d(2026, 8, 12, 14) });
    expect(daySpanDays(task)).toBe(3);
    expect(isMultiDay(task)).toBe(true);
  });
});

describe("segmentFor (área de tiempo)", () => {
  it("mismo día → bloque real", () => {
    const task = t({ id: 5, title: "X", start: d(2026, 8, 10, 9), end: d(2026, 8, 10, 11) });
    const seg = segmentFor(task, d(2026, 8, 10));
    expect(seg).toEqual({ start: d(2026, 8, 10, 9), end: d(2026, 8, 10, 11), kind: "full" });
    expect(segmentFor(task, d(2026, 8, 9))).toBeNull();
  });

  it("mismo día que termina a medianoche → bloque completo hasta 24:00", () => {
    const task = t({ id: 6, title: "X", start: d(2026, 8, 10, 10), end: d(2026, 8, 11, 0) });
    const seg = segmentFor(task, d(2026, 8, 10));
    expect(seg).toEqual({ start: d(2026, 8, 10, 10), end: d(2026, 8, 11, 0), kind: "full" });
  });

  it("multi-día: inicio → stub 2h; fin → stub 2h; medio → null", () => {
    const task = t({ id: 7, title: "X", start: d(2026, 8, 10, 10), end: d(2026, 8, 12, 14) });
    expect(segmentFor(task, d(2026, 8, 10))).toEqual({
      start: d(2026, 8, 10, 10),
      end: d(2026, 8, 10, 12),
      kind: "inicio",
    });
    expect(segmentFor(task, d(2026, 8, 11))).toBeNull();
    expect(segmentFor(task, d(2026, 8, 12))).toEqual({
      start: d(2026, 8, 12, 12),
      end: d(2026, 8, 12, 14),
      kind: "fin",
    });
  });

  it("multi-día que termina a medianoche: stub de fin el último día cubierto, no el de 00:00", () => {
    const task = t({ id: 8, title: "X", start: d(2026, 8, 10, 10), end: d(2026, 8, 12, 0) });
    expect(lastCoveredDayMs(task)).toBe(d(2026, 8, 11).getTime());
    expect(segmentFor(task, d(2026, 8, 11))).toEqual({
      start: d(2026, 8, 11, 22),
      end: d(2026, 8, 12, 0),
      kind: "fin",
    });
    expect(segmentFor(task, d(2026, 8, 12))).toBeNull();
  });
});

describe("chips", () => {
  const allDay = t({ id: 20, title: "AD", start: d(2026, 8, 10), end: d(2026, 8, 11), allDay: true });
  const multi = t({ id: 21, title: "M", start: d(2026, 8, 10, 10), end: d(2026, 8, 12, 14) });
  const single = t({ id: 22, title: "S", start: d(2026, 8, 10, 9), end: d(2026, 8, 10, 11) });

  it("allDayChipsOn: solo día completo que cubre el día", () => {
    expect(allDayChipsOn([allDay, multi, single], d(2026, 8, 10))).toEqual([allDay]);
    // multi-día all-day también cubre días intermedios
    expect(allDayChipsOn([t({ ...allDay, start: d(2026, 8, 10), end: d(2026, 8, 13) })], d(2026, 8, 12))).toHaveLength(1);
  });

  it("multiDayChipsOn: solo días intermedios de multi-día con horario", () => {
    expect(multiDayChipsOn([allDay, multi, single], d(2026, 8, 11))).toEqual([multi]);
    expect(multiDayChipsOn([allDay, multi, single], d(2026, 8, 10))).toEqual([]);
    expect(multiDayChipsOn([allDay, multi, single], d(2026, 8, 12))).toEqual([]);
  });

  it("topChipsOn: todo el día + intermedios", () => {
    // allDay de UN día (10→11) no aparece el 11: solo el multi-día intermedio
    expect(topChipsOn([allDay, multi, single], d(2026, 8, 11))).toEqual([multi]);
    // allDay multi-día (10→13) sí aparece en día intermedio
    const adMulti = t({ ...allDay, end: d(2026, 8, 13) });
    expect(topChipsOn([adMulti, multi, single], d(2026, 8, 11)).map((x) => x.id)).toEqual([20, 21]);
  });

  it("monthChipsOn: todo lo que cubre el día, incluido el día intermedio", () => {
    const adMulti = t({ ...allDay, end: d(2026, 8, 13) });
    expect(monthChipsOn([adMulti, multi, single], d(2026, 8, 11)).map((x) => x.id)).toEqual([20, 21]);
    expect(monthChipsOn([multi], d(2026, 8, 12))).toEqual([multi]);
  });

  it("tasksOnDay incluye completadas", () => {
    const done = t({ id: 23, title: "D", start: d(2026, 8, 10, 9), end: d(2026, 8, 10, 10), status: "completada" });
    expect(tasksOnDay([done], d(2026, 8, 10))).toEqual([done]);
  });

  it("monthChipsOn: pendientes antes que completadas", () => {
    const done1 = t({ id: 60, title: "D1", start: d(2026, 8, 10, 9), end: d(2026, 8, 10, 10), status: "completada" });
    const pend1 = t({ id: 61, title: "P1", start: d(2026, 8, 10, 10), end: d(2026, 8, 10, 11) });
    const done2 = t({ id: 62, title: "D2", start: d(2026, 8, 10, 11), end: d(2026, 8, 10, 12), status: "completada" });
    const pend2 = t({ id: 63, title: "P2", start: d(2026, 8, 10, 12), end: d(2026, 8, 10, 13) });
    expect(monthChipsOn([done1, pend1, done2, pend2], d(2026, 8, 10)).map((x) => x.id)).toEqual([
      61, 63, 60, 62,
    ]);
  });

  it("topChipsOn: pendientes antes que completadas sobre el resultado concatenado", () => {
    const adDone = t({
      id: 70,
      title: "AD done",
      start: d(2026, 8, 10),
      end: d(2026, 8, 12),
      allDay: true,
      status: "completada",
    });
    const multiPend = t({ id: 71, title: "M pend", start: d(2026, 8, 10, 10), end: d(2026, 8, 12, 14) });
    expect(topChipsOn([adDone, multiPend], d(2026, 8, 11)).map((x) => x.id)).toEqual([71, 70]);
  });

  it("pendientes-primero conserva el orden cronológico dentro de cada grupo", () => {
    const done9 = t({ id: 80, title: "D9", start: d(2026, 8, 10, 9), end: d(2026, 8, 10, 10), status: "completada" });
    const pend8 = t({ id: 81, title: "P8", start: d(2026, 8, 10, 8), end: d(2026, 8, 10, 9) });
    const done13 = t({ id: 82, title: "D13", start: d(2026, 8, 10, 13), end: d(2026, 8, 10, 14), status: "completada" });
    const pend15 = t({ id: 83, title: "P15", start: d(2026, 8, 10, 15), end: d(2026, 8, 10, 16) });
    expect(monthChipsOn([done9, pend8, done13, pend15], d(2026, 8, 10)).map((x) => x.id)).toEqual([
      81, 83, 80, 82,
    ]);
  });

  it("multi-día completada aparece como chip en días intermedios", () => {
    const multiDone = t({
      id: 90,
      title: "MD",
      start: d(2026, 8, 10, 10),
      end: d(2026, 8, 12, 14),
      status: "completada",
    });
    expect(multiDayChipsOn([multiDone], d(2026, 8, 11))).toEqual([multiDone]);
    expect(topChipsOn([multiDone], d(2026, 8, 11))).toEqual([multiDone]);
  });
});

describe("chipTextFor", () => {
  const multi = t({ id: 30, title: "Viaje", start: d(2026, 8, 10, 10), end: d(2026, 8, 12, 14) });

  it("único → título", () => {
    const s = t({ id: 31, title: "Reunión", start: d(2026, 8, 10, 9), end: d(2026, 8, 10, 10) });
    expect(chipTextFor(s, d(2026, 8, 10))).toBe("Reunión");
  });

  it("inicio/fin/medio", () => {
    expect(chipTextFor(multi, d(2026, 8, 10))).toBe("Inicio · Viaje");
    expect(chipTextFor(multi, d(2026, 8, 11))).toBe("Viaje");
    expect(chipTextFor(multi, d(2026, 8, 12))).toBe("Fin · Viaje");
  });
});

describe("layoutMetrics", () => {
  it("bloque normal dentro de la cuadrícula", () => {
    const seg = { start: d(2026, 8, 10, 9), end: d(2026, 8, 10, 11), kind: "full" as const };
    expect(layoutMetrics(seg, 6, 22, 60)).toEqual({ top: 180, height: 120 });
  });

  it("fin a medianoche → 1440 min relativos, se recorta a la cuadrícula", () => {
    const seg = { start: d(2026, 8, 10, 10), end: d(2026, 8, 11, 0), kind: "full" as const };
    expect(layoutMetrics(seg, 6, 22, 60)).toEqual({ top: 240, height: 720 });
  });

  it("stub de fin que termina a medianoche no dibuja barra completa: 20 px mínimos en el borde", () => {
    const seg = { start: d(2026, 8, 11, 22), end: d(2026, 8, 12, 0), kind: "fin" as const };
    const m = layoutMetrics(seg, 6, 22, 60);
    expect(m.top).toBe(960); // 22:00 dentro de 6-22
    expect(m.height).toBe(20); // solo 2h visibles fuera → recorte, no barra de 16h
  });

  it("bloque que empieza antes de la cuadrícula se recorta al inicio", () => {
    const seg = { start: d(2026, 8, 10, 5), end: d(2026, 8, 10, 7), kind: "full" as const };
    expect(layoutMetrics(seg, 6, 22, 60)).toEqual({ top: 0, height: 60 });
  });
});

describe("agendaDays / groupAgenda", () => {
  const today = d(2026, 8, 10);

  it("multi-día aparece en cada día cubierto a partir de hoy", () => {
    const multi = t({ id: 40, title: "M", start: d(2026, 8, 9, 10), end: d(2026, 8, 12, 14) });
    expect(agendaDays(multi, d(2026, 8, 10).getTime())).toEqual([
      d(2026, 8, 10).getTime(),
      d(2026, 8, 11).getTime(),
      d(2026, 8, 12).getTime(),
    ]);
  });

  it("vencida que termina hoy → aparece hoy; pasada sin terminar → no aparece", () => {
    const vencida = t({ id: 41, title: "V", start: d(2026, 8, 9, 16), end: d(2026, 8, 10, 10), status: "vencida" });
    const pasada = t({ id: 42, title: "P", start: d(2026, 8, 9, 16), end: d(2026, 8, 9, 17), status: "vencida" });
    expect(agendaDays(vencida, d(2026, 8, 10).getTime())).toEqual([d(2026, 8, 10).getTime()]);
    expect(agendaDays(pasada, d(2026, 8, 10).getTime())).toEqual([]);
  });

  it("completada → fuera de agenda", () => {
    const done = t({ id: 43, title: "D", start: d(2026, 8, 10, 9), end: d(2026, 8, 10, 10), status: "completada" });
    expect(agendaDays(done, d(2026, 8, 10).getTime())).toEqual([]);
  });

  it("groupAgenda: hoy primero, orden horario → todo el día", () => {
    const allDay = t({ id: 50, title: "AD", start: d(2026, 8, 11), end: d(2026, 8, 12), allDay: true });
    const tarde = t({ id: 51, title: "Tarde", start: d(2026, 8, 10, 15), end: d(2026, 8, 10, 16) });
    const mananaTemprano = t({ id: 52, title: "Temprano", start: d(2026, 8, 11, 8), end: d(2026, 8, 11, 9) });
    const groups = groupAgenda([mananaTemprano, allDay, tarde], d(2026, 8, 10));
    expect(groups.map((g) => g.dayMs)).toEqual([d(2026, 8, 10).getTime(), d(2026, 8, 11).getTime()]);
    expect(groups[0].tasks.map((x) => x.id)).toEqual([51]);
    expect(groups[1].tasks.map((x) => x.id)).toEqual([52, 50]); // horario antes que todo el día
  });
});
