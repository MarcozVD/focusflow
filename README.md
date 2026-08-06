# FocusFlow

> Productividad personal para Windows: calendario, tareas, agenda, recordatorios y widget de escritorio en una sola superficie **local, instantánea y bella**.

FocusFlow concentra en una sola app todo lo que normalmente vives repartido entre calendarios, apps de tareas y notas. Captura cualquier compromiso escribiendo una frase ("Mañana estudiar cálculo de 3pm a 5pm"), te recuerda en el momento exacto y funciona **sin nube**: los datos viven en SQLite local.

- 💾 **Local-first**: 100 % funcional sin internet, sin cuentas, sin suscripción
- ⚡ **Captura en segundos**: entrada por lenguaje natural con Ctrl+Shift+Espacio
- 🧩 **Un solo modelo mental**: eventos, entregas, citas y pagos son *tareas*
- 🪟 **Widget de escritorio**: transparente, sin marco, siempre visible
- 🤖 **IA enchufable**: parser por reglas + proveedor de IA configurable (compatible OpenAI)
- 📬 **Sincronización de correo**: sugerencias de tareas desde tus emails (IMAP)

---

## Características

### Calendario y agenda
- Vistas **mes, semana y día** con horario visible 6:00–22:00 que **se expande automáticamente** si una tarea ocurre fuera de esa franja
- **Agenda mixta**: tareas y eventos en una línea de tiempo unificada
- **Drag & drop** para reprogramar y redimensionar tareas con ajuste a bloques de 15 min y **validación de conflictos** (aviso al soltar si se solapa con otra tarea)
- Tareas **multi-día** (marcadas "⟳ continúa" en días intermedios) y tareas **todo el día** (sección fija estilo Google Calendar)
- Hoy destacado, navegación rápida por días/semanas/meses

### Captura
- **Entrada rápida global** con `Ctrl+Shift+Espacio` desde cualquier aplicación
- **Lenguaje natural**: "Tengo examen de física el próximo lunes a las 8 AM", "Pagar internet el 15" → la IA completa categoría, prioridad y horario. Si no hay hora explícita, la tarea se crea como **Todo el día** (nunca se inventan horas)
- Formulario completo: título, descripción, categoría, prioridad, etiquetas, fechas, recordatorio, notas y enlaces

### Organización
- 6 categorías con color e icono (Universidad, Trabajo, Personal, Finanzas, Salud, Otros)
- Prioridades Alta / Media / Baja, etiquetas libres, progreso, estado (pendiente, en curso, completada, vencida)
- Arrastrar para reprogramar actualiza recordatorios y notificaciones al instante

### Widget de escritorio
- Ventana **transparente sin marco**: solo el contenido flotando sobre el escritorio
- Mismo diseño que la app (colores, sombras, tipografía, animaciones) y **tema sincronizado** con persistencia claro/oscuro
- Escalado automático: compacto con pocas tareas, crece con más; con más de 8–10 muestra "+N tareas más" y abre la app en Agenda
- Clic en una tarea → abre la app con esa tarea; completar directamente desde el widget

### IA y correo
- **Parser por reglas** (offline) + **proveedor IA** configurable (endpoint y modelo compatibles con OpenAI, clave opcional)
- **Sincronización de correo IMAP**: detecta compromisos en emails, genera *sugerencias* que puedes aceptar, rechazar, editar o fusionar con tareas existentes
- Filtros por remitentes, dominios y palabras clave + lista de remitentes de confianza
- Estado de sincronización "de hoy": correos revisados, tareas creadas, errores y próxima sincronización

### Sistema
- Bandeja del sistema: cerrar → minimizar a bandeja, menú con acciones rápidas
- Inicio con Windows opcional, empezar en segundo plano con solo el widget
- Tema claro/oscuro Soft UI con persistencia entre reinicios
- Persistencia en SQLite con WAL, sin dependencia de red

---

## Arquitectura

```
┌─────────────────────────────────────────────────────────────────────┐
│  WINDOW 1: App principal (WebView2)      WINDOW 2: Widget (WebView2) │
│  Svelte 5 + TypeScript (misma SPA,       Svelte 5 (layout widget,    │
│  views: mes/semana/día/agenda/…)          transparente, auto-altura) │
└──────────────┬───────────────────────────┬───────────────────────────┘
               │ IPC: comandos Tauri + eventos (tasks:changed, …)     │
┌──────────────▼──────────────────────────────────────────────────────┐
│  SHELL: Tauri 2 (Rust)                                               │
│  · ventanas (main, widget, captura) · tray · autostart · hotkey      │
│  · notificaciones nativas (Windows Toast) · single-instance          │
└──────────────┬───────────────────────────────────────────────────────┘
┌──────────────▼───────────────────────────────────────────────────────┐
│  DOMINIO (Rust — sin dependencias de UI)                             │
│  · store: SQLite (rusqlite, WAL, migraciones)                        │
│  · ai: parser de lenguaje natural por reglas + proveedor IA remoto   │
│  · sync: motor IMAP → sugerencias de tareas + scheduler periódico    │
│  · comandos IPC: task_*, suggestion_*, ai_*, email_*, widget_*, …    │
└──────────────────────────────────────────────────────────────────────┘
```

