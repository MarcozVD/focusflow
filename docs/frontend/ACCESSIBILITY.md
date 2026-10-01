# ACCESSIBILITY.md — Accesibilidad de FocusFlow

**Método:** auditoría sobre el código real (`spike/frontend/src/`) + warnings de `svelte-check`/`vite-plugin-svelte` observados en el arranque del dev server. Cada hallazgo indica: estado actual, evidencia y mejora propuesta.
**Última actualización:** 2026-10-01 (spec 19: superficie = fondo gris, relieve en controles, línea sutil en campos, interruptores accesibles).

> **Principio rector:** el neumorfismo no debe destruir la accesibilidad. La profundidad visual nunca es el único indicador de estado. El producto ya cumple varios principios clave; esta auditoría separa lo que está bien de lo que es deuda.

---

## 1. Resumen ejecutivo

| Área | Estado |
|------|--------|
| Keyboard navigation | 🟡 Bueno con deudas (drag & drop sin alternativa) |
| Focus-visible | 🟢 Global y único (los campos ya no añaden un anillo propio) |
| Semantic HTML | 🟡 Landmarks OK; diálogos no nativos |
| Contrast | 🟢 Cumple AA en tokens principales |
| Icon labels | 🟡 Mayormente OK (SVG aria-hidden + botones con label/title) |
| Button sizes | 🟢 ≥ 44 px targets en acciones principales |
| Reduced motion | 🟢 Global respetado (120 ms) + GSAP off en onboarding |
| Status announcements | 🟡 Faltan `aria-live` en varios estados |
| Color independence | 🟢 Sólida (estados multi-canal) |
| Calendar accessibility | 🟡 Estructura OK; drag sin teclado |

---

## 2. Keyboard navigation

**Lo que ya funciona (evidencia en código):**

- Elementos clicables no-nativos usan `role="button"` + `tabindex="0"` + manejo de `Enter`/`Espacio`:
  - Celdas del mes (`Calendar.svelte:454-456`)
  - EventBlocks (`EventBlock.svelte:61-70`)
  - TaskCard (`TaskCard.svelte:27-29`)
- `Escape` cierra: drawer de tarea, popup de mes, propuesta de plan, widget (`App.svelte:100-103`, `PlanProposal.svelte:27-34`).
- Flujo de navegación completo por Tab: sidebar → topbar → calendario → acciones.
- Resize handles: `role="separator"` + `aria-label` (opcional pero presente).

**Deuda detectada:**

1. **Drag & drop sin alternativa de teclado** — mover/redimensionar tareas solo es posible con puntero. Un usuario de teclado no puede reorganizar su calendario. *Propuesta:* en el TaskDrawer, campos fecha/hora ya permiten ajuste por teclado (paliativo existente); a futuro, mover tarea por atajos (Alt+↑/↓ con confirmación).
2. Los botones de la grid de mes usan `role="button"` sobre `<div>`; semánticamente un `<button>` real sería más robusto (ya lo son los chips internos).

---

## 3. Focus-visible

**Bien:** `app.css:157-161` define globalmente:

```css
:focus-visible {
  outline: 2px solid var(--primary-soft-2);
  outline-offset: 2px;
  border-radius: var(--r-sm);
}
```

- Visible siempre (ratón y teclado) con anillo de contraste alto (`--primary-soft-2`).
- **Un solo idioma de foco (spec 19):** los campos de todo el repo (incluidos los del `TaskDrawer`) solo cambian `border-color` a `--primary` al enfocar y **no añaden `box-shadow` propio**, para que el anillo global no choque con el pozo del campo. Se retiró el `box-shadow: 0 0 0 3px var(--primary-soft)` del drawer, que era el segundo idioma.
- QuickAdd mantiene doble indicación: borde primario en `:focus-within` + anillo global.
- La sidebar en modo iconos conserva el texto de las etiquetas fuera de flujo (no `display: none`), así que el botón sigue teniendo nombre accesible aunque solo se vea el icono.

**Deuda detectada:**

