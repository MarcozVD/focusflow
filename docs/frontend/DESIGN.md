# DESIGN.md — Sistema visual de FocusFlow

**Estado:** Documento vivo · **Última actualización:** 2026-08-31
**Fuente:** código real (`spike/frontend/src/app.css` + componentes `.svelte`). Ningún valor es inventado: todos se extrajeron del código en producción.

---

## 1. Visual identity

FocusFlow utiliza **neumorfismo refinado/moderno** como lenguaje visual principal.

### 1.1 Qué es (y qué no es)

| Es | No es |
|----|-------|
| Un **sistema de jerarquía visual** (base / raised / inset / floating) | Neumorfismo exagerado tipo Dribbble 2020 |
| Profundidad **sutil y controlada**, con zonas planas | Soft UI genérico lleno de sombras por todas partes |
| Luz coherente desde arriba-izquierda en todas las superficies | Interfaces completamente redondeadas |
| Tactilidad física en micro-interacciones (press = hundirse) | Sombras gigantes u oscuras |

**Principio clave (del spec original, D-01):** *el relieve es de las superficies, no de los controles*. Si todo tiene relieve, nada es interactivo. La combinación **espacio plano + profundidad sutil** es lo que da el carácter.

La identidad en una frase: superficies blancas cálidas que flotan sobre una luz de mañana, un solo azul como voz del producto, y micro-movimientos que hacen que cada acción se sienta física pero suave.

---

## 2. La jerarquía de profundidad (Base / Raised / Inset / Floating)

### Base
La superficie principal. **Plana, sin sombras.** Es el fondo sobre el que viven las demás capas.

| Token real | Light | Dark |
|-----------|-------|------|
| `--bg` | `#F8F8F8` | `#16181E` |

- Se usa en: fondo de ventana, fondo del área de contenido, login/onboarding.
- Regla: la base **nunca** lleva sombra ni relieve.

### Raised
Elementos ligeramente elevados sobre la base: tarjetas, calendario, agenda, paneles de ajustes, widget, mensajes del asistente.

```css
--shadow-raised: inset 0 1px 0 rgba(255,255,255,0.85),  /* filo de luz superior */
                 -6px -6px 14px rgba(255,255,255,0.95), /* luz arriba-izq */
                  6px  6px 14px rgba(31,41,55,0.08);    /* sombra abajo-der */
--shadow-raised-lg: inset 0 1px 0 rgba(255,255,255,0.9),
                    -12px -12px 24px rgba(255,255,255,0.9),
                     12px  12px 24px rgba(31,41,55,0.10);
```

**Dark mode** — misma geometría, luz apagada:
```css
--shadow-raised: inset 0 1px 0 rgba(255,255,255,0.04),
                 -6px -6px 14px rgba(0,0,0,0.3),
                  6px  6px 14px rgba(0,0,0,0.55);
--shadow-raised-lg: inset 0 1px 0 rgba(255,255,255,0.04),
                    -12px -12px 24px rgba(0,0,0,0.35),
                     12px  12px 24px rgba(0,0,0,0.6);
```

### Inset
Elementos hundidos: inputs, QuickAdd, campos de búsqueda, filtros, slots del calendario, zonas de drop.

```css
--shadow-inset: inset 4px 4px 10px rgba(31,41,55,0.06),     /* sombra adentro */
                inset -4px -4px 10px rgba(255,255,255,0.85); /* luz adentro */
--shadow-inset-sm: inset 2px 2px 5px rgba(31,41,55,0.05),
                   inset -2px -2px 5px rgba(255,255,255,0.8);
```

**Dark:**
```css
--shadow-inset: inset 4px 4px 10px rgba(0,0,0,0.4),
                inset -4px -4px 10px rgba(255,255,255,0.03);
--shadow-inset-sm: inset 2px 2px 5px rgba(0,0,0,0.35),
                   inset -2px -2px 5px rgba(255,255,255,0.03);
```

### Floating
Elementos temporales que sobrevuelan: modales, popovers, menús, previsualización del QuickAdd, widget, toasts, drawers.

| Nivel | Token | Uso |
|-------|-------|-----|
| e1 | `0 4px 8px -2px rgba(31,41,55,0.08), 0 2px 4px -2px rgba(31,41,55,0.06)` | Hover de cards, botones, kbd |
| e2 | `0 10px 20px -4px rgba(31,41,55,0.12), 0 4px 8px -4px rgba(31,41,55,0.08)` | Toasts, slow-AI banner |
| e3 | `0 18px 36px -6px rgba(31,41,55,0.18), 0 8px 16px -8px rgba(31,41,55,0.12)` | Modales, popup de día, drawer, preview QuickAdd |

Dark: mismas estructuras con `rgba(0,0,0,0.5–0.6)`.

