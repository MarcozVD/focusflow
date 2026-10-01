# DESIGN.md — Sistema visual de FocusFlow

**Estado:** Documento vivo · **Última actualización:** 2026-10-01
**Fuente:** código real (`spike/frontend/src/app.css` + componentes `.svelte`). Ningún valor es inventado: todos se extrajeron del código en producción. El cambio de sistema de esta fecha es el del spec 19 (neumorfismo real + layout responsivo); antes era un sistema de sombras plano sobre superficies blancas.

---

## 1. Visual identity

FocusFlow utiliza **neumorfismo real** como lenguaje visual principal: un único gris como material y todo lo demás separado por relieve (sombra oscura + luz), nunca por color plano.

### 1.1 Qué es (y qué no es)

| Es | No es |
|----|-------|
| Profundidad por relieve en **superficies y controles** | Sombras gigantes u oscuras |
| Una sola luz, arriba-izquierda, en toda la app | Invertir la luz en un mismo panel |
| Superficie = fondo: el gris es el material | Blanco sobre blanco (la app se veía plana) |
| Press = hundirse | Press = cambiar de color o escalar |
| Contenido denso con relieve mínimo | Sombrear cada celda por igual |

**Principio vigente (spec 19 · D3, reemplaza la antigua regla D-01):** *el relieve está en las superficies **y** en los controles* —botones elevados que se hunden al pulsar, inputs y selects hundidos, nav activo hundido con hover elevado, pills y chips elevados—. **Excepción:** el contenido denso del calendario (bloques y chips) conserva su tinte de categoría con relieve mínimo de 2 px, porque ahí la sombra fuerte compite con la información. Lo que separa relieve de elevación es la jerarquía, no la ausencia de sombra en los controles.

La identidad en una frase: gris niebla con relieve de dos luces y un acento que solo aparece degradado donde hay acción.

---

## 2. La jerarquía de profundidad

### Base

`--bg` y `--surface` son **el mismo gris** (`#E9EDF2` en claro, `#262A32` en oscuro). No hay fondo "vacío" ni tarjetas blancas: el relieve lo hace todo (spec 19 · D2).

| Token | Light | Dark | Uso |
|-------|-------|------|-----|
| `--bg` / `--surface` | `#E9EDF2` | `#262A32` | Superficie única: ventana, tarjetas, calendario, sidebar |
| `--surface-2` | `#E3E8EF` | `#22252C` | Pozos: fila "todo el día", completadas, celdas del mes, statistics |
| `--surface-3` | `#DDE3EB` | `#1F2228` | Fondo de campos hundidos (`--input-bg`) |

Regla: la base **nunca** lleva sombra. Lo que flota, la lleva.

### Raised — tres niveles

El relieve se define con dos colores —`--neu-dark` (sombra) y `--neu-light` (luz)—, así que el tema cambia la geometría en **una sola pareja de tokens**.

| Token | Valor | Uso |
|-------|-------|-----|
| `--shadow-raised-sm` | `4px 4px 8px var(--neu-dark), -4px -4px 8px var(--neu-light)` | Controles elevados: botones, pills, nav hover, riel del interruptor |
| `--shadow-raised` | `8px 8px 16px var(--neu-dark), -8px -8px 16px var(--neu-light)` | Superficies: tarjetas de Ajustes/Sugerencias, panel del sidebar, widget, burbujas del asistente |
| `--shadow-raised-lg` | `12px 12px 28px rgba(150,165,190,0.7), -12px -12px 28px #fff` (claro) · los mismos offsets con `neu-*` (oscuro) | Piezas marcadas: tarjeta del calendario, paneles de modal, TaskDrawer |

- **Claro:** `--neu-dark: rgba(163, 177, 198, 0.6)` · `--neu-light: rgba(255, 255, 255, 0.9)`.
- **Oscuro:** `--neu-dark: rgba(0, 0, 0, 0.55)` · `--neu-light: rgba(255, 255, 255, 0.05)`. Misma geometría, luz casi nula.

### Inset — el pozo

| Token | Valor | Uso |
|-------|-------|-----|
| `--shadow-inset` | `inset 4px 4px 8px …, inset -4px -4px 8px …` | Cuadrícula horaria (`time-area`), fila "todo el día", completadas |
| `--shadow-inset-sm` | `inset 2px 2px 5px …, inset -2px -2px 5px …` | Campos pequeños, celdas del mes, nav activo, chips completados, estado pulsado |

