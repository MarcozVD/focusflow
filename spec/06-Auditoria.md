# 06 — Auditoría Crítica del Proyecto

**Producto:** FocusFlow · **Versión:** v0.1 · **Fecha:** 2026-08-04
**Alcance:** evaluación honesta del proyecto, comparativa competitiva (Todoist, TickTick, Google Calendar) y propuestas de cambio — algunas disruptivas.

---

## 1. Veredicto ejecutivo

| Dimensión | Nota (1–10) | Comentario |
|-----------|-------------|------------|
| Claridad de problema | 9 | Problema real, dolor concreto, bien acotado |
| Diferenciación | 6 | "Calendario+tareas+recordatorios+widget" es diferenciador **de combinación**, no de feature aislada. La defensa real está en NL + local-first + velocidad |
| Arquitectura | 8 | Local-first + Rust/Svelte/Tauri es defendible y habilita el futuro sin servidor |
| Modelo de datos | 8 | "Todo es una tarea" es la decisión correcta; RRULE estándar, operation log bien puesto |
| Diseño | 8* | Pendiente de la referencia visual (doc 04). El riesgo es el "neumorfismo recargado" (ver §4) |
| Riesgo de mercado | 6 | Dominio saturado de incumbentes **gratis**. La barrera de entrada es la confianza y el hábito, no la tecnología |
| **Nota global** | **7.3** | Producto sólido y ejecutable. Competir exige pulir las 5 propuestas del §5, no solo cumplir el brief |

---

## 2. Comparativa competitiva (capas de producto)

| Capacidad | **FocusFlow** | Todoist | TickTick | Google Calendar |
|-----------|---------------|---------|----------|-----------------|
| Entrada por lenguaje natural | **Núcleo (P0)** | Excelente (es su apuesta) | Muy buena | Débil |
| Calendario integrado | **Sí (vistas nativas)** | No (solo lista; calendario en suscripción pro via integración) | Sí (fuerte) | Nativo |
| Agenda mixta tareas+eventos | **Sí (P0)** | No | Parcial | No |
| Recordatorios múltiples por ítem | **Sí (P0)** | Limitado (1–2) | Sí | 1 notificación |
| Widget de escritorio | **Sí (transparente, always-on-top)** | No | Sí (Win widgets, menos flexible) | No |
| Offline-first total | **Sí (100 %)** | Parcial | Parcial | No (web) |
| Modo oscuro + identidad propia | Sí | Sí | Sí | Sí |
| Repeticiones avanzadas (RRULE) | V1 | Básicas | Avanzadas | Avanzadas |
| IA | V3 (local-first) | IA decente (nube) | IA (nube) | Gemini (nube) |
| Sync calendarios externos | V4 | Solo tareas ↔ GCal pro | Sí (2-way) | Nativo |
| Precio | **Compra única / gratis MVP** | 4–8 €/mes | 3–5 €/mes | Gratis (cuenta obligatoria) |
| Cuenta obligatoria | **No** | Sí | Sí | Sí |
| Privacidad (datos locales) | **Máxima** | Nube | Nube | Nube |
| Estadísticas y rachas | V2 | Débiles | Buenas (Pomodoro incluido) | No |
| Pomodoro / hábitos | No planificado | No | Sí | No |

**Lectura:** FocusFlow gana en *privacidad, velocidad, widget y modelo unificado*; pierde en *ecosistema, madurez, sync, IA integrada y curva de confianza*. Nadie compra un gestor de tareas por 10 features; lo compran por **el hábito**. El plan debe crear el hábito en los primeros 3 minutos (onboarding + primer recordatorio en el mismo día).

---

## 3. Debilidades del proyecto actual (autocrítica)

