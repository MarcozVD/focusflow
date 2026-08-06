# 01 — Product Requirements Document (PRD)

**Producto:** FocusFlow — productividad personal para Windows
**Versión:** v0.1 · **Fecha:** 2026-08-04 · **Autor:** Equipo de producto

---

## 1. Visión del producto

### 1.1 Vision statement

> Para estudiantes y profesionales que olvidan fechas importantes porque las tienen repartidas entre calendarios, apps de tareas y notas, **FocusFlow** es una aplicación de productividad personal para Windows que concentra calendario, tareas, agenda, recordatorios y widget de escritorio en **una sola superficie local, instantánea y bella**. A diferencia de las suites empresariales, captura cualquier compromiso escribiendo una frase ("Mañana estudiar cálculo de 3pm a 5pm"), recuerda en el momento exacto y nunca depende de la nube para funcionar.

### 1.2 Principios de producto (no negociables)

| # | Principio | Implicación concreta |
|---|-----------|----------------------|
| P1 | **Capturar en segundos** | Entrada rápida por lenguaje natural disponible en cualquier pantalla con Ctrl+Shift+Espacio. Tiempo objetivo: frase → tarea creada < 2 s |
| P2 | **Recordar en el momento correcto** | Motor de recordatorios fiable con varios avisos por tarea (un día antes, 3 h, 1 h, 15 min) y personalizados. Notificaciones nativas de Windows incluso con la app cerrada |
| P3 | **Un solo modelo mental** | Eventos, entregas, citas, pagos y recordatorios son *tareas*. El usuario nunca debe decidir "¿esto va en el calendario o en la lista?" |
| P4 | **Local-first** | 100 % funcional sin internet. Los datos viven en SQLite local. La nube (futuro) es un espejo, nunca un requisito |
| P5 | **Rápida y ligera** | Arranque < 1.5 s, interacciones < 100 ms, RAM < 150 MB, binario < 15 MB |
| P6 | **Diseño Soft UI 2.0 premium** | Neumorfismo suave, luz superior-izquierda, superficies blancas cálidas, azul #2563EB como primario. Ver Design System (doc 04) |
| P7 | **IA y sync enchufables** | La arquitectura define contratos de interfaz desde el día uno para que IA, sync y colaboración se agreguen sin refactor |

### 1.3 Lo que NO es el producto

- ❌ No es una app de gestión de proyectos (no kanban corporativo, ni Gantt, ni asignación de equipos)
- ❌ No es un clon de Google Calendar (el calendario es una *vista*, no la identidad del producto)
- ❌ No requiere cuenta, login ni nube en ninguna versión local
- ❌ No es una app móvil (la móvil llega en V5 como extensión, no como base)

---

## 2. Problema y oportunidad

### 2.1 Problema (en voz del usuario)

- "Tengo exámenes, entregas, pagos y citas en cinco apps distintas. Al final la más olvidada es la que estaba solo en una nota."
- "Escribir una tarea requiere 12 campos. Cuando tengo prisa no escribo nada y después se me olvida."
- "Puse el recordatorio en la app equivocada y la notificación nunca llegó."
- "No quiero abrir una app pesada de 400 MB solo para ver qué tengo mañana."
- "Trabajo sin internet en el campus; la app de tareas en la nube me deja a medias."

### 2.2 Hipótesis central

> **H1:** si la captura toma < 2 segundos y los recordatorios son fiables, el usuario registrará el 100 % de sus fechas importantes (no el 60 %), y la retención será impulsada por el hábito de captura, no por la gestión.

Métricas de validación: % de tareas creadas vía entrada rápida (objetivo ≥ 70 %), % de tareas con recordatorio (≥ 60 %), tareas por semana (≥ 15), retención semanal (≥ 60 %).

### 2.3 Oportunidad competitiva

Todoist y TickTick son excelentes gestores de tareas; Google Calendar es excelente calendario. **Ninguno domina el espacio "citas + entregas + recordatorios de vida" para uso personal en Windows**, con captura por lenguaje natural, widget de escritorio transparente y local-first sin suscripción. FocusFlow compite en el cruce de los tres.

---