En oscuro ambos derivan igual de `neu-*` (sin literales propios).

### Elevación (`--e1`–`--e3`) — solo lo que flota SIN overlay

Toasts, menús, banners y popups conservan la elevación clásica (`--e1` hover de tarjetas, `--e2` toasts, `--e3` popup del día y preview). **Los modales ya no usan elevación**: usan `--shadow-raised-lg` sobre el overlay claro desenfocado (§3.5), porque la luz blanca de `--e1`–`--e3` dejaba un halo sobre un overlay oscuro.

### Reglas del sistema de sombras (nunca romper)

1. **Una sola luz:** arriba-izquierda en todo. Prohibido invertir la luz en un mismo panel.
2. **Superficie = fondo** (D2). No reintroducir superficies blancas.
3. **Hover = un nivel más de relieve** (`--btn-shadow` → `--btn-shadow-hover`, de 4 a 6 px). Sin `translateY`.
4. **Press = hundirse** (`--btn-shadow-active`, que es `--shadow-inset-sm`). Es un evento táctil, no un cambio de color.
5. **Contenido denso = relieve mínimo** (2 px). Bloques y chips del calendario: el tinte de categoría manda, la sombra acompaña.
6. **Intensidad:** media (8 px, luz 90 %) por defecto; marcada (12 px, luz 100 %) solo en piezas destacadas.
7. **Con overlay:** panel con `--shadow-raised-lg` + `--overlay` claro desenfocado. Sin overlay: elevación (`--e1`–`--e3`).
8. `--shadow-reach: 48px` es el alcance máximo de la sombra marcada: ningún contenedor con scroll recorta relieve.

---

## 3. Color system

### 3.1 Light (default)

| Token | Valor | Uso |
|-------|-------|-----|
| `--bg` = `--surface` | `#E9EDF2` | Superficie única |
| `--surface-2` | `#E3E8EF` | Pozos, chips en reposo, nav hover |
| `--surface-3` | `#DDE3EB` | Inputs hundidos |
| `--accent` = `--primary` | `#2563eb` | Acción, selección, hoy, enlaces |
| `--primary-hover` | color-mix 88 % + `#000` (≈`#1D4ED8`) | Hover de primario |
| `--primary-active` | color-mix 76 % + `#000` (≈`#1E40AF`) | Press de primario |
| `--primary-soft` | color-mix 14 % accent + surface | Nav activo, fondos de selección |
| `--primary-soft-2` | color-mix 42 % accent + `#fff` | Anillo de `:focus-visible` |
| `--success` / `--success-bg` | `#059669` / `#D1FAE5` | Completado, éxito |
| `--warning` / `--warning-bg` | `#B45309` / `#FEF3C7` | Próximo, urgente, aviso |
| `--danger` / `--danger-bg` | `#DC2626` / `#FEE2E2` | Vencida, prioridad alta, borrar |
| `--study` / `--study-bg` | `#0D9488` / `#CCFBF1` | Sesiones de estudio (identidad propia, no es una categoría) |
| `--text-1` | `#1F2937` | Texto primario — **12.5:1** sobre `--surface` |
| `--text-2` | `#5B6472` | Texto secundario, horas, meta — **5.09:1** (AA) |
| `--text-3` | `#7C8594` | Placeholder, meta, overlines — **3.17:1** (nunca contenido esencial) |
| `--border` | `#D3DAE3` | Divisores finos |
| `--line-input` | `rgba(91, 100, 114, 0.22)` | Línea sutil de inputs y selects (D14) |
| `--overlay` | `color-mix(in srgb, var(--bg) 60%, transparent)` | Fondo de modales |

### 3.2 Dark (misma identidad, luz apagada)

| Token | Valor | Nota |
|-------|-------|------|
| `--bg` = `--surface` | `#262A32` | Gris oscuro, no negro puro |
| `--surface-2` | `#22252C` | Un paso más oscuro (el pozo) |
| `--surface-3` | `#1F2228` | El más oscuro (input) |
| `--accent` | `#3b82f6` | Aclarado por defecto |
| `--primary` | `color-mix(in srgb, var(--accent) 75%, #fff)` | **Derivado** del acento elegido (fix D7) |
| `--primary-hover` / `--primary-active` | color-mix 62 % / 90 % + `#fff` | Derivados igual |
| `--success` / `--warning` / `--danger` | `#34D399` / `#FBBF24` / `#F87171` | Semánticos aclarados (luminancia diseñada, no espejada) |
| `--study` | `#2DD4BF` | Teal claro |
| `--text-1` | `#F3F4F6` | **13.07:1** sobre `--surface` |
| `--text-2` | `#A6ADBB` | **6.38:1** (AA) |
| `--text-3` | `#6B7280` | **2.98:1** — solo meta |
| `--border` | `#333845` | |
| `--overlay` | `rgba(12, 14, 18, 0.55)` | Overlay oscuro, desenfocado |

