# 02 — Arquitectura

**Producto:** FocusFlow · **Versión:** v0.1 · **Fecha:** 2026-08-04
**Propósito de este documento:** fijar decisiones técnicas, justificarlas y garantizar que IA, sincronización y extensibilidad futuras sean *adiciones*, no *refactors*.

---

## 1. Visión arquitectónica en una frase

> Un **núcleo de dominio en Rust** (datos, reglas, recordatorios, parser) con una **capa de presentación web ligera** (Svelte) dentro de un **shell nativo de Windows** (Tauri 2), almacenamiento **local-first en SQLite**, y **contratos de interfaz** (`INLParser`, `IReminderScheduler`, `ISyncProvider`, `IAIService`) definidos desde el día uno para que las funciones futuras se conecten como plugins.

```
┌─────────────────────────────────────────────────────────────────────┐
│  WINDOW 1: App principal (WebView2)      WINDOW 2: Widget (WebView2) │
│  Svelte 5 + TypeScript                   Svelte 5 (misma app,        │
│  Design System (doc 04)                  layout widget)              │
└──────────────┬───────────────────────────┬───────────────────────────┘
               │ IPC (comandos + eventos)  │ IPC
┌──────────────▼───────────────────────────▼───────────────────────────┐
│  SHELL: Tauri 2 (Rust)                                                │
│  · gestión de ventanas (principal, widget, captura flotante, toast)   │
│  · tray + autostart + power events (suspend/resume)                   │
│  · notificaciones nativas (Windows Toast)                             │
└──────────────┬────────────────────────────────────────────────────────┘
┌──────────────▼────────────────────────────────────────────────────────┐
│  DOMINIO (Rust crates, sin dependencias de UI)                        │
│  · core: Task, Category, Reminder, Recurrence (RRULE), events         │
│  · engine: ReminderScheduler (persistente, reloj simulado para tests) │
│  · parser: NLParser trait → rules_es / rules_en (V3: ai_provider)     │
│  · sync: SyncProvider trait (V4) · ai: AIService trait (V3)           │
└──────────────┬────────────────────────────────────────────────────────┘
┌──────────────▼────────────────────────────────────────────────────────┐
│  PERSISTENCIA: SQLite (WAL) · migraciones versionadas · backups        │
│  Operation Log (append-only) → base de sync futura                    │
└───────────────────────────────────────────────────────────────────────┘
```

---

## 2. ¿Backend? — Decisión deliberada: **no hay backend**

**La app es 100 % local y no requiere backend.** Preguntas de rigor y respuestas:

| Pregunta | Respuesta |
|----------|-----------|
| ¿Sync en la nube? | Es **futuro opcional** (V4) y se diseña ahora como contrato de interfaz. No se construye servidor en MVP |
| ¿Cuentas de usuario? | No. Sin login. Sin cuenta = sin infraestructura de cuentas |
| ¿IA? | V3: primero IA **local** (llm.cpp / Ollama / modelo pequeño) para privacidad; proveedores en la nube como alternativa tras contrato `IAIService`. En MVP, el parser de reglas no requiere red |
| ¿Telemetría? | Off por defecto. Cero endpoints en MVP |

**Consecuencia de diseño:** el límite del sistema no es "cliente/servidor" sino "UI ↔ dominio". El dominio en Rust es *independiente del shell*: la misma biblioteca podrá alimentar en V5 una app móvil (vía FFI) o un servicio de sync (como proceso de respaldo).

---

## 3. Selección de tecnologías

### 3.1 Shell de escritorio — comparativa

| Criterio (peso) | **Tauri 2** ✅ | Electron | Flutter Desktop | .NET MAUI | WPF (nativo) |
|-----------------|----------------|----------|-----------------|-----------|--------------|
| Tamaño instalador (30%) | **~8–15 MB** | 90–200 MB | 40–80 MB | 40–100 MB | 30–60 MB |
| RAM en reposo (25%) | **30–80 MB** | 150–400 MB | 80–150 MB | 80–150 MB | 60–120 MB |
| Arranque (15%) | **<1 s** (WebView2 caliente) | 1.5–4 s | 1–2 s | 1–2 s | rápido |
| Notificaciones nativas Windows (10%) | **Excelente** (Windows Toast) | Media (depende) | Media (depende) | Buena | Buena |
| Transparencia/forma libre para widget (10%) | **Nativa** (windows transparente + alfa) | Limitada (transparente, sin alfa estable en algunos casos) | Limitada | Limitada | Nativa (compleja) |
| Stack UI moderno con design system propio (10%) | Web (máxima libertad) | Web | Propietario (Skia) | XAML | XAML (legado) |
| Costo de la web a futuro (móvil) | Reuso parcial (webview) | Reuso parcial | Reuso directo UI | — | — |
| Skill/mercado 2026 | Activo y creciendo | Maduro | Desktop secundario | Ecosistema MS | Legado, sin design moderno |