| # | Debilidad | Severidad | Diagnóstico |
|---|-----------|-----------|-------------|
| D1 | **Sin estrategia de adquisición** | Alta | El documento asume producto; no hay canal (¿cómo llega "Diego"?). Para app de Windows local: redes sociales + Product Hunt + YouTube "setups de estudio" + descarga directa sin registro |
| D2 | **El MVP aun así es grande** | Alta | 24 HS para 2 devs ≈ 3 meses para lo "mínimo". Riesgo de entregar tarde sin diferenciación probada. **Mitigación:** beta pública tras la primera mitad del MVP con el quick add NL funcionando — valida H1 antes de terminar el resto |
| D3 | **TickTick ya es "la" alternativa para estudiantes** | Media | Es gratis, multiplataforma, con calendario y widget. La única carta real de FocusFlow es **local-first + diseño premium + cero cuenta**. Eso se comunica en la landing, no en el feature set |
| D4 | **Riesgo de diseño: neumorfismo decorativo** | Media | El neumorfismo puro (sombras dobles, relieves) falla en accesibilidad (contraste) y en dark mode. Debe aplicarse a superficies de "contenido", no a controles densos (ver §4) |
| D5 | **Un solo usuario es un callejón sin salida comercial** | Media | Sin cuenta, sin nube, sin telemetría: **no hay datos de retención**. Decisión deliberada y ética, pero exige KPI proxy (crash reports opt-in, encuestas, email de beta) para no operar a ciegas |
| D6 | **El parser NL ES es el trabajo más caro y menos visible** | Media | 50+ frases de regresión son pocas para la variedad real ("el 15 a las 9", "pa' la otra semana", "cada jueves de por medio"). El preview-editable mitiga, pero el usuario que falla 2 veces abandona la feature |
| D7 | **Windows-only limita la vida social del producto** | Baja | Aceptado por brief; el roadmap móvil (V5) lo corrige tarde. Mitigación: en V2, exportar/leer desde el celular vía web view de solo-lectura (opcional) |
| D8 | **Sin onboarding de "valor rápido"** | Media | 3 pasos + primer recordatorio: el usuario debe oír su propia notificación el día 1. Eso convierte en tangible la promesa |
| D9 | **Estadísticas llegan en V2 (poco a poco)** | Baja | El % de cumplimiento y las rachas son el gancho de retorno diario en TickTick. Subirlas a V1 reduce abandono |

---

## 4. Riesgos de diseño detectados (Soft UI)

1. **Contraste**: el neumorfismo con `#F8F8F8` + sombras suaves produce texto gris sobre fondo gris → fracasa en luz diurna y WCAG. **Regla:** el texto jamás se difumina con sombra; se usa negro `#1F2937` con peso 500+; las sombras solo dan *profundidad de superficie*, no legibilidad.
2. **Dark mode**: el neumorfismo oscuro tiende a "plata sucia". **Regla:** dark usa la misma paleta con valores invertidos y sombras *más profundas pero igual de difusas*; los controles activos siguen con azul primario saturado (el azul es el ancla de identidad en ambos temas).
3. **Sobrecarga**: si *todos* los elementos son tarjetas con relieve, el usuario no sabe qué es interactivo. **Regla:** solo los *contenedores* (panel, tarjeta de tarea, calendario, widget) llevan relieve; los *controles* (botón, checkbox, input) llevan una sola sombra sutil y el hover revela la profundidad.
4. **Movimiento**: 150–250 ms es correcto; prohibido animar layout (solo transform/opacity) para 60 fps en webview.

---

## 5. Propuestas de cambio — las que harían competitivo el producto

Priorizadas por impacto/riesgo. Las tres primeras son **cambios de producto**, las dos últimas son **de modelo de negocio/mercado**.

### P1. "Quick Add que enseña" (cambio de onboarding + retención)
**Problema:** el usuario no escribe frases porque no confía en que la app las entienda.
**Solución:** en la primera semana, el placeholder y una mini-guía muestran **3 frases ejemplo que cambian según el contexto** (hora del día: "Mañana…", "Este viernes…", día 15: "Pagar…"). Añadir un "traductor": al escribir, cada entidad detectada se **subraya con su color** (fecha en azul, hora en verde, categoría en su color) antes de crear. Enseña el parser jugando.
**Impacto:** sube % captura por NL (KPI), reduce abandono de la feature (D6).

### P2. "Revisión semanal" (la feature de retención que nadie tiene bien hecha)
**Problema:** los gestores de tareas son reactivos; el valor real está en la reflexión.
**Solución:** domingo 19:00, la app abre (o notifica) con un resumen de 10 segundos: cumplimiento de la semana, tareas movidas, próximos 7 días con carga por día, sugerencia (V3 IA) de 3 tareas para el lunes. En V1 sin IA: solo el resumen con acción de "mover todo lo vencido a la próxima semana en 1 clic".
**Impacto:** la app pasa de herramienta a *ritual*. TickTick tiene stats; nadie tiene el ritual de revisión como feature de primera clase.

### P3. "La bandeja de captura eterna" (diferenciador técnico visible)
**Problema:** la captura requiere abrir algo.
**Solución:** además de Ctrl+Shift+Espacio, ofrecer **captura desde el widget** y **captura desde el tray** (menú con campo de texto). Y en V2: **global hotkey configura por usuario**. Cuando el usuario no necesite abrir la app para capturar, la app se convierte en el "pegamento" de su día.
**Impacto:** diferencia real contra Google Calendar (que exige abrir el navegador).

