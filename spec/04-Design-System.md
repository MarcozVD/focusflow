# 04 — Design System (Soft UI 2.0)

**Producto:** FocusFlow · **Versión:** v0.1 · **Fecha:** 2026-08-04

> **⚠️ Estado de fidelidad:** este documento se escribió a partir de la descripción del brief + referencia de imagen que **no pudo verificarse visualmente** (modelo sin soporte de imagen). Paleta, sombras y layout siguen literalmente lo especificado; cualquier desviación real de la imagen se ajustará en la primera revisión visual.

**Declaración de identidad (una frase):**
> FocusFlow se ve como una tarjeta de contacto de cristal esmerilado sobre luz de mañana: superficies blancas cálidas que flotan sobre sombras grandes y difusas, un solo azul profundo como voz del producto, y micro-movimientos que hacen que cada acción se sienta física pero suave. Limpio, luminoso, premium. Nada de Material, nada de Flat.

---

## 1. Principios de diseño (reglas innegociables)

| # | Principio | Regla concreta |
|---|-----------|----------------|
| D-01 | **El relieve es de las superficies, no de los controles** | Paneles, tarjetas, calendario y widget llevan la doble sombra neumórfica (luz superior-izquierda). Botones, inputs y chips llevan **una** sombra sutil; su profundidad se revela en hover/press. Si todo tiene relieve, nada es interactivo |
| D-02 | **Un solo azul manda** | El primario `#2563EB` es el único color "de acción". Verde/ámbar/rojo son *semánticos* (éxito/próximo/vencido) y siempre en versión suave como fondo |
| D-03 | **Luz desde arriba-izquierda** | Toda sombra se compone con fuente de luz superior-izquierda: highlight claro en `-x,-y` y sombra profunda tenue en `+x,+y`. Invertir la luz en un mismo panel rompe la ilusión → prohibido |
| D-04 | **El texto nunca se difumina** | Los textos usan color plano (`#1F2937` / `#6B7280`) con peso ≥ 400. Las sombras dan profundidad a superficies, jamás legibilidad |
| D-05 | **Todo se mueve entre 150 y 250 ms** | Nada más rápido que 100 ms (se siente seco), nada más lento que 300 ms (se siente pesado). Solo `transform`/`opacity` para 60 fps |
| D-06 | **Dark mode es el mismo alma con luz apagada** | Mismas proporciones, mismas sombras (más oscuras), el mismo azul como ancla. No es un tema aparte: es la misma identidad |
| D-07 | **Menos es premium** | Un máximo de 2 niveles de énfasis por pantalla. La jerarquía se hace con tamaño/peso/tinte, no con más colores |
| D-08 | **Accesibilidad sin excusas** | Contraste AA en texto (4.5:1), foco visible siempre, `prefers-reduced-motion` respetado (ver §12) |

---

## 2. Paleta y tokens de color

### 2.1 Modo claro (default)

| Token | Valor | Uso |
|-------|-------|-----|
| `--bg` | `#F8F8F8` | Fondo de ventana (blanco cálido) |
| `--surface` | `#FFFFFF` | Tarjetas, paneles, calendario, widget, inputs |
| `--surface-2` | `#F1F2F4` | Zonas hundidas: slots del calendario, área de drop, chips en reposo |
| `--surface-3` | `#EAECEF` | Inputs con "relieve hundido" (Quick Add, búsqueda) |
| `--primary` | `#2563EB` | Acción, selección, hoy, enlaces, progreso, botón principal |
| `--primary-hover` | `#1D4ED8` | Hover de primario |
| `--primary-active` | `#1E40AF` | Press de primario |
| `--primary-soft` | `#DBEAFE` | Fondo de estados activos, selección de día, chips de hoy |
| `--primary-soft-2` | `#BFDBFE` | Borde de foco primario |
| `--success` | `#059669` | Texto/icono de completado |
| `--success-bg` | `#D1FAE5` | Fondo de tarea completada, badge éxito |
| `--warning` | `#B45309` | Texto/icono de próximo/urgente |
| `--warning-bg` | `#FEF3C7` | Fondo de aviso, contador regresivo cercano |
| `--danger` | `#DC2626` | Texto/icono vencida, prioridad alta |
| `--danger-bg` | `#FEE2E2` | Fondo de vencida |
| `--text-1` | `#1F2937` | Texto primario |
| `--text-2` | `#6B7280` | Texto secundario, etiquetas de fecha |
| `--text-3` | `#9CA3AF` | Placeholder, texto deshabilitado |
| `--border` | `#E7E9EC` | Bordes finos cuando hacen falta (inputs, divisores) |