**Decisión: Tauri 2.**

Justificación compacta:
1. **Cumple los NFR de peso**: binario y RAM ~10x menores que Electron — crítico para "app que vive en bandeja todo el día" (NFR-03/05).
2. **WebView2 (Chromium/Edge)** ya está en Windows 10/11: nada que instalar, motor actualizado por el propio sistema.
3. **Transparencia y forma libre reales** para el widget (FR-37): Tauri expone la ventana transparente con alfa por píxel, esencial para el widget Soft UI con esquinas redondeadas.
4. **Notificaciones nativas** vía plugin (`tauri-plugin-notification` + API de Windows Toast): las notificaciones sobreviven al cierre de la app.
5. **Rust en el núcleo** habilita el dominio de alto rendimiento y la extensión móvil futura vía FFI.
6. **Riesgo a vigilar**: WebView2 requiere Windows 10 21H2+ (aceptado, NFR-09). La inyección de código en WebView2 debe ser defensiva (Rust como autoridad de datos; JS nunca confía en la red).

### 3.2 Frontend — comparativa

| Criterio | **Svelte 5** ✅ | React | Vue |
|----------|-----------------|-------|-----|
| Performance runtime | Excelente (compila, sin VDOM) | Bueno | Bueno |
| Bundle/arranque | **Mínimo** | Mayor | Medio |
| Curva / simplicidad | Baja-media | Media | Baja |
| Reactividad finesa para animaciones | Muy buena (runes) | Buena | Buena |
| Ecosistema para esto (drag & drop, date libs) | Suficiente (`dnd-kit`-like, agnostics) | Máximo | Suficiente |

**Decisión: Svelte 5 (runes) + TypeScript.** El requisito es una UI con *muchas* micro-animaciones, drag & drop fino y cero fricción de estado: Svelte compila a JS mínimo y su modelo reactivo simplifica sincronizar calendario/agenda/lista (misma store). TypeScript obligatorio.

**Librerías frontend (elecciones a confirmar en spike):**

| Necesidad | Opción base | Nota |
|-----------|-------------|------|
| Fechas/zonas | `Temporal` (TC39) o `date-fns` | El dominio en Rust recibe/entrega ISO 8601; la UI solo formatea |
| Drag & drop | `@dnd-kit/core` (o implementación propia liviana) | Gestos: mover tarea, reordenar, doble-click |
| Iconos | `lucide-svelte` | Delgado, moderno, match con el design system |
| Gráficas stats | `uplot` (ligero) o SVG propio | Preferir SVG propio para identidad Soft UI |
| Estado | Svelte stores/runes (sin Redux) | Store único tipado; sincronizado con el dominio vía eventos |

### 3.3 Persistencia

**Decisión: SQLite (WAL) vía `sqlx` o `rusqlite` en Rust.** Justificación:
- Transaccional, cero servidor, madura, fiable en cortes de luz (WAL + checkpoint).
- El acceso **solo desde el dominio Rust** evita condiciones de carrera con el WebView.
- Migraciones versionadas (cada versión agrega una migración inmutable).
- Escala holgada para 10k–100k tareas (NFR-08) sin tocar arquitectura.
- Alternativa evaluada: `IndexedDB` (solo webview — no compartible con el widget como fuente única), `Tauri store` plugin (es para settings, no datos), `Redb/Sled` (menos maduros para consultas relacionales tipo calendario).

### 3.4 Otros componentes clave