## 3. Personas objetivo

### 3.1 Persona primaria — "Diego, el estudiante saturado" (20 años)

| Dimensión | Detalle |
|-----------|---------|
| Contexto | Estudiante universitario de ingeniería, 5 materias, trabajo a tiempo parcial |
| Dolor | Entregas y exámenes mezclados con turnos de trabajo y pagos; olvida 1-2 fechas por mes con consecuencias reales |
| Herramientas actuales | Notas del celular, recuerdos en WhatsApp, a veces Google Calendar |
| Comportamiento | Captura en el momento, con prisa, desde el celular o el escritorio; revisa "qué tengo" 3-5 veces al día |
| Éxito para él | Nunca volver a olvidar una entrega; ver su semana en 5 segundos |
| Hardware | Portátil Windows de gama media (8 GB RAM), sin SSD rápido, a veces sin internet |
| Costo | No quiere pagar suscripción; tolera una compra única pequeña |

### 3.2 Persona secundaria — "Laura, la profesional de vida ocupada" (29 años)

| Dimensión | Detalle |
|-----------|---------|
| Contexto | Diseñadora freelance: proyectos freelance, cursos, vida personal, citas médicas, pagos de servicios |
| Dolor | Mezcla proyectos (con entregas) y vida personal (citas, pagos) en un solo lugar sin contaminar lo laboral |
| Herramientas actuales | TickTick + Calendario del celular, recuerda poco |
| Comportamiento | Planifica los domingos 30 min; usa etiquetas y repeticiones (pagos mensuales) |
| Éxito para ella | Planificación semanal fluida + estadísticas de cumplimiento que la motiven |
| Hardware | Windows 11, pantalla 14" con alta densidad de píxeles |

### 3.3 Anti-persona

- Usuario corporativo que necesita gestión de equipos, permisos y reportes para jefes → fuera de alcance.
- Usuario de escritorio ligado 100 % al ecosistema Apple → fuera de alcance inicial.

---

## 4. Casos de uso (jerarquía de valor)

Formato: **UC-XX — nombre** (prioridad de negocio / usuario).

### A. Captura

| ID | Caso de uso | Detalle | Prioridad |
|----|-------------|---------|-----------|
| UC-01 | Crear tarea manualmente | Formulario completo: título, descripción, categoría, prioridad, etiquetas, fechas, horas, estado, progreso, enlaces, notas | P1 |
| UC-02 | Crear tarea por lenguaje natural | Frase → campos autocompletados. "Mañana estudiar cálculo de 3pm a 5pm", "Recordarme pagar internet el 15", "Tengo examen de física el próximo lunes a las 8 AM", "Tarea de programación desde mañana hasta el jueves" | P0 |
| UC-03 | Entrada rápida global | Atajo de sistema Ctrl+Shift+Espacio abre ventana de captura sobre cualquier app; al confirmar crea la tarea | P1 |
| UC-04 | Editar tarea | Edición completa + edición rápida de fecha arrastrando en calendario | P1 |

### B. Organización y vista

| ID | Caso de uso | Detalle | Prioridad |
|----|-------------|---------|-----------|
| UC-05 | Vista mensual / semanal / diaria | Navegación fluida entre vistas con animación; hoy destacado | P0 |
| UC-06 | Agenda mixta | Línea de tiempo con tareas + eventos del calendario juntos | P0 |
| UC-07 | Arrastrar para reprogramar | Drag & drop de tareas entre días/horas; al soltar recalcula recordatorios | P1 |
| UC-08 | Categorías con color e icono | Universidad, Trabajo, Personal, Salud, Finanzas, Otros (editables) | P1 |
| UC-09 | Prioridades Alta / Media / Baja | Indicador visual + filtros | P1 |
| UC-10 | Etiquetas | Multietiqueta libre por tarea | P2 |
| UC-11 | Filtros combinables | Categoría + prioridad + estado + etiqueta + rango de fechas | P1 |
| UC-12 | Búsqueda rápida | Búsqueda incremental sobre título/descripción/etiquetas; Ctrl+K (paleta de comandos en V2) | P1 |
| UC-13 | Tareas repetitivas | Diarias, semanales (lun, mié), mensuales, anuales, personalizadas (RRULE subset) | P1 |
| UC-14 | Progreso | % de progreso por tarea + barra visual | P2 |

