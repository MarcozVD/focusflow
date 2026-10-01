# DESIGN.md — Sistema visual de FocusFlow

**Estado:** Documento vivo · **Última actualización:** 2026-09-30
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
| Completada | `--surface-2` **hundida** (`--shadow-inset`; chips y minichips con `--shadow-inset-sm`) | ✓ (`TaskCheck` relleno `--text-3`, siempre visible) + borde izquierdo `--text-3` | Título tachado (animado izq→der) en `--text-2`, meta en `--text-3`. **Sin `opacity`** |
| Vencida | `--danger-bg` | Borde izquierdo **dashed** `--danger` | `--danger` |
| Prioridad alta | — | Punto/barra `--danger`, badge `--danger-bg` | `--danger` |
| Prioridad media | — | Badge `--primary-soft` | `--primary` |
| Hoy | — | Círculo `--primary` con número blanco | — |
| En curso (widget) | — | Etiqueta "Ahora" `--primary` | — |
| Conflicto (drag) | toast `--danger` | — | blanco |

**Regla de oro:** el color nunca es el único indicador de estado. Vencida = fondo tintado + borde punteado + texto; completada = tachado + superficie hundida gris + ✓; prioridad = badge + punto.

> Las completadas son **plenamente visibles en el calendario** (semana, día, mes y popup): hundidas en gris, nunca difuminadas con `opacity`. El título usa `--text-2` y no `--text-3` porque es contenido esencial (ver ACCESSIBILITY §5); `--text-3` queda para hora, descripción y punto de categoría.

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
| Título de sección / card | `--fs-lg` | 600 | — | h2 en Ajustes, h3 de propuesta, drawer |
| Cuerpo | `--fs-base`–`--fs-md` | 400–500 | — | Base de la app, inputs |
| Nombre de tarea | `--fs-xs`–`--fs-md` | 500–600 | — | EventBlock / TaskCard |
| Meta/horas | `--fs-2xs`–`--fs-sm` | 600 | — | `tabular-nums` SIEMPRE |
| Overline | `--fs-2xs` | 600 | +0.06–0.1em, uppercase | Etiquetas de sección, días |
| Hora del calendario | `--fs-xs` | 600 | — | gutter, tabular-nums |
| Placeholder | `--fs-base` | 400 | — | `--text-3` |

### 4.3 Regla de pesos

400 / 500 / 600 son la escala normal. **700 solo en el título de pantalla y el hero de onboarding**; el peso 800 queda eliminado (el hero pasó de 800 a 700). Los overlines van en 600 manteniendo uppercase + tracking.

### 4.4 Reglas tipográficas

1. **Números y horas siempre `font-variant-numeric: tabular-nums`** (calendario, contadores, widget).
2. Jerarquía por tamaño + peso + tinte; el título nunca en gris (`--text-1`).
3. Títulos cortos: 1 línea con ellipsis; descripciones máx. 2–3 líneas con `-webkit-line-clamp`.
4. `text-wrap: balance` en titulares largos (onboarding).
5. Texto nunca se difumina: color plano, peso ≥ 400.
6. **Ningún `font-size` literal:** todo sale de `--fs-*`.

---

## 5. Spacing

Escala real de tokens (grid de 4 px + micro-gaps):

```css
--s-0_5: 2px  --s-1: 4px  --s-1_5: 6px  --s-2: 8px  --s-3: 12px --s-4: 16px --s-5: 20px
--s-6: 24px --s-8: 32px --s-10: 40px --s-12: 48px --s-16: 64px
```

**Regla de aire:** en **contenedores** (cards, paneles, drawer, sidebar, topbar, popup) el padding y el gap suben **un escalón** respecto al contenido que alojan. En **contenido denso** (EventBlock, chips, minichips, celdas del mes) no se sube nada: se conserva la densidad para que quepa más información en el mismo espacio. `--s-0_5` y `--s-1_5` cubren los micro-gaps internos (gap de una lista de chips, separación label/valor).

Los hairlines de `1px`/`1.5px` en bordes y outlines quedan literales; no son espaciado.

**Convenciones reales:**
- Padding de cards: `--s-5`/`--s-6` en contenedores grandes, `--s-4`/`--s-5` en medianos.
- Gap entre tarjetas: `--s-4`–`--s-6` (16–24 px). Nunca apilar superficies sin aire.
- Gap interno de filas: `--s-1_5`–`--s-2` (6–8 px).
- Altura mínima de interacción: 44 px (QuickAdd 44, botones 36–44).
- Sidebar: 232 px fija. Calendario contenido: padding 0 `--s-8`.

---

## 6. Radius

```css
--r-xs: 6px   --r-sm: 10px  --r-md: 16px  --r-lg: 22px  --r-xl: 28px  --r-full: 999px
```

| Token | Uso real |
|-------|----------|
| `--r-xs` (6 px) | Elementos pequeños y densos: minichips y chips del calendario, `.prio-dot`, `kbd`, checks, botones muy pequeños |
| `--r-sm` (10 px) | EventBlocks, botones, filas del popup, notas de diálogo |
| `--r-md` (16 px) | Celdas de mes, tarjetas de agenda, day-head, input del QuickAdd, icon-button |
| `--r-lg` (22 px) | Contenedor del calendario, cards de sugerencias, modales, dialogs |
| `--r-xl` (28 px) | Widget, toast contextual |
| `--r-full` | Checkboxes, badges, pills, swatches, avatares, TaskCheck |

**Mapeo de literales:** `≤ 9px → --r-xs` · `10–12px → --r-sm` · `14–18px → --r-md` · `22–24px → --r-lg` · `28px → --r-xl` · `999px → --r-full`. `50%` (círculos) se queda literal.

**Regla:** sin esquinas rectas y **no todo es pill**: los pills se reservan para badges/chips/checkbox; el contenido denso del calendario baja a `--r-xs`/`--r-sm` y los contenedores usan `--r-md`/`--r-lg`.

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
| Completar tarea | 250 | ease-out | hundir + tachar en el calendario (`--dur-slow`: sombra, fondo y color); el ✓ del `TaskCheck` hace scale(0.92) al pulsar |
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
1. **Radios de botón inconsistentes:** el núcleo diario ya usa tokens (`--r-sm` en TaskDrawer, `--r-md` en QuickAdd), pero quedan literales en Suggestions, Settings, ContextualToast y los pills de Onboarding (fase 2).
2. **`.btn` duplicado con variantes locales:** cada componente redefine su `.btn` (Suggestions, Settings, TaskDrawer, ContextualToast, PlanProposal) con diferencias sutiles (padding 8/16 vs 9/14 vs 7/12). No existe un `Button.svelte` global — el componente está **propuesto**.
3. **Foco:** `:focus-visible` global define outline 2 px `--primary-soft-2`, pero TaskDrawer y Settings usan `box-shadow: 0 0 0 3px var(--primary-soft)` en inputs — dos idiomas de foco.
4. **Overlines:** la mayoría usa `text-transform: uppercase` + `letter-spacing 0.06–0.1em`, pero algunas etiquetas (día del popup, título del drawer) usan `text-transform: capitalize` — mezcla de convenciones.
5. ~~Sombras de drawer vs modal~~ **resuelto:** el drawer y su diálogo de borrado usan ya `--e3`, la misma elevación que los modales.
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
