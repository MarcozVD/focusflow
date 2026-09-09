import { describe, expect, it } from "vitest";
import { friendlySyncError, fmtWait } from "./syncError";

describe("friendlySyncError", () => {
  it("ia_429 con segundos → advertencia con tiempo de reintento", () => {
    const r = friendlySyncError(
      "ia_429 92 429 Too Many Requests {\"error\":{\"message\":\"Rate limit reached for model `openai/gpt-oss-120b` in organization `org_01m08…`\"}}",
    );
    expect(r.transient).toBe(true);
    expect(r.retrySecs).toBe(92);
    expect(r.text).toContain("límite diario de análisis");
    expect(r.text).toContain("Se reintentará en 2 min");
    expect(r.text).not.toContain("org_01m08");
    expect(r.text).not.toContain("{");
  });

  it("ia_429 sin segundos → reintento genérico", () => {
    const r = friendlySyncError("ia_429 FreeUsageLimitError");
    expect(r.transient).toBe(true);
    expect(r.retrySecs).toBeUndefined();
    expect(r.text).toContain("Se reintentará automáticamente");
  });

  it("ia_fail → transitorio", () => {
    const r = friendlySyncError("ia_fail timeout de 90 s");
    expect(r.transient).toBe(true);
  });

  it("Gmail 403 SERVICE_DISABLED con JSON crudo → advertencia limpia", () => {
    const raw =
      'INBOX: gmail api 403 Forbidden: { "error": { "code": 403, "message": "Gmail API has not been used in project 546739884199…", "status": "PERMISSION_DENIED" }, "details": [ { "reason": "SERVICE_DISABLED" } ] }';
    const r = friendlySyncError(raw);
    expect(r.transient).toBe(true);
    expect(r.text).toContain("No se pudo conectar con Gmail");
    expect(r.text).not.toContain("546739884199");
    expect(r.text).not.toContain("{");
  });

  it("Gmail 429 → transitorio con reintento", () => {
    const r = friendlySyncError("INBOX: gmail api 429 Too Many Requests");
    expect(r.transient).toBe(true);
    expect(r.text).toContain("Gmail");
  });

  it("Gmail 401 → sesión caducada, no transitorio", () => {
    const r = friendlySyncError("gmail api 401 Unauthorized");
    expect(r.transient).toBe(false);
    expect(r.text).toContain("caducó");
  });

  it("Gmail 403 sin SERVICE_DISABLED → permisos", () => {
    const r = friendlySyncError("gmail api 403 Forbidden");
    expect(r.transient).toBe(false);
    expect(r.text).toContain("permiso");
  });

  it("error de red → transitorio", () => {
    const r = friendlySyncError("conexión fallida: gmail api: connection reset");
    expect(r.transient).toBe(true);
  });

  it("fallo de configuración pasa tal cual", () => {
    const r = friendlySyncError("email deshabilitado en Ajustes");
    expect(r.transient).toBe(false);
    expect(r.text).toBe("email deshabilitado en Ajustes");
  });

  it("vacío/null → mensaje genérico", () => {
    expect(friendlySyncError("").text).toContain("No se pudo");
    expect(friendlySyncError(null).transient).toBe(false);
  });
});

describe("fmtWait", () => {
  it("segundos, minutos y horas", () => {
    expect(fmtWait(45)).toBe("45 s");
    expect(fmtWait(92)).toBe("2 min");
    expect(fmtWait(1047)).toBe("17 min");
    expect(fmtWait(3600)).toBe("1 h");
    expect(fmtWait(4500)).toBe("1 h 15 min");
  });
});