| Componente | Elección | Motivo |
|------------|----------|--------|
| Motor RRULE (repetición) | crate `rrule` (RFC 5545 subset) | Estándar interoperable con Google/Outlook en V4 |
| Manejo de tiempo | `chrono` + política "local-first" (guardar hora local + offset; zonas como dato, no como dependencia de render) | Sincronización futura honesta |
| Notificaciones | `tauri-plugin-notification` + Windows Toast API | Nativas, con acciones (Abrir/Hecho/Posponer) |
| Ventana captura global | Ventana Tauri sin foco-robado + registro `RegisterHotKey` | Atajo de sistema Ctrl+Shift+Espacio |
| Widget | Segunda ventana Tauri transparente, same bundle | Misma app, ruta de layout distinta |
| Auto-update | `tauri-plugin-updater` | Instalador MSI/NSIS firmado |

---

## 4. Arquitectura por capas y módulos

### 4.1 Shell (Tauri, Rust) — responsabilidades

- Ciclo de vida de ventanas: principal, widget, captura global, y ventana mínima de toast si la notificación requiere UI.
- **Tray**: cerrar → bandeja; menú (Abrir, Captura rápida, Widget, Salir).
- **Autostart** y reactivación tras sleep/suspend (re-sync del scheduler de recordatorios al despertar).
- Registro del hotkey global.
- Actos de sistema: apagado → persistir estado, flush WAL.

### 4.2 Dominio (Rust) — crates

```
crates/
├── core/        # entidades + reglas puras (Task, Category, Reminder, RRULE, validaciones)
├── parser/      # trait NLParser + impl RulesParser(es|en) → TaskIntent
├── engine/      # ReminderScheduler, AgendaEngine (cálculo de ventanas), StatsEngine
├── store/       # SQLite (migraciones, repositorios), OperationLog, backups
├── sync/        # trait SyncProvider (V4: google/outlook/ical)
├── ai/          # trait AIService + impl LocalOffline / Remote (V3)
└── app/         # coordinación: comandos IPC, eventos hacia la UI, arranque
```

Reglas de dependencia (dirección única):
- `core ← parser/engine/store/sync/ai ← app ← shell`
- `core` no importa nada del resto (ninguna dependencia ascendente).
- La UI **nunca escribe directamente en SQLite**: solo comandos IPC → dominio → store. Garantiza invariantes (un recordatorio no puede quedar sin tarea, etc.).

### 4.3 Capa de presentación (Svelte)

- **Stores reactivos**: `tasksStore`, `calendarStore` (ventana visible), `uiStore` (tema, layout). Una fuente de verdad por dominio; el calendario, la agenda, la lista y el widget renderizan la misma data.
- **Patrón de comandos**: `invoke('task.create', intent)` → respuesta + evento de dominio difundido (`task:changed`) → todos los stores se actualizan (optimistic UI con reconciliación).
- **Eventos de dominio** (emitidos por Rust): `task.created`, `task.updated`, `task.completed`, `reminder.fired`, `conflict.detected`, `sync.status`. La UI es un *proyector* de estos eventos; así el widget, la ventana principal y los toasts quedan siempre consistentes.
- **Routing por estados de vista**, no por URL: `{view: calendar, granularity: week, date: 2026-08-10}` — el calendario con animación transiciona entre granularidades sin recargar.

### 4.4 Motor de recordatorios (la parte más delicada)

Requisitos: disparos exactos con app cerrada, tolerancia ±60 s, recálculo al mover tareas, sin duplicados, y testabilidad.

**Diseño:**

```
┌────────────┐  insert/update/delete task/reminder
│  Dominio   ├─────────────►┌─────────────────────────────┐
└────────────┘              │ ReminderScheduler           │
                            │ · tabla reminder_events     │
   reloj (mockeable)        │   (reminder_id, fire_at,    │
   ──► engine.poll()        │    state, task_id)          │
                            │ · persistido en SQLite      │
                            └────────────┬────────────────┘
                                         │ fire_at <= now
                                         ▼
                            ┌─────────────────────────────┐
                            │ Windows Toast + acciones    │
                            │ (Abrir / Hecho / Posponer)  │
                            └─────────────────────────────┘
```