**Reglas del sistema de sombras (nunca romper):**
1. Contraste bajo siempre: la sombra más fuerte es `rgba(31,41,55,0.18)` en light.
2. Blur grande y difuso: mínimo 14 px para relieve, 20+ para elevación.
3. Un solo origen de luz: arriba-izquierda. Prohibido invertir la luz en un mismo panel.
4. Hover = subir un nivel (raised → e1/e2, translateY(-1px)).
5. Active/pressed = **hundirse** (inset o scale 0.98). El press es un evento táctil, no un cambio de color.

---

## 3. Color system

### 3.1 Light mode (default)

| Token | Valor | Uso |
|-------|-------|-----|
| `--bg` | `#F8F8F8` | Base de la ventana |
| `--surface` | `#FFFFFF` | Raised: tarjetas, calendario, drawer, widget |
| `--surface-2` | `#F1F2F4` | Hover de nav, slots, chips en reposo, pop-items |
| `--surface-3` | `#EAECEF` | Inset: inputs, QuickAdd, botones secundarios |
| `--accent` / `--primary` | `#2563EB` | Acción, selección, hoy, enlace, botón principal |
| `--primary-hover` | color-mix 88% + #000 (`≈#1D4ED8`) | Hover de primario |
| `--primary-active` | color-mix 76% + #000 (`≈#1E40AF`) | Press de primario |
| `--primary-soft` | color-mix 14% accent + surface (`≈#DBEAFE`) | Nav activo, chips de estado, fondos de selección |
| `--primary-soft-2` | color-mix 42% accent + #fff (`≈#BFDBFE`) | Anillo de foco, borde de hoy |
| `--success` | `#059669` | Completado, éxito |
| `--success-bg` | `#D1FAE5` | Fondo de completado, badges éxito |
| `--warning` | `#B45309` | Próximo/urgente/aviso |
| `--warning-bg` | `#FEF3C7` | Fondo de avisos |
| `--danger` | `#DC2626` | Vencida, prioridad alta, borrar |
| `--danger-bg` | `#FEE2E2` | Fondo de vencida/error |
| `--text-1` | `#1F2937` | Texto primario |
| `--text-2` | `#6B7280` | Texto secundario, horas, meta |
| `--text-3` | `#9CA3AF` | Placeholder, deshabilitado, overlines |
| `--border` | `#E7E9EC` | Divisores finos, bordes de inputs |

### 3.2 Dark mode (misma identidad, luz apagada)

| Token | Valor | Nota de luminancia |
|-------|-------|--------------------|
| `--bg` | `#16181E` | Base oscura cálida (no negro puro) |
| `--surface` | `#1E2129` | Ligeramente más claro que base |
| `--surface-2` | `#262A34` | Un paso más |
| `--surface-3` | `#2C313C` | El más claro de las superficies (inset) |
| `--accent` | `#3B82F6` | **Aclarado** para mantener contraste AA |
| `--primary-soft` | color-mix 20% accent + transparent | Tinte translúcido (no blanco) |
| `--success` | `#34D399` | Semánticos aclarados |
| `--warning` | `#FBBF24` | |
| `--danger` | `#F87171` | |
| `--text-1` | `#F3F4F6` | |
| `--text-2` | `#A6ADBB` | 4.6:1 sobre surface (AA) |
| `--text-3` | `#6B7280` | Solo para meta/placeholder |
| `--border` | `#333845` | |

> **No es una inversión de colores.** Light usa sombras con luz blanca al 85–95 %; dark usa luces al 4 % (casi imperceptibles) y sombras negras profundas. Los semánticos se aclaran en dark (luminancia diseñada, no espejada). El azul es el ancla de identidad en ambos temas.

### 3.3 Acentos de categoría (fijos en código)

```ts
Universidad #2563EB · Trabajo #7C3AED · Personal #EC4899
Finanzas    #F59E0B · Salud    #10B981 · Otros    #0EA5E9
```

Acento configurable (Ajustes → Apariencia): `#2563EB, #7C3AED, #EC4899, #F59E0B, #10B981, #0EA5E9`. Se aplica a `--accent` vía CSS custom property y se difunde app + widget.

### 3.4 Uso del color por estado (semántica)

| Estado | Superficie | Indicador | Texto |
|--------|-----------|-----------|-------|
| Completada | — (opacity 0.5–0.55) | — | Título tachado, `--text-2` |
| Vencida | `--danger-bg` | Borde izquierdo **dashed** `--danger` | `--danger` |
| Prioridad alta | — | Punto/barra `--danger`, badge `--danger-bg` | `--danger` |
| Prioridad media | — | Badge `--primary-soft` | `--primary` |
| Hoy | — | Círculo `--primary` con número blanco | — |
| En curso (widget) | — | Etiqueta "Ahora" `--primary` | — |
| Conflicto (drag) | toast `--danger` | — | blanco |