> Categorías (Universidad, Trabajo…) usan **colores propios asignados desde esta paleta de acentos**: `#2563EB` (azul), `#7C3AED` (violeta), `#0EA5E9` (cielo), `#10B981` (esmeralda), `#F59E0B` (ámbar), `#EC4899` (rosa), `#EF4444` (rojo), `#14B8A6` (teal). Cada categoría elige un acento + su versión soft automática (mezcla 12 % sobre blanco).

### 2.2 Modo oscuro (misma identidad, luz apagada)

| Token | Valor | Nota |
|-------|-------|------|
| `--bg` | `#16181E` | Fondo oscuro cálido (no negro puro) |
| `--surface` | `#1E2129` | Superficies |
| `--surface-2` | `#262A34` | Hundidos |
| `--surface-3` | `#2C313C` | Inputs |
| `--primary` | `#3B82F6` | Se aclara para mantener contraste AA en oscuro |
| `--primary-soft` | `#1E3A8A` (tinte oscuro) | Estados activos |
| `--success` | `#34D399` / `--success-bg` `#0F2E22` | Semánticos aclarados |
| `--warning` | `#FBBF24` / `--warning-bg` `#33250F` | |
| `--danger` | `#F87171` / `--danger-bg` `#3B1A1A` | |
| `--text-1` | `#F3F4F6` | |
| `--text-2` | `#A6ADBB` | |
| `--text-3` | `#6B7280` | |
| `--border` | `#333845` | |

**Regla:** en oscuro las sombras son `rgba(0,0,0,0.55)` y los highlights `rgba(255,255,255,0.04)` (casi imperceptibles). El azul primario es el punto de color de identidad en ambos temas.

### 2.3 Uso del color por significado

| Estado de tarea | Superficie (chip) | Borde/indicador | Texto |
|-----------------|-------------------|-----------------|-------|
| Completada | `--success-bg` | — | `--success`, título tachado |
| Vencida | `--danger-bg` | Barra izquierda `--danger` | `--danger` |
| Próxima (< 24 h) | `--warning-bg` | — | `--warning` |
| Activa/hoy | `--primary-soft` | Barra `--primary` | `--text-1` |
| Normal | `--surface` | `--border` | `--text-1` |

---

## 3. Tipografía

| Token | Valor |
|-------|-------|
| Familia | **Inter** (fallback: system-ui, "Segoe UI") |
| Pesos | 400 (regular), 500 (medium), 600 (semibold), 700 (bold) |
| Escala | display 32/1.2 · h1 24 · h2 20 · h3 18 · body-lg 16 · body 14 · caption 12 · overline 11 |

| Uso | Tamaño | Peso | Color |
|-----|--------|------|-------|
| Título de pantalla | 24 | 700 | `--text-1` |
| Nombre de tarea | 14 | 500 | `--text-1` |
| Fecha/tiempo (calendario) | 13–14 | 500 | `--text-2` |
| Número de día (calendario) | 14 | 600 | hoy: blanco sobre `--primary` |
| Número grande widget | 40 | 700 | `--text-1` (contador regresivo) |
| Placeholder | 14 | 400 | `--text-3` |
| Overline (categoría, etiquetas) | 11 | 600, mayúsculas +0.08em | `--text-3` |

**Reglas:** título nunca en gris; los números del calendario y contadores en `tabular-nums`. Longitudes: títulos cortos (1 línea + ellipsis), descripciones hasta 3 líneas.

---

## 4. Espaciado y grid