### C. Recordatorio y notificación

| ID | Caso de uso | Detalle | Prioridad |
|----|-------------|---------|-----------|
| UC-15 | Recordatorios predefinidos | Un día antes / 3 h / 1 h / 15 min (combinables, múltiples por tarea) | P0 |
| UC-16 | Recordatorio personalizado | Fecha/hora absoluta o relativa ("2 días antes a las 9:00") | P1 |
| UC-17 | Notificación nativa Windows | Toast del sistema con la app minimizada, cerrada o en bandeja; sonido opcional; acciones: abrir tarea, completar, posponer | P0 |
| UC-18 | Centro de notificaciones perdidas | Si la app estuvo cerrada, al abrir muestra "mientras no estabas" (notificaciones atrasadas agrupadas) | P1 |

### D. Widget

| ID | Caso de uso | Detalle | Prioridad |
|----|-------------|---------|-----------|
| UC-19 | Widget de escritorio | Muestra próximas tareas/entregas/exámenes, contador regresivo, tareas del día | P1 |
| UC-20 | Modos compacto / expandido | Alternar con animación | P1 |
| UC-21 | Configuración del widget | Tamaño, transparencia, always-on-top, qué mostrar | P1 |
| UC-22 | Interacción mínima desde el widget | Completar tarea desde el widget; clic abre el detalle en la app principal | P2 |

### E. Datos y analítica

| ID | Caso de uso | Detalle | Prioridad |
|----|-------------|---------|-----------|
| UC-23 | Estadísticas | Completadas, pendientes, productividad semanal/mensual, % cumplimiento, categorías más usadas | P2 |
| UC-24 | Backup y export | Backup automático en segundo plano; export/import JSON; export iCal | P1 |
| UC-25 | Tema claro / oscuro | Sincronizado con el sistema por defecto, con override manual | P1 |

---

## 5. Requisitos funcionales detallados

Convención: `ID` — `Descripción` — [Prioridad MoSCoW]. IDs estables para el backlog de ingeniería.

### FR-CAPTURA

- **FR-01** Entrada rápida por lenguaje natural — P0: campo de texto único global. Parser interpreta fecha(s), horas, duración, categoría, etiquetas y prioridad. Resultado editable antes de confirmar (preview). Ver FR-NLP.
- **FR-02** Formulario manual de tarea — P0: todos los campos del modelo (título*, descripción, categoría, prioridad, etiquetas, fecha inicio/fin, hora inicio/fin, estado, % progreso, enlaces, notas). Validación en vivo.
- **FR-03** Edición completa — P0: mismo formulario en modal o panel lateral.
- **FR-04** Edición rápida por arrastre — P1: en vista semanal/diaria, arrastrar tarea a otra hora/día actualiza fecha/hora. Mantener Ctrl al soltar duplica (crear tarea similar). Múltiples tareas seleccionadas se mueven juntas.
- **FR-05** Duplicar tarea — P2: con o sin fechas (duplica "plantilla").
- **FR-06** Papelera y restauración — P2: eliminación reversible 30 días.

### FR-NLP (parser de lenguaje natural)

- **FR-07** Entidades mínimas — P0: fechas relativas (mañana, hoy, pasado mañana, próximo lunes, el 15), fechas absolutas (15/03, 15 de marzo), horas (3pm, 15:00), rangos ("de 3pm a 5pm", "desde mañana hasta el jueves"), duración ("2 horas").
- **FR-08** Acciones — P0: "Recordarme X el [fecha]" → crea tarea + recordatorio en esa fecha; "Entregar/Examen/Cita" detecta contexto semántico y sugiere categoría e icono.
- **FR-09** Categoría y prioridad por palabra clave — P1: "urgente" → Alta; "examen/entregar" → Universidad (si existe); "pagar" → Finanzas.
- **FR-10** Repetición por texto — P1: "todos los lunes", "cada mes", "cada año", "cada 2 días".
- **FR-11** Confianza del parseo — P1: si el parser no está seguro (score < umbral), previsualiza la interpretación y pide confirmación con campos editables; nunca crea silenciosamente algo ambiguo.
- **FR-12** Soporte idiomas — P0 español, P1 inglés. Arquitectura de parser multicapa: reglas por idioma + motor compartido.
- **FR-13** Interfaz de parser intercambiable — P0 (arquitectura): contrato `parse(texto, contexto) → Intent`; MVP implementa reglas; V3 conecta IA (ver doc 02 §7).

