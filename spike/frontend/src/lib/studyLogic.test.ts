import { describe, expect, it } from "vitest";
import {
  STUDY_COLOR,
  combineDateAndTime,
  fmtDuration,
  fmtRange,
  overlaps,
  plusMinutes,
  sortStudies,
  studiesOnDay,
  studyDurationMin,
  studySegment,
  tasksOverlapping,
  toStudy,
  validateStudyForm,
  type StudyRow,
} from "./studyLogic";

function d(y: number, m: number, day: number, h = 0, min = 0): Date {
  return new Date(y, m, day, h, min, 0, 0);
}

function session(id: number, start: Date, end: Date, title = `s${id}`) {
  return { id, title, start, end, taskId: null, notes: "" };
}

const row: StudyRow = {
  id: 7,
  title: "Estudiar cálculo",
  start_at: d(2026, 8, 15, 10, 0).getTime(),
  end_at: d(2026, 8, 15, 12, 0).getTime(),
  task_id: null,
  notes: "",
  created_at: 0,
  updated_at: 0,
};

describe("toStudy / duración", () => {
  it("convierte la fila del backend en sesión con fechas", () => {
    const s = toStudy(row);
    expect(s.id).toBe(7);
    expect(s.title).toBe("Estudiar cálculo");
    expect(s.start.getHours()).toBe(10);
    expect(s.end.getHours()).toBe(12);
    expect(s.taskId).toBeNull();
    expect(studyDurationMin(s)).toBe(120);
  });

  it("formatea la duración de forma legible", () => {
    expect(fmtDuration(120)).toBe("2 h");
    expect(fmtDuration(90)).toBe("1 h 30 min");
    expect(fmtDuration(45)).toBe("45 min");
    expect(fmtDuration(0)).toBe("—");
  });

  it("formatea la franja en 24 h", () => {
    expect(fmtRange(toStudy(row))).toBe("10:00 – 12:00");
  });
});

describe("validación del formulario (regla 6)", () => {
  const base = { title: "Estudiar cálculo", startMs: row.start_at, endMs: row.end_at };

  it("acepta una sesión válida", () => {
    expect(validateStudyForm(base)).toBeNull();
  });

  it("exige título", () => {
    expect(validateStudyForm({ ...base, title: "   " })).toBe(
      "El título de la sesión es obligatorio",
    );
  });

  it("exige hora final posterior a la inicial y duración > 0", () => {
    expect(validateStudyForm({ ...base, endMs: base.startMs })).toBe(
      "La hora de finalización debe ser posterior a la de inicio",
    );
    expect(validateStudyForm({ ...base, endMs: base.startMs - 60_000 })).toBe(
      "La hora de finalización debe ser posterior a la de inicio",
    );
  });

  it("rechaza fecha/hora inválida", () => {
    expect(validateStudyForm({ ...base, startMs: NaN })).toBe(
      "La fecha u hora de la sesión no es válida",
    );
    expect(validateStudyForm({ ...base, startMs: 0, endMs: 0 })).toBe(
      "La fecha u hora de la sesión no es válida",
    );
  });

  it("combina fecha + hora de los inputs y calcula la duración", () => {
    const start = combineDateAndTime("2026-09-15", "10:00");
    const end = combineDateAndTime("2026-09-15", "12:00");
    expect(new Date(start).getHours()).toBe(10);
    expect(studyDurationMin({ start: new Date(start), end: new Date(end) })).toBe(120);
    expect(combineDateAndTime("15/09/2026", "10:00")).toBeNaN();
    expect(combineDateAndTime("2026-09-15", "25:00")).toBeNaN();
    expect(plusMinutes(start, 120)).toBe(end);
  });
});

describe("presencia por día y segmentos", () => {
  const s = session(1, d(2026, 8, 15, 10, 0), d(2026, 8, 15, 12, 0));

  it("la sesión aparece el día que cubre (semiabierto)", () => {
    expect(studiesOnDay([s], d(2026, 8, 15))).toHaveLength(1);
    expect(studiesOnDay([s], d(2026, 8, 14))).toHaveLength(0);
    expect(studiesOnDay([s], d(2026, 8, 16))).toHaveLength(0);
  });

  it("el segmento horario respeta la ventana real", () => {
    const seg = studySegment(s, d(2026, 8, 15));
    expect(seg).not.toBeNull();
    expect(seg!.start.getHours()).toBe(10);
    expect(seg!.end.getHours()).toBe(12);
    expect(seg!.kind).toBe("full");
    expect(studySegment(s, d(2026, 8, 16))).toBeNull();
  });

  it("ordena por hora de inicio", () => {
    const late = session(2, d(2026, 8, 15, 16, 0), d(2026, 8, 15, 18, 0));
    const early = session(3, d(2026, 8, 15, 8, 0), d(2026, 8, 15, 9, 0));
    expect(sortStudies([late, early, s]).map((x) => x.id)).toEqual([3, 1, 2]);
  });
});

describe("solapes con tareas (regla 11: aviso, nunca bloqueo)", () => {
  const t = (id: number, h1: number, h2: number) => ({
    id,
    title: `t${id}`,
    start: d(2026, 8, 15, h1, 0),
    end: d(2026, 8, 15, h2, 0),
  });

  it("detecta solape real y no la adyacencia", () => {
    expect(overlaps(660, 780, 660, 720)).toBe(true); // 11:00-13:00 vs 11:00-12:00
    expect(overlaps(600, 660, 660, 720)).toBe(false); // 10:00-11:00 vs 11:00-12:00
  });

  it("lista solo las tareas que se solapan", () => {
    const hits = tasksOverlapping(
      [t(1, 11, 12), t(2, 8, 9), t(3, 12, 14), t(4, 13, 15)],
      d(2026, 8, 15, 11, 0).getTime(),
      d(2026, 8, 15, 13, 0).getTime(),
    );
    // 1 (11-12) y 3 (12-14) solapan; 2 (8-9) no; 4 (13-15) solo toca el borde
    expect(hits.map((x) => x.id)).toEqual([1, 3]);
  });
});

describe("color propio", () => {
  it("las sesiones no usan el color de una categoría", () => {
    expect(STUDY_COLOR).toMatch(/^#[0-9a-fA-F]{6}$/);
  });
});