| Token | Valor |
|-------|-------|
| Base | 4 px (escala 4/8/12/16/20/24/32/40/48/64) |
| Padding tarjeta | 20 px (interior); 24 px en widget |
| Gap entre tarjetas | 16–20 px |
| Gap sidebar ↔ contenido | 24 px |
| Altura mínima de interacción | 44 px |
| Grid app principal | Sidebar 256 px fija (colapsable a 64 px) · contenido fluido · panel de detalle 360 px (sliding, opcional) |
| Grid calendario mes | 7 columnas iguales, celdas mín. 96 px de alto |
| Widget compacto | 300 px ancho · 120–180 px alto |
| Widget expandido | 320–360 px ancho · hasta 60 % de la altura de pantalla |

**Regla:** el espaciado entre tarjetas es siempre ≥ 12 px; nunca apilar superficies sin aire (la separación amplia es parte del look premium).

---

## 5. Border radius (esquinas suaves)

| Token | Valor | Uso |
|-------|-------|-----|
| `--r-sm` | 10 px | Chips, badges, inputs pequeños |
| `--r-md` | 16 px | Inputs, botones, celdas de calendario |
| `--r-lg` | 22 px | Tarjetas, paneles |
| `--r-xl` | 28 px | Ventana del widget, modal grande |
| `--r-full` | 999 px | Botones tipo píldora, avatares |

**Regla:** el calendario usa 16–22 px en sus componentes (celdas 16, contenedor 22) — integrado con el resto, nunca un "calendario clásico con rectángulos".

---

## 6. Sistema de sombras (el alma del neumorfismo)

Fuente de luz: **arriba-izquierda**. Tres familias:

### 6.1 Relieve "raised" (superficies flotantes — el look de la referencia)

```
--shadow-raised:
  inset 0 1px 0 rgba(255,255,255,0.85),        /* filo de luz superior */
  -6px -6px 14px rgba(255,255,255,0.95),       /* luz desde arriba-izq */
   6px  6px 14px rgba(31,41,55,0.08);          /* sombra profunda abajo-der */

--shadow-raised-lg:  (tarjetas grandes, widget)
  inset 0 1px 0 rgba(255,255,255,0.9),
  -12px -12px 24px rgba(255,255,255,0.9),
   12px  12px 24px rgba(31,41,55,0.10);
```

### 6.2 Relieve "inset" (zonas hundidas: inputs, slots de calendario, área de drop)

```
--shadow-inset:
  inset  4px  4px 10px rgba(31,41,55,0.06),    /* sombra adentro (abajo-der) */
  inset -4px -4px 10px rgba(255,255,255,0.85); /* luz adentro (arriba-izq) */
```

### 6.3 Elevación (elementos que sobrevuelan: drag, modales, toasts, menús)

| Nivel | Token | Uso |
|-------|-------|-----|
| e1 | `0 4px 8px -2px rgba(31,41,55,0.08), 0 2px 4px -2px rgba(31,41,55,0.06)` | Tooltip, dropdown |
| e2 | `0 10px 20px -4px rgba(31,41,55,0.12), 0 4px 8px -4px rgba(31,41,55,0.08)` | Modal, paleta de comandos |
| e3 | `0 18px 36px -6px rgba(31,41,55,0.18), 0 8px 16px -8px rgba(31,41,55,0.12)` | Quick Add flotante, widget expandido, drag activo |

**Reglas de sombra:**
- Contraste **bajo** siempre: la sombra más fuerte es `rgba(31,41,55,0.18)`. Nada de sombras negras densas.
- Tamaño **grande** y difuso: mínimo 14 px de blur para relieve, 20+ para elevación.
- El elemento en **drag** sube un nivel de elevación (e2 → e3) y su sombra se agranda; al soltar, rebote 200 ms.
- En dark mode: `rgba(0,0,0,0.55)` para profundas y `rgba(255,255,255,0.04)` para luces.

---

## 7. Iconografía

- **Librería:** Lucide (`lucide-svelte`), trazo 1.5–2 px (1.75 default), tamaño 16 (inline) / 20 (sidebar) / 24 (vacío estados).
- **Estilo:** delgado, minimalista, redondeado; prohibidos iconos rellenos o con gradiente.
- **Color:** hereda `currentColor`; tinte `--text-2` en reposo → `--text-1` en hover.
- **Categorías:** icono + color de acento de la categoría (graduation-cap, briefcase, user, heart-pulse, wallet, sparkles…).
- **Animación de iconos:** permitida solo en 2 casos: checkbox completando (marca dibujándose, 200 ms) y notificación de "creada" (check en toast).