1. En widgets con muchos elementos, el outline de 2 px puede quedar oculto por transform de hover; se recomienda verificar con `:focus-visible` que el anillo no se recorte por `overflow: hidden` de las cards (EventBlocks tienen `overflow: hidden`).
2. La línea de los campos es muy sutil (`--line-input: rgba(91, 100, 114, 0.22)`, D14) a propósito: el borde no es el indicador de foco, el anillo global lo es. En fondos de alto contraste forzados puede perderse el contorno del campo, aunque el foco siga siendo visible.

---

## 4. Semantic HTML

**Bien:**

- Landmarks: `<header>` (TitleBar), `<main class="content">`, `<nav>` (Sidebar), `<aside>` (TaskDrawer).
- Títulos jerárquicos en páginas (h2/h3 correctos en Sugerencias, Asistente, Ajustes).
- Formularios con `<label for>` / label envolvente.
- `role="alert"` en error fatal y errores de formulario (Onboarding `ferrbox`, Login `err`).
- `role="status"` en confirmaciones de conexión (Onboarding: `fok`).
- `aria-modal="true"` + `aria-labelledby` en el diálogo de confirmación de borrado (TaskDrawer:253).
- **Interruptores = `<input type="checkbox">` reales** (spec 19): «Todo el día» del drawer y los ajustes de correo/Google Calendar se estilizan con `appearance: none` como riel 40×22 con knob, pero el elemento, su estado y su teclado son los nativos (`bind:checked`, label envolvente). El switcher de tema usa `role="group"` + `aria-label="Tema"`. Ningún estado se comunica solo con el color del degradado.

**Deuda detectada (warnings reales del dev server):**

1. `TitleBar.svelte:41` — `<header>` con handlers de `contextmenu`/`dblclick` **sin ARIA role**. El doble clic (maximizar) no es accesible por teclado. *Propuesta:* `role="toolbar"` o al menos documentar el atajo; idealmente duplicar la acción en el botón de maximizar (que sí existe y es accesible) — mitigación razonable.
2. `TaskDrawer.svelte:128` — `<div class="overlay">` con click handler: falta role y handler de teclado. *Propuesta:* usar `role="presentation"` con `aria-hidden` (el cierre real está en el botón ✕ accesible) o convertir en botón.
3. `TaskDrawer.svelte:229-230` — diálogo de confirmación: `<div class="dlg" role="dialog">` **sin `tabindex`**, y su overlay con click handler sin role. El `<dialog>` nativo resolvería foco y trampa de foco.
4. `TaskDrawer.svelte:230` — overlay de confirmación con click handler sin role/keyboard.
5. `Onboarding.svelte:288` — `autofocus` evitado por linter (a11y). Aceptable en onboarding (única acción primaria) pero idealmente enfocar programáticamente sin `autofocus`.
6. Popup del mes (`Calendar.svelte`) — el overlay de cierre no existe (popup usa botón ✕), OK; pero el `day-popup` no tiene `role="dialog"` ni focus trap. *Propuesta:* al abrir, mover foco al popup.

---

## 5. Contrast

**Superficie = fondo (spec 19):** todo se calcula sobre el gris, no sobre blanco. Ratios verificados con la fórmula WCAG 2.1 sobre los valores reales de `app.css`.

**Claro (`--surface` `#E9EDF2`)**

| Par | Ratio | Veredicto |
|-----|-------|-----------|
| `--text-1` `#1F2937` | **12.48:1** | AAA |
| `--text-2` `#5B6472` | **5.09:1** | AA (se oscureció desde `#6B7280`, que daba 4.4:1) |
| `--text-3` `#7C8594` | **3.17:1** | Solo meta/placeholder, nunca contenido esencial |
| Blanco sobre `--primary` `#2563EB` | **5.17:1** | AA |
| `--text-2` sobre `--surface-2` `#E3E8EF` (completadas) | **4.86:1** | AA — el título tachado sigue siendo legible |

**Oscuro (`--surface` `#262A32`)**

