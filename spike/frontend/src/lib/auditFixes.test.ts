import { describe, expect, it } from "vitest";
import { localIsoDate } from "./dateUtils";
import { suggestionAllDay, suggestionEditDate } from "./suggestionLogic";
import { taskStatus } from "./data.svelte";

// Regresiones de la auditoría: fecha UTC vs local, "vencida" por inicio y
// all_day inferido del formulario.

describe("localIsoDate", () => {
  it("usa el día LOCAL, no el UTC (22:30 local sigue siendo el mismo día)", () => {
    const d = new Date(2026, 8, 22, 22, 30); // 22-sep 22:30 local
    expect(localIsoDate(d)).toBe("2026-09-22");
    expect(localIsoDate(d.getTime())).toBe("2026-09-22");
  });
  it("rellena mes y día con cero", () => {
    expect(localIsoDate(new Date(2026, 0, 5, 0, 0))).toBe("2026-01-05");
  });
});

describe("taskStatus", () => {
  const row = { completed_at: null, status: "pendiente" };
  it("una tarea en curso (empezó pero no terminó) NO está vencida", () => {
    const now = 1_000_000;
    expect(taskStatus(row, now + 60_000, now)).toBe("pendiente");
  });
  it("vencida solo cuando ya pasó el FIN", () => {
    const now = 1_000_000;
    expect(taskStatus(row, now - 1, now)).toBe("vencida");
  });
  it("completada gana sobre vencida", () => {
    expect(taskStatus({ completed_at: 5, status: "pendiente" }, 0, 10)).toBe("completada");
  });
});

describe("suggestionAllDay", () => {
  it("evento con hora e inicio==fin a las 10:00 NO es todo el día", () => {
    const t = new Date(2026, 8, 25, 10, 0).getTime();
    expect(suggestionAllDay({ kind: "event", start_at: t, end_at: t })).toBe(false);
  });
  it("inicio==fin a medianoche = marcador de todo el día", () => {
    const t = new Date(2026, 8, 25, 0, 0).getTime();
    expect(suggestionAllDay({ kind: "deadline", start_at: t, end_at: t })).toBe(true);
  });
  it("rango con horas distintas no es todo el día", () => {
    const s = new Date(2026, 8, 25, 10, 0).getTime();
    expect(suggestionAllDay({ kind: "event", start_at: s, end_at: s + 3_600_000 })).toBe(false);
  });
  it("sin fechas = todo el día", () => {
    expect(suggestionAllDay({ kind: "event", start_at: null, end_at: null })).toBe(true);
  });
});

describe("suggestionEditDate", () => {
  it("de noche muestra el mismo día local (antes saltaba a mañana en UTC-5)", () => {
    const t = new Date(2026, 8, 22, 21, 0).getTime();
    expect(suggestionEditDate(t)).toBe("2026-09-22");
  });
});