---

## 8. Componentes

> Cada componente define: estructura, tokens, y tabla de estados (reposo / hover / active / focus / disabled).

### 8.1 Botón primario (el protagonista)

```
Fondo: --primary · Texto: blanco · Radio: 16px · Altura: 44px (L) / 36px (S)
Padding: 16/20px horizontal · Sombra: --shadow-raised (recortada) · Fuente: 14/500
```

| Estado | Cambio |
|--------|--------|
| Reposo | `--primary`, sombra raised sutil |
| Hover | `--primary-hover`, sombra elevada e1, translateY(-1px) — 150 ms |
| Active | `--primary-active`, **sombra inset** (se hunde), translateY(0) — 120 ms |
| Focus | anillo `--primary-soft-2` 2 px, offset 2 px |
| Disabled | `--surface-2`, texto `--text-3`, sin sombra |

Variantes: **Secundario** (fondo `--surface`, texto `--primary`, borde `--border`) · **Ghost** (sin fondo, hover `--surface-2`) · **Peligro** (misma anatomía con `--danger`).

### 8.2 Tarjeta (Card)

```
Fondo: --surface · Radio: 22px · Sombra: --shadow-raised
Interior: 20px padding · Entre tarjetas: 16–20px gap
```

| Estado | Cambio |
|--------|--------|
| Reposo | raised |
| Hover (si es clicable) | `--shadow-raised` con blur mayor, -1px translateY, 200 ms |
| Presionada | raised invertida (inset) |
| Drag sobre ella (target de drop) | borde punteado `--primary` 1.5 px + fondo `--primary-soft` al 40 % |

### 8.3 Input / campo de texto

```
Fondo: --surface-3 (hundido, --shadow-inset) · Radio: 16px · Altura: 44px
Borde: 1px --border transparente en reposo → --primary al focus
```

| Estado | Cambio |
|--------|--------|
| Reposo | inset, borde `--border` |
| Focus | borde `--primary` + anillo 3 px `--primary-soft`, 150 ms |
| Error | borde `--danger` + mensaje `--danger` 12 px |
| Disabled | opacidad 50 %, sin sombra |

El **Quick Add** es el input más grande de la app: altura 56 px, radio 18 px, texto 15 px, con "antorcha" de icono en el borde izquierdo y hint de atajo (Ctrl+Shift+Espacio) a la derecha.

### 8.4 Checkbox (completar tarea)

```
Círculo de 22px: borde 2px --text-3 en reposo · completado: relleno --success
Marca dibujada con stroke-dashoffset, 200 ms · radio --r-full
```

| Estado | Cambio |
|--------|--------|
| Reposo | borde `--text-3` |
| Hover | borde `--success`, sombra suave verde |
| Completado | fondo `--success`, marca blanca; título tacha con fade 200 ms |

### 8.5 Chips y badges

- **Chip de categoría**: fondo = soft del acento (mezcla 12 %), texto = acento oscurecido, radio `--r-full`, icono 14 px.
- **Badge de prioridad**: Alta (`--danger-bg`/`--danger` + barra vertical), Media (azul soft), Baja (gris). 
- **Badge de estado**: Completada (`--success-bg`), En curso (`--primary-soft`), Vencida (`--danger-bg`).

### 8.6 Barra de progreso

```
Track: --surface-3 (hundido, inset) · radio --r-full · altura 8px
Fill: --primary, con micro-sombra suave del mismo color · transición width 250 ms
```

### 8.7 Switch (toggle)

Track 44×26 px, radio full; thumb 20 px blanco con sombra e1. ON: track `--primary`. Transición 200 ms con spring suave.

### 8.8 Calendario

