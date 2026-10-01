# 19 — Neumorfismo real + layout responsivo

**Estado:** Plan aprobado · **Fecha:** 2026-09-30
**Origen:** sesión de grilling (3 rondas) tras las fases del spec 18.
**Referencias visuales:** 3 capturas de la galería del usuario (`OneDrive\Imágenes\Screenshots\Captura de pantalla 2026-09-30 225824.png`, `…225843.png`, `…225848.png`): kits de *soft UI* clásico. Complementa `spec/refimg.jpeg`.
**Rama:** `feat/ui-neumorfismo` (sale de `feat/ui-polish-resto`) → PR #4 apilado sobre #3.

**Diagnóstico de partida**
- La app se ve plana porque usa superficies blancas (`#FFFFFF`) sobre un fondo casi blanco (`#F8F8F8`) con sombras de contraste muy bajo. Los tokens de sombra son los mismos desde el commit inicial: no hay un "diseño anterior" en el historial que restaurar.
- **Scroll:** `.page-wrap` / `.cal-wrap` (`App.svelte`) scrollean con `padding-top: 0` justo debajo de una TopBar del mismo color que el fondo. El contenido se corta en una línea invisible ("se mete debajo de la barra") y además se recorta la luz superior del relieve.
- **2560×1440:** el calendario no llena el alto (`.cal` mide 605 px de 1306 disponibles, porque el wrapper de la transición rompe la cadena flex). Ajustes queda pegado a la izquierda con ~1200 px vacíos y el texto se ve diminuto.
- **1024×640:** la barra lateral de 232 px se come un cuarto del ancho, "Añadir sesión de estudio" parte en dos líneas, el QuickAdd se trunca y los 7 días quedan de ~90 px.

---

## 1. Decisiones

| # | Tema | Decisión |
|---|------|----------|
| D1 | Objetivo visual | Las **3 imágenes** de la galería: neumorfismo clásico. |
| D2 | Superficies | **Superficie = fondo** (`#E9EDF2` en claro). Tarjetas, calendario y sidebar se separan solo por la sombra doble. Las zonas de contenido (cuadrícula horaria, inputs) van **hundidas**. |
| D3 | Relieve | **En superficies y controles** (reemplaza la regla D-01 de DESIGN.md): botones elevados que se hunden al pulsar; inputs, selects y la búsqueda del QuickAdd hundidos; toggles con knob elevado sobre riel hundido; nav activo hundido y hover elevado; pills y chips de filtro elevados. **Excepción:** bloques de tarea y chips del calendario conservan el tinte de categoría con relieve mínimo. |
| D4 | Forma | Botones, inputs, selects y chips en **píldora** (`--r-full`); tarjetas y paneles en `--r-xl` (28 px); botones de icono **circulares**. Los bloques del calendario mantienen `--r-sm`. |
| D5 | Acento | **Degradado + brillo** derivados del acento elegido en Ajustes (`color-mix`, válido para los 6 acentos) en: botón principal, toggles activos, barras de progreso, círculo de "hoy" y nav activo. Los colores de categoría siguen planos. |
| D6 | Intensidad | **Media** (sombras de 8 px, luz al 90 %) en general; **marcada** (12 px, luz al 100 %) solo en piezas destacadas (tarjeta del calendario, botón principal). |
| D7 | Modo oscuro | **Neumorfismo oscuro** con el mismo sistema (superficie = fondo gris oscuro, sombra negra profunda, luz casi imperceptible). Se arregla el bug del acento: el acento elegido se aplica como estilo inline y pisa el azul aclarado del tema oscuro, así que en oscuro botones y enlaces quedan sin contraste. En oscuro se usa una versión aclarada del acento elegido. |
| D8 | Modales | Panel neumórfico sobre un **overlay claro desenfocado** (el mismo gris translúcido + `backdrop-filter: blur`), como el "Pop Up" de la imagen 1. Así se conserva el arreglo del halo (no hay luz blanca sobre fondo oscuro). En oscuro el overlay es oscuro y la luz del relieve es casi nula. |
| D9 | Scroll | **Todo scrollea junto:** la TopBar sube con el contenido y nada pasa "por debajo" de una barra. Excepción: la conversación del Asistente tiene scroll propio, con un fundido superior. Los paddings de los contenedores cubren el alcance de la sombra, así que nada se recorta. |
| D10 | Ventana grande | El contenido (TopBar y vista) se limita a ~1680 px y se centra. Las vistas de lectura (Ajustes, Sugerencias, Asistente) se limitan a ~880 px, centradas. Paddings y gaps de contenedor con `clamp()` hasta un tope; la tipografía queda fija. El calendario **llena el alto**. |
| D11 | Ventana chica | Por debajo de 1200 px de ancho la barra lateral pasa sola a una **columna de iconos** (~72 px). Además hay un **botón manual** para colapsar o expandir a cualquier tamaño, que guarda la preferencia. El mínimo de ventana sube de 800×600 a **960×640** (`tauri.conf.json`). |
| D12 | Columna de iconos | Iconos del nav con tooltip, contador de Sugerencias sobre su icono, "Añadir horario" y "Añadir sesión" como botones circulares, botón de tema y botón para expandir. Se ocultan las categorías y la tarjeta "MIÉ 30 · pendientes". |
| D13 | Tipografía | Se mantiene **Inter**. Sobre el gris, `--text-2` → `#5B6472` y `--text-3` → `#7C8594` para mantener AA (título ≥ 4.5:1; `--text-3` solo para meta). |
| D14 | Accesibilidad | Línea muy sutil solo en **inputs y selects**, más un anillo de foco fuerte (`:focus-visible`) en todo. Botones y pills quedan puros. |
| D15 | Widget | Incluido, con los mismos tokens. |
| D16 | Arreglos previos | Se conservan **todos** los de los PR #2/#3: completadas hundidas, TaskCheck, multi-día solo inicio/fin, toast `nlToast`, capitalización, horas completas, gutter, recortes, etc. |
| D17 | Flujo | `developer` implementa, el orquestador revisa (tests, `svelte-check`, build, capturas en 3 tamaños × claro/oscuro) y `docs-commits` commitea y documenta. |