> **No es una inversión de colores.** El relieve usa la misma fórmula en ambos temas; lo que cambia es la pareja `--neu-dark`/`--neu-light` y el gris del material. Los acentos de categoría siguen planos.

### 3.3 Acento: degradado, brillo y el arreglo del oscuro

| Token | Valor | Dónde |
|-------|-------|-------|
| `--grad-accent` | `linear-gradient(135deg, color-mix(in srgb, var(--accent) 78%, #fff), var(--accent))` | Botón principal, opción activa de los switchers, badge de Sugerencias, número de "hoy", píldora activa |
| `--glow-accent` | `0 6px 16px -4px color-mix(in srgb, var(--accent) 45%, transparent)` | Brillo del principal y del círculo de "hoy" |
| `--btn-primary-shadow` | `var(--shadow-raised-sm), var(--glow-accent)` | Sombra + brillo del botón principal |

- Los **6 acentos** funcionan porque todo se deriva con `color-mix` del acento elegido en Ajustes.
- **Fix del oscuro (D7):** `applyUiPrefs` fija `--accent` como estilo **inline**, así que en oscuro `--primary` tiene que **derivarse**: `color-mix(in srgb, var(--accent) 75%, #fff)`. Con `--primary: var(--accent)` el azul aclarado del tema quedaba pisado y botones y enlaces perdían contraste.
- «Añadir sesión» mantiene su identidad con un degradado propio derivado de `--study` (teal), no del acento.
- Los colores de categoría **no** degradan: el degradado es para acción y estado activo, no para identidad.

### 3.4 Uso del color por estado (semántica)

| Estado | Superficie | Indicador | Texto |
|--------|-----------|-----------|-------|
| Completada | `--surface-2` **hundida** (`--shadow-inset`; chips con `--shadow-inset-sm`) | ✓ (`TaskCheck` relleno `--text-3`, siempre visible) + borde izquierdo `--text-3` | Título tachado (izq→der) en `--text-2`, meta en `--text-3`. **Sin `opacity`** |
| Vencida | `--danger-bg` | Borde izquierdo **dashed** `--danger` | `--danger` |
| Prioridad alta | — | Punto/barra `--danger`, badge `--danger-bg` | `--danger` |
| Prioridad media | — | Badge `--primary-soft` | `--primary` |
| Hoy | — | Círculo `--grad-accent` con `--glow-accent` y número blanco; celda del mes con anillo de acento | — |
| En curso (widget) | — | Etiqueta "Ahora" `--primary` | — |
| Conflicto (drag) | toast `--danger` | — | blanco |

**Regla de oro:** el color nunca es el único indicador de estado. Vencida = fondo tintado + borde punteado + texto; completada = tachado + pozo gris + ✓; prioridad = badge + punto.

> Las completadas son **plenamente visibles** en el calendario (semana, día, mes y popup): hundidas en gris, nunca difuminadas con `opacity`. El título usa `--text-2` y no `--text-3` porque es contenido esencial (ver ACCESSIBILITY §5); `--text-3` queda para hora, descripción y punto de categoría.
>
> **Excepción multi-día:** una completada de varios días solo se ve en su día de inicio y en su día de fin.

### 3.5 Modales: overlay claro desenfocado (por qué)

| Token | Valor | Uso |
|-------|-------|-----|
| `--overlay` | ver §3.1/§3.2 | Fondo del overlay: gris translúcido **del propio tema** |
| `--overlay-blur` | `8px` | `backdrop-filter: blur()` (con prefijo `-webkit-`) |

Los paneles de modal (formularios de clase y sesión, conflictos, PlanProposal, diálogo de borrado, TaskDrawer) usan `--surface` + `--r-xl` + `--shadow-raised-lg`.

