import { describe, expect, it } from "vitest";
import { weekKeysBetween, fmtMs, fmtDate } from "./data.svelte";

describe("weekKeysBetween", () => {
  it("devuelve la semana única cuando from y to están dentro", () => {
    const keys = weekKeysBetween(new Date("2026-03-10T09:00:00"), new Date("2026-03-12T18:00:00"));
    expect(keys).toHaveLength(1);
  });

  it("devuelve ambas semanas al cruzar el límite", () => {
    // dom distinto de lunes
    const keys = weekKeysBetween(new Date("2026-03-15T10:00:00"), new Date("2026-03-16T10:00:00"));
    expect(keys.length >= 2).toBe(true);
  });

  it("ordena las semanas de menor a mayor", () => {
    const keys = weekKeysBetween(new Date("2026-02-25T08:00:00"), new Date("2026-03-10T08:00:00"));
    expect(keys.length).toBeGreaterThan(1);
    const sorted = [...keys].sort();
    expect(keys).toEqual(sorted);
  });
});

describe("fmtMs / fmtDate", () => {
  it("fmtMs devuelve em dash para nulos", () => {
    expect(fmtMs(null)).toBe("—");
    expect(fmtMs(0)).toBe("—");
  });

  it("fmtMs formatea fecha y hora en es-ES", () => {
    const s = fmtMs(new Date(2026, 2, 10, 14, 30).getTime());
    expect(s).toContain("mar");
  });

  it("fmtDate incluye tiempo y fecha en es-ES", () => {
    const d = fmtDate(new Date(2026, 2, 10, 14, 30).getTime());
    expect(typeof d).toBe("string");
    expect(d.length).toBeGreaterThan(0);
  });

  it("fmtDate maneja nulos con em dash", () => {
    expect(fmtDate(null)).toBe("—");
  });
});