---

## 2. Tokens de partida (`app.css`)

Valores iniciales: se pueden afinar mirando las capturas, pero cada cambio se documenta aquí y en DESIGN.md.

**Claro**
```css
--bg: #E9EDF2;
--surface: #E9EDF2;           /* superficie = fondo */
--surface-2: #E3E8EF;         /* pozo sutil: fila "todo el día", completadas */
--surface-3: #DDE3EB;         /* fondo de inputs hundidos */
--neu-dark: rgba(163, 177, 198, 0.6);
--neu-light: rgba(255, 255, 255, 0.9);
--shadow-raised-sm: 4px 4px 8px var(--neu-dark), -4px -4px 8px var(--neu-light);      /* controles */
--shadow-raised: 8px 8px 16px var(--neu-dark), -8px -8px 16px var(--neu-light);       /* media */
--shadow-raised-lg: 12px 12px 28px rgba(150, 165, 190, 0.7), -12px -12px 28px #fff;   /* marcada */
--shadow-inset: inset 4px 4px 8px rgba(163, 177, 198, 0.55), inset -4px -4px 8px rgba(255, 255, 255, 0.85);
--shadow-inset-sm: inset 2px 2px 5px rgba(163, 177, 198, 0.5), inset -2px -2px 5px rgba(255, 255, 255, 0.8);
--grad-accent: linear-gradient(135deg, color-mix(in srgb, var(--accent) 78%, #fff), var(--accent));
--glow-accent: 0 6px 16px -4px color-mix(in srgb, var(--accent) 45%, transparent);
--text-2: #5B6472;
--text-3: #7C8594;
--line-input: rgba(91, 100, 114, 0.22); /* D14 */
--shadow-reach: 48px; /* alcance máximo de la sombra marcada: mínimo de padding en contenedores con scroll */
```

**Oscuro**
```css
--bg: #262A32; --surface: #262A32; --surface-2: #22252C; --surface-3: #1F2228;
--neu-dark: rgba(0, 0, 0, 0.55); --neu-light: rgba(255, 255, 255, 0.05);
/* mismas fórmulas de sombra con estos colores */
--primary: color-mix(in srgb, var(--accent) 75%, #fff); /* acento aclarado (fix D7) */
```

`--e1/--e2/--e3` se mantienen solo para lo que flota sobre contenido **sin** overlay (menús, popovers, toasts).

---

## 3. Tareas

Paso 0 (orquestador): capturas **antes** en claro y oscuro a 1024×640, 1440×900 y 2560×1440.

### T1 · Layout: scroll unificado, anchos máximos y calendario que llena el alto
**Archivos:** `App.svelte`, `Calendar.svelte` (y otras vistas si hace falta para centrar).
- El contenedor de scroll pasa a ser la columna de contenido entera (TopBar + vista): la TopBar scrollea con el contenido.
- **Vistas que llenan el alto** (semana, día, mes, horario, sesiones, asistente): ocupan el alto disponible y solo scrollean (junto con la TopBar) si su alto mínimo no cabe.
- **Asistente:** la conversación conserva su scroll interno con el input fijo abajo, y lleva un fundido superior (`mask-image`) de ~24 px.
- **Calendario que llena el alto:** el wrapper de la transición `{#key}` de `App.svelte` debe propagar la cadena flex (`display: flex; flex-direction: column; flex: 1; min-height: 0`). Aceptación: a 2560×1440, `.cal` ≈ alto disponible.
- **Ancho máximo:** contenido ≤ 1680 px centrado; Ajustes, Sugerencias y Asistente ≤ 880 px centrados.
- Padding y gap de contenedor: `clamp(var(--s-6), 2.5vw, var(--s-12))` o equivalente. Horizontal ≥ `--shadow-reach` cuando haya overflow.
- Sin scrollbars horizontales a 960, 1440 y 2560 px.

