# UX.md — Reglas de experiencia de FocusFlow

**Fuente:** comportamiento real de `spike/frontend/src/lib/` + backend Rust (vía comandos IPC).

---

## 1. Principios de experiencia

1. **El tiempo es información espacial** — el usuario debe distinguir ocupado/libre/urgente/completado/all-day/pasado/futuro de un vistazo, sin leer.
2. **Nada se modifica sin aprobación** — QuickAdd y Asistente proponen; el usuario decide.
3. **El fallback es instantáneo** — si la IA tarda, el parser local responde; la experiencia nunca se bloquea.
4. **Predecible y honesto** — la priorización es determinista; los errores se explican con causa y solución.
5. **Un vistazo basta** — el widget es la prueba: 3 secciones, 3 botones, cero fricción.

---

## 2. Navegación

- **Vistas:** Semana (default) · Mes · Día · Agenda · Sugerencias · Asistente · Ajustes — sidebar izquierdo.
- **Temporal:** TopBar arrows (◀ ▶) + "Hoy". Semana/Mes navegan por unidad (7 días / 1 mes); Día por día.
- **Seleccionar un día** en semana/mes → salta a vista Día.
- **Hash routing** para el widget (`#/widget`).
- El **sidebar fija 232 px**; la navegación es un conjunto de botones (no `<a>`), coherente con app SPA de escritorio.
- Regla: el usuario nunca pierde el contexto — al volver a Semana se conserva `date`.

---

## 3. Quick Add (captura rápida)

- **Atajo:** `Ctrl+Shift+Espacio` desde cualquier aplicación (global) o el input del TopBar.
- **Entrada:** lenguaje natural. Ejemplo en placeholder: *"Mañana estudiar cálculo de 3pm a 5pm"*.
- **Feedback inmediato:** mientras escribes se muestran **chips de entidades detectadas** (Mañana / Fecha / Próximo lunes / Horario / Prioridad alta / Recordatorio / Categoría).
- **Doble flujo:**
  - IA disponible: Enter → propuesta de plan → si es **evento único con fecha** se auto-acepta (feedback "Tarea creada"); si es plan multi-item → modal de revisión.
  - IA lenta (> 8 s): banner "La IA está tardando más de lo normal" + botón **"Usar interpretación rápida"** (parser local).
- **Errores:** "No se pudo interpretar la tarea" con toast; nunca deja al usuario sin respuesta.
- **Anti-duplicados:** Enter repetido mientras procesa se ignora.

---

## 4. Creación y edición de tareas

- **Crear:** QuickAdd (lenguaje natural) es la vía principal; el backend deriva categoría/prioridad por palabras clave.
- **Editar:** clic en cualquier tarea → **TaskDrawer** (panel derecho, 400 px): título, descripción, categoría, prioridad, todo-el-día, inicio/fin (fecha + hora), recordatorio, etiquetas, notas, enlaces.
- **Acciones del drawer:** Completar/Reabrir, Duplicar, Eliminar (con diálogo de confirmación), Guardar.
- **Reglas de validación:** fin ≤ inicio → fin = inicio + 1 h; all-day ignora horas; guardado con feedback ("Guardado ✓").
- **Duplicar** crea copia inmediata en caché.
- **Eliminar** pide confirmación explícita (dialog con nombre de la tarea).

---

## 5. Drag & drop (calendario)

- **Mover:** arrastra un EventBlock → sigue al cursor con precisión (grabY en px, sin snap durante el arrastre); al soltar, snap a 5 min.
- **Redimensionar:** handles superior/inferior (aparecen en hover) — mínimo 30 min.
- **Convertir a todo-el-día:** soltar sobre la fila "Todo el día" → la tarea pasa a all-day.
- **Ghost:** el original se atenúa y un fantasma con tiempo muestra el destino en vivo.
- **Auto-scroll:** cerca de bordes superior/inferior del área, el scroll avanza solo.
- **Conflictos:** el backend valida. Por defecto **permite con aviso** ("Movida con aviso: se solapa con «X»"); en modo estricto (Ajustes) **bloquea**.
- **Errores de drag:** toast "No se pudo mover: conflicto de horario".

---

## 6. Calendario — el tiempo como información espacial