### FR-CALENDARIO

- **FR-14** Vistas: día, semana (7 días), mes — P0. Transiciones animadas (150–250 ms).
- **FR-15** Agenda mixta — P0: pestaña "Agenda": lista cronológica de hoy + próximos 7 días mezclando tareas con fecha y eventos.
- **FR-16** Navegación — P0: flechas, "Hoy", clic en fecha para saltar; gesto rueda+shift para cambiar semana (P2).
- **FR-17** Hoy destacado — P0: día actual con acento primario; línea de hora actual en vistas día/semana.
- **FR-18** Días completos — P1: tareas "todo el día" con cabecera de bloque en mes.
- **FR-19** Resaltado por prioridad/categoría — P1: color de categoría en la tarea; borde izquierdo prioridad alta; badge vencida en rojo.
- **FR-20** Mini-mes lateral — P2: navegador de meses mini para saltar de mes con un clic.

### FR-TAREAS

- **FR-21** Modelo de tarea completo — P0: ver doc 03 (modelo de datos).
- **FR-22** Estados — P0: pendiente, en curso, completada, cancelada. Completar por checkbox (una pulsación), con animación.
- **FR-23** Progreso — P2: slider 0–100 %, también por "+25 %" en menú contextual.
- **FR-24** Subtareas/checklist — P2 (en V2): checklist dentro de la tarea con contador.
- **FR-25** Enlaces y notas — P1: campos de nota multilínea (markdown ligero P3) y enlaces clicables.
- **FR-26** Archivos adjuntos — P3: arrastrar archivos a la tarea (almacenados en carpeta local; futuro sync).

### FR-RECORDATORIOS / NOTIFICACIONES

- **FR-27** Múltiples recordatorios por tarea — P0: lista de recordatorios (predefinidos + personalizados).
- **FR-28** Predefinidos — P0: 1 día antes, 3 h antes, 1 h antes, 15 min antes (toggles).
- **FR-29** Personalizado — P1: fecha/hora absoluta o relativa a la tarea ("-2d 09:00").
- **FR-30** Recálculo al mover tarea — P0: mover una tarea propaga el cambio a sus recordatorios (se recalculan, no se duplican).
- **FR-31** Toast nativo Windows — P0: notificación del sistema; app minimizada, en bandeja o cerrada. Sonido configurable por tarea y por categoría.
- **FR-32** Acciones en la notificación — P1: Abrir tarea / Completar / Posponer (10 min, 1 h, 1 día).
- **FR-33** Bandeja del sistema — P0: icono de bandeja cuando la ventana se cierra (comportamiento: cerrar → bandeja; salir → menú de bandeja).
- **FR-34** Autostart — P1: opción de iniciar con Windows en segundo plano para garantizar recordatorios.
- **FR-35** "Mientras no estabas" — P1: notificaciones vencidas agrupadas al abrir la app.
- **FR-36** Silenciar/posponer todo — P2: modo "No molestar" con duración.

### FR-WIDGET

- **FR-37** Ventana flotante transparente — P1: always-on-top, sin borde, fondo con transparencia opcional, esquinas redondeadas (recorte real por forma de la ventana).
- **FR-38** Contenido configurable — P1: próximas tareas, próximas entregas/exámenes (por categoría), contador regresivo de la próxima entrega, tareas del día.
- **FR-39** Modo compacto / expandido — P1: alternar con animación (150–250 ms); configuración de tamaño.
- **FR-40** Tema heredado — P1: sigue el tema de la app principal.
- **FR-41** Acciones — P2: checkbox completar desde el widget; clic abre la app en esa tarea.
- **FR-42** Multi-monitor — P1: recordar posición por monitor; elegir monitor de anclaje.