| Par | Ratio | Veredicto |
|-----|-------|-----------|
| `--text-1` `#F3F4F6` | **13.07:1** | AAA |
| `--text-2` `#A6ADBB` | **6.38:1** | AA |
| `--text-3` `#6B7280` | **2.98:1** | Solo meta/placeholder |

**Otras comprobaciones**

- **Acento en oscuro (fix D7):** `--primary` se deriva del acento elegido con `color-mix(... 75%, #fff)`; al dejar de pisar el azul aclarado del tema, botones y enlaces vuelven a tener contraste con cualquiera de los 6 acentos.
- Chips de categoría: texto = `color-mix(var(--c) 60%, var(--text-1))` sobre `color-mix(var(--c) 13%, var(--surface))` → conserva contraste sobre su fondo soft.
- Línea de campos (D14): `--line-input: rgba(91, 100, 114, 0.22)` es deliberadamente tenue. No es el indicador de foco (lo es el anillo global) y no es texto, así que no se mide contra 4.5:1; aun así sirve de contorno visible para el pozo del campo.
- Relieve: la sombra oscura bajo texto nunca es el único indicador — el texto mantiene su color plano (`--text-1/2/3`), que es donde se apoya el contraste.

**Deuda / vigilancia:**

1. `--text-3` en botones secundarios o `kbd` hints: correcto como decorativo, pero si un botón lo usa como único label falla AA (no detectado actualmente).
2. `color-mix` con alpha en dark (`--primary-soft: 20% transparent`) sobre fondo varía según contexto — verificar por superficie.
3. Semánticos en dark (`--success #34D399`, `--warning #FBBF24`, `--danger #F87171`) cumplen AA sobre superficies oscuras (≈ 4.5:1+).
4. Al ser el relieve puramente decorativo, conviene comprobar que en modo alto contraste (fuerza colores) los campos siguen delimiting su área: `--line-input` y `--shadow-inset-sm` no sobreviven a un `forced-colors`.

---

## 6. Icon labels

**Bien:**

- Iconos decorativos: SVG con `aria-hidden="true"` o `stroke="currentColor"` heredando de botón con texto (Sidebar, TopBar).
- Botones solo-icono con `aria-label` o `title`:
  - TitleBar: minimizar/maximizar/cerrar (`title` + `aria-label`).
  - TopBar arrows: `aria-label="Anterior"/"Siguiente"`.
  - TaskDrawer ✕: `aria-label="Cerrar"`.
  - Popup ✕: `aria-label="Cerrar"`.
  - Widget acciones: `title` ("Completar", "Posponer 1 hora", "Empezar ahora").
  - Swatches de acento: `aria-label="Acento #…"`.

**Deuda:**

1. Los iconos SVG de Sidebar **no** tienen `aria-hidden` explícito (heredan `currentColor` dentro de un botón con texto → el texto ya es el label; aceptable pero mejoraria con `aria-hidden`).
2. `aria-label` con contenido duplicado del `title` (TitleBar) — inofensivo, evitable.

---

## 7. Button sizes

**Bien:** los targets de acción superan 44 px de alto de interacción (QuickAdd 44 px; botones principales 36–44 px con padding generoso; checkbox 22 px pero dentro de card de 44+ px; icon buttons circulares 28–40 px). Los interruptores miden 40×22 px con label envolvente, así que el área clicable incluye el texto.

**Vigilancia:** chips del calendario (`minichip` ~20 px) son targets pequeños — mitigado porque el área clicable del día/popup es mayor y el "más" abre el popup.

---

## 8. Reduced motion

**Excelente — es el más maduro:**

- `app.css:203-209`: `@media (prefers-reduced-motion: reduce)` fuerza 120 ms en TODAS las animaciones/transiciones (incluidas las del neumorfismo: el press de los botones no se anima si el usuario lo pide).
- Onboarding (`Onboarding.svelte`): detecta `matchMedia("(prefers-reduced-motion: reduce)")` y **desactiva GSAP por completo** (solo fade CSS de 0 ms).
- Svelte transitions (`fade`, `slide`, `scale`) se reducen vía la media query global.