**Principios**:
- **Sin backend**: el límite del sistema es "UI ↔ dominio", no "cliente ↔ servidor". El dominio en Rust es independiente del shell.
- La UI **nunca escribe directamente en SQLite**: solo comandos IPC → dominio → store, garantizando invariantes.
- La IA y la sincronización son **adiciones** sobre contratos definidos desde el día uno; la app funciona sin clave de IA ni correo configurado.

**Stack**: Tauri 2 (Rust) · Svelte 5 (runes) + TypeScript · SQLite (rusqlite, WAL) · tauri-plugin-notification / global-shortcut / single-instance. Justificación completa en [`spec/02-Arquitectura.md`](spec/02-Arquitectura.md).

---

## Estructura del repositorio

```
focusflow/
├── README.md              ← este archivo
├── spec/                  Especificación completa del producto (7 documentos)
│   ├── 01-PRD.md          Visión, requisitos, casos de uso, AC del MVP
│   ├── 02-Arquitectura.md Stack, capas, decisiones justificadas
│   ├── 03-Modelo-Datos.md Modelo de datos (ER + tablas + índices)
│   ├── 04-Design-System.md Tokens Soft UI 2.0 (claro/oscuro, sombras, motion)
│   ├── 05-Roadmap.md      Fases MVP → V5 con esfuerzos y exit criteria
│   ├── 06-Auditoria.md    Crítica competitiva + propuestas
│   └── 07-Spike-Tecnico.md Resultados de la validación técnica
├── spike/                 App real (Tauri 2 + Svelte 5 + Rust)
│   ├── frontend/          UI (Svelte 5 + Vite + TypeScript)
│   ├── src-tauri/         Shell y dominio Rust (store, ai, sync, comandos)
│   └── tools/             Utilidades de desarrollo (mocks IA/IMAP, e2e)
└── proto/                 Prototipo Svelte 5 del diseño (validación de UX)
```

---

## Compilar y ejecutar

Requisitos: Windows 10 21H2+, Rust (stable), Node.js 18+, WebView2 (incluido en Windows 11).

```powershell
# 1) Frontend
cd spike\frontend
npm install
npm run build                # → dist/ (assets que Tauri embebe)

# 2) Backend / app
cd ..\src-tauri
cargo build --release        # → target\release\focusflow-spike.exe

# 3) Ejecutar
.\target\release\focusflow-spike.exe
```

Alternativa en desarrollo (hot reload):
```powershell
cd spike\frontend
npm run dev                  # Vite en http://localhost:5173
cd ..\src-tauri
cargo tauri dev
```

### Configuración

| Dato | Dónde se guarda |
|------|-----------------|
| Base de datos | `%APPDATA%\com.focusflow.spike\focusflow.db` (SQLite, fuera del repo) |
| Clave de IA | Configurada desde **Ajustes** (nunca en el repositorio) |
| Contraseña de correo | Configurada desde **Ajustes** |
| Variables de entorno (dev) | `AI_ENDPOINT`, `AI_MODEL`, `AI_API_KEY`, `FF_EMAIL_PASSWORD` |
| Logs de diagnóstico | `%TEMP%\focusflow-spike\spike.log` |

### Herramientas de desarrollo

```powershell
# Servidores mock para probar IA y correo sin red
node spike\tools\mock-ai.mjs      # OpenAI-compatible en :9410
node spike\tools\mock-imap.mjs    # IMAP local en :1143

# Prueba e2e completa (mocks + app)
.\spike\tools\run-e2e-email.ps1
```

---

## Estado del proyecto

| Componente | Estado |
|------------|--------|
| Spec (7 documentos) | ✅ Completa |
| Prototipo Svelte | ✅ Funcional |
| App Tauri + Svelte | ✅ En desarrollo activo (v0.1) |

Ver hoja de ruta en [`spec/05-Roadmap.md`](spec/05-Roadmap.md) y auditoría competitiva en [`spec/06-Auditoria.md`](spec/06-Auditoria.md).
