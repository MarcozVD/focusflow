# 05 — Roadmap

**Producto:** FocusFlow · **Versión:** v0.1 · **Fecha:** 2026-08-04
**Equipo asumido:** 2 devs (1 Rust/dominio, 1 frontend Svelte) + PM/design 25 %.

> Esfuerzos en hombre-semanas (HS). Las fases están secuenciadas por **narrativa de producto** (primero captura + recordatorio = valor visible), no solo por RICE.

---

## Fase 0 — Fundación y Spike técnico (2–3 semanas)

**Objetivo:** despejar los 6 riesgos técnicos del doc 02 §11 y fijar los cimientos que todas las demás fases usan.

| Entregable | Detalle | HS |
|------------|---------|----|
| Spike técnico | Arranque en HDD, transparencia widget Win11, hotkey global, sqlx vs rusqlite, Temporal vs date-fns | 1.5 |
| Esqueleto Tauri | Shell + ventana principal + ventana widget vacía + tray + autostart | 1.5 |
| Esqueleto dominio | Crates core/store/engine/app; comandos IPC mínimos | 2 |
| Migraciones 0001 | Esquema completo (doc 03) + backup automático | 1.5 |
| CI | Build Windows + lint + tests de dominio + benchmarks NFR | 1 |
| Design tokens v1 | Tokens CSS del Design System (doc 04) aplicados a un par de pantallas de humo | 1 |

**Exit criteria:** arranque ≤1.5 s en HDD; ventana transparente alfa por píxel funcionando; hotkey global dispara; CI verde con NFR-02 benchmark.

---

## Fase 1 — MVP: captura + recordatorio + calendario (8–10 semanas)

**Narrativa:** "escribo una frase, aparece en mi calendario, y me avisa a tiempo". Sin widget, sin IA, sin sync.

### Alcance (Must del PRD)

| Área | Contenido |
|------|-----------|
| Captura | Quick Add NL (parser reglas ES) + formulario manual + preview editable + undo |
| Tareas | Modelo completo, estados, categorías (color/icono), prioridades, completar con animación |
| Calendario | Vistas día/semana/mes + agenda mixta, hoy destacado, navegación animada |
| Recordatorios | Predefinidos + personalizados, motor scheduler (reloj mockeable), toast nativo, acciones Abrir/Hecho, tray + autostart, "Mientras no estabas" |
| Datos | Backup automático, export/import JSON, export iCal |
| UX | Design System completo, tema claro/oscuro, onboarding 3 pasos, i18n ES/EN, atajo global de captura |
| Calidad | Tests: parser (50+ frases ES), scheduler (reloj simulado), E2E de los 3 flujos críticos; benchmarks NFR |

### Fuera de alcance MVP (explícito)

Drag & drop · repetición RRULE · filtros combinables · búsqueda avanzada · widget · estadísticas · etiquetas · papelera extendida · auto-update completo.

### Criterios de aceptación

Los **AC-01 … AC-15** del PRD §8, todos verificables por la persona que firma el MVP.

| | HS |
|--|----|
| Estimación total | ~24 HS |

**Exit criteria de calidad:** AC verde; 0 P1 bugs abiertos; benchmark NFR-02 < 100 ms; test manual en Windows 10 y 11.

---

## Fase 2 — V1: organización y repetición (6–8 semanas)

**Narrativa:** "ahora mi semana se administra sola: arrastro, repito, filtro, encuentro".

| Área | Contenido | HS |
|------|-----------|----|
| Calendar | Drag & drop reprogramar (+recalcula recordatorios), días completos, resaltado prioridad/vencidas, mini-mes | 4 |
| Repetición | Motor RRULE (diaria/semanal/mensual/anual, intervalos, BYDAY), excepciones por instancia (omitir/editar/completar), parseo de repetición por texto | 5 |
| Organización | CRUD categorías, etiquetas (N:M), filtros combinables, búsqueda incremental (FTS5) | 4 |
| Recordatorios | Acción Posponer en toast, personalizados absolutos, recálculo propagado | 2 |
| Datos | Papelera 30 días, auto-update MSI firmado | 2 |
| UX | Paleta de comandos Ctrl+K, i18n completado, accesibilidad AA base | 2 |
| Calidad | Suite E2E ampliada (drag & drop, excepciones RRULE), fuzz de parser ES/EN | 1 |

**Exit criteria:** drag&drop estable; series con excepciones verificadas contra casos RFC 5545; búsqueda < 50 ms en 10k tareas.

---

## Fase 3 — V2: Widget + profundidad personal (6–7 semanas)

**Narrativa:** "FocusFlow vive en mi escritorio y conoce mi día antes de que yo lo mire".

| Área | Contenido | HS |
|------|-----------|----|
| Widget | Ventana transparente always-on-top, compacto/expandido, tamaño/transparencia/posición por monitor, temas, tareas del día + próximas + contador regresivo | 5 |
| Widget avanzado | Completar desde widget, click→abrir app en tarea, refresh por eventos (sin polling) | 1.5 |
| Producto | Subtareas/checklist, % progreso, duplicar tarea, modo "No molestar", mini-estadísticas rápidas en el widget | 3 |
| Experiencia | Templates de tareas (V3-IA los hará superiores; aquí solo los básicos), temas de acento | 1.5 |
| Calidad | Tests multi-monitor (2 monitores, escalas 100/125/150 %), pruebas de transparencia con DWM | 1 |

