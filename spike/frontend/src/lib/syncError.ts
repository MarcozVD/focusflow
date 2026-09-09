export interface FriendlySyncError {
  /** Texto corto para la UI: advertencia, no error crudo. */
  text: string;
  /** true = transitorio (429, red, Gmail caído): el sync se reintentará solo. */
  transient: boolean;
  /** Segundos hasta el reintento automático, si se conocen. */
  retrySecs?: number;
}

/**
 * Convierte errores crudos de sync (`ia_429 [s] detalle`, JSON de Gmail API,
 * errores de red) en advertencias legibles con tiempo de reintento.
 * El detalle técnico (org, códigos, JSON) nunca llega a la UI.
 */
export function friendlySyncError(e: unknown): FriendlySyncError {
  const s = String(e ?? "").trim();
  if (!s || s === "undefined") {
    return { text: "No se pudo completar la revisión de correo.", transient: false };
  }

  // ia_429 [segundos] detalle — límite diario de la IA
  const m = s.match(/^ia_429(?:\s+(\d+(?:\.\d+)?))?(?:\s+(.*))?$/s);
  if (m) {
    const waitSecs = m[1] ? Math.round(Number(m[1])) : undefined;
    const wait = waitSecs
      ? ` Se reintentará en ${fmtWait(waitSecs)}.`
      : " Se reintentará automáticamente en unos minutos.";
    return {
      text: `La IA alcanzó su límite diario de análisis; hoy no puede procesar más correos hasta el reinicio.${wait}`,
      transient: true,
      retrySecs: waitSecs,
    };
  }
  if (s.startsWith("ia_fail")) {
    return { text: "La IA no respondió correctamente. Se reintentará automáticamente en unos minutos.", transient: true };
  }

  // Gmail API: 403 SERVICE_DISABLED / accessNotConfigured (JSON crudo incluido)
  if (s.includes("SERVICE_DISABLED") || s.includes("accessNotConfigured")) {
    return {
      text: "No se pudo conectar con Gmail: el servicio aún no está activado en el proyecto de Google. No es un problema de tu cuenta ni de la app; activa Gmail API en Google Cloud Console y la app seguirá reintentando hasta entonces.",
      transient: true,
    };
  }
  if (/gmail api 429|Too Many Requests/i.test(s)) {
    return { text: "Gmail está recibiendo demasiadas consultas seguidas de la app. Espera unos minutos y volverá a funcionar solo; no se perdió nada.", transient: true };
  }
  if (/gmail api 401|invalid credentials/i.test(s)) {
    return { text: "Tu sesión de Google caducó. Cierra sesión y vuelve a iniciarla en Ajustes para seguir revisando el correo.", transient: false };
  }
  if (/gmail api 403/i.test(s)) {
    return { text: "La app no tiene permiso para leer este buzón. Revisa los permisos de la cuenta en Ajustes o vuelve a conectarla.", transient: false };
  }
  if (/gmail api 5\d\d/i.test(s)) {
    return { text: "Gmail está temporalmente caído (problema de Google, no tuyo). Se reintentará automáticamente.", transient: true };
  }
  if (/conexión fallida|network|timeout|timed out|connection/i.test(s)) {
    return { text: "Problema de conexión con Gmail. Se reintentará automáticamente en unos minutos.", transient: true };
  }

  // fallos de configuración: el mensaje ya es accionable, se pasa tal cual
  return { text: s, transient: false };
}

/** Segundos → texto humano: "2 min", "17 min", "1 h 15 min". */
export function fmtWait(secs: number): string {
  if (secs >= 3600) {
    const h = Math.floor(secs / 3600);
    const m = Math.round((secs % 3600) / 60);
    return m > 0 ? `${h} h ${m} min` : `${h} h`;
  }
  if (secs >= 90) return `${Math.round(secs / 60)} min`;
  return `${secs} s`;
}
