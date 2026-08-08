# 10 — Motor de Restricciones (Fase 5)

Estado: ✅ **Diseñado e implementado.** El planner que orquesta los slots es
la Fase 6 (roadmap: `05-Roadmap.md`).

## 1. Objetivo

Separar dos responsabilidades que el sistema jamás mezcla:

| Capa | Qué hace | Qué usa |
|---|---|---|
| **Interpretación** (módulo `ai`, fases 3-4) | texto libre → `Intent` | LLM o heurística |
| **Motor de restricciones** (este módulo, `engine`) | matemática de calendario: qué se puede y qué no | aritmética de intervalos, **sin LLM, sin aleatoriedad** |

Regla de oro: *la IA interpreta lenguaje natural; el motor hace cuentas
deterministas.* Ningún LLM participa en decidir si un intervalo choca con
otro.

```
texto ──► interpret() ──► IntentBatch ──► ConstraintEngine::from_intents()
                                                   │
                      ┌────────────────────────────┤
                      ▼                            ▼
        Consultas (qué es posible)      suggest_slot (qué slot proponer)
        is_available / available_minutes /        │
        blocked_intervals_on / violations         ▼
                                                  SlotProposal
```

## 2. Modelo completo

### 2.1 Convenciones temporales

- Todo tiempo en **ms epoch local** (mismo convenio que el modelo de datos,
  spec/03 y spec/09).
- Los intervalos son **semiabiertos `[start, end)`**: `[9,10)` y `[10,11)`
  son adyacentes pero **no** se solapan. Un evento que termina a las 10:00 y
  otro que empieza a las 10:00 son compatibles.

### 2.2 Tipos del motor

| Tipo | Descripción |
|---|---|
| `Interval { start, end }` | intervalo semiabierto; `minutes()`, `contains()`, `overlaps()`, `contains_interval()` |
| `Severity { Hard, Soft }` | severidad de una restricción |
| `Block { interval, label, severity }` | bloqueo con etiqueta ("qué bloquea") |
| `DayWindow { start_min, end_min }` | horario laboral diario en minutos desde medianoche |
| `Night { start_min, end_min }` | sueño; `end_min <= start_min` → cruza medianoche (ej: 23:00 → 07:00) |
| `Deadline { at_ms, label }` | vencimiento duro |
| `SoftPreference` | `StartAfter { minute }` (estudiar ≥ 16:00) · `Order { first, second }` (A antes de B) |
| `Violation { rule, severity, message }` | resultado de evaluar una propuesta |
| `SlotProposal { prep_start_ms, task_start_ms, task_end_ms, soft_violations }` | slot propuesto |

### 2.3 Restricciones soportadas

#### Hard (no violables)

| Restricción | Origen | Cómo se aplica |
|---|---|---|
| Compromiso existente (evento) | `Intent::Event` con ventana | intervalo bloqueado; no solapar |
| Bloques explícitos | `engine.blocks` | intervalo bloqueado ("tengo clase de 2 a 4") |
| Vencimiento | `Intent::Deadline` / `deadline_ms` | el ítem debe **terminar** antes |
| Sueño | `Night` | intervalo bloqueado, con cruce de medianoche |
| Horario laboral | `DayWindow` (default 09:00-18:00) | fuera de la ventana → no se agenda |
| Ventana de disponibilidad | `Intent::Availability` | región permitida = disponibilidad ∩ horario laboral |
| Duración mínima | `min_duration_min` | `suggest_slot` rechaza tareas más cortas |

#### Soft (violables con penalización)

| Preferencia | Ejemplo del enunciado | Penalización |
|---|---|---|
| `StartAfter { minute }` | "I prefer studying after 4 PM" | minutos de adelanto del inicio (se minimiza) |
| `Order { first, second }` | "preferred task order" | se conserva y reporta; la aplica el planner (Fase 6) |

### 2.4 Semántica de `daily_cap` (ejemplo "Don't schedule anything before 6 AM")

- El cap `"HH:MM"` fija el inicio mínimo del horario laboral (hard).
- Sobre el horario **por defecto** (09:00): el cap lo **reemplaza**.
- Sobre un horario **explícito**: solo lo **eleva** (nunca lo reduce).