### T2 · Barra lateral en modo iconos + mínimo de ventana
**Archivos:** `Sidebar.svelte`, `App.svelte`, `data.svelte.ts` (persistencia de la preferencia), `spike/src-tauri/tauri.conf.json` (`minWidth: 960`, `minHeight: 640`).
- Automática por debajo de 1200 px; botón manual de colapsar/expandir que se guarda (localStorage o `ui_prefs` si ya existe el mecanismo; si no, localStorage con try/catch).
- Contenido de la columna según D12; tooltips; `aria-label` en cada botón de icono; el contador de Sugerencias como badge.
- Transición de ancho suave (`--dur-base`) con `prefers-reduced-motion` respetado.

### T3 · Tokens neumórficos (claro, oscuro, acento)
**Archivos:** `app.css`, `data.svelte.ts` (`applyUiPrefs`).
- Tokens del §2. `applyUiPrefs` deja de pisar el acento aclarado en oscuro: fija `--accent` y deja que cada tema derive `--primary`.
- Comprobar el contraste AA de `--text-1/2` sobre `--surface` en ambos temas.

### T4 · Controles con relieve y píldoras
**Archivos:** componentes con botones, inputs, selects, toggles, chips de filtro y nav (`TopBar`, `QuickAdd`, `Sidebar`, `TaskDrawer`, `Settings`, `Suggestions`, `Assistant`, `PlanProposal`, `ClassForm`, `StudyForm`, `Login`, `Onboarding`, `TaskCheck`).
- Botón: `--shadow-raised-sm`, `:active` → `--shadow-inset-sm` (press = hundirse); principal con `--grad-accent` y `--glow-accent`.
- Input, select y textarea: `--surface-3` + `--shadow-inset-sm` + borde `1px solid var(--line-input)` (D14).
- Toggles: riel hundido, knob elevado; activo con degradado.
- Nav: activo hundido, hover elevado.
- Píldoras: `--r-full`; botones de icono circulares.

### T5 · Superficies de todas las vistas
Calendario (tarjeta `--shadow-raised-lg`, cuadrícula y fila superior hundidas, círculo de "hoy" con degradado), Sidebar, tarjetas de Ajustes, Sugerencias, Asistente y PlanProposal, TaskDrawer, popup del mes, Login y Onboarding. Bloques y chips del calendario: tinte de categoría con relieve mínimo (D3). El estado completado sigue hundido (`--shadow-inset`).

### T6 · Modales y overlays
ClassForm, StudyForm, ClassConflictDialog, diálogo de borrado del TaskDrawer y modal de PlanProposal: overlay claro desenfocado (D8) y panel `--shadow-raised-lg`. Toasts y popovers siguen con `--e2`.

### T7 · Widget
`Widget.svelte` y `WidgetPage.svelte` con los mismos tokens (ventana fija: sin cambios de espaciado).

### T8 · Documentación
DESIGN.md (D-01 reemplazado por D3, superficie = fondo, tokens nuevos, reglas de relieve, píldoras, acento, modales, scroll y anchos), COMPONENTS.md (Sidebar en modo iconos, layout de App), ACCESSIBILITY.md (D14, contraste de `--text-*` sobre gris) y este spec.

---

## 4. Verificación (cada tarea)

1. `npx vitest run`, `npx svelte-check --tsconfig ./tsconfig.app.json` (0 errores, ≤ 32 warnings) y `npm run build`.
2. Capturas antes/después en claro y oscuro a 1024×640, 1440×900 y 2560×1440 (mock de `__TAURI_INTERNALS__`).
3. Antes del PR: prueba en `tauri dev` (scroll, columna de iconos, modales, completar y reabrir).

## 5. Fuera de alcance

Cambiar la fuente · rediseñar la anatomía de los bloques del calendario (solo cambia su relieve) · landing page · lógica de negocio. El backend solo cambia en `tauri.conf.json` (tamaño mínimo de ventana). Pendientes registrados aparte: el foco de los inputs del TaskDrawer (DESIGN.md) y el horario por defecto de StudyForm que cruza la medianoche.