### P4. Modelo económico honesto (no asumir "gratis para siempre")
**Recomendación:** MVP/V1 gratuitas, V2 (widget avanzado + stats) con compra única ~20–30 € o "paga lo que quieras" para estudiantes, V3+ IA local incluida (costo marginal ~0). No suscripción (anti-posición vs Todoist/TickTick). La decisión de monetizar se toma **después** de validar retención con la beta, no antes.
**Impacto:** la sostenibilidad no mata la diferenciación "sin cuenta, sin nube".

### P5. Prueba de concepto pública en la mitad del MVP (validar H1)
**Cambio de proceso:** liberar beta pública con *solo* quick add NL + recordatorios + vista agenda (≈ mitad del alcance MVP) durante 4 semanas y medir: % captura NL, retención semanal, tareas/semana. Si H1 falla, se redirige el alcance restante (la segunda mitad del MVP se diseña con datos, no con suposiciones).
**Impacto:** la decisión más barata y más importante de todo el roadmap.

### P6 (recomendada, media prioridad). Plantillas y "día típico"
Permitir plantillas de tareas (exámenes con repetición semestral, pagos de servicios, rutinas) para que "Diego" configure en 5 minutos su semestre. Todoist/TickTick lo tienen desordenado; como feature de onboarding es oro.

---

## 6. Matriz de riesgos de negocio

| Riesgo | Prob. | Imp. | Mitigación |
|--------|-------|------|------------|
| Retención baja (la gente no vuelve) | Alta | Alta | P2 (revisión semanal) + primer recordatorio el día 1 + KPI desde beta |
| "¿Y esto qué tiene de mejor que TickTick?" | Alta | Media | Comunicar local-first + cero cuenta + diseño; demo de captura de 10 s en la landing |
| Abandono del parser NL | Media | Alta | P1 (subrayado de entidades) + preview editable + IA en V3 |
| WebView2 ausente en máquinas viejas | Media | Media | Detección en instalador con link de instalación de WebView2 |
| Fatiga de "otra app de productividad" | Media | Media | Posicionamiento emocional ("la app que no te deja olvidar tu vida") + beta con estudiantes reales |
| Escala 125/150% rompe el widget | Media | Baja | Tests visuales multi-escala (ya en roadmap) |
| Coste de IA local (RAM/CPU de usuarios modestos) | Media | Media | Modelo 3B cuantizado + fallback reglas + opción remota |

---

## 7. Features que NO deben entrar (y por qué)

| Feature que pedirán | Decisión | Motivo |
|---------------------|----------|--------|
| Kanban / tableros | ❌ | Cambia el modelo mental "todo es una tarea"; su consumo está fuera de persona |
| Colaboración multiusuario | ❌ hasta V6 | Costo enorme (servidor, cuentas, permisos) para una app personal; mata la diferenciación local |
| Email nativo / Slack en la app | ❌ | Scope creep clásico; la integración llega por plugins V6 |
| Asistente de voz | ⏸ V6 | Windows Speech está maduro, pero el valor marginal es bajo frente al teclado para captura |
| Widget de calendario completo (mes entero) | ⏸ V3 | El widget debe ser *pequeño y mirar hacia adelante*; un calendario en el widget compite con la app |

---

## 8. Lo que haría competitivo a FocusFlow (resumen ejecutivo de la auditoría)

1. **Validar la hipótesis de captura en beta antes de terminar el MVP** (P5) — es la decisión de mayor retorno.
2. **Crear el ritual de revisión semanal** (P2) — la única feature que los incumbentes no tienen como producto.
3. **El widget es la armadura**: transparente, always-on-top, 60 MB extra máx., y *nadie más* en Windows ofrece eso con tareas. Máxima inversión de diseño ahí.
4. **Diseño premium como identidad, no como adorno**: Soft UI moderado (superficies con relieve, controles limpios), porque "premium 2026" es la promesa que Google/Todoist no pueden cumplir en Windows.
5. **Sin cuenta, sin nube, sin telemetría** es una ventaja que se *vende* (privacidad como feature), no un defecto que se oculta.
6. **No competir en features**: competir en el primer minuto (captura en 2 s) y en el hábito diario (widget + recordatorios + revisión semanal).

---

## 9. Checklist de "¿estamos listos para construir?"

- [ ] Ruta de imagen de referencia recibida → doc 04 (Design System) con paleta exacta
- [ ] Spike técnico (doc 02 §11) cerrado
- [ ] Decisiones P4 (monetización) y P5 (beta temprana) tomadas con el dueño del producto
- [ ] Los 15 AC del MVP firmados
- [ ] Primer prototipo interactivo de la pantalla principal (Svelte, sin dominio) para validar diseño con 5 personas de la persona objetivo