| Elemento | Spec |
|----------|------|
| Contenedor | `--surface`, radio 22, raised |
| Celda de día (mes) | radio 16, hover `--surface-2` 150 ms; celdas del mes actual opacidad 1, fuera de mes 0.45 |
| Día de hoy | círculo `--primary`, número blanco, 34 px |
| Día seleccionado | `--primary-soft` + borde suave `--primary-soft-2` |
| Tarea dentro de celda | chip 12 px alto: fondo = soft del acento de categoría, texto 11/500 del acento, radio 8; vencida = `--danger-bg`; completada = tachada + 50 % opacidad |
| Slot vacío (semana/día) | `--surface-3` con inset — "hueco" listo para recibir drag |
| Línea de hora actual | 2 px `--primary` con punto, arriba de las tareas (z-index) |
| Drop de arrastre | slot se ilumina `--primary-soft` + anillo punteado |

### 8.9 Sidebar

```
Ancho 256px (64px colapsada) · fondo: transparente sobre --bg (sin tarjeta propia)
Ítem activo: --primary-soft, texto --primary, radio 14, icono 20px
Ítem hover: --surface-2
Sección categorías: chips con punto de color + icono
Mini-calendario del mes: tarjeta raised pequeña (radio 16)
```

### 8.10 Item de agenda (lista)

```
Tarjeta 44px+ alto, radio 16, raised (sutil) · hora 13/600 tabular-nums --text-2
Barra izquierda 4px con color de categoría (redondeada, radio full)
Derecha: checkbox, título, badges (prioridad, vencida), etiquetas truncadas
```

### 8.11 Quick Add flotante

```
Ventana 480px, radio 22, elevación e3, --surface
Input 56px centered + línea de "entidades detectadas" (ver PRD P1: fecha→azul, hora→verde, categoría→su acento)
Preview de tarea debajo: mini tarjeta con chips editables antes de confirmar
Botón: Enter (primario) · Esc (cancelar) · tooltip de ejemplos rotativos
```

### 8.12 Toast / notificación in-app

```
Elevación e2, radio 18, --surface, borde izquierdo 4px por tipo (éxito/aviso/info)
Slide-in desde arriba 200 ms · auto-dismiss 4 s con barra de progreso sutil
Acciones: Undo (link primario) en "Tarea creada"
```

### 8.13 Modal y panel de detalle (Sheet)

- Modal: `--surface`, radio 24, e3; overlay `rgba(31,41,55,0.30)` con blur 4 px (fade 150 ms).
- Sheet de tarea: desliza desde la derecha 250 ms (easeOut), 360 px, radio 22 izquierdo.

### 8.14 Empty states (primer uso)

```
Icono Lucide 48px --text-3 sobre círculo --surface-2 · título 18/600 · sub 14/--text-2
Ejemplo educativo: "Prueba: 'Mañana estudiar cálculo de 3pm a 5pm'" con botón que pre-rellena el Quick Add
```

### 8.15 Widget de escritorio

| Elemento | Spec |
|----------|------|
| Contenedor | `--surface` al 92 % (transparencia configurable 40–100 %), radio 28, `--shadow-raised-lg`, borde 1 px `rgba(255,255,255,0.6)` para el filo de luz |
| Compacto | 300 px: header (logo 20 px + fecha) + 3 filas de próxima tarea + contador regresivo grande |
| Expandido | lista de próximas (máx 8), agrupadas por "Hoy / Esta semana / Después", contador al tope |
| Contador regresivo | 40/700 tabular-nums `--text-1` + overline "Entrega en" `--text-3`; < 24 h → `--warning`, vencida → `--danger` |
| Tarea del día | checkbox pequeño funcional + chip categoría |
| Animación compacto↔expandido | 220 ms scale+opacity (0.96→1) con transform-origin inferior-izquierda |
| Hover del widget | eleva 2 px y la sombra crece (e2→e3) 200 ms |

---

## 9. Estados globales

| Estado | Definición estándar |
|--------|---------------------|
| Reposo | Sin sombra extra, colores de token base |
| Hover | -1px translateY + sombra un nivel arriba, 150 ms (`ease-out-quint`) |
| Active | inset (se hunde), 120 ms |
| Focus | anillo 2 px `--primary-soft-2` + offset 2 px; **visible siempre** (ratón y teclado) |
| Disabled | opacidad 45 %, sombra eliminada, cursor default |
| Drag | elevación +1 nivel, rotación 2°, escala 1.02 |
| Drop target | iluminado con anillo punteado + soft del primario |