### 6.1 Qué se debe distinguir de un vistazo

| Estado | Señal visual |
|--------|--------------|
| **Ocupado** | Bloque con color de categoría al 13 % + borde izquierdo de 3 px |
| **Libre** | Slots vacíos con línea `--border` |
| **Urgente** | Punto/barra `--danger`, badge prioridad alta |
| **Completado** | Tachado + opacidad 50 % |
| **All-day** | Fila superior "Todo el día" con chips |
| **Pasado** | Eventos vencidos con borde izquierdo **dashed** + opacidad |
| **Futuro/próximo** | Línea "ahora" (`--primary` con punto) separa pasado/futuro |
| **Multi-día** | Stub "Inicio ·" / "Fin ·" + chip continuo intermedio |

### 6.2 Reglas de legibilidad

- Grid horario dinámico 6–22 h (se expande si hay tareas fuera).
- Semana: máx. 8 eventos visibles + "+N más" → clic abre el día completo.
- Mes: chips truncados antes que "+N más"; fuera de mes opacity 0.4.
- La **línea "ahora"** es la única referencia temporal absoluta: todo lo demás es relativo a ella.
- Números y horas siempre tabulares — el tiempo se lee, no se descifra.

---

## 7. Tareas sin hora (all-day)

- Ocupan la fila superior ("Todo el día") en semana/día; chips en mes.
- No bloquean el time-area: no consumen espacio horario.
- En Agenda aparecen después de las tareas con horario del día.
- En el widget: "todo el día" como due label.
- Son **marcadores del día**, no bloques de tiempo (regla del motor de planificación).

---

## 8. Urgencia y prioridad

- **Prioridades:** alta / media / baja. Alta y media tienen badge visible en cards; baja no muestra badge (menos ruido).
- **Determinismo:** el asistente clasifica respuestas en URGENTE / IMPORTANTE / NORMAL por reglas (vencimiento, prioridad, all-day), no por la IA.
- **Widget:** sección "Importante" prioriza lo que termina pronto / prioridad alta.
- **Vencidas:** estado derivado (start < now); marcado visual en calendario (dashed) y agenda (badge "Vencida").

---

## 9. Tareas vencidas

- Se derivan automáticamente (no requieren acción del usuario).
- Visual: borde izquierdo dashed + fondo tintado + badge.
- El usuario las completa o las mueve; al completarlas se "limpian" del flujo.
- En Agenda y widget aparecen primero (relevancia).

---

## 10. AI scheduling (planificación inteligente)

**Flujo completo:** texto → entendimiento (intents) → plan propuesto (sesiones por ítem) → revisión → aprobación → aplicación.

- **Entendimiento explícito:** la propuesta muestra "Entendí" con tipo (Evento/Tarea/Vencimiento/Preparación/Disponibilidad/Recordatorio/Restricción), categoría y "cuando".
- **Honestidad de tiempo:** si el plan no cabe en el tiempo disponible, lo dice: *"Tiempo insuficiente: se planificaron 2 de 4 horas"*.
- **Edición antes de aceptar:** el usuario puede mover/eliminar/añadir bloques por sesión (fecha + hora + duración).
- **Registro de resultados:** aceptar/rechazar/error quedan registrados por propuesta (el hilo no re-muestra botones resueltos).
- **Fallback local:** el mismo pipeline con parser de reglas (source "Local" en el chip de la propuesta).

---

## 11. Email intelligence

- **Detección:** exámenes, entregas, reuniones, vencimientos, disponibilidad → sugerencias con `confidence` y `reason`.
- **Revisión:** la bandeja de Sugerencias muestra remitente, confianza %, razón (itálica), duplicados.
- **Acciones:** Aceptar (crea tarea) · Editar (ajustar antes de aceptar) · Fusionar (con tarea existente, elige destino) · Rechazar · Revertir (si aceptaste y cambiaste de opinión, 1 h) · Borrar.
- **Auto-aprobación:** remitentes de confianza pueden añadirse directamente al calendario (config explícita del usuario).
- **Anti-spam:** sincronización por checkpoints, solo correos recientes, filtros por remitente/dominio/palabra clave.
- **Nunca** se afirma que "cualquier email se convierte en evento": es una bandeja de revisión.

---