**Única nota:** 120 ms no es 0 ms; para usuarios con vestibular severa, 120 ms de fade es aceptable y mantiene la percepción de cambio. Correcto.

---

## 9. Status announcements (aria-live)

**Faltante — deuda principal de accesibilidad:**

- Toast de QuickAdd ("Tarea creada") — sin `role="status"`.
- Toast de drag ("Movida con aviso…") — sin `role="status"`.
- Toast `nlToast` de `App.svelte` — **resuelto**: `role="alert"` para errores, `role="status"` + `aria-live="polite"` en el resto.
- Cambios de sección del widget (Ahora → Siguiente) — sin `aria-live`.
- Contador "N pendientes" del sidebar — sin anuncio.
- La propuesta de plan que aparece (modal) — no anuncia su apertura (el foco no se mueve al modal).

**Propuesta:** añadir `role="status"`/`aria-live="polite"` a los toasts y al widget (contenedor `.body`), y mover foco al abrir modales.

---

## 10. Color independence

**Sólido — el diseño ya es multi-canal:**

| Estado | Color | + Otro canal |
|--------|-------|--------------|
| Vencida | `--danger` | Borde izquierdo **dashed** + badge de texto "Vencida" |
| Completada | superficie hundida `--surface-2` | `text-decoration: line-through` animado + ✓ del `TaskCheck` (relleno `--text-3`, siempre visible) |
| Prioridad alta | `--danger` | Badge de texto "Alta" |
| Hoy | círculo primary | Número en blanco dentro del círculo (forma + texto) |
| Fuera de mes | opacity 0.4 | — (solo opacidad; aceptable como jerarquía secundaria) |

**Regla:** mantener el principio — profundidad + color + texto. No añadir estados que dependan solo de un canal.

---

## 11. Calendar accessibility

**Estructura OK:**

- Celdas de mes: `role="button"` + teclado + popup accesible por botón.
- EventBlocks: `role="button"` + teclado + tooltip.
- Chips: botones reales con `title`.
- `TaskCheck` (check rápido): botón real con `aria-label` ("Marcar como completada" / "Reabrir tarea") y `aria-pressed`, por lo que es **operable con teclado desde cualquier calendario**. Detiene `keydown` para que Enter/Espacio no abra además el drawer.
- Línea "ahora": decorativa (aria-hidden implícito por ser div).

**Deuda:**

1. Drag & drop sin teclado (ver §2) — la principal.
2. Popup de día sin `role="dialog"` ni focus management.
3. Los "+N más" de semana son botones que abren el día; OK.
4. Los eventos vencidos se distinguen por dashed border + texto en tooltip; en el bloque visible solo dashed — el `title` lo comunica al hover, pero un screen reader lee el título de la tarea, no el estado vencida. *Propuesta:* añadir `aria-label` con estado cuando aplique.

---

## 12. Plan de acción (priorizado)

| # | Acción | Impacto |
|---|--------|---------|
| 1 | `role="status"`/`aria-live` en toasts y widget | Alto (anuncios) |
| 2 | `<dialog>` nativo en confirmación de borrado + focus trap en modales | Alto (foco) |
| 3 | Foco al abrir popup de día y propuesta de plan | Alto (navegación) |
| 4 | ~~Unificar idioma de foco en inputs (TaskDrawer)~~ **hecho (spec 19)** | — |
| 5 | `aria-label` con estado en EventBlocks vencidos | Medio (screen readers) |
| 6 | `aria-hidden` explícito en iconos decorativos del sidebar | Bajo |
| 7 | Alternativa de teclado para drag & drop (largo plazo) | Medio (requiere diseño) |
| 8 | Resolver warnings de `svelte-check` (TitleBar role, TaskDrawer overlays) | Bajo–Medio |

> **Nota para la landing:** debe heredar estos estándares (focus-visible, reduced-motion, role=status en interacciones, contraste AA) desde el inicio, no como añadido.
