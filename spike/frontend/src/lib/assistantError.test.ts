import { describe, expect, it } from "vitest";
import { friendlyAssistantError } from "./assistantError";

describe("friendlyAssistantError", () => {
  it("desarma ia_429 con segundos y pone remanente retryable", () => {
    const r = friendlyAssistantError("ia_429 30 detalle técnico del provider");
    expect(r.retryable).toBe(true);
    expect(r.waitSecs).toBe(30);
    expect(r.text).toContain("~30 s.");
  });

  it("ia_429 sin segundos es retryable y sin waitSecs", () => {
    const r = friendlyAssistantError("Error: ia_429 algo");
    expect(r.retryable).toBe(true);
    expect(r.waitSecs).toBeUndefined();
  });

  it("detecta mensaje alternativo de saturación", () => {
    const r = friendlyAssistantError("saturado por el límite de peticiones");
    expect(r.retryable).toBe(true);
  });

  it("errores genéricos no son retryable y pasan texto", () => {
    const r = friendlyAssistantError("network reset");
    expect(r.retryable).toBe(false);
    expect(r.text).toBe("network reset");
  });

  it("null y vacío se tratan como genéricos", () => {
    expect(friendlyAssistantError(null).retryable).toBe(false);
    expect(friendlyAssistantError("").text).toBe("");
  });

  it("segundos con decimales se redondean", () => {
    const r = friendlyAssistantError("ia_429 15.6 x");
    expect(r.waitSecs).toBe(16);
    expect(r.text).toContain("~16 s.");
  });
});