- **Modelo por eventos**: cada `(tarea, recordatorio)` se materializa en un `reminder_events` con `fire_at` resuelto (fecha/hora UTC+offset). Cambiar la tarea = recálculo de sus eventos pendientes (FR-30), *no* acumulación.
- **Polling interno**: el scheduler dormita hasta `next_fire_at` (timer de precisión media) y al despertar (o al abrir la app, o al volver de suspend) procesa eventos vencidos. Con la app cerrada el disparo ocurre igual: el **proceso ligero de bandeja permanece vivo** (opción autostart) — esto es lo que garantiza FR-31.
- **Reloj inyectado** (`Clock` trait): los tests simulan horas y verifican que un evento programado a las 09:00 se dispara a las 09:00 ±60 s, y que mover la tarea regenera exactamente un evento (sin duplicados).
- **Exactitud de disparo**: la notificación *local* es la que se muestra al usuario (la generada por el plugin en el momento del disparo). No se programa un toast anticipado.
- **Fallo de sistema**: si la máquina estuvo apagada, los eventos vencidos entran al flujo "Mientras no estabas" (FR-35), agrupados y sin spam.

### 4.5 Parser de lenguaje natural — multicapa

**Interfaz estable (contrato desde MVP):**

```rust
pub struct TaskIntent {
    pub title: String,
    pub category_hint: Option<String>,
    pub priority_hint: Option<Priority>,
    pub start: Option<DateTime<Local>>,
    pub end: Option<DateTime<Local>>,
    pub all_day: bool,
    pub recurrence: Option<Rrule>,
    pub reminder_hints: Vec<ReminderHint>, // ej: "1 día antes"
    pub confidence: f32,                  // 0..1
    pub matched_language: Lang,
}

pub trait NLParser {
    fn parse(&self, text: &str, ctx: &ParseContext) -> ParseResult;
}
pub struct ParseContext { now: DateTime<Local>, tz: Tz, user_categories: &[Category] }
```

**Pipeline MVP (reglas, sin IA):**

```
texto → normalización (mayúsculas, "3pm"→"15:00", "prox lunes") 
      → detección de idioma (ligero)
      → extracción de expresiones temporales (relativas: mañana/próximo lunes; absolutas: 15/03, 8 AM)
      → resolución de rangos y duraciones ("de 3 a 5", "2 horas")
      → detección semántica por diccionario de categorías (examen→Universidad, pagar→Finanzas, médico→Salud)
      → detección de repetición ("todos los lunes")
      → score de confianza (entidades cubiertas vs. texto residual sin resolver)
      → TaskIntent
```

- **Reglas por idioma** (`rules_es`, `rules_en`) comparten el motor de resolución de tiempo. Nada de NLP de tercera parte en MVP → determinista, testeable, rápido, offline.
- **Contrato de V3**: una segunda implementación `AIEntityParser` del mismo trait usa un LLM local/remoto para frases complejas; el caller (la UI) elige por score o por flag. **No cambia ninguna otra capa** — esa es la razón del trait.
- **Definición de éxito del parser en tests**: las 5 frases de ejemplo del brief + 50 frases de regresión por idioma con aserciones de cada entidad.

### 4.6 Calendario y tiempo

- El dominio trabaja con `DateTime` local + offset (chrono), y serializa ISO 8601.
- La repetición se resuelve con RRULE expandido bajo demanda por ventana visible (nunca materializar toda la serie en BD).
- **Días completos** se modelan como flag `all_day` (rango [00:00, 23:59] local), no como hack de horas.
- Regla de negocio: una tarea con `start` y `end` forma bloque; si solo tiene `start`, es un *deadline* (aparece en agenda como hit de día, ideal para entregas y recordatorios de pago).

### 4.7 Widget (ventana secundaria)

```
WINDOW 2 (widget): transparente, always-on-top, frameless
└─ carga la misma Svelte app con layout "widget" (compacto/expandido)
   · datos: escucha los mismos eventos de dominio (task:changed, reminder.fired)
   · acciones mínimas: complete (comando IPC), abrir tarea (trae ventana 1 al frente + navega)
Configuración del widget persistida en settings.json (per-window en multi-monitor)
```

- El widget **no abre conexiones nuevas ni consulta BD**: es un proyector de eventos del proceso de dominio — bajo consumo, cero riesgo de corrupción.
- Transparencia real: la ventana Tauri con `transparent: true` + CSS con opacidad; las esquinas redondeadas las dibuja la propia tarjeta (nada de ventanas con esquinas negras).
- Al pausar el sistema el widget se reconstruye al reanudar desde los eventos de estado.

---

