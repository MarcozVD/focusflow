# 09 — Sistema de Intenciones (Fase 3)

Estado: ✅ **Diseñado e implementado (tipos + validación + parser).** El
planner que orquesta los intents es la Fase 4 (roadmap: `05-Roadmap.md`).

## 1. Objetivo

Todo input libre del usuario pasa por un paso único de análisis que produce
**intenciones estructuradas** (`Intent`). La IA extrae la información que el
texto declara —y **solo** la que declara—; el planner de la Fase 4 las
convierte en acciones sobre el calendario.

```
texto libre ──► parse_intent(text, provider, configured)
                     │
                     ├─ AI configurada ─► provider.chat_json(prompt, schema) ─► JSON
                     │                        │
                     │                        ▼
                     │              parse_intent_json (tolerante, unknown=null)
                     │                        │
                     │                        ▼
                     │              validate_intent (invariantes) ──► errores → BadResponse
                     │
                     └─ Sin IA ──► heurística módulo 1 (parse_task_nl) ─► Intent de tipo Task
                                          │
                                          ▼
                              IntentBatch { intents: Vec<Intent>, source: "ai"|"local" }
```

## 2. Esquema del `Intent`

| Campo | Tipo | Semántica | Desconocido |
|---|---|---|---|
| `intent_type` | enum | qué es (abajo) | — (obligatorio) |
| `title` | string | título en español, sin prefijos | — (obligatorio) |
| `description` | string | detalle libre | `""` |
| `category_id` | id | uni/trab/per/fin/sal/otr | `otr` |
| `priority` | enum | Alta/Media/Baja | Media |
| `window.start/end` | i64 ms | ventana temporal local | `None` |
| `window.all_day` | bool | día completo sin horas | — |
| `duration` | `{minutes}` | duración declarada | `None` |
| `deadline` | i64 ms | vencimiento | `None` |
| `preparation` | `{minutes,note}` | esfuerzo previo requerido | `None` |
| `recurrence` | `{frequency,interval,by_day,count,until}` | repetición | `None` |
| `reminders` | `[{minutes_before \| at}]` | avisos | `[]` |
| `constraints` | `[{kind,target,value}]` | restricciones entre ítems | `[]` |
| `confidence` | f64 [0,1] | seguridad del análisis | 0.0 |
| `reason` | string | justificación (modo explicar) | `""` |
| `source` | string | `"ai"` o `"local"` | — |

### `intent_type` (7 tipos discriminados — sin "tarea genérica")

| Tipo | Cuándo | Qué exige la validación |
|---|---|---|
| `event` | actividad con fecha/hora concreta | título |
| `task` | actividad sin fecha (backlog) | título |
| `deadline` | entrega con vencimiento | `deadline` presente |
| `preparation` | esfuerzo de preparación sin fecha | `preparation` presente |
| `availability` | ventana de disponibilidad (2 extremos) | `window.start` presente |
| `reminder` | solo aviso | ≥1 `reminders` |
| `constraint` | restricción entre ítems | ≥1 `constraints` |

### Compuestos

Un input puede producir varios intents (`IntentBatch`). Regla de oro:
*"Examen el viernes y necesito 4 horas"* → **1** `event` con
`preparation_minutes: 240` adjunto (el planner reserva el tiempo antes). Dos
actividades distintas → tantos intents como actividades.

## 3. Principios (no negociables)

1. **Unknown = `null`/`None`.** El prompt lo exige explícitamente
   (regla 6). Jamás un default inventado de hora/día/duración.
2. **Sin puente intermedio.** IA → JSON → `Intent` directo. Sin
   `ParsedTask`/marcos de por medio: un solo formato canónico.
3. **No hard-codear el modelo.** El prompt y el schema son invariantes de
   proveedor. Gemini, OpenAI, Anthropic o Zen implementan el mismo trait
   `AiProvider` (ver §6).
4. **Fechas relativas → absolutas** dentro del prompt (recibe "hoy" real del
   sistema); la validación es agnóstica al idioma.
5. **Tolerancia en el parseo, estrictez en la validación.** El parser acepta
   campos ausentes, `null`, vacíos y campos desconocidos (los ignora). La
   validación rechaza solo inconsistencia interna.

