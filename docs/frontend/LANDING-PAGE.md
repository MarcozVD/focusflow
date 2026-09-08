# LANDING-PAGE.md — Dirección creativa de la landing oficial

> La landing NO es una plantilla SaaS. Es **FocusFlow convertido en una experiencia de marca**: mismo lenguaje visual, misma profundidad, mismo producto real como protagonista.

---

## 1. Mensaje central

> **FocusFlow helps you understand your time and decide what to do with it.**

No se vende "IA" como feature principal. Se vende:

**clarity + time + intelligent planning**

La IA es el motor invisible que hace posible la claridad; el producto (calendario, tareas, widget) es el héroe visible.

---

## 2. Audiencia y tono

- Usuario técnico-cercano que valora **local-first, rápido y predecible**.
- Escéptico de "IA que hace cosas sola" → la landing debe mostrar el **flujo de aprobación** (propone → revisa → acepta) desde el primer vistazo.
- Tono: sobrio, tangible, confiado. Nada de hype, nada de miedo, nada de promesas que el producto no demuestra.

---

## 3. Narrativa de secciones (orden y lógica)

| # | Sección | Por qué | Composición |
|---|---------|---------|-------------|
| 1 | **Navigation** | Anclaje mínimo: logo + Product + How it works + AI scheduling + CTA | Barra plana sobre base, sin card |
| 2 | **Hero** | La primera impresión ES el producto real | Centrada: headline + producto interactivo debajo |
| 3 | **Problem** | Fragmentación → un solo lugar | Editorial: columnas separadas convergiendo en FocusFlow |
| 4 | **AI scheduling** | La feature estrella como flujo, no como chatbot | Vertical: pipeline Tasks → … → Organized day |
| 5 | **Calendar** | El tiempo como sistema espacial | Full-width: semana real con libre/ocupado/urgente |
| 6 | **Quick capture** | Lenguaje natural → estructura | Asimétrica: input + conversión visual en Task/Fecha/Hora |
| 7 | **Email intelligence** | Correo → contexto → calendario | Horizontal: email → análisis → sugerencia → aprobación |
| 8 | **Widget** | open → glance → act | Floating: widget sobre fondo, acciones rápidas |
| 9 | **Local-first** | Desktop, rápido, privado por diseño | Editorial sobria, sin miedo |
| 10 | **Final CTA** | "Spend less time organizing your time." | Centrada, emocional pero sobria |

**Regla de composición:** cada sección usa un layout distinto. Prohibido el patrón "heading izquierda + card derecha" repetido.

---

## 4. Reglas anti-genérico-AI

**Prohibido:**
- ❌ Gradientes púrpura/azul
- ❌ Glassmorphism
- ❌ Gradient text
- ❌ Glow effects / sparkles
- ❌ Blobs flotantes
- ❌ Cards redondeadas infinitas / icon cards genéricas
- ❌ Badges y pills en exceso
- ❌ Headings gigantes vacíos
- ❌ Fake dashboard screenshots / mockups de laptop

**Permitido (es la identidad):**
- ✅ Espacio plano + profundidad neumórfica sutil
- ✅ Un solo azul de acción + semánticos sobrios
- ✅ Inter, jerarquía por tamaño/peso/tracking
- ✅ El producto real (calendario, tareas, widget) como protagonista
- ✅ Animación con propósito (producto entrando, tarea→evento, sugerencia apareciendo)

---

## 5. Regla de neumorfismo en la landing

**DO NOT PUT A NEUMORPHIC CARD AROUND EVERYTHING.**

- La base de la landing es plana (`--bg`).
- El relieve se reserva para: el "app frame" del hero, el widget, cards de pipeline concretas.
- Las secciones de texto son **planas con jerarquía tipográfica** (la profundidad no sustituye al contenido).
- La combinación **flat space + subtle depth** es lo que hace que la landing se sienta de la misma familia que la app.

---

## 6. Sistema visual heredado (mismos tokens)

| Token | Valor light | Uso en landing |
|-------|-------------|----------------|
| `--bg` | `#F8F8F8` | Fondo base |
| `--surface` | `#FFFFFF` | App frames, widget, cards clave |
| `--surface-2` | `#F1F2F4` | Zonas hundidas, hover |
| `--surface-3` | `#EAECEF` | Inputs, slots |
| `--primary` | `#2563EB` | Acción, hoy, enlaces |
| `--primary-soft` | `#DBEAFE` | Estados activos |
| `--success` | `#059669` | Completado |
| `--warning` | `#B45309` | Urgencia |
| `--danger` | `#DC2626` | Vencido |
| `--text-1/2/3` | `#1F2937 / #6B7280 / #9CA3AF` | Jerarquía de texto |
| `--border` | `#E7E9EC` | Divisores |
| Sombras | raised / inset / e1-e3 | Sistema de profundidad |
| Motion | 150/200/250 ms, ease-out, spring | Micro-interacciones |
| Radius | 10/16/22/28 | Jerarquía de forma |
| Font | Inter | Identidad |

**Acentos de categoría** (Universidad azul, Trabajo violeta, Personal rosa, Finanzas ámbar, Salud esmeralda, Otros cielo) para colorear las tareas del hero — así la landing luce "con datos reales" sin ser un fake dashboard.

---

## 7. El hero: el producto real como protagonista