**Regla de oro:** el color nunca es el único indicador de estado. Vencida = fondo tintado + borde punteado + texto; completada = tachado + opacidad; prioridad = badge + punto.

---

## 4. Typography

**Familia real:** `"Inter", "Segoe UI", system-ui, sans-serif` — Inter es parte de la identidad (licencia libre, excelente en 13–14 px, disponible en Windows). **No cambiar sin un estudio previo.**

### 4.1 Escala real usada en el código

| Rol | Tamaño | Peso | Track | Uso real |
|-----|--------|------|-------|----------|
| Pantalla (onboarding hero) | 38 px | 800 | -0.03em | h1 de onboarding |
| Título de pantalla | 20–22 px | 700 | -0.02em | TopBar, encabezados de página |
| Título de sección | 17 px | 700 | — | h2 en Ajustes/Sugerencias |
| Título de card | 15 px | 700 | — | h3 propuestas, drawer |
| Cuerpo | 13–14 px | 400–500 | — | Base de la app |
| Nombre de tarea | 11.5–14 px | 500–600 | — | EventBlock / TaskCard |
| Meta/horas | 10–12.5 px | 600 | — | `tabular-nums` SIEMPRE |
| Overline | 9.5–11 px | 700 | +0.06–0.1em, uppercase | Etiquetas de sección, días |
| Hora del calendario | 11 px | 600 | — | gutter, tabular-nums |
| Placeholder | 13–13.5 px | 400 | — | `--text-3` |

### 4.2 Reglas tipográficas

1. **Números y horas siempre `font-variant-numeric: tabular-nums`** (calendario, contadores, widget).
2. Jerarquía por tamaño + peso + tinte; el título nunca en gris (`--text-1`).
3. Títulos cortos: 1 línea con ellipsis; descripciones máx. 2–3 líneas con `-webkit-line-clamp`.
4. `text-wrap: balance` en titulares largos (onboarding).
5. Texto nunca se difumina: color plano, peso ≥ 400.
6. Inter 400/500/600/700/800 — sin pesos menores a 400 en texto legible.

---

## 5. Spacing

Escala real de tokens (base 4 px):

```css
--s-1: 4px  --s-2: 8px  --s-3: 12px --s-4: 16px --s-5: 20px
--s-6: 24px --s-8: 32px --s-10: 40px --s-12: 48px --s-16: 64px
```

**Convenciones reales:**
- Padding de cards: `--s-5` (20 px) en sugerencias, `--s-3/--s-4` en TaskCard (12/16 px), `--s-6` (24 px) en secciones de ajustes.
- Gap entre tarjetas: `--s-4`–`--s-6` (16–24 px). Nunca apilar superficies sin aire.
- Gap interno de filas: 6–10 px.
- Altura mínima de interacción: 44 px (QuickAdd 44, botones 36–44).
- Sidebar: 232 px fija. Calendario contenido: padding 0 `--s-8`.

---

## 6. Radius

```css
--r-sm: 10px   --r-md: 16px   --r-lg: 22px   --r-xl: 28px   --r-full: 999px
```

| Token | Uso real |
|-------|----------|
| `--r-sm` (10 px) | EventBlocks, chips de calendario, botones pequeños, kbd |
| `--r-md` (16 px) | Celdas de mes, tarjetas de agenda, day-head, secciones |
| `--r-lg` (22 px) | Contenedor del calendario, cards de sugerencias, modales, inputs de QuickAdd |
| `--r-xl` (28 px) | Widget, toast contextual |
| `--r-full` | Checkboxes, badges, pills, swatches, avatares |

**Regla:** radios ≥ 10 px en todo; sin esquinas rectas. Pero **no todo es pill**: el calendario usa 10–22 px, los event blocks 10 px. Los pills se reservan para badges/chips/checkbox.

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
| Hover card/button | 150 | ease-out | translateY(-1px) + sombra e1 |
| Press botón | 120–150 | ease-out | scale(0.92–0.98) o inset |
| Cambio de vista calendario | 160 | fade | `transition:fade` de Svelte, `{#key view}` |
| Apertura drawer | 200 | slide x | `transition:slide` |
| Modal / overlay | 120–160 | fade | overlays + pop (scale 0.94→1 + translateY) |
| Completar tarea | 200–250 | spring | check + "pop" (scale 0.98) en TaskCard |
| Toast contextual | 250 | ease-out | rise (translateY 12px → 0) |
| Preview QuickAdd | 200 | cubic ease | `transition:scale` |
| Mensajes del asistente | 160 | fade | entrada de mensajes AI |
| Cambios en widget | 140 | fade | secciones Ahora/Siguiente/Importante |
| Transición de tema | 200 | ease-out | background/color del body |
| Ghost de drag | instantáneo | — | sigue al cursor, `will-change: top, height` |
| Scrollbar hover | 150 | ease-out | thumb se oscurece |