**Por qué overlay claro:** un overlay oscuro bajo una luz blanca fuerte produce el halo que obligaba antes a bajar los modales a `--e3`. Desenfocar el gris del tema mantiene el relieve sin halo; en oscuro el overlay es oscuro y la luz del relieve es casi imperceptible, así que el problema desaparece por construcción.

---

## 4. Typography

**Familia real:** `"Inter", "Segoe UI", system-ui, sans-serif` — Inter es parte de la identidad (licencia libre, excelente en 13–14 px, disponible en Windows). **No cambiar sin un estudio previo.**

### 4.1 Escala de tokens (`app.css`)

| Token | Tamaño | Uso |
|-------|--------|-----|
| `--fs-2xs` | 10 px | Overlines, hora mini, contadores pequeños |
| `--fs-xs` | 11 px | Meta, horas, chips, gutter |
| `--fs-sm` | 12 px | Secundario, labels, widget |
| `--fs-base` | 13 px | Cuerpo UI (base de la escala) |
| `--fs-md` | 14 px | Inputs, cuerpo destacado — **lo usa `body`** |
| `--fs-lg` | 16 px | Título de card/sección |
| `--fs-xl` | 20 px | Título de pantalla (TopBar) |
| `--fs-2xl` | 32 px | Hero de onboarding |

**Mapeo de literales a tokens** (al migrar): `9–10.5 → 2xs` · `11–11.5 → xs` · `12–12.5 → sm` · `13–13.5 → base` · `14–14.5 → md` · `15–18 → lg` · `20–27 → xl` · `38 → 2xl`. `body { font-size: var(--fs-md) }` mantiene los 14 px actuales.

### 4.2 Roles tipográficos reales

| Rol | Token | Peso | Track | Uso real |
|-----|-------|------|-------|----------|
| Pantalla (onboarding hero) | `--fs-2xl` | 700 | -0.03em | h1 de onboarding |
| Título de pantalla | `--fs-xl` | 700 | -0.02em | TopBar |
| Título de sección / card | `--fs-lg` | 600 | — | h2/h3 de sección (Ajustes, propuesta, drawer) |
| Cuerpo | `--fs-base`–`--fs-md` | 400–500 | — | Base de la app, inputs |
| Nombre de tarea | `--fs-xs`–`--fs-md` | 500–600 | — | EventBlock / TaskCard |
| Meta/horas | `--fs-2xs`–`--fs-sm` | 600 | — | `tabular-nums` SIEMPRE |
| Overline | `--fs-2xs` | 600 | +0.06–0.1em, uppercase | Etiquetas de sección, días |
| Hora del calendario | `--fs-xs` | 600 | — | gutter, tabular-nums |
| Placeholder | `--fs-base` | 400 | — | `--text-3` |

> **Migración completa (spec 18):** la tabla describe el sistema real. Ningún componente de `spike/frontend/src/lib` ni `App.svelte` conserva un `font-size` literal.

### 4.3 Regla de pesos

400 / 500 / 600 son la escala normal. **700 solo en el título de pantalla y el hero de onboarding**; el peso 800 ya no existe en el código. Los overlines van en 600 manteniendo uppercase + tracking.

### 4.4 Reglas tipográficas

1. **Números y horas siempre `font-variant-numeric: tabular-nums`** (calendario, contadores, widget).
2. Jerarquía por tamaño + peso + tinte; el título nunca en gris (`--text-1`).
3. Títulos cortos: 1 línea con ellipsis; descripciones máx. 2–3 líneas con `-webkit-line-clamp`.
4. `text-wrap: balance` en titulares largos (onboarding).
5. Texto nunca se difumina: color plano, peso ≥ 400.
6. **Ningún `font-size` literal:** todo sale de `--fs-*`.
7. **Capitalización en español: solo la primera letra.** Nada de `text-transform: capitalize` (daría «Sesiones De Estudio»). La inicial la aplica el helper compartido `capitalizeFirst` (`dateUtils.ts`) — lo usan la TopBar y el encabezado del popup del día.

---

## 5. Spacing

Escala real de tokens (grid de 4 px + micro-gaps):

```css
--s-0_5: 2px  --s-1: 4px  --s-1_5: 6px  --s-2: 8px  --s-3: 12px --s-4: 16px --s-5: 20px
--s-6: 24px --s-8: 32px --s-10: 40px --s-12: 48px --s-16: 64px
```