---

## 10. Motion (animaciones)

### 10.1 Tokens

| Token | Valor |
|-------|-------|
| `--dur-fast` | 150 ms |
| `--dur-base` | 200 ms |
| `--dur-slow` | 250 ms |
| `--ease-out` | `cubic-bezier(0.22, 1, 0.36, 1)` (estándar de salida) |
| `--ease-spring` | `cubic-bezier(0.34, 1.56, 0.64, 1)` (entradas físicas: drag, toasts) |

### 10.2 Mapa de micro-interacciones

| Interacción | Duración | Curva | Detalle |
|-------------|----------|-------|---------|
| Hover botón/card | 150 | ease-out | translateY(-1px) + sombra |
| Press botón | 120 | ease-out | inset + 0 |
| Cambio de vista calendario (mes↔semana) | 250 | ease-out | slide + fade 12 px |
| Abrir sheet de tarea | 250 | ease-out | slide derecha |
| Modal / Quick Add | 200 | ease-out | scale 0.96→1 + fade |
| Completar tarea | 200 | spring | check dibujado + título tachado con fade |
| Drag & drop | 150 / soltar 200 | spring al soltar | elevación + rebote 1.02→1 |
| Widget compacto↔expandido | 220 | ease-out | scale + opacity |
| Toast entrar/salir | 200 | spring | slide down + fade |
| Notificación creada (subrayado de entidades) | 150 | ease-out | reveal de chips |

**Reglas de motion:**
- Solo `transform` y `opacity` (nunca animar width/height/box-shadow masivamente).
- Con `prefers-reduced-motion: reduce`: todo se vuelve crossfade de 120 ms (mantener la navegación funcional).
- Sin rebote en elementos grandes (widget, modal): solo en elementos pequeños.

---

## 11. Grid y layout de la ventana principal

```
┌────────────────────────────────────────────────────────────┐
│ Barra superior (56px, transparente sobre --bg)             │
│  logo 20px · título · [búsqueda 320px inset] · widget btn  │
│  · tema · [Quick Add 240px]                                │
├──────────┬─────────────────────────────────────────────────┤
│ Sidebar  │ Pestañas: Calendario · Agenda · Tareas · Stats  │
│ 256px    │ Contenido (scroll independiente)                │
│ (or 64)  │ Panel detalle: sheet derecho 360px (opcional)   │
└──────────┴─────────────────────────────────────────────────┘
```

**Reglas:** la barra superior y el sidebar flotan sobre `--bg` sin tarjeta; solo los "contenedores de contenido" (calendario, lista, widget) llevan raised. Eso da aire premium y evita saturación de relieves.

---

## 12. Accesibilidad (mínimo de diseño)

| Requisito | Spec |
|-----------|------|
| Contraste | Texto ≥ 4.5:1 (`--text-1`/`--text-2` cumplen en claro; `--text-2` en oscuro `#A6ADBB` cumple 4.6:1 sobre `#1E2129`) |
| Texto sobre color | En chips soft: acento oscurecido (≥ 4.5:1). Texto blanco solo sobre `--primary` y sobre `--danger` |
| Tamaño mínimo | 14 px body, 44 px objetivos táctiles |
| Foco | Anillo 2 px offset 2 px en todos los interactivos |
| Tema | Sigue al sistema por defecto (prefers-color-scheme), override manual en ajustes |
| Reduced motion | Mapa completo en §10.2 |
| Lectura de pantalla | roles de landmark (nav/main/aside), aria-live en toasts y en "entidades detectadas" del Quick Add |

---

## 13. Checklist de revisión visual (aplicar en cada pantalla)