## 12. Widget

- **open → glance → act**: siempre visible, se lee en segundos, se actúa sin abrir la app.
- **Jerarquía de secciones:** Ahora (actividad en curso) → Por hacer (si no hay ahora) → Siguiente → Importante → "Todo claro por ahora".
- **Acciones:** ✓ completar · ⟳ posponer 1 h · ▶ empezar ahora (según contexto).
- **Abrir tarea** → salta a la app con el drawer abierto; **Preguntar** → Asistente.
- El widget **nunca es una mini-app**: no tiene formularios, solo glance + act.

---

## 13. Notificaciones

- **Contextuales, no spam:** tipos — vence pronto, tarea atrasada, conflicto, tiempo disponible, compromiso importante, sugerencia.
- **Controles:** horario de silencio (default 22:00–08:00), tope diario (default 5), mínimo de tiempo libre para sugerir (default 120 min).
- **Acciones en el toast:** Plan (salta al asistente con el contexto precargado) · Más tarde · Descartar.
- **Respuesta registrada** (planned/later/dismissed) para ajustar cadencia anti-spam.
- **Nativas de Windows** con identidad propia (backend Rust).

---

## 14. Tema

- Claro / oscuro + **6 acentos** configurables.
- El tema se aplica **antes del primer render** (localStorage → dataset.theme), sin parpadeo.
- Backend como fuente de verdad; localStorage como fast-path; se **difunde a todas las ventanas** (app + widget) vía evento `ui:prefs`.
- Transición suave de fondo/color (200 ms).

---

## 15. Estados vacíos

| Contexto | Mensaje |
|----------|---------|
| Sugerencias sin eventos | "Sin eventos detectados todavía." + sub: conecta el correo y te avisará |
| Popup de día sin tareas | "Sin tareas este día." |
| Widget sin actividad | "Todo claro por ahora" |
| Propuesta sin duración | "Sin duración: no hay sesiones que planificar." + cómo arreglarlo ("2 horas", "el viernes") |

**Regla:** los vacíos son **accionables** (explican qué hacer) o **tranquilizadores** (nada pendiente = estado bueno).

---

## 16. Estados de loading

- Botones con texto de estado: "Procesando…", "Comprobando…", "Guardando…", "Aplicando…", "Verificando…".
- Asistente: indicador de typing "Analizando tu calendario…".
- Sync: barra de progreso con mailbox + contador (procesados/total).
- **Regla:** nunca bloquear la UI con un spinner gigante; el estado vive en el control que disparó la acción.

---

## 17. Errores

- **Fatal:** banner inferior con `role="alert"` + botón Cerrar (evita UI congelada silenciosa).
- **Asistente:** mensaje amigable con causa (429 → "Reintentar"; autenticación, DNS, TLS, tiempo de espera, clave inválida → explicación + solución).
- **QuickAdd:** toast "No se pudo interpretar la tarea" + fallback local.
- **Drag:** toast con el motivo (conflicto).
- **Formularios:** errores inline junto al campo (`aria-invalid` en onboarding) o mensaje de estado ("Error: …").
- **Regla:** todo error explica la causa y da una salida (reintentar, cambiar config, cancelar).

---

## 18. Onboarding (primera ejecución)

1. **Hero:** "Tu calendario, al día. Sin copiar ni pegar." + 3 valores (correo conectado, IA para planear, privado por diseño) + Comenzar / Omitir.
2. **Conexión de servicios:** Google (OAuth2, se abre navegador, vuelve solo) + IA (presets Groq/OpenCode/Gemini con endpoint/modelo, verificación en vivo).
- Cada paso puede saltarse; la verificación no bloquea si la IA no se configuró.
- Los errores se humanizan (contraseña de aplicación, DNS, TLS, 401…).
- Se puede reabrir desde Ajustes → "Volver a configurar servicios".

---

## 19. Accesibilidad (resumen de reglas UX)

- Foco visible siempre (ver ACCESSIBILITY.md).
- Todas las acciones tienen alternativa de teclado (Enter/Espacio en elementos role=button; Escape cierra overlays).
- Los estados usan color + forma + texto (nunca solo color).
- `prefers-reduced-motion` respetado.
- Los diálogos piden confirmación para acciones destructivas.