### 7.3 Reglas de motion

1. **La animación comunica estado o continuidad.** El ghost del drag muestra adónde va la tarea; el fade del calendario suaviza el cambio de vista; el pop del check confirma la acción.
2. Solo `transform`/`opacity` en transiciones CSS (nunca animar width/height/box-shadow masivamente).
3. Sin rebotes en elementos grandes (widget, modal): solo micro-elementos.
4. **`prefers-reduced-motion` respetado globalmente** en `app.css`: toda animación/transición se reduce a 120 ms; el onboarding desactiva GSAP por completo si el usuario lo pide.
5. Sin animación "porque se ve bonita". El único uso de GSAP es la entrada escalonada del onboarding, y se desactiva con reduced motion.

---

## 8. Iconografía

- **Estilo:** SVG inline, stroke 1.4–2.5 (típicamente 2), `fill="none"`, `stroke-linecap/linejoin round`.
- **Tamaños:** 18 px sidebar, 16 px controles, 11–12 px dentro de botones pequeños, 20–22 px en dialogs.
- **Color:** hereda `currentColor`; reposo `--text-2`/`--text-3` → hover `--text-1` o `--primary`.
- **Categorías:** iconos Lucide-style (graduation-cap, briefcase, user, heart-pulse, wallet, sparkles).
- Prohibido: iconos rellenos, con gradiente, o animados (salvo el check dibujado del checkbox).

---

## 9. Patrones repetidos (deuda visual y consistencia)

### 9.1 Lo que se repite de forma consistente
- Botón primario: `background: var(--primary); color: #fff; border-radius: 12px;` (definido ~5 veces pero con el mismo aspecto).
- Chip de categoría: `color-mix(in srgb, var(--c) 60%, var(--text-1))` sobre `color-mix(in srgb, var(--c) 13%, var(--surface))`, `--r-full`.
- Input inset: `background: var(--surface-3); box-shadow: var(--shadow-inset-sm); border-radius: 10–12px;` + focus con anillo `--primary-soft-2`.
- Cards: `--surface` + `--shadow-raised` + `--r-lg`.
- Hover universal: translateY(-1px) + sombra un nivel arriba, 150 ms.

### 9.2 Inconsistencias detectadas (deuda visual — registrar, no "arreglar" sin plan)
1. **Radios de botón inconsistentes:** 12 px (TaskDrawer, Suggestions, Settings), 15 px (QuickAdd), 10 px (ContextualToast), 999 px (pills de Onboarding).
2. **`.btn` duplicado con variantes locales:** cada componente redefine su `.btn` (Suggestions, Settings, TaskDrawer, ContextualToast, PlanProposal) con diferencias sutiles (padding 8/16 vs 9/14 vs 7/12). No existe un `Button.svelte` global — el componente está **propuesto**.
3. **Foco:** `:focus-visible` global define outline 2 px `--primary-soft-2`, pero TaskDrawer y Settings usan `box-shadow: 0 0 0 3px var(--primary-soft)` en inputs — dos idiomas de foco.
4. **Overlines:** la mayoría usa `text-transform: uppercase` + `letter-spacing 0.06–0.1em`, pero algunas etiquetas (día del popup, título del drawer) usan `text-transform: capitalize` — mezcla de convenciones.
5. **Sombras de drawer vs modal:** el drawer usa sombra lateral dura (`-12px 0 36px rgba(31,41,55,0.25)`) que no es ninguna de las elevaciones tokenizadas.
6. **Drag toast vs toast contextual:** dos sistemas de toast distintos (`.drag-toast` en Calendar, `.toast` en ContextualToast, `.toast` en QuickAdd) con estilos diferentes.
7. **`.ghost` (sin fondo)** y **`.danger`** como variantes de botón solo existen en algunos componentes.

---

## 10. Checklist de identidad (aplicar siempre)

1. ¿Un solo azul de acción visible? ¿Semánticos solo para estados?
2. ¿La luz viene de arriba-izquierda en TODAS las superficies?
3. ¿Espacio plano + profundidad sutil, sin saturar de relieves?
4. ¿Controles con UNA sombra y contenedores con relieve doble?
5. ¿Radios ≥ 10 px, sin esquinas rectas, sin pills abusivos?
6. ¿Números tabulares, overlines uppercase con tracking?
7. ¿Animaciones 120–250 ms, solo transform/opacity, con reduced-motion?
8. ¿Dark mode = misma identidad con luz apagada (no inversión)?
9. ¿El estado nunca depende solo del color?