## 3. Consultas del motor

| Pregunta del enunciado | Método | Respuesta |
|---|---|---|
| ¿Está disponible este tiempo? | `is_available(start, end)` | `Vec<Block>` (vacío = libre; incluye etiqueta de cada bloqueo) |
| ¿Cuánto tiempo disponible hay? | `available_minutes(from, to)` | minutos libres en la región permitida |
| ¿Qué intervalos están bloqueados? | `blocked_intervals_on(day)` | `Vec<Block>` recortados al día, ordenados |
| ¿Qué restricciones se violarían? | `violations(start, end, deadline_ms)` | `Vec<Violation>` hard + soft |
| ¿Cuáles son soft? | `all_constraints()` | lista `(regla, Severity)`; filtrar `Soft` |

## 4. Planeo determinista: `suggest_slot`

```rust
suggest_slot(duration_min, prep_min, deadline_ms, preferred_after_min) -> Option<SlotProposal>
```

Algoritmo (sin RNG, sin LLM):

1. `duration_min < min_duration_min` → `None` (duración insuficiente).
2. Escanea `lookahead_days` (14) días desde hoy, en pasos de `step_min`
   (15') sobre los **intervalos libres** (`free_intervals_on`).
3. Un candidato es viable solo si: cabe en la región permitida, no choca con
   nada hard y `task_end <= deadline_ms`. La **preparación** (`prep_min`)
   ocupa un bloque contiguo inmediatamente anterior a la tarea y también
   debe terminar antes del vencimiento.
4. Penalización soft = minutos que el inicio de la tarea se adelanta a
   `preferred_after_min` (hora del día, no absoluta).
5. Gana el candidato con **menor penalización**; empate → el **más
   temprano**.

Determinismo garantizado: mismo estado → misma respuesta, sin importar
cuántas veces se consulte (verificado por test).

## 5. Puente con la fase 3 (`from_intents`)

| `Intent` | → estado del motor |
|---|---|
| `event` con ventana | `commitments` (hard, label = título) |
| `event` all-day con fecha | bloqueo del día completo |
| `availability` | `availability` (hard) |
| `deadline` | `deadlines` (hard) |
| `constraint` `daily_cap "HH:MM"` | inicio mínimo del horario laboral (§2.4) |

## 6. Límites del motor (diseño consciente)

- **No planifica**: elige un slot para un ítem; elegir entre varios ítems,
  ordenarlos por prioridad y aplicar `SoftPreference::Order` es el trabajo
  del planner de la Fase 6.
- **Prioridad** (Alta/Media/Baja) no entra en la matemática de slots; el
  planner la usa para ordenar.
- **Horario laboral uniforme**: todos los días iguuales; un `by_day` por día
  de la semana queda para una evolución posterior.
- Conflicto irresoluble dentro del horizonte → `None` + reporte de
  `violations` para que la UI pida decidir al usuario.

## 7. Tests (33, todos en `engine::tests`)

| Área | Cobertura |
|---|---|
| Semántica de intervalos | semiabierto, adyacencia, `merge` (une solape, no adyacencia), `subtract` |
| Eventos | solapados → unión bloqueada; adyacentes → sin falso solape |
| Bloques | tiempo bloqueado explícito con etiqueta |
| Vencimientos | fin ≤ deadline, cruce de días, violación reportada |
| Duración mínima | rechazo por corta, encaje en hueco |
| Disponibilidad | restringe región, intersección con horario laboral |
| Múltiples restricciones | compromiso + bloque + vencimiento combinados |
| Conflictos | horario vs vencimiento → infactible → `None` |
| Sueño | cruce de medianoche, día empieza hábil |
| Soft | `StartAfter` respetado, cede si infactible, listado soft |
| Preparación | bloque contiguo previo, con vencimiento |
| Reportes | `blocked_intervals_on` etiquetado y ordenado, intervalo inválido |
| Consultas | `available_minutes`, horario 24h, grid 15' |
| Determinismo | misma consulta → misma respuesta |
| Puente | `from_intents` mapea eventos, caps, deadlines, disponibilidad |
| Ejemplos del enunciado | "don't schedule before 6 AM", "class from 2 to 4" |