### FR-CATEGORÍAS / PRIORIDADES / FILTROS

- **FR-43** Categorías por defecto — P0: Universidad, Trabajo, Personal, Salud, Finanzas, Otros. Cada una con color e icono (Lucide/Phosphor).
- **FR-44** CRUD de categorías — P1: crear, editar (color, icono, nombre), eliminar (con reasignación o archivo).
- **FR-45** Prioridades — P0: Alta / Media / Baja, con color semántico (rojo suave / azul / gris).
- **FR-46** Filtros combinables — P1: categoría, prioridad, estado, etiqueta(s), rango de fechas, vencidas; combinables entre sí y persistibles como "vistas guardadas" (P2).
- **FR-47** Búsqueda — P1: búsqueda incremental (fuzzy) sobre título, descripción, etiquetas, notas, enlaces; muestra tareas y eventos; navegación con teclado.

### FR-ESTADÍSTICAS

- **FR-48** Panel de estadísticas — P2: tareas completadas/pendientes por semana y mes, % cumplimiento (completadas / programadas), racha actual, categorías más usadas (top 5), productividad por día de la semana.
- **FR-49** Export de estadísticas — P3: CSV.

### FR-REPETICIÓN

- **FR-50** RRULE subset — P1: `FREQ=DAILY/WEEKLY/MONTHLY/YEARLY` + `BYDAY`, `INTERVAL`, `COUNT`, `UNTIL`. Gestión de instancias: editar solo una (excepción) o la serie completa.
- **FR-51** Ocurrencias — P1: generar ocurrencias bajo demanda (rolling window, no materializar infinito); completar una instancia no completa la serie.

### FR-DATOS Y APP

- **FR-52** Offline-first — P0: sin red la app funciona 100 %.
- **FR-53** Backup automático — P1: copia rotativa diaria del archivo SQLite; restauración desde ajustes.
- **FR-54** Export/Import JSON — P1: interoperabilidad y portabilidad.
- **FR-55** Export/Import iCal (.ics) — P1: puente con Google/Outlook/Apple manual (antes de sync nativa).
- **FR-56** Temas — P0: claro / oscuro / sistema. Dark es equivalente semántico (mismas variables, luminosidad invertida).
- **FR-57** Shortcuts globales — P1: Ctrl+Shift+Espacio captura; Ctrl+K paleta de comandos (V2).
- **FR-58** i18n — P1: español y inglés; catalizador por design tokens (texto no hardcodeado).
- **FR-59** Onboarding — P0: primera ejecución en 3 pasos (crear categorías básicas → probar captura por texto → activar recordatorios/autostart). Sin cuenta ni registro.
- **FR-60** Cero telemetría por defecto — P0: sin datos fuera del dispositivo; telemetría opt-in anónima (P3).

---

## 6. Requisitos no funcionales (NFR)

| ID | Categoría | Requisito | Objetivo |
|----|-----------|-----------|----------|
| NFR-01 | Rendimiento | Arranque en frío a primera pantalla útil | ≤ 1.5 s (disco HDD) |
| NFR-02 | Rendimiento | Interacción (cambio de vista, filtro, búsqueda) | ≤ 100 ms de percepción |
| NFR-03 | Rendimiento | RAM en reposo (app en bandeja) | ≤ 60 MB; con widget ≤ 100 MB |
| NFR-04 | Rendimiento | RAM en uso activo | ≤ 150 MB |
| NFR-05 | Tamaño | Instalador | ≤ 15 MB (Tauri) |
| NFR-06 | Fiabilidad | Recordatorios con app cerrada | 100 % de disparos en hora (tolerancia ±60 s) |
| NFR-07 | Fiabilidad | Pérdida de datos | 0 en uso normal; backup automático cubre fallos de disco |
| NFR-08 | Escalabilidad | Tareas totales | Diseñado para 10 000+ sin degradación perceptible |
| NFR-09 | Compatibilidad | Windows | 10 (21H2+) y 11, x64; soporte ARM64 (P3) |
| NFR-10 | Seguridad | Datos locales | Sin transmisión; opcional cifrado DPAPI (P2) |
| NFR-11 | Privacidad | Telemetría | Off por defecto (ninguna en MVP) |
| NFR-12 | Accesibilidad | Contraste, foco visible, teclado navegable | WCAG 2.1 AA (P2 completo, base desde MVP) |
| NFR-13 | Actualizaciones | Auto-update | Instalador MSI + update auto (P1) |
| NFR-14 | Tests | Cobertura del núcleo de dominio | ≥ 80 % en parser y motor de recordatorios (Rust) |

