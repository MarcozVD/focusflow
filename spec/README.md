# FocusFlow — Especificación del Producto

> **Codename:** FocusFlow (provisional)
> **Tipo:** Aplicación de productividad personal para Windows (desktop-first)
> **Estado del documento:** v0.1 — Borrador para revisión
> **Fecha:** 2026-08-04
> **Autor:** Spec redactada en modo Senior PM / Architect / UX / Tech Lead

---

## Mapa de documentos

| # | Documento | Contenido | Estado |
|---|-----------|-----------|--------|
| 01 | [PRD — Product Requirements Document](01-PRD.md) | Visión, problema, personas, casos de uso, requisitos funcionales y no funcionales, flujos de navegación, UX, criterios de aceptación del MVP, backlog MoSCoW/RICE, funcionalidades futuras | ✅ Redactado |
| 02 | [Arquitectura](02-Arquitectura.md) | Comparativa y selección de tecnologías, decisiones justificadas, capas del sistema, notificaciones, parser de lenguaje natural, widget, calendario, IA y sincronización futuras, extensibilidad, riesgos técnicos | ✅ Redactado |
| 03 | [Modelo de Datos](03-Modelo-Datos.md) | Diagrama entidad-relación, tablas, relaciones, índices, migraciones, escalabilidad, export/import | ✅ Redactado |
| 04 | [Design System](04-Design-System.md) | Paleta, tokens, tipografía, espaciado, sombras, elevaciones, border radius, iconografía, componentes, estados, animaciones | ✅ Redactado (pendiente verificación visual contra imagen) |
| 05 | [Roadmap](05-Roadmap.md) | Fases MVP → V5, alcance por versión, exit criteria, esfuerzo, dependencias, estrategia de calidad | ✅ Redactado |
| 06 | [Auditoría Crítica](06-Auditoria.md) | Análisis competitivo vs Todoist / TickTick / Google Calendar, debilidades, funciones faltantes, propuestas de mejora disruptiva, matriz de riesgos | ✅ Redactado |
| 07 | [Informe del Spike Técnico](07-Spike-Tecnico.md) | Validación empírica de Tauri 2 en Windows 11: arranque 971 ms, RAM 26.5–33.2 MB, exe 3.27 MB, Temporal disponible, widget transparente, hotkey con fallbacks | ✅ Completado 2026-08-04 |

**Artefactos de validación:**
- `focusflow-spike/` — app Tauri 2 funcional (tray, widget transparente, hotkey global, notificaciones nativas). Binario: `src-tauri\target\release\focusflow-spike.exe`
- `focusflow-proto/` — prototipo Svelte 5 del diseño (semana/mes/día/agenda, Quick Add con detección de entidades, widget, tema oscuro). Screenshots en `shots/`
- Preview del prototipo: `http://localhost:4173` (servidor activo) · `#/dark` tema oscuro · `#/widget` widget

---

## Resumen ejecutivo (1 minuto)

**Problema:** las fechas importantes de un estudiante/profesional viven repartidas entre calendario, notas, chats y memoria. Las que se pierden son las que causan daño: entregas, exámenes, pagos.

**Producto:** una sola aplicación local para Windows que combina calendario, tareas, agenda, recordatorios y widget de escritorio, con **entrada rápida por lenguaje natural** ("Mañana estudiar cálculo de 3pm a 5pm") como núcleo de la experiencia. Offline-first, instantánea, con identidad visual Soft UI 2.0.

**Decisión técnica central:** Tauri 2 (Rust + WebView2) + Svelte 5 + TypeScript + SQLite. Binario pequeño (~10 MB), RAM baja (<150 MB), notificaciones nativas de Windows y transparencia real para el widget. La IA y la sincronización se diseñan como interfaces enchufables desde el día uno, no como refactors futuros.

**Filosofía de producto:** capturar en segundos, recordar en el momento correcto, mostrar solo lo importante. No es una app de empresa; es una app que se siente como una herramienta nativa de Apple: rápida, bella, predecible.

---

## Decisiones que definen el producto (resumen)

1. **Local-first radical.** Los datos son del usuario. SQLite en disco, backups automáticos, export/import JSON e iCal. La nube (si llega) es un espejo, nunca el origen.
2. **Lenguaje natural como primera clase.** Toda captura pasa por el mismo parser. La forma manual existe, pero el atajo por texto es el camino feliz.
3. **Todo es una tarea.** Un evento del calendario, una entrega, un recordatorio de pago y una cita son la misma entidad con distintas políticas de recordatorio y repetición. Un solo modelo, cero duplicación conceptual.
4. **El widget es producto, no accesorio.** Está en la arquitectura desde el día uno (segunda ventana, proceso compartido) y es la razón principal por la que el usuario abre la app todos los días.
5. **IA y sync son interfaces, no features.** La primera versión define contratos estables (parser, proveedor de sync, servicio de IA). Cambiar de motor después no toca el dominio.

---

## Glosario rápido

| Término | Definición |
|---------|-----------|
| Tarea (Task) | Entidad única que cubre eventos, entregas, citas, recordatorios y hábitos. Tiene fechas, prioridad, categoría, estado y recordatorios |
| Entrada rápida (Quick Add) | Campo de texto que interpreta lenguaje natural y crea tareas con campos completados |
| Agenda | Vista que mezcla tareas y eventos del calendario en una línea de tiempo |
| Widget | Ventana secundaria flotante, transparente y always-on-top que muestra próximas tareas |
| Local-first | Los datos viven en el dispositivo; la nube es opcional y solo un espejo |
| Motor de recordatorios | Subsistema que calcula *cuándo* disparar cada notificación y sobrevive al reinicio de la app |