## 4. Reglas de validación (`validate_intent`)

Todas se verifican contra el intent ya construido; devuelven **la lista
completa** de errores (no solo el primero), para diagnóstico.

| Regla | Error |
|---|---|
| `confidence` fuera de [0,1] | fuera de rango |
| `window.end < window.start` | end antes de start |
| `duration.minutes == 0` o > 1440 | duración 0 / > 24 h |
| `deadline` en el pasado | deadline en el pasado |
| `deadline` < `window.start` | deadline antes de la ventana |
| `preparation.minutes` 0 o > 1440 | preparation inválida |
| `recurrence.interval` 0 o > 365 | interval inválido |
| `by_day` fuera de 1..=7 (ISO) | by_day inválido |
| `recurrence.count` 0 | count 0 |
| `recurrence.until` < `window.start` | until antes de la ventana |
| recordatorio sin `minutes_before` ni `at` | recordatorio vacío |
| `minutes_before` 0 o > 43200 (30 d) | recordatorio inválido |
| `at` en el pasado | recordatorio en el pasado |
| tipo ↔ contenido (tabla §2) | ej: `deadline` sin deadline |

Fuera de la validación (decisión): **errores de parsing del enum
(`intent_type`, `frequency`, `constraint.kind`) son errores** (no se silencian
ni se mapean a `other`): la IA debe devolver el vocabulario del schema o el
input cae al fallback local. `priority`/`category` desconocidos se degradan a
Media/`otr` (documentado en el schema).

## 5. Archivos

```
src-tauri/src/ai/intent.rs            — tipos (Intent, IntentBatch, TimeWindow…)
                                        + from_task (heurística módulo 1 → Intent)
src-tauri/src/ai/intent_validator.rs  — parse_intent_json (tolerante) + validate_intent
src-tauri/src/ai/intent_parser.rs     — INTENT_SCHEMA, SYSTEM_PROMPT, parse_intent
                                        (único punto de entrada), parse_batch_json
src-tauri/src/ai/mod.rs               — registro de módulos
```

Cobertura de pruebas: 14 nuevos tests (38 total en el crate):
fixtures de los 4 ejemplos de la fase, nulls válidos, timezone local
(regresión de la zona horaria), clamping de confidence, y cada regla de la
tabla §4.

## 6. Cómo enchufar Gemini (u otro proveedor)

`parse_intent` solo necesita `&dyn AiProvider`:

```rust
impl AiProvider for GeminiProvider {
    fn id(&self) -> &str { "gemini" }
    fn chat_json(&self, system: &str, user: &str, schema: &str) -> AiResult<serde_json::Value> {
        // Endpoint: https://generativelanguage.googleapis.com/v1beta/models/
        //           {model}:generateContent?key={API_KEY}
        // o el endpoint OpenAI-compatible de Gemini: /v1beta/openai/chat/completions
        // Cuerpo: { "system_instruction": { "parts": [{ "text": system }] },
        //           "contents": [{ "role": "user", "parts": [{ "text": user }] }],
        //           "generationConfig": { "responseMimeType": "application/json",
        //                                  "responseSchema": <schema JSON-Schema real> } }
        // → extraer candidates[0].content.parts[0].text y parsear el JSON.
    }
}
```

Nota: `INTENT_SCHEMA` está en formato "compacto" (humano+LLM). Para
proveedores con `responseSchema` estricto, conviértelo a JSON-Schema y pásalo
por el mismo campo. Nada en el pipeline depende del transport.

## 7. Límites de la fase (no implementado aún)

- **No** hay comandos Tauri que expongan el análisis (la UI aún no lo llama).
- **No** hay planner: convertir `Intent` → bloques de calendario + confirmación
  es la Fase 4 (`05-Roadmap.md`).
- **No** se muta la base de datos: los intents son puros; su materialización
  (store `tasks`) ocurre tras aprobación en la Fase 4.
- `parse_task_nl` sigue siendo la puerta de entrada de los comandos actuales;
  la Fase 4 los migrará a `parse_intent` con el mismo fallback local.
