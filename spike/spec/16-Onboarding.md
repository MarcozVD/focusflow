# Onboarding de primer arranque

Fase 16. Pantalla de bienvenida + configuración de servicios (correo IMAP e IA) al primer uso.

## Flujo

1. **Primer arranque**: `settings.onboarding.completed` ausente → la ventana principal muestra el onboarding. El widget y la bandeja no se ven afectados.
2. **Paso 1 — Bienvenida**: propuesta de valor, 3 beneficios, "Comenzar →" y "Omitir y explorar" (marca completado y entra a la app). Identidad visual: **icono real de la app** (`/icon.png`, copiado de `src-tauri/icons/128x128.png`) y colores del design system (`--primary`, gradientes del tema); sin emojis.
3. **Paso 2 — Configuración** (45/55):
   - Izquierda: instrucciones adaptativas por proveedor (Gmail / Outlook / Yahoo / Otro).
   - Derecha: formulario correo IMAP (proveedor, correo, contraseña, servidor, puerto, SSL) + IA (endpoint, modelo, clave API).
   - "Continuar →": guarda configuración, comprueba conexiones (`verify_connections`) y muestra estado por sección: ✓ conectado / error humano + "Ver detalles" técnico / omitido.
4. **Éxito**: `onboarding.completed=1` (persiste) → app normal. Nunca reaparece.
5. **Ajustes → "Volver a configurar servicios"**: reabre el onboarding con los datos pre-rellenados; cerrar no cambia el flag.

## Backend

- `onboarding_status` → `{ completed, ai: {endpoint, model, has_key}, email }` (prefill).
- `onboarding_complete` → setea `onboarding.completed=1` + log.
- `onboarding_reset` → setea `0` (tests).
- `settings_default("onboarding.completed", "0")` en setup.
- `wipe_data` borra settings → próximo arranque = onboarding de nuevo (primer arranque real).
- Sin secretos nuevos en DB: contraseña IMAP → Credential Manager (`email:{user}`), API key → `ai_api_key`.

## Seguridad

- Contraseñas y claves nunca en DB, logs ni localStorage.
- Logs: `onboarding_completed`, `verify ai=ok email=ok` (sin credenciales; `sanitize_log_line` ya activo).
- Errores técnicos en `<details>` (revelan host/modelo, no credenciales).

## Accesibilidad

- Labels + `aria-invalid`/`aria-pressed`/`role=status|alert`.
- Foco visible en todos los controles; `autofocus` en CTA principal.
- `prefers-reduced-motion`: animaciones desactivadas (fade a 0ms, spinner sin rotar).
- Responsive: 45/55 → 1 columna < 860px.

## Tests

- `lib.rs`: flag ausente por defecto / persiste / reset / wipe lo borra.
- `tests/e2e.rs` s5: ciclo completo de vida del flag con DB real en disco.
- Suite: 161 lib + 5 e2e + 6 flows + 6 phase7 = 178 verdes.

## Verificación manual

1. `data_wipe` (Ajustes → Privacidad, tecleando el token `WIPE`) → reload → onboarding visible.
2. "Omitir y explorar" → app normal; relanzar → no reaparece.
3. Configurar correo inválido → error humano con detalle técnico; bloquea Continuar.
4. Configurar IMAP+IA válidos → "Comprobando…" → ✓ ✓ → app.
5. Ajustes → "Volver a configurar servicios" → prefill correcto; cerrar → sin cambios.
6. Log: `spike.log` sin credenciales.