## 5. Modelo de datos — resumen (detalle en doc 03)

| Entidad | Tabla | Nota |
|---------|-------|------|
| Tarea | `tasks` | Núcleo: título, desc, categoría, prioridad, estado, progreso, fechas, notas, enlaces |
| Categoría | `categories` | Nombre, color, icono, orden |
| Etiqueta | `tags`, `task_tags` | N:M |
| Recordatorio | `reminders` | Relativo o absoluto + predefinidos |
| Evento de recordatorio | `reminder_events` | Materialización persistente (motor §4.4) |
| Repetición | `tasks.recurrence_rule` (RRULE) + `task_exceptions` | Instancias excepcionadas |
| Checklist | `task_checklist_items` | V2 |
| Adjuntos | `attachments` (P3) | Ruta local |
| Log de operaciones | `operation_log` | Append-only → sync V4 |
| Sync | `sync_state` (V4) | Watermark por proveedor |
| Ajustes | `settings` (key/value) | Tema, widget, preferencias |

---

## 6. Sincronización futura (V4) — diseño anticipado

**No se construye en MVP, pero la arquitectura ya lo habilita:**

1. **Operation Log**: cada mutación del dominio se registra (append-only) con idempotency key. Base para delta sync sin re-envío de la BD entera.
2. **Trait `SyncProvider`** (en crate `sync`):

```rust
#[async_trait]
pub trait SyncProvider {
    async fn push(&self, ops: &[Op]) -> Result<SyncWatermark>;
    async fn pull(&self, since: SyncWatermark) -> Result<Vec<Op>>;
    async fn auth(&mut self) -> Result<()>;           // OAuth local, token en DPAPI
}
```

3. **Estrategia de conflictos**: last-write-wins por campo con timestamp, más resolución manual en pantalla para casos raros; los conflictos nunca se pierden (van a una cola visible).
4. **Proveedores**: primero Google Calendar (API pública bien documentada), luego Outlook, luego iCal por archivo (file-sync). Dirección inicial 1-way (calendario → FocusFlow) para lectura; 2-way después.
5. **Sin servidor propio**: la app es peer; si algún día se necesita nube propia, el operation log es el formato de transferencia natural.
6. **Multi-dispositivo**: el mismo operation log permite replicar a móvil (V5) con la misma maquinaria.

---

## 7. IA futura (V3) — diseño anticipado

**Contrato en crate `ai` desde el MVP (aunque sin implementación):**

```rust
pub trait AIService {
    fn parse(&self, text: &str, ctx: &ParseContext) -> Result<TaskIntent>;      // parser nivel 2
    fn suggest_schedule(&self, tasks: &[Task], constraints: &Constraints) -> Result<ScheduleSuggestion>; // ventanas libres
    fn detect_conflicts(&self, tasks: &[Task]) -> Result<Vec<Conflict>>;         // solapamientos, exceso de carga
    fn decompose(&self, task: &Task) -> Result<Vec<Task>>;                      // "hacer tesis" → subtareas
    fn prioritize(&self, tasks: &[Task]) -> Result<Vec<RankedTask>>;            // qué toca primero
    fn organize(&self, tasks: &[Task]) -> Result<Vec<SuggestedCategory>>;       // categoriza lo sin categoría
}
```

**Decisiones de diseño:**
- **IA local primero** (llm.cpp / Ollama con modelo 3–8B, cuantizado): privacidad, offline, cero costo. UI igual que si fuera remota: el trait no expone dónde corre el modelo.
- **Fallback determinista**: si el modelo no está disponible o el score es bajo, el parser de reglas sigue funcionando (el producto nunca depende de IA).
- **Never-loose**: la IA sugiere; el usuario confirma (undo + preview). Nada de auto-organizar sin permiso.
- **Privacidad**: la IA remota (si se activa) es opt-in y anónima; la local es la default.
- **Coste de adopción ~cero**: porque el trait existía desde el MVP, conectar Ollama o un API es una implementación nueva, no un refactor.

---

## 8. Escalabilidad y extensibilidad (la app en el futuro)

