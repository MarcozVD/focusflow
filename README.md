# FocusFlow

### Gestor personal de tiempo con IA — local, instantáneo y bonito

FocusFlow es una aplicación de escritorio para Windows diseñada para **organizar, planificar y gestionar tu tiempo de forma inteligente**.

Combina calendario, gestión de tareas, notificaciones y un asistente de IA para convertir lenguaje natural y correos electrónicos en un horario organizado, manteniendo tus datos localmente en tu equipo.

> **FocusFlow está actualmente en Demo / Beta.**

---

## Demo

![FocusFlow Demo](image.png)

---

## Características

* **Calendario inteligente** — Vistas de mes, semana y día con drag & drop, validación de conflictos, tareas multi-día y tareas de todo el día.
* **Gestión de tareas** — Crea, edita, mueve, duplica, fusiona y completa tareas con categorías, prioridades, etiquetas, notas y enlaces.
* **QuickAdd** — `Ctrl+Shift+Espacio` desde cualquier aplicación para crear tareas mediante lenguaje natural.
* **Procesamiento local** — Si la IA tarda, no está disponible o no hay API configurada, un parser local basado en reglas permite procesar las entradas inmediatamente.
* **Asistente IA** — Consulta tu agenda y tareas utilizando lenguaje natural.
* **Priorización determinista** — Clasifica las tareas como URGENTE, IMPORTANTE o NORMAL sin permitir que la IA invente datos.
* **Planificación inteligente** — Genera propuestas de horario teniendo en cuenta tareas, prioridades, disponibilidad y conflictos.
* **Procesamiento de correo** — Analiza correos mediante IMAP para detectar tareas, exámenes y reuniones.
* **Sugerencias de correo** — Las actividades detectadas pueden aceptarse, editarse, rechazarse o fusionarse con tareas existentes.
* **Notificaciones nativas** — Recordatorios de Windows con identidad propia, horario silencioso y configuración personalizada.
* **Widget de escritorio** — Ventana transparente, sin marco y siempre visible para consultar rápidamente la agenda.
* **Diseño Neumorphism** — Interfaz consistente con soporte para modo claro y oscuro.
* **Reporte de errores** — Sistema integrado para enviar reportes junto con los logs de la aplicación.
* **Privacidad local-first** — Las credenciales se almacenan de forma segura y los datos principales permanecen en el dispositivo.

---

## Cómo funciona

```text
             Usuario
                │
                ▼
     ┌────────────────────┐
     │ QuickAdd / Correo  │
     │ Lenguaje natural   │
     └──────────┬─────────┘
                │
                ▼
       ┌────────────────┐
       │ Procesamiento  │
       │ IA / Local     │
       └───────┬────────┘
               │
               ▼
     ┌────────────────────┐
     │ Motor de           │
     │ planificación      │
     │                    │
     │ Huecos disponibles │
     │ Prioridades        │
     │ Conflictos         │
     └──────────┬─────────┘
                │
                ▼
       ┌────────────────┐
       │    SQLite      │
       │     Local      │
       └───────┬────────┘
               │
               ▼
        Notificaciones
```

La meta es pasar de **gestionar manualmente un calendario** a tener un asistente que ayude activamente a organizar el tiempo.

---

## Stack tecnológico

### Frontend

* Svelte 5
* TypeScript
* Vite
* CSS personalizado
* Diseño Neumorphism

### Desktop y Backend

* Tauri 2
* Rust
* Tokio
* IPC
* `rusqlite`
* `native-tls`
* `keyring`

### Inteligencia Artificial

* APIs compatibles con OpenAI Chat Completions
* Groq
* `openai/gpt-oss-120b`
* Parser local basado en reglas como fallback

### Correo

* IMAP
* SMTP
* TLS
* Sistema de checkpoints y reintentos
* Detección y deduplicación de sugerencias

### Base de datos

* SQLite
* `rusqlite`
* Base de datos local y embebida

### Calidad

* 210 tests de Rust
* 36 tests de frontend
* Vitest
* Cargo Test

---

## Arquitectura

```text
FocusFlow
│
├── Svelte 5
│   ├── Calendario
│   ├── Agenda
│   ├── Asistente
│   ├── Sugerencias
│   └── Ajustes
│
├── Tauri 2
│   └── Rust
│       ├── Comandos IPC
│       ├── Planning Engine
│       ├── AI
│       ├── Email / Sync
│       ├── Notifications
│       └── Error Reporting
│
└── SQLite
    ├── Tareas
    ├── Sugerencias
    ├── Ajustes
    └── Historial de sincronización
```

---

## Instalación

### Opción A — Instalador