---

## 7. Flujos de navegación y UX

### 7.1 Mapa de navegación (app principal)

```
┌──────────────────────────────────────────────────────────────┐
│ Barra superior: Logo · Búsqueda (Ctrl+K V2) · Quick Add (⌘⌥) │
│                    · Tema · Bandeja · Ajustes                │
├───────────┬──────────────────────────────────────────────────┤
│ Sidebar   │  Área principal (vistas)                         │
│ · Hoy     │  [ Calendario ] [ Agenda ] [ Tareas ] [ Stats ]  │
│ · Agenda  │   ▼                                               │
│ · Próximos│   Según pestaña:                                 │
│ · Tareas  │   - Calendario: mes/semana/día + panel tarea     │
│ · Stats   │   - Agenda: línea de tiempo                      │
│ ───────── │   - Tareas: lista agrupada por vencimiento       │
│ Categorías│   - Stats: gráficas                              │
│  ▸ Colores│  Panel derecho: detalle de tarea (sliding)       │
│ Mini-mes  │                                                  │
│ Widget    │                                                  │
└───────────┴──────────────────────────────────────────────────┘
     ↑                                                          │
  Quick Add global (Ctrl+Shift+Espacio) → ventana flotante      │
  Widget de escritorio (ventana 2) → clic → abre app + tarea    │
  Toast de Windows → Abrir/Completar/Posponer                   │
```

### 7.2 Flujo crítico 1 — Captura por lenguaje natural (camino feliz)

1. Usuario pulsa **Ctrl+Shift+Espacio** (o el campo Quick Add en la barra).
2. Aparece ventana flotante centrada, input único, foco automático.
3. Escribe: `Mañana estudiar cálculo de 3pm a 5pm` + Enter.
4. Parser resuelve → preview en tarjeta editable (título, categoría sugerida, fecha, horas) con animación de confirmación.
5. Enter confirma → tarea creada → toast discreto "Creada" con Undo (5 s) → la ventana se cierra.
6. (Si score bajo) → preview en modo edición para corregir antes de crear.

**Éxito:** 2 segundos de manos a teclado a tarea en agenda. El 80 % de las capturas no requieren abrir formulario.

### 7.3 Flujo crítico 2 — Reprogramar con un arrastre

1. Vista semanal. Tarea "Entregar proyecto de redes" el jueves 14:00.
2. Profe cambia la fecha → arrastrar al lunes 09:00.
3. Soltar → tarea se reubica, recordatorios se recalculan ("un día antes" pasa a domingo 09:00), sin preguntas.
4. Animación: la tarea "flota" con elevación; al soltar, rebote suave (200 ms).

### 7.4 Flujo crítico 3 — Recordatorio que llega

1. Usuario crea "Pagar internet el 15" → parser sugiere recordatorio "mismo día 09:00" + "1 día antes".
2. App en bandeja / cerrada. El **motor de recordatorios** (independiente de la UI) detecta que el plazo venció.
3. Windows muestra toast con sonido: "Pagar internet — hoy 09:00 · [Abrir] [Hecho] [Posponer]".
4. "Hecho" marca la tarea completada; la app (si abierta) reacciona en vivo vía evento.

### 7.5 Principios UX

