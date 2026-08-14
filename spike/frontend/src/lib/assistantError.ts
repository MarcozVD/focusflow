export interface FriendlyAssistantError {
  text: string;
  retryable: boolean;
  waitSecs?: number;
}

/** Mensaje amigable para el usuario; el detalle técnico queda en el log. */
export function friendlyAssistantError(e: unknown): FriendlyAssistantError {
  const s = String(e);
  const m = s.match(/^ia_429(?:\s+(\d+(?:\.\d+)?))?\s/);
  if (m) {
    const wait = m[1] ? Math.round(Number(m[1])) : 0;
    let text =
      "El proveedor de IA está temporalmente saturado (límite de peticiones). Espera un momento y vuelve a intentarlo.";
    if (wait > 0) text += ` Inténtalo de nuevo en ~${wait} s.`;
    return { text, retryable: true, waitSecs: wait || undefined };
  }
  if (s.startsWith("Error: ia_429") || s.includes("saturado por el límite de peticiones")) {
    return { text: "El proveedor de IA está temporalmente saturado (límite de peticiones). Inténtalo de nuevo en un momento.", retryable: true };
  }
  return { text: s, retryable: false };
}