**Exit criteria:** widget en modo expandido no excede 60 MB extra de RAM; contador regresivo exacto a nivel de día; test en Windows 10/11 con escalas mixtas.

---

## Fase 4 — V3: IA (8–10 semanas)

**Narrativa:** "escribo mal, hablo de más, y FocusFlow me ordena la semana".

La arquitectura ya definió el contrato (doc 02 §7): esta fase es **implementación de proveedores**, no refactor.

| Área | Contenido | HS |
|------|-----------|----|
| Parser IA | Nivel 2 de parser (trait NLParser) con LLM local (llm.cpp/Ollama 3–8B cuantizado); fallback a reglas; score y preview siguen igual | 5 |
| Organización | Sugerir categoría/prioridad/etiquetas para tareas sin completar; agrupación semanal automática opcional | 2 |
| Planificador | Sugerir horarios (ventanas libres + duración), detectar conflictos y sobrecarga diaria | 3 |
| Descomposición | Dividir tareas grandes en subtareas sugeridas | 2 |
| Privacidad | Panel de control de IA (local vs remota opt-in, qué datos se usan) | 1.5 |
| Calidad | Evaluación de parser IA vs reglas (misma suite de frases), benchmarks latencia local | 1 |

**Exit criteria:** IA local parsea ≥ 90 % de las frases que las reglas no resuelven; detección de conflicto con 0 falsos positivos en pruebas de regresión; todo sigue funcionando con IA desactivada.

---

## Fase 5 — V4: Sincronización (8–10 semanas)

**Narrativa:** "mis fechas me siguen entre la app y Google Calendar, sin perder ninguna".

| Área | Contenido | HS |
|------|-----------|----|
| Base | SyncProvider trait, OAuth local (DPAPI), operation log → delta sync, conflictos LWW + cola visible | 4 |
| Google | 1-way (Google → FocusFlow) con merge de categorías; luego 2-way (tareas → Google como eventos) | 3 |
| Outlook | Igual que Google con API MS Graph | 3 |
| iCal | Sync por archivo (import/export programados) | 1.5 |
| UI | Pantalla de conexiones, estado de sync en sidebar, resolución manual de conflictos | 2 |
| Calidad | Suite de conflictos (edición simultánea), tests con sandboxes de cuenta de test | 1 |

**Exit criteria:** sincronización bidireccional con Google sin pérdida de datos en 100 ciclos de test; conflictos resueltos sin silencio; desconexión = app sigue 100 % operativa.

---

## Fase 6 — V5: App móvil (10–14 semanas, tras validar demanda)

**Narrativa:** "capturo en el celular, veo en el escritorio".

| Área | Contenido | HS |
|------|-----------|----|
| Core compartido | El crate de dominio compilado vía FFI (Kotlin/Swift) reutilizado en móvil | 4 |
| Móvil v1 | Lectura ("qué tengo hoy"), notificaciones locales, captura rápida NL | 6 |
| Móvil v2 | Edición completa, widget móvil, sync con el mismo operation log | 4 |
| Distribución | Play Store / App Store (firmado, política de privacidad) | 2 |

**Exit criteria:** la misma suite de tests de dominio corre en 3 plataformas; sync móvil↔escritorio sin divergencias.

---

## Después (V6+, sin fecha)

Colaboración · plugins · hábitos/Pomodoro (si la auditoría lo valida) · templates de IA · telemetría opt-in.

---

## Mapa temporal

```
2026 Q3        Q4              2027 Q1        Q2            Q3        Q4
Fase 0  Fase 1 ─MVP→   Fase 2  Fase 3 ─V1/V2→  Fase 4 ─V3→   Fase 5 ─V4→   Fase 6 ─V5→
2-3 sem 8-10 sem        6-8    6-7            8-10          8-10      10-14
```

---

## Estrategia de calidad transversal

| Mecanismo | Cuándo | Métrica |
|-----------|--------|---------|
| Tests de dominio (Rust) | Continuo en CI | Cobertura ≥ 80 % en parser y scheduler |
| Benchmarks NFR | CI + release | Arranque/RAM/interacción (doc 02 NFR) |
| E2E (flujos críticos) | Cada fase | 3 flujos + regresión |
| Pruebas visuales de Design System | Cada fase | Checklist de tokens (doc 04) |
| Beta cerrada de 20 usuarios | Fin MVP | KPI §12 (PRD) + encuesta de olvidos |
| Release candidates | Antes de cada fase | 1 semana de hardening + manual QA en Win10/11 |

---

## Definición de Done (por feature)

1. Código con tests (dominio) / E2E (UI) verdes en CI.
2. Benchmark NFR sin regresión.
3. Design System aplicado (checklist visual).
4. Docs: PRD actualizado, changelog, ayuda en-app si aplica.
5. Sin P1/P2 abiertos asignados a la feature.
6. Aceptado por PM contra sus AC.