| Principio | Aplicación |
|-----------|-----------|
| Menos clicks que apps de empresa | Cada acción primaria ≤ 2 interacciones |
| El teclado manda | Quick Add, navegación de calendario, búsqueda, paleta |
| Feedback en <100 ms | Optimistic UI: la acción se aplica y se sincroniza con el store |
| Undo universal | Crear, completar, mover, borrar → deshacer (5 s) |
| Consistencia Soft UI | Ver Design System (doc 04) |
| Micro-animaciones 150–250 ms | Ver tokens de motion (doc 04) |
| Estados vacíos educativos | Primer uso: "Agrega tu primera tarea escribiendo 'Mañana...'" |

---

## 8. Criterios de aceptación del MVP

El MVP (ver Roadmap) se acepta si y solo si **todos** los siguientes son verificables:

| ID | Criterio | Verificación |
|----|----------|--------------|
| AC-01 | Crear tarea manual con todos los campos del modelo | Test E2E: formulario → tarea visible en calendario y agenda |
| AC-02 | Las 5 frases de ejemplo del brief se parsean correctamente al 100 % | Suite de tests de parser (unit, español) |
| AC-03 | Frase ambigua muestra preview editable, nunca crea sin confirmar | Test de bajo score |
| AC-04 | Vistas mes / semana / día / agenda muestran la misma fuente de verdad | Test E2E + visual |
| AC-05 | Arrastrar tarea en semana cambia su fecha y recalcula recordatorios | Test E2E + unit del motor |
| AC-06 | Recordatorios predefinidos disparan toast nativo incluso con la app cerrada | Test de humo manual + test del scheduler con reloj simulado |
| AC-07 | Acciones del toast: Abrir / Completar / Posponer | Manual + E2E |
| AC-08 | Categorías con color+icono; filtros por categoría, prioridad, estado, rango | Test E2E |
| AC-09 | Búsqueda incremental encuentra por título/etiqueta | Test E2E |
| AC-10 | Repetición: serie semanal con excepción en una instancia | Test unit (RRULE engine) |
| AC-11 | App 100 % funcional sin red (desconectar → operar normal) | Manual + test de red simulada |
| AC-12 | Arranque ≤ 1.5 s; RAM reposo ≤ 60 MB (perfil medido) | Benchmark CI |
| AC-13 | Tema claro/oscuro; identidad Soft UI según doc 04 en todas las pantallas | Revisión de diseño por checklist |
| AC-14 | Backup automático diario + export/import JSON + export iCal | Test de integración |
| AC-15 | Cero telemetría; sin cuenta | Revisión de código (sin endpoints) |

---

## 9. Backlog priorizado (MoSCoW)

### Debe (Must — MVP)

| ID | Item | Justificación |
|----|------|---------------|
| FR-01/FR-02 | Quick Add NL + formulario manual | Núcleo de la propuesta de valor |
| FR-14/15/16/17 | Vistas calendario + agenda | Base del producto |
| FR-21/22 | Modelo de tarea + estados | Entidad fundamental |
| FR-27/28/31 | Recordatorios múltiples + toast nativo | Promesa de "no olvidar" |
| FR-33 | Bandeja del sistema | Recordatorios con app cerrada |
| FR-43 | Categorías con color/icono | Organización mínima |
| FR-45 | Prioridades | Escala del brief |
| FR-52 | Offline-first (arquitectura) | Principio P4 |
| FR-56 | Temas claro/oscuro | Identidad |
| FR-59 | Onboarding 3 pasos | Activación |
| FR-06* | Papelera (*P2 pero crítica para confianza) | Seguridad psicológica |

### Debería (Should — V1)

| ID | Item |
|----|------|
| FR-04 | Drag & drop reprogramar |
| FR-05/06 | Duplicar / papelera |
| FR-07..13 | Parser avanzado (repetición por texto, confianza, inglés) |
| FR-18/19 | Días completos, resaltado prioridad |
| FR-25 | Enlaces y notas |
| FR-29/30 | Recordatorio personalizado + recálculo |
| FR-32/35 | Acciones de toast + "Mientras no estabas" |
| FR-34 | Autostart |
| FR-44/46/47 | CRUD categorías, filtros combinables, búsqueda |
| FR-50/51 | Repetición RRULE |
| FR-53/54/55 | Backup, export JSON, export iCal |
| FR-57 | Atajo global captura |
| FR-58 | i18n ES/EN |
| NFR-13 | Auto-update |

