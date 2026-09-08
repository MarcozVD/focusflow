# PRODUCT.md — FocusFlow

**Estado:** Documento vivo · **Última actualización:** 2026-08-31 · **Fuente:** código real de `spike/frontend/` + `spec/`

---

## 1. Qué es FocusFlow

FocusFlow es una aplicación de escritorio para Windows (Tauri 2 + Svelte 5) que funciona como **asistente personal de tiempo**. No es un calendario con IA encima: es una herramienta que ayuda a decidir **qué hacer, cuándo hacerlo y cómo organizarlo usando el tiempo disponible**.

El producto combina cuatro piezas que normalmente viven separadas:

| Pieza | Rol en FocusFlow |
|-------|------------------|
| **Tareas** | La unidad central de trabajo: tienen categoría, prioridad, estado, horario (o día completo) y duración. |
| **Calendario** | El **espacio visual del tiempo**: semana, día y mes. Ocupado/libre/urgente/completado se leen de un vistazo. |
| **Correo (Gmail/IMAP)** | Una fuente de **contexto**: detecta fechas, exámenes, reuniones y vencimientos y los convierte en *sugerencias* que el usuario aprueba. |
| **IA** | El **razonamiento sobre el tiempo**: interpreta lenguaje natural, detecta tiempo libre, propone planes y explica sus decisiones. |

### 1.1 Relación entre las piezas

```
QuickAdd / Correo / Asistente
        │  (lenguaje natural)
        ▼
  Interpretación (IA + parser local)
        │
        ▼
   Motor de planificación
   (disponibilidad, prioridades, conflictos)
        │
        ▼
      PROPUESTA  ──►  revisión del usuario  ──►  aprobación
        │                                          │
        ▼                                          ▼
     SQLite local                        Calendario / Agenda / Widget
```

**Regla fundamental:** nada toca el calendario sin aprobación explícita. La IA **propone → explica → espera aprobación → aplica**.

---

## 2. Problema que resuelve

La mayoría de las herramientas de productividad te obligan a **gestionar constantemente tu propio horario**: copiar fechas de correos, calcular manualmente si "tengo tiempo", arrastrar tareas una a una, y vivir con la sensación de que el calendario es un registro del pasado en vez de un plan del futuro.

El tiempo está fragmentado en: tareas (Todoist/notas), calendario (Google Calendar), correo (Gmail), notas (cerebro). Reunir todo eso es trabajo manual.

FocusFlow resuelve el problema central: **ayudarte a entender tu tiempo y decidir qué hacer con él**, sin que organizar se convierta en otro trabajo de tiempo completo.

---

## 3. Usuario principal

Estudiante universitario / profesional joven en Windows que:

- vive entre **tareas, correo y calendario** sin un lugar único;
- necesita **estudiar/trabajar con fechas límite** (exámenes, entregas, reuniones);
- quiere **aprovechar el tiempo libre** en lugar de perderlo;
- valora que las cosas sean **locales, rápidas y predecibles**;
- desconfía de la IA que "hace cosas sola" — quiere que **proponga y explique**.

Es un usuario técnico-cercano (puede configurar un endpoint de IA), pero el producto está diseñado para no requerirlo: hay presets (Groq, OpenCode Zen, Gemini) y un parser local como fallback.

---

## 4. Jobs-to-be-done

| JTBD | Cómo lo resuelve FocusFlow |
|------|----------------------------|
| "Captura esto antes de que se me olvide" | **QuickAdd** (`Ctrl+Shift+Espacio`) acepta lenguaje natural: *"Mañana estudiar cálculo de 3pm a 5pm"*. |
| "¿Cuándo tengo tiempo para X?" | **Asistente** analiza disponibilidad real y responde con propuestas de bloques. |
| "¿Qué hago ahora / después?" | **Widget** muestra Ahora / Siguiente / Importante de un vistazo, con acciones rápidas. |
| "Ese correo menciona una fecha" | **Email Intelligence** extrae sugerencias con confianza y razón; el usuario acepta/edita/fusiona. |
| "Tengo que entregar esto el viernes" | Parser de lenguaje natural crea la tarea + plan con sesiones hasta el vencimiento. |
| "Mi día está lleno pero siento que no hice nada" | El calendario hace visible el tiempo: ocupado vs libre, urgencia, vencidos. |

---

## 5. Propuesta de valor

> **FocusFlow helps you understand your time and decide what to do with it.**

Tres ejes, en orden de importancia (la IA nunca es la protagonista del mensaje):

1. **Claridad** — el tiempo se vuelve legible: ocupado, libre, urgente, completado, vencido.
2. **Tiempo** — recuperar el tiempo que hoy se pierde organizando.
3. **Planificación inteligente** — la IA sugiere y explica; el usuario decide.

---

## 6. Principios del producto

1. **El usuario manda.** La IA propone, nunca aplica en silencio.
2. **Local-first.** Los datos viven en SQLite local; credenciales en el Administrador de credenciales de Windows.
3. **Rápido por diseño.** Parser local de reglas como fallback instantáneo cuando la IA tarda (umbral de 8 s en QuickAdd).
4. **Predecible.** La priorización es **determinista** (URGENTE/IMPORTANTE/NORMAL), la IA no inventa datos.
5. **Contextual, no intrusivo.** Notificaciones solo cuando hay algo útil (con tope diario y horario de silencio).
6. **Táctil y tangible.** Neumorfismo como sistema de profundidad, no decoración.
7. **Sin humo.** Las afirmaciones de privacidad se limitan a lo que el producto puede demostrar.