**NO** usar stock image, laptop mockup, dashboard falso ni ilustración abstracta.

La hero debe ser una **composición del frontend real de FocusFlow** construida en HTML/CSS (como la app): un frame de ventana con:

- **TopBar** (título de semana + arrows + "Hoy")
- **WeekView** con:
  - gutter de horas (6a–10p, tabular-nums)
  - columnas de días (Lun…Dom) con la línea "ahora"
  - **tareas reales** con colores de categoría, prioridad (punto danger), duración
  - **tiempo libre visible** (slots vacíos)
  - una **sugerencia de IA** ("Sugerencia: mover 'Estudiar cálculo' a las 4pm — 2 h libres") con botón Aceptar/Editar
  - un evento en **drag/ghost** o **resize handle** para mostrar interactividad
- **QuickAdd** con texto natural y chips detectados

La persona debe entender el producto **solo observándolo**: "ahí veo mi semana, mis tareas, mis horas libres, y una IA que me sugiere dónde meterlas".

---

## 8. Sección AI scheduling: inteligencia contextual, no chatbot

Pipeline visual (vertical, con flechas de flujo):

```
Tasks
   ↓
Existing commitments
   ↓
Available time
   ↓
AI suggestion        ← card con explicación ("Entendí: Tarea de 4h, vence viernes")
   ↓
User review          ← botones Aceptar / Editar / Descartar
   ↓
Organized day
```

Claves:
- La IA **explica** (muestra su razonamiento en texto), no solo propone.
- El **usuario aprueba** — esto es el corazón del producto y debe leerse sin leer.
- Nada de burbujas de chat; es un flujo de decisión.

---

## 9. Sección Calendar: el tiempo como sistema

Full-width: un week-view grande que enfatiza:

- **free blocks** (espacio vacío visible)
- **busy blocks** (color de categoría)
- **duration** (tamaño = tiempo)
- **priorities** (punto/barra danger)
- **urgency** (badge "Vence hoy", dashed para vencidas)
- **upcoming events** (próximos días)

El mensaje visual: *ocupado vs libre se entiende en milisegundos.*

---

## 10. Sección Quick capture

```
"Finish database report tomorrow at 4 PM"
        ↓
   [Task]  Database report     Fecha: Tomorrow     Hora: 4:00 PM
```

Mostrar que FocusFlow **entiende lenguaje natural**: una línea de texto se convierte en una tarea estructurada con fecha y hora. Incluir el fallback local como señal de confianza ("si la IA tarda, la interpretación local responde al instante").

---

## 11. Sección Email intelligence

```
Email  →  Context understanding  →  Relevant activity  →  Calendar
```

- Mostrar una sugerencia real con **confianza %**, **razón** ("El correo menciona 'examen parcial' y una fecha"), y acciones (Aceptar / Editar / Fusionar).
- **No afirmar** que cualquier email se convierte automáticamente en evento: la revisión humana es parte del flujo (salvo remitentes de confianza).

---

## 12. Sección Widget

```
open → glance → act
```

Composición: el widget (transparente, con las secciones Ahora/Siguiente/Importante, dot de categoría, acciones ✓ ⟳ ▶) **flotando** sobre el escritorio. Un clic → abre la tarea en la app.

No mostrar una mini-app gigante: es una pieza pequeña que se lee en 3 segundos.

---

## 13. Sección Local-first

Mensajes honestos y sobrios:

- **Desktop-first**: una app de Windows que arranca rápido.
- **Local-first**: tus datos viven en tu equipo (SQLite local).
- **User-controlled integrations**: tú eliges qué correo conectar y qué aceptar.
- **Predictable behavior**: la priorización es determinista.

**Sin miedo, sin afirmaciones de seguridad no demostrables.** (La app cifra credenciales en el Administrador de credenciales de Windows y solo envía contexto mínimo a la IA — se puede decir eso, no más.)

---

## 14. Performance y motion

- CSS transforms/opacity en todas las animaciones.
- Lazy rendering de secciones fuera de viewport (IntersectionObserver) — sin librerías pesadas.
- Sin WebGL, sin video gigante, sin parallax excesivo, sin partículas.
- La landing debe sentirse instantánea: 100% estático, sin framework runtime, un solo CSS.
- `prefers-reduced-motion` respetado (misma regla que la app).

---

## 15. Responsive

- **Desktop/laptop**: composiciones completas, profundidad neumórfica.
- **Tablet**: las composiciones de 2 columnas colapsan a 1, se reduce spacing.
- **Mobile**: 
  - el week-view del hero se simplifica (o se muestra como lista de tareas),
  - profundidad reducida (sombras más pequeñas),
  - navegación mínima (solo CTA),
  - CTA siempre visible,
  - legibilidad protegida (≥ 14 px).

---

## 16. Prueba anti-genérica

> "¿Podría cambiar el logo por el de otra startup SaaS y nadie notaría la diferencia?"

Si la respuesta es sí → **rehacer**. La prueba pasa cuando:

1. El héroe es el producto (week-view real, no un mockup genérico).
2. El color viene de las categorías y el azul de la app, no de un gradiente.
3. El neumorfismo está usado como sistema (base/raised/inset/floating), no como decoración.
4. El texto es específico de FocusFlow (QuickAdd, widget, sugerencias de correo, determinismo).
5. La composición varía por sección (nada de "heading + card" repetido).