| Eje | Mecanismo |
|-----|-----------|
| Colaboración (V6) | Operation log → replicación entre peers; permisos por categoría (futuro) |
| Sync nube | `SyncProvider` (§6) |
| Multi-dispositivo | Operation log + mobile core vía FFI (Rust → móvil) |
| Multi-usuario | El dominio es single-user por diseño; el paso a multi-user afecta solo la capa de sync + permisos, no el core |
| Plugins (V6) | API de comandos Tauri expuesta; eventos de dominio publicados; manifesto de plugin JSON |
| Datos grandes | Índices cubiertos (doc 03 §4); ventanas por rango; agenda virtualizada |
| i18n/mercados | Tokens de texto centralizados; parser multilingüe por trait |

---

## 9. Seguridad y privacidad

| Tema | Política |
|------|----------|
| Datos | 100 % locales en MVP; SQLite en `%APPDATA%\FocusFlow` |
| Red | Cero llamadas de red en MVP; en V3/V4 solo si el usuario activa IA/sync |
| Tokens de sync futuros | Windows DPAPI (usuario local) |
| WebView | `shell` hardening: puerto de IPC local efímero, `contextIsolation`, sin `dangerous_remote_domain`; el frontend nunca toca red salvo funciones explícitas |
| Adjuntos | Rutas validadas contra path traversal |
| Telemetría | Off por defecto; opt-in anónima (P3) documentada en ajustes |
| Backups | Copia rotativa del WAL; verificación de checksum al restaurar |

---

## 10. Riesgos técnicos y mitigaciones

| # | Riesgo | Prob. | Impacto | Mitigación |
|---|--------|-------|---------|------------|
| R1 | Parser NL subestima variedad de frases reales | Alta | Media | Contrato trait + tests de regresión de 50+ frases; score de confianza con preview editable; V3 IA como nivel 2 |
| R2 | Recordatorios fallan con app cerrada (proceso de bandeja muerto) | Media | Alta | Arquitectura proceso vivo + autostart opcional; suite con reloj simulado; test de humo manual en CI humano; "Mientras no estabas" como red de seguridad UX |
| R3 | Rendimiento del WebView con vistas grandes | Media | Media | Virtualización de listas; consultas por rango; benchmark en CI (NFR-02) |
| R4 | RRULE edge cases (semana del año, "último lunes") | Media | Baja | Usar crate probado `rrule`; tests contra ejemplos de RFC 5545 |
| R5 | Migraciones de BD en beta (datos de usuarios tempranos) | Media | Media | Migraciones versionadas inmutables + backup pre-migración |
| R6 | Windows Toast varía entre versiones de OS | Baja | Media | Abstracción de notificación en el shell; fallback a notificación in-app |
| R7 | Efecto Soft UI mal renderizado en pantallas con escala ≠100% / dark | Media | Media | Design tokens con valores en rem; tests visuales en escalas 100/125/150 % |
| R8 | Feature creep de "app de empresa" | Media | Alta | Backlog MoSCoW fijo; review de cada feature contra persona primaria |
| R9 | Fragilidad del drag & drop (touch/mouse/estilos) | Media | Media | librería probada o implementación propia acotada; tests E2E de los 3 gestos críticos |
| R10 | Adopción: usuario no descubre el NL | Media | Alta | Onboarding con demo animada; el input NL es el elemento dominante de la UI; ejemplos inline en placeholder |

---

## 11. Preguntas abiertas (para spike técnico de 1 semana)

> **✅ RESUELTO — ver doc 07 (Informe del Spike, 2026-08-04):**
> 1. **Temporal** confirmado en WebView2 151 → usar Temporal en la UI.
> 2. **`rusqlite`** (bundled) + `spawn_blocking` — sin necesidad de sqlx.
> 3. Transparencia widget + always-on-top **verificados** en Windows 11 (WS_EX_TOPMOST + capas LAYERED).
> 4. Hotkey global **verificado**; conflicto ya no crashea (fallback por candidatos).
> 5. Arranque 971 ms frío / 328 ms caliente (NVMe); RAM 26.5–33.2 MB; exe 3.27 MB.
> 6. Firmado MSI: pendiente (requiere certificado; planificar antes de V1 release).

**Cambios incorporados tras el spike:**
- `tauri-plugin-single-instance` pasa a ser requisito del MVP (2ª instancia delega a la 1ª).
- Hotkey global **configurable + detección de conflictos** sube de V2 a MVP (FR-57 parcial).
- Tests visuales de QA usarán Playwright (el AV bloquea captura por CopyFromScreen).