---

## 7. Funcionalidades principales

- **Calendario** — mes, semana y día; drag & drop; redimensionado; conflictos validados (aviso o bloqueo estricto); tareas multi-día y de día completo; línea de "ahora".
- **Agenda** — próximos 5 días con presencia, hoy primero; agrupación cronológica.
- **QuickAdd** — captura por lenguaje natural con previsualización de entidades detectadas (fecha, hora, categoría, prioridad, recordatorio); auto-aceptación de eventos únicos.
- **Asistente IA** — hilo conversacional que responde sobre tu tiempo, propone planes y acciones; cada acción requiere confirmación; reintento ante rate-limit.
- **Planificación inteligente** — texto → *propuesta* (entendimiento + plan con sesiones + advertencias "tiempo insuficiente") → edición de bloques → aceptación.
- **Email Intelligence** — IMAP/OAuth2, detección de eventos/vencimientos/disponibilidad con confianza y razón, deduplicación, fusión con tareas existentes, remitentes de confianza (auto-aprobación opcional).
- **Widget de escritorio** — ventana transparente, siempre visible; Ahora/Siguiente/Importante; acciones rápidas (completar, posponer, empezar); abre la app.
- **Notificaciones contextuales** — vencimientos, atrasadas, conflictos, tiempo libre; cadencia, silencio, tope diario.
- **Temas** — claro/oscuro + 6 acentos; aplicados de forma sincronizada app+widget.
- **Privacidad** — exportar datos (JSON), borrar todo, reporte de errores sin datos personales.

---

## 8. Modelo de interacción con IA

### 8.1 El contrato: propone → explica → espera aprobación → aplica

1. **Propone** — el usuario escribe en lenguaje natural (QuickAdd o Asistente).
2. **Explica** — la propuesta muestra "Entendí" (intents: Evento/Tarea/Vencimiento/Preparación/Disponibilidad/Recordatorio/Restricción), horas requeridas vs planificadas y notas ("Tiempo insuficiente: se planificaron 2 de 4 horas").
3. **Espera aprobación** — botones Aceptar / Editar / Cancelar. El usuario puede reordenar bloques antes de aceptar.
4. **Aplica** — solo al aceptar se escriben las tareas en el calendario.

### 8.2 Tipos de respuesta del asistente

| Tipo | Qué hace | Control del usuario |
|------|----------|---------------------|
| `Answer` | Responde preguntas sobre la agenda, con referencias a tareas (URGENTE/IMPORTANTE/NORMAL). | Nada se modifica. |
| `Plan` | Propuesta de plan con sesiones por ítem. | Aceptar / Editar / Descartar. |
| `Action` | Acción concreta (completar, reagendar, crear evento, cancelar propuesta). | Confirmar / Descartar. |
| `Nothing` | No hay nada útil que hacer. | — |

### 8.3 Fallbacks honestos

- **Parser local** de reglas (instantáneo, sin red) cuando la IA tarda > 8 s.
- Si la IA falla, se informa con un mensaje claro y un botón de **Reintentar** (para errores 429).
- La última petición gana: respuestas tardías se descartan (anti-condición-de-carrera).

### 8.4 Qué NO es la IA de FocusFlow

- No es un chatbot genérico pegado al calendario.
- No toma control silencioso: **ninguna** tarea se crea, mueve o completa sin confirmación.
- No inventa datos: la priorización es determinista y el contexto enviado es mínimo.
- No convierte *cualquier* correo en evento automáticamente (salvo remitentes de confianza elegidos por el usuario).

---

## 9. Papel del widget

El widget es la **extensión de un vistazo** del producto, no una mini-app:

- **open → glance → act**: abre con la app (o desde bandeja), se lee en segundos, y permite actuar (✓ completar, ⟳ posponer 1 h, ▶ empezar) sin abrir la ventana principal.
- Prioridad de información: **Ahora** (actividad en curso) → **Por hacer** → **Siguiente** → **Importante** → "Todo claro por ahora".
- Abre tareas en la app principal; "Preguntar" salta al Asistente.
- Comparte tema y acento con la app principal (evento `ui:prefs`).

---

## 10. Qué NO debe convertirse FocusFlow

- ❌ **No** es un calendario genérico con IA de relleno.
- ❌ **No** es una app móvil/web-first (es desktop-first para Windows).
- ❌ **No** es una plataforma social ni colaborativa.
- ❌ **No** es una nube de calendarios (local-first; la nube es futuro opcional).
- ❌ **No** es un CRM, un gestor de proyectos ni un reemplazo de Slack.
- ❌ **No** es un asistente autónomo que "se encarga de todo" sin supervisión.

---

## 11. Estado actual y pruebas

- Frontend: Svelte 5 + TypeScript + Vite, CSS custom (neumorfismo), Vitest (lógica pura en `taskDayLogic.ts`, errores del asistente, utils).
- Backend: Rust/Tauri 2, SQLite, motor de planificación, IMAP/OAuth2, notificaciones nativas (216 tests Rust).
- La demo en navegador (`npm run dev`) funciona con tareas de ejemplo y sin auth (flujo completo solo en Tauri).