**Regla de aire:** en **contenedores** (cards, paneles, drawer, sidebar, topbar, popup) el padding y el gap suben **un escalón** respecto al contenido que alojan. En **contenido denso** (EventBlock, chips, minichips, celdas del mes) no se sube nada.

Los hairlines de `1px`/`1.5px` en bordes y outlines quedan literales; no son espaciado.

**Convenciones reales:**
- Padding de cards: `--s-5`/`--s-6` en contenedores grandes, `--s-4`/`--s-5` en medianos.
- Gap entre tarjetas: `--s-4`–`--s-6` (16–24 px). Nunca apilar superficies sin aire.
- Gap interno de filas: `--s-1_5`–`--s-2` (6–8 px).
- Altura mínima de interacción: 44 px (QuickAdd 44, botones 36–44, botones de icono 40 px).
- **Sidebar:** panel de 232 px con margen `--s-4` (72 px en modo iconos, padding incluido).
- **Columna de contenido:** `padding: 0 clamp(--s-10, 3vw, --s-16) clamp(--s-6, 2.5vw, --s-12)`; el colchón lateral cubre `--shadow-reach` (48 px) para que el relieve no se recorte al hacer scroll.
- **Panel del TaskDrawer:** margen `--s-4` en los cuatro lados.

---

## 6. Radius

```css
--r-xs: 6px   --r-sm: 10px  --r-md: 16px  --r-lg: 22px  --r-xl: 28px  --r-full: 999px
```

| Token | Uso real |
|-------|----------|
| `--r-xs` (6 px) | Elementos pequeños y densos: `.prio-dot`, `kbd`, insignias muy pequeñas |
| `--r-sm` (10 px) | EventBlocks y celdas del mes (contenido denso) |
| `--r-md` (16 px) | Celdas del mes, `day-head`, campos con etiqueta larga, notas |
| `--r-lg` (22 px) | Fichas hundidas de estadística, ficha auxiliar del modal |
| `--r-xl` (28 px) | **Tarjetas y paneles:** calendario, sidebar, Ajustes, Sugerencias, PlanProposal, Login, Onboarding, widget, modales, TaskDrawer |
| `--r-full` | **Píldoras:** botones, inputs, selects, chips, pills, switchers, badges, interruptores, botones de diálogo |
| `50%` | Botones de icono y botones circulares (flechas, cerrar, añadir, tema) |

**Mapeo de literales:** `≤ 9px → --r-xs` · `10–12px → --r-sm` · `14–18px → --r-md` · `22–24px → --r-lg` · `28px → --r-xl` · `999px → --r-full`. `50%` se queda literal.

**Reglas de forma (spec 19 · D4):**
1. **Todo control es píldora:** botones, inputs, selects, textareas, chips y pills usan `--r-full`.
2. **Todo contenedor es `--r-xl`:** tarjetas, paneles y modales.
3. **Botón de icono = círculo** (`border-radius: 50%`).
4. **La excepción es el contenido denso:** bloques y celdas del calendario se quedan en `--r-sm`/`--r-md` para no regalar espacio.
5. Sin esquinas rectas en ninguna parte.

---

## 7. Motion

### 7.1 Tokens

```css
--dur-fast: 150ms   --dur-base: 200ms   --dur-slow: 250ms
--ease-out: cubic-bezier(0.22, 1, 0.36, 1)
--ease-spring: cubic-bezier(0.34, 1.56, 0.64, 1)
```

### 7.2 Mapa de animaciones reales

| Interacción | Duración | Curva | Detalle real |
|-------------|----------|-------|--------------|
| Hover de botón/pill | 150 | ease-out | `--btn-shadow` → `--btn-shadow-hover` (sin `translateY`) |
| Press botón | 150 | ease-out | `--btn-shadow-active` (inset) o `brightness` en degradados |
| Ancho de la sidebar (modo iconos) | 200 | ease-out | transición de `width` y `padding` (`--dur-base`) |
| Cambio de vista calendario | 160 | fade | `transition:fade` de Svelte, `{#key view}` |
| Apertura drawer | 200 | slide x | `transition:slide` |
| Modal / overlay | 120–160 | fade | overlay + pop |
| Completar tarea | 250 | ease-out | hundir + tachar (`--dur-slow`); el ✓ del `TaskCheck` hace scale |
| Toast contextual | 250 | ease-out | rise |
| Preview QuickAdd | 200 | cubic ease | `transition:scale` |
| Mensajes del asistente | 160 | fade | entrada de mensajes AI |
| Transición de tema | 200 | ease-out | background/color del body |
| Ghost de drag | instantáneo | — | sigue al cursor, `will-change: top, height` |
| Scrollbar hover | 150 | ease-out | thumb se oscurece |

