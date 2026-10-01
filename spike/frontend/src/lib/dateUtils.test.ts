import { describe, expect, it } from "vitest";
import { capitalizeFirst } from "./dateUtils";

describe("capitalizeFirst", () => {
  it("solo pone mayúscula en la primera letra", () => {
    expect(capitalizeFirst("miércoles, 30 de septiembre")).toBe("Miércoles, 30 de septiembre");
  });

  it("deja igual un texto que ya empieza en mayúscula (mes)", () => {
    expect(capitalizeFirst("Septiembre 2026")).toBe("Septiembre 2026");
  });

  it("cadena vacía no rompe", () => {
    expect(capitalizeFirst("")).toBe("");
  });
});