1. ¿Un solo azul de acción? (solo 1 color primario visible por pantalla)
2. ¿La luz viene siempre de arriba-izquierda? (sin sombras invertidas)
3. ¿El texto tiene contraste plano, sin difuminar?
4. ¿Los controles tienen UNA sombra y los contenedores relieve doble? (sin confusión de jerarquía)
5. ¿Radios ≥ 10 px en todo? ¿Sin esquinas rectas?
6. ¿Espaciado entre tarjetas ≥ 12 px?
7. ¿Todas las animaciones entre 150–250 ms y solo transform/opacity?
8. ¿Dark mode mantiene identidad (mismos radios, misma luz, azul ancla)?
9. ¿Foco visible en todos los interactivos?
10. ¿Widget con transparencia opcional y filo de luz superior?

---

## 14. Tokens (formato de implementación)

```css
:root {
  /* Color — claro */
  --bg: #F8F8F8; --surface: #FFFFFF; --surface-2: #F1F2F4; --surface-3: #EAECEF;
  --primary: #2563EB; --primary-hover: #1D4ED8; --primary-active: #1E40AF;
  --primary-soft: #DBEAFE; --primary-soft-2: #BFDBFE;
  --success: #059669; --success-bg: #D1FAE5;
  --warning: #B45309; --warning-bg: #FEF3C7;
  --danger: #DC2626; --danger-bg: #FEE2E2;
  --text-1: #1F2937; --text-2: #6B7280; --text-3: #9CA3AF; --border: #E7E9EC;
  /* Tipografía */
  --font: "Inter", system-ui, "Segoe UI", sans-serif;
  /* Radio */
  --r-sm: 10px; --r-md: 16px; --r-lg: 22px; --r-xl: 28px; --r-full: 999px;
  /* Sombras (neumorfismo, luz arriba-izquierda) */
  --shadow-raised: inset 0 1px 0 rgba(255,255,255,.85),
                   -6px -6px 14px rgba(255,255,255,.95),
                    6px  6px 14px rgba(31,41,55,.08);
  --shadow-raised-lg: inset 0 1px 0 rgba(255,255,255,.9),
                      -12px -12px 24px rgba(255,255,255,.9),
                       12px  12px 24px rgba(31,41,55,.10);
  --shadow-inset: inset  4px  4px 10px rgba(31,41,55,.06),
                  inset -4px -4px 10px rgba(255,255,255,.85);
  --e1: 0 4px 8px -2px rgba(31,41,55,.08), 0 2px 4px -2px rgba(31,41,55,.06);
  --e2: 0 10px 20px -4px rgba(31,41,55,.12), 0 4px 8px -4px rgba(31,41,55,.08);
  --e3: 0 18px 36px -6px rgba(31,41,55,.18), 0 8px 16px -8px rgba(31,41,55,.12);
  /* Motion */
  --dur-fast: 150ms; --dur-base: 200ms; --dur-slow: 250ms;
  --ease-out: cubic-bezier(.22,1,.36,1);
  --ease-spring: cubic-bezier(.34,1.56,.64,1);
  /* Espaciado */
  --s-1: 4px; --s-2: 8px; --s-3: 12px; --s-4: 16px; --s-5: 20px;
  --s-6: 24px; --s-8: 32px; --s-10: 40px; --s-12: 48px; --s-16: 64px;
}
[data-theme="dark"] { /* valores de §2.2 + sombras rgba(0,0,0,.55) / rgba(255,255,255,.04) */ }
```

---

## 15. Decisiones de diseño registradas

| Decisión | Por qué |
|----------|---------|
| Relieve solo en contenedores, no en controles | El neumorfismo total destruye la affordance (auditoría D4); premium ≠ todo con doble sombra |
| Un solo azul + semánticos suaves | Identidad calmada, consistente con "calendario premium"; los semánticos solo para estados |
| Inter y no SF Pro Display | Licencia libre, disponible en Windows, excelente legibilidad 13–14 px |
| Luces con rgba(255,255,255,.85–.95) en claro | El filo de luz blanca es lo que da el "look cristal" de la referencia; en oscuro se reduce a 0.04 para evitar gris plata |
| Widget con radio 28 y borde de luz | Es la pieza más vista del producto; máxima suavidad + filo que la separa del escritorio |
| Motion estándar 150/200/250 | Refleja el brief; el spring solo en piezas pequeñas para no "mover de más" |