### Podría (Could — V2/V3)

| ID | Item |
|----|------|
| FR-37..42 | Widget de escritorio completo |
| FR-24 | Subtareas/checklist |
| FR-23 | Progreso slider |
| FR-36 | No molestar |
| FR-41/22 | Acciones desde widget |
| FR-48/49 | Estadísticas + CSV |
| FR-20 | Mini-mes |
| FR-12 | Parser con IA (V3) |
| FR-10 | Repetición por texto IA |
| FR-16 | Paleta de comandos Ctrl+K |

### No por ahora (Won't — fuera del horizonte actual)

| ID | Item | Motivo |
|----|------|--------|
| — | Colaboración en tiempo real | V6+; requiere sync y cuentas |
| — | App móvil | V5 por roadmap |
| — | Plugins de terceros | V6; API de comandos V3 |
| — | Modo multiusuario / workspaces | Antí-persona |

---

## 10. Priorización RICE (primeras 12 candidatas a V1)

Escala 1–10 (R=Reach usuarios afectados, I=Impacto, C=Confianza, E=Esfuerzo hombre-semana).

| Feature | R | I | C | E | RICE | Orden |
|---------|---|---|---|----|------|-------|
| Quick Add lenguaje natural | 10 | 10 | 0.9 | 6 | 15.0 | 1 |
| Toast nativo + recordatorios múltiples | 10 | 10 | 0.9 | 4 | 22.5 | 2 |
| Vista Agenda mixta | 10 | 9 | 0.9 | 3 | 27.0 | 3 |
| Vista semana + arrastre | 10 | 9 | 0.8 | 5 | 14.4 | 4 |
| Categorías color/icono | 9 | 8 | 1.0 | 2 | 36.0 | 5 |
| Filtros combinables | 9 | 8 | 0.8 | 4 | 14.4 | 6 |
| Búsqueda incremental | 9 | 7 | 0.9 | 3 | 18.9 | 7 |
| Repetición RRULE | 7 | 8 | 0.9 | 6 | 8.4 | 8 |
| Bandeja + autostart | 8 | 9 | 0.9 | 2 | 32.4 | 9 |
| Tema oscuro | 9 | 7 | 1.0 | 3 | 21.0 | 10 |
| Backup + export JSON/iCal | 7 | 8 | 0.9 | 3 | 16.8 | 11 |
| "Mientras no estabas" | 7 | 8 | 0.8 | 3 | 14.9 | 12 |

> Nota: RICE puro premia features baratas; la **secuencia del roadmap** (doc 05) prioriza además dependencias técnicas y narrativa de producto (primero captura+recordatorio, luego organización, luego widget).

---

## 11. Funcionalidades futuras (horizonte)

| Versión | Feature | Nota |
|---------|---------|------|
| V2 | Widget completo, paleta de comandos, subtareas, progreso, mini-mes | Ver roadmap |
| V3 | IA: parser de alto nivel, organización automática, sugerencia de horarios, detección de conflictos, división de tareas grandes, recomendación de prioridad | Contratos ya definidos en arquitectura |
| V4 | Sync: Google / Outlook / iCal (unidireccional primero), multi-dispositivo | Operation log ya diseñado |
| V5 | App móvil (lectura de "qué tengo hoy" + captura rápida; escritura completa después) | Comparte dominio y modelo de datos |
| V6 | Colaboración, plugins, templates, hábitos/Pomodoro (si auditoría valida demanda) | |

---

## 12. KPI de producto (post-MVP)

| KPI | Definición | Objetivo 90 días |
|-----|-----------|------------------|
| Tareas por semana (WAU) | Tareas creadas / usuario / semana | ≥ 15 |
| % captura por NL | Tareas vía quick add NL / total | ≥ 70 % |
| % tareas con recordatorio | Con ≥ 1 recordatorio | ≥ 60 % |
| Retención semanal | Usuarios activos semana N / semana N-1 | ≥ 60 % |
| Cumplimiento | Completadas / programadas (semana) | ≥ 70 % |
| Olvidos reportados | Encuesta trimestral "¿olvidaste una fecha?" | < 15 % |
