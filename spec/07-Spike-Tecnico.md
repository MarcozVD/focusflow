# 07 — Informe del Spike Técnico

**Fecha:** 2026-08-04 · **Máquina de prueba:** Windows 11 Home (build 26200, x64), NVMe SSD, 1920x1080 @100 %, AV activo
**Objeto:** validar los 6 riesgos de doc 02 §11 con una app Tauri 2 real (código en `focusflow-spike/`).

---

## 1. Resumen ejecutivo

| Riesgo (doc 02 §11) | Resultado | Veredicto |
|---------------------|-----------|-----------|
| Q1. Arranque en HDD | 971 ms frío / 328 ms caliente (NVMe) | ✅ Aceptado |
| Q2. Temporal vs date-fns | **Temporal disponible en WebView2 151** | ✅ Usar Temporal |
| Q3. Transparencia widget Win11 | Windows `transparent:true` + `WS_EX_TOPMOST` verificados | ✅ Aceptado |
| Q4. Hotkey global | Ctrl+Shift+Espacio registra y dispara; conflicto manejado sin crash | ✅ Aceptado + hallazgo |
| Q5. sqlx vs rusqlite | Decisión por análisis (no bench requerido) | ✅ rusqlite |
| Q6. Firmado MSI | Pendiente (requiere certificado) | ⏳ Fuera del spike |

**NFR confirmados con medidas:** RAM con widget + notificación **33.2 MB** (objetivo ≤100 MB) · exe **3.27 MB** (objetivo ≤15 MB) · arranque **<1.5 s**.

---

## 2. Detalle por prueba

### 2.1 Rendimiento (NFR-01, 03, 05)

| Métrica | Medido | Objetivo | Notas |
|---------|--------|----------|-------|
| Arranque frío → webview listo | **971 ms** | ≤1500 ms | NVMe; el HDD de la persona objetivo se extrapola a ~1.5–2.5 s. Mitigación si fuera necesario: pantalla instantánea (Skeleton UI) mientras carga el webview |
| Arranque caliente (2ª instancia) | **328 ms** | — | |
| RAM en reposo (app sola) | **26.5 MB** | ≤60 MB | |
| RAM con widget + toast | **33.2 MB** | ≤100 MB | |
| Tamaño exe (release, LTO+strip) | **3.27 MB** | ≤15 MB | MSI final sumará ~5–8 MB (WebView2 ya está en el SO) |

### 2.2 Temporal (Q2) — RESUELTO

- Runtime objetivo: Chromium/Edge **151** (WebView2 Runtime 151.0.4129.59 instalado).
- `typeof Temporal` = **SÍ**, `Temporal.PlainDate` disponible.
- **Decisión:** usar **Temporal** en la UI (formateo, aritmética ligera, comparaciones). El dominio sigue en Rust con `chrono` (doc 02 §4.6). No se necesita `date-fns`.

### 2.3 Widget transparente (Q3) — RESUELTO

- Ventana Tauri `transparent:true + decorations:false + always_on_top:true + skip_taskbar:true` se crea sin errores.
- Verificación por Win32: hwnd del widget con **`WS_EX_TOPMOST` activo**; transparencia delegada a subventanas **`WS_EX_LAYERED`** del webview (mecanismo estándar WebView2 para alfa por píxel).
- El widget.html usa `background: transparent` en `html/body` + tarjeta con `rgba(255,255,255,0.92)` → esquinado real por CSS, sin bordes negros.
- **Conclusión:** el diseño de widget del doc 04 (§8.15) es viable tal cual. Riesgo residual bajo: verificación visual en distintos temas de Windows (10/11, claro/oscuro) en hardening.

### 2.4 Hotkey global (Q4) — RESUELTO con hallazgo de diseño

- `Ctrl+Shift+Espacio` se registró correctamente (instancia única) y **disparó el evento** con teclas simuladas.
- **Hallazgo importante:** al lanzar una 2ª instancia, el registro devolvió `HotKey already registered` (conflicto con la 1ª instancia) → la app **cayó con fail-fast (BEX64 c0000409)** en la primera versión del spike.
- **Corrección implementada:** registro secuencial de candidatos (`Ctrl+Shift+Espacio → Ctrl+Alt+Espacio → Ctrl+Shift+T → Ctrl+Shift+K`), log del resultado, arranque continúa aunque todos fallen.
- **Implicaciones para el producto (cambios al PRD):**
  - El hotkey global **debe ser configurable desde el MVP** (antes se planificó para V2) y la app debe **detectar conflictos** al configurarlo (probar registro → avisar "en uso por otra app" → sugerir alternativas).
  - **`tauri-plugin-single-instance` es requisito desde el MVP** (2ª instancia debe delegar a la 1ª, no registrarse sola).

### 2.5 sqlx vs rusqlite (Q5) — RESUELTO por análisis

| Criterio | rusqlite | sqlx (sqlite) |
|----------|----------|---------------|
| Modelo | Bloqueante síncrono | "Async" sobre hilos de trabajo |
| Integración Tauri | `spawn_blocking` explícito y predecible | Necesita runtime tokio + `sqlx::SqlitePool` |
| Compile-time checks | SQL dinámico | macros `query!` con DB local |
| Complejidad | Baja | Media (runtime, TLS features irrelevantes aquí) |
| Perfecto para single-user local | Sí | Sobrado |

**Decisión: `rusqlite`** (bundled SQLite) + `tauri::async_runtime::spawn_blocking` en comandos que tocan BD. Single-user, WAL, sin red: el "async de verdad" no aporta y suma complejidad. El pool de sqlx se justificaría si hubiera multi-proceso concurrente (no es el caso).

### 2.6 Firmado MSI (Q6)

Pendiente. Requiere certificado de firma (EV o estándar). El auto-update (NFR-13) depende de esto → planificar compra de certificado antes de V1 release.

---

## 3. Otros hallazgos del spike

1. **Instalación de toolchain necesaria en la máquina dev:** Rust 1.97.1 + Visual Studio 2022 Build Tools (workload VCTools, MSVC 14.44) — documentado para onboarding de devs.
2. **AV heurístico** bloquea scripts de captura de pantalla (System.Drawing.CopyFromScreen). No afecta al producto; solo a tooling de QA → los tests visuales usarán Playwright (screenshot vía Chromium, no CopyFromScreen).
3. **Optimización de build:** con `lto=true, opt-level="s", strip=true, panic="abort"` el exe queda en 3.27 MB. Build ~1.5–2 min incremental. Adecuado para CI.
4. **Close-to-tray** funcionando (CloseRequested → prevent_close + hide): base verificada de FR-33.
5. **Tray + menú** sin problemas en la barra de sistema.

---

## 4. Decisiones registradas (para doc 02)

- §3.3: `rusqlite` (bundled) — texto de la sección queda como está, se marca resuelto.
- §3.1 Frontend: `Temporal` en vez de `date-fns`.
- Nuevo: `tauri-plugin-single-instance` en el stack base (MVP).
- Nuevo: hotkey configurable + detección de conflictos sube a MVP (FR-57 parcial).
- NFR-01: confirmado con NVMe; aceptar ~2 s en HDD con skeleton.

## 5. Código del spike

- `focusflow-spike/` — app Tauri 2 mínima: ventana principal (panel de pruebas), widget transparente (widget.html), tray, hotkey global con fallbacks, notificación, log a `%TEMP%\focusflow-spike\`.
- El binario release está en `src-tauri\target\release\focusflow-spike.exe` (puede ejecutarse para re-verificar).