La forma recomendada de probar FocusFlow es descargar el instalador `.msi` disponible en la sección **Releases**.

La aplicación se instala como una aplicación de Windows independiente.

### Opción B — Desde el código fuente

#### Requisitos

* Windows 10/11
* Node.js
* npm
* Rust
* Cargo
* Git
* Tauri CLI

#### Clonar el repositorio

```bash
git clone https://github.com/MarcozVD/focusflow.git
cd focusflow/spike/frontend
```

#### Instalar dependencias

```bash
npm install
```

#### Ejecutar en desarrollo

```bash
cd ../src-tauri
cargo tauri dev
```

#### Generar el instalador MSI

Desde la raíz del repositorio:

```bash
build-focusflow.bat
```

---

## Tests

### Rust

```bash
cd spike/src-tauri
cargo test --lib
```

Actualmente existen **210 tests** para el motor de planificación, parsers, sincronización, notificaciones y lógica interna.

### Frontend

```bash
cd ../frontend
npx vitest run
```

Actualmente existen **36 tests** para el frontend.

---

## Configuración

La configuración se realiza directamente desde la aplicación mediante la sección **Ajustes**.

### Asistente IA

Puedes configurar cualquier proveedor compatible con OpenAI Chat Completions.

Se recomienda Groq por su velocidad y disponibilidad de modelos gratuitos.

La aplicación permite probar la conexión directamente desde los ajustes.

### Correo electrónico

Puedes configurar una cuenta mediante IMAP utilizando:

* Servidor IMAP
* Correo electrónico
* Contraseña de aplicación

FocusFlow analiza los mensajes para detectar posibles tareas, eventos, exámenes y reuniones.

Las sugerencias siempre requieren confirmación del usuario antes de modificar el calendario.

### Seguridad

Las credenciales se almacenan mediante el **Windows Credential Manager** y no directamente en la base de datos SQLite.

> Nunca subas API keys, contraseñas o credenciales al repositorio.

---

## Descarga

La versión más reciente de FocusFlow estará disponible en la sección **Releases**.

### Plataformas

* Windows 10/11 x64 — Disponible
* Linux — Planeado
* macOS — Planeado

---

## Roadmap

### Actual

* [x] Calendario de mes, semana y día
* [x] Drag & drop
* [x] Validación de conflictos
* [x] Gestión completa de tareas
* [x] QuickAdd
* [x] Asistente IA
* [x] Parser local
* [x] Planificación inteligente
* [x] Sugerencias desde correo
* [x] Detección de duplicados
* [x] Widget de escritorio
* [x] Modo claro y oscuro
* [x] Diseño Neumorphism
* [x] Notificaciones nativas de Windows
* [x] Reporte de errores integrado

### Siguiente

* [ ] Planificación avanzada con más restricciones
* [ ] Sincronización con calendarios externos
* [ ] Priorización automática mejorada
* [ ] Mejor procesamiento contextual de correos
* [ ] Soporte para Linux
* [ ] Soporte para macOS

### Futuro

* [ ] Sincronización en la nube opcional
* [ ] Soporte multidispositivo
* [ ] Aplicación móvil complementaria
* [ ] Analítica de productividad
* [ ] Integraciones adicionales con servicios externos

---

## Privacidad

FocusFlow sigue un enfoque **local-first**.

Tus tareas, calendario, ajustes y demás información principal se almacenan en una base de datos SQLite local.

No necesitas crear una cuenta ni utilizar una suscripción para utilizar las funciones principales de la aplicación.

Las funciones de IA requieren comunicación con el proveedor seleccionado. En estos casos, FocusFlow envía únicamente el contexto necesario para procesar la solicitud.

El procesamiento de correo también minimiza la información antes de enviarla a servicios externos.

> La privacidad de los datos enviados a servicios de IA también depende de las políticas del proveedor que hayas configurado.

---

## Visión

La mayoría de las aplicaciones de productividad requieren que el usuario organice constantemente su propio horario.

**FocusFlow busca cambiar ese modelo.**

En lugar de limitarse a mostrar un calendario, FocusFlow utiliza el contexto de tus tareas, prioridades y disponibilidad para ayudarte a decidir:

**qué hacer, cuándo hacerlo y cómo organizar tu tiempo.**

---

## Licencia

Este proyecto está bajo la licencia MIT.

Consulta el archivo [`LICENSE`](LICENSE) para conocer los términos completos.

---

## Autor

**Marcos**

Estudiante de Ingeniería de Sistemas · Full Stack Developer

* Full Stack Development
* Inteligencia Artificial
* Linux
* Bases de datos
* Desarrollo Desktop y Mobile

---

<p align="center">
  FocusFlow — AI Personal Time Manager
</p>