### 7.3 Reglas de motion

1. **La animación comunica estado o continuidad.**
2. Solo `transform`/`opacity`/`box-shadow` en transiciones CSS (nunca animar `width`/`height` de forma masiva; la sidebar anima su `width`, excepción consciente y corta).
3. Sin rebotes en elementos grandes (widget, modal): solo micro-elementos.
4. **`prefers-reduced-motion` respetado globalmente** en `app.css`: toda animación/transición baja a 120 ms; el onboarding desactiva GSAP por completo si el usuario lo pide.
5. Sin animación "porque se ve bonita". El único uso de GSAP es la entrada escalonada del onboarding.

---

## 8. Iconografía

- **Estilo:** SVG inline, stroke 1.4–2.5 (típicamente 2), `fill="none"`, `stroke-linecap/linejoin round`.
- **Tamaños:** 18 px sidebar, 16 px controles, 11–12 px dentro de botones pequeños, 20–22 px en dialogs.
- **Color:** hereda `currentColor`; reposo `--text-2`/`--text-3` → hover `--text-1` o `--primary`.
- **Categorías:** iconos Lucide-style (graduation-cap, briefcase, user, heart-pulse, wallet, sparkles).
- **Botones de icono:** circulares de 40 px (`border-radius: 50%`) con `--btn-shadow`; en la sidebar en modo iconos son el único elemento visible del nav y llevan `title` + `aria-label`.
- Prohibido: iconos rellenos, con gradiente, o animados (salvo el check del checkbox).

---

## 9. Layout, scroll y ventanas

### 9.1 Estructura

```
.app (100vh, column)
└── .body (row, flex: 1)
    ├── Sidebar            (panel con margen)
    └── main.content       ← único scroller
        └── .content-inner (columna centrada, max-width)
            ├── TopBar
            └── vista
```

- **Scroll unificado (D9):** scrollea la columna de contenido **entera**; la TopBar sube con el contenido y nada pasa "por debajo" de una barra. Antes cada vista tenía su propio scroller con `padding-top: 0` y el contenido se cortaba en una línea invisible. Excepción: la conversación del Asistente conserva su scroll interno con el input fijo abajo y un fundido superior de 24 px.
- **Reset al cambiar de vista:** un `$effect` sobre `view` y `hmode` pone `scrollTop = 0` en `.content`. Sin él, el desplazamiento de la vista anterior se heredaba al cambiar de vista o de submodo (horario/sesiones). Las flechas de fecha dentro de la misma vista no lo tocan.
- **Anchos máximos (D10):** contenido ≤ **1680 px** centrado; vistas de lectura (Ajustes, Sugerencias, Asistente) ≤ **880 px** centradas (`class:reading`). Sin scrollbars horizontales a 960, 1440 y 2560 px.

### 9.2 Calendario que llena el alto

- La cadena flex se propaga con `.cal-wrap { flex: 1 0 auto; min-height: 0 }` y `.view-fill`, para que el wrapper de la transición `{#key}` no la rompa.
- La grilla del mes usa `grid-template-rows: repeat(6, minmax(80px, 1fr))` + `flex: 1`: las filas **se estiran** para llenar la tarjeta (a 2560×1440 `.cal` medía 605 px y dejaba ~170 px vacíos) y, si la ventana no alcanza, el overflow cae en `.content` junto con la TopBar.
- **`ResizeObserver` del calendario:** mide en `requestAnimationFrame` y **solo asigna `timeAreaH` si el valor cambió**, cancelando el frame en el cleanup. La asignación síncrona encadenaba otra medida y disparaba el aviso `ResizeObserver loop completed with undelivered notifications`; `App.svelte` además lo filtra del manejador de errores fatales (`console.debug`), porque es benigno.

### 9.3 Barra lateral

- **Panel:** margen `--s-4` a izquierda/alto/bajo, `--r-xl`, `--shadow-raised`, `var(--surface)`. Ancho 232 px. Su zona scrolleable (`.side-scroll`) lleva fundido de 16 px arriba y abajo con `mask-image` y padding extra inferior (`--s-5`) para que el último elemento no toque el borde redondeado.
- **Modo iconos (D11/D12):** automático por debajo de **1200 px** de ancho (`matchMedia("(max-width: 1199px)")`); botón manual que guarda la preferencia en `localStorage` (`ff.sidebar` → `auto | collapsed | expanded`) y **vuelve a `auto`** cuando la elección coincide con la automática. En modo iconos (72 px, padding incluido): tooltips + `aria-label` en cada icono, contador de Sugerencias como badge sobre su icono, botones de añadir circulares, botón de tema y de expandir circulares, y se ocultan categorías y caja de "hoy".
- La caja de "hoy" es un **pozo** (`--surface-2` + `--shadow-inset-sm`).

### 9.4 Ventana

- **Mínimo 960×640** (`tauri.conf.json`), antes 800×600: a 1024×640 la sidebar de 232 px se comía un cuarto del ancho, "Añadir sesión de estudio" partía en dos líneas y los 7 días quedan de ~90 px.

---

## 10. Patrones repetidos

### 10.1 Lo que se repite de forma consistente

- **Botón primario:** `--grad-accent` + `--btn-primary-shadow` + `#fff`, píldora; hover `filter: brightness(1.05)`; active `--btn-shadow-active`.
- **Botón secundario/pill:** `--surface` + `--btn-shadow`; hover `--btn-shadow-hover`; active hundido.
- **Botón de icono:** 40 px circular con `--btn-shadow`.
- **Botón peligroso:** texto `--danger` sobre la superficie en relieve (dejó de pintarse en rojo sólido).
- **Chip de categoría:** texto `color-mix(var(--c) 60%, var(--text-1))` sobre `color-mix(var(--c) 13%, var(--surface))`, `--r-full` y relieve mínimo de 2 px.
- **Input:** `--input-bg` + `--input-shadow` + `--input-border` en `--r-full` (los textarea en `--r-lg`); el foco **solo cambia el borde** a `--primary` y el anillo lo da el `:focus-visible` global.
- **Interruptor:** riel 40×22 hundido (`--shadow-inset-sm`) con knob circular elevado (`--btn-shadow`); activo con `--grad-accent`.
- **Tarjetas:** `--surface` + `--r-xl` + `--shadow-raised`; calendario, modales y TaskDrawer usan `--shadow-raised-lg`.

### 10.2 Deuda visual (registrar, no "arreglar" sin plan)

1. **`.btn` duplicado con variantes locales:** cada componente redefine su `.btn` (padding 8/16 vs 9/14 vs 7/12). No existe un `Button.svelte` global — **propuesto**. Los tokens ya convergen el aspecto.
2. **Drag toast vs toast contextual:** tres sistemas de toast distintos (`.drag-toast` en Calendar, `.toast` en ContextualToast, `.toast` en QuickAdd).
3. **`.ghost` (sin fondo)** y **`.danger`** como variantes solo en algunos componentes.
4. **Widget con relieve a propósito:** conserva `--shadow-raised` (no elevación) porque es la tarjeta de su propia ventana sobre el escritorio, no una capa sobre un overlay; además lleva `margin: var(--s-4)` para que la sombra no se recorte contra el borde.

### 10.3 Resuelto en el spec 19

- **Foco:** `TaskDrawer` ya no usa anillo propio; todo el repo usa el `:focus-visible` global.
- **Superficie = fondo:** ya no hay superficies blancas.
- **Radios literales:** todo sale de `--r-*` (o `50%` en círculos).
- **Pesos:** 700 solo en título de pantalla y hero.

---

## 11. Checklist de identidad (aplicar siempre)

1. ¿Un solo acento, y solo degradado donde hay acción o estado activo?
2. ¿La luz viene de arriba-izquierda en TODAS las superficies?
3. ¿Superficie = fondo (nunca blanco sobre blanco)?
4. ¿Hover = un nivel más de relieve y press = hundido?
5. ¿Botones/inputs/chips en píldora, tarjetas en `--r-xl`, iconos circulares?
6. ¿Contenido denso con relieve mínimo de 2 px?
7. ¿Overlay claro desenfocado bajo los modales (nunca halo)?
8. ¿Números tabulares, overlines uppercase con tracking?
9. ¿Animaciones 120–250 ms con reduced-motion respetado?
10. ¿Dark = misma identidad con luz apagada (no inversión)?
11. ¿El estado nunca depende solo del color?
