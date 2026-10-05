<script lang="ts">
  import { onMount } from "svelte";
  import { fade, fly } from "svelte/transition";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { listen } from "@tauri-apps/api/event";
  import TitleBar from "./lib/TitleBar.svelte";
  import TopBar from "./lib/TopBar.svelte";
  import Sidebar from "./lib/Sidebar.svelte";
  import Calendar from "./lib/Calendar.svelte";
  import Schedule from "./lib/Schedule.svelte";
  import ClassConflictDialog from "./lib/ClassConflictDialog.svelte";
  import StudySessions from "./lib/StudySessions.svelte";
  import WidgetPage from "./lib/WidgetPage.svelte";
  import Suggestions from "./lib/Suggestions.svelte";
  import Assistant from "./lib/Assistant.svelte";
  import Settings from "./lib/Settings.svelte";
  import Onboarding from "./lib/Onboarding.svelte";
  import Login from "./lib/Login.svelte";
  import { init, loadSuggestions, loadAiConfig, loadEmailConfig, loadSyncStatus, loadGeneralSettings, loadNotifPrefs, loadOnboardingStatus, loadAuthStatus, authUser, ensureRange, taskDetail, openTaskDetail, closeTaskDetail, applySavedTheme, loadUiPrefs, applyUiPrefs, tasks, aiConfig, setAssistantDraft, onboarding, loadStudies, nlToast } from "./lib/data.svelte";
  import TaskDrawer from "./lib/TaskDrawer.svelte";
  import ContextualToast from "./lib/ContextualToast.svelte";

  let view = $state<"mes" | "semana" | "dia" | "horario" | "sesiones" | "sugerencias" | "asistente" | "ajustes">("semana");
  // Submodo semana/día compartido por Horario (clases) y Sesiones de estudio:
  // mismas interacciones de TopBar y navegación ‹ › para las dos vistas.
  let hmode = $state<"semana" | "dia">("semana");
  let date = $state(new Date());
  let hash = $state(window.location.hash);
  let isWidget = $state(false);
  let bootReady = $state(false);
  let showOnboarding = $state(false);
  // Último error de UI no capturado: sin esto, una excepción en un effect o
  // render dejaba la app congelada sin ninguna pista visible.
  let fatalError = $state("");

  onMount(() => {
    const onErr = (e: ErrorEvent) => {
      // Aviso benigno del navegador al redimensionar (ResizeObserver): no es fatal.
      if (/ResizeObserver loop/.test(e.message)) {
        console.debug("[ui] aviso benigno ignorado:", e.message);
        return;
      }
      fatalError = e.message || "Error inesperado de interfaz";
    };
    const onRej = (e: PromiseRejectionEvent) => {
      fatalError = String(e.reason ?? "Error inesperado de interfaz");
    };
    window.addEventListener("error", onErr);
    window.addEventListener("unhandledrejection", onRej);
    return () => {
      window.removeEventListener("error", onErr);
      window.removeEventListener("unhandledrejection", onRej);
    };
  });

  const onboardingPending = $derived(showOnboarding || onboarding()?.completed === false);

  // Scroll unificado: al cambiar de vista o de submodo (horario/sesiones) la
  // columna vuelve arriba; las flechas de fecha dentro de la misma vista no.
  let contentEl = $state<HTMLElement | null>(null);
  $effect(() => {
    void view;
    void hmode;
    if (contentEl) contentEl.scrollTop = 0;
  });

  // Toast global de data.svelte (setNlToast): errores de acciones rápidas
  // (crear/actualizar tarea…) que antes no se pintaba en ningún sitio.
  const nlToastMsg = $derived(nlToast());
  const reduceMotion =
    typeof window !== "undefined" && window.matchMedia("(prefers-reduced-motion: reduce)").matches;

  $effect(() => {
    if (onboarding()?.completed) showOnboarding = false;
  });

  applySavedTheme();

  onMount(() => {
    init();
    loadSuggestions();
    loadAiConfig();
    loadEmailConfig();
    loadSyncStatus();
    loadUiPrefs();
    loadNotifPrefs();
    loadOnboardingStatus();
    loadAuthStatus().then(() => (bootReady = true));
    try {
      isWidget = getCurrentWindow().label === "widget";
      if (isWidget) document.documentElement.dataset.widget = "";
    } catch {
      // navegador
    }
    const un1 = listen("task:open", (e) => {
      const id = Number(e.payload);
      const t = tasks().find((x) => x.id === id);
      if (t) openTaskDetail(t);
      else refreshAndOpen(id);
    });
    const un2 = listen("nav:study", () => {
      view = "sesiones";
    });
    const un3 = listen("ui:prefs", (e) => {
      applyUiPrefs(e.payload as { theme?: string; accent?: string });
    });
    const un4 = listen("nav:assistant", () => {
      view = "asistente";
    });
    return () => {
      un1.then((f) => f());
      un2.then((f) => f());
      un3.then((f) => f());
      un4.then((f) => f());
    };
  });

  async function refreshAndOpen(id: number) {
    await ensureRange(new Date(Date.now() - 7 * 86400000), new Date(Date.now() + 35 * 86400000));
    const t = tasks().find((x) => x.id === id);
    if (t) openTaskDetail(t);
  }

  window.addEventListener("hashchange", () => (hash = window.location.hash));
  window.addEventListener("keydown", (e) => {
    if (e.key === "Escape" && hash === "#/widget") window.location.hash = "";
    if (e.key === "Escape" && taskDetail()) closeTaskDetail();
  });

  function setView(v: "mes" | "semana" | "dia" | "horario" | "sesiones" | "sugerencias" | "asistente" | "ajustes") {
    view = v;
    if (v === "sugerencias") loadSuggestions();
    if (v === "sesiones") loadStudies();
    if (v === "ajustes") {
      loadAiConfig();
      loadEmailConfig();
      loadSyncStatus();
      loadGeneralSettings();
    }
  }

  function planFromNotif(n: { task_id: number; task_title: string; kind: string }) {
    if (aiConfig()?.configured) {
      setAssistantDraft(`Crea un plan para: «${n.task_title}». Ten en cuenta el resto de mi agenda.`);
      setView("asistente");
    } else {
      const t = tasks().find((x) => x.id === n.task_id);
      if (t) openTaskDetail(t);
    }
  }
  function navigate(dir: -1 | 1) {
    const d = new Date(date);
    if (view === "mes") d.setMonth(d.getMonth() + dir);
    else if (view === "dia") d.setDate(d.getDate() + dir);
    else if (view === "horario" || view === "sesiones") d.setDate(d.getDate() + dir * (hmode === "dia" ? 1 : 7));
    else d.setDate(d.getDate() + dir * 7);
    date = d;
  }
  function setDate(d: Date) {
    date = d;
  }
  function setHmode(m: "semana" | "dia") {
    hmode = m;
  }
  function goToday() {
    date = new Date();
  }
  function selectDate(d: Date) {
    date = d;
    view = "dia";
  }

  $effect(() => {
    if (view === "sesiones" || view === "sugerencias" || view === "asistente" || view === "ajustes" || view === "horario") return;
    const d = new Date(date);
    const start = new Date(d.getFullYear(), d.getMonth(), d.getDate());
    let from = start;
    let to = start;
    if (view === "semana") {
      const dow = (start.getDay() + 6) % 7;
      from = new Date(start);
      from.setDate(start.getDate() - dow);
      to = new Date(from);
      to.setDate(from.getDate() + 6);
    } else if (view === "mes") {
      from = new Date(start.getFullYear(), start.getMonth(), 1);
      to = new Date(start.getFullYear(), start.getMonth() + 1, 0);
      from.setDate(from.getDate() - from.getDay());
      to.setDate(to.getDate() + (6 - to.getDay()));
    }
    ensureRange(from, to);
  });
</script>

{#if isWidget || hash === "#/widget"}
  <WidgetPage />
{:else if !bootReady}
  <div class="app">
    <TitleBar />
  </div>
{:else if !authUser()}
  <div class="app">
    <TitleBar />
    <Login />
  </div>
{:else if onboardingPending}
  <div class="app">
    <TitleBar />
    <Onboarding />
  </div>
{:else}
  <div class="app">
    <TitleBar />
    <div class="body">
      <Sidebar {view} {setView} {navigate} />
      <main class="content" bind:this={contentEl}>
        <div class="content-inner">
          <TopBar {date} {view} {navigate} {goToday} {hmode} {setHmode} />
          {#if view === "sesiones"}
            <div class="cal-wrap">
              {#key "sesiones-" + hmode + date.toDateString()}
                <div class="view-fill" transition:fade={{ duration: 160 }}>
                  <StudySessions {date} smode={hmode} {setDate} setSmode={setHmode} />
                </div>
              {/key}
            </div>
          {:else if view === "horario"}
            <div class="cal-wrap">
              {#key "horario-" + hmode + date.toDateString()}
                <div class="view-fill" transition:fade={{ duration: 160 }}>
                  <Schedule {date} {hmode} {setDate} {setHmode} />
                </div>
              {/key}
            </div>
          {:else if view === "sugerencias"}
            <div class="page-wrap">
              <Suggestions />
            </div>
          {:else if view === "asistente"}
            <div class="page-wrap assistant-wrap">
              <Assistant />
            </div>
          {:else if view === "ajustes"}
            <div class="page-wrap">
              <Settings onReopenOnboarding={() => (showOnboarding = true)} />
            </div>
          {:else}
            <div class="cal-wrap">
              {#key view + date.toDateString()}
                <div class="view-fill" transition:fade={{ duration: 160 }}>
                  <Calendar {view} {date} onSelectDate={selectDate} />
                </div>
              {/key}
            </div>
          {/if}
        </div>
      </main>
    </div>
    {#if taskDetail()}
      <TaskDrawer />
    {/if}
    <ClassConflictDialog />
    {#if nlToastMsg}
      <div
        class="nl-toast"
        class:error={nlToastMsg.source === "error"}
        role={nlToastMsg.source === "error" ? "alert" : "status"}
        aria-live={nlToastMsg.source === "error" ? "assertive" : "polite"}
        transition:fly={{ y: 12, duration: reduceMotion ? 0 : 200 }}
      >
        {nlToastMsg.text}
      </div>
    {/if}
    {#if !isWidget}
      <ContextualToast onplan={planFromNotif} />
    {/if}
    {#if fatalError}
      <div class="fatal" role="alert">
        <span>Algo falló en la interfaz: {fatalError}</span>
        <button onclick={() => (fatalError = "")}>Cerrar</button>
      </div>
    {/if}
  </div>
{/if}

<style>
  .app {
    display: flex;
    flex-direction: column;
    height: 100vh;
    overflow: hidden;
  }
  .body {
    display: flex;
    flex: 1;
    min-height: 0;
  }
  /* Scroll unificado: scrollea la columna de contenido entera (TopBar + vista) */
  .content {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    overflow-y: auto;
    overflow-x: hidden;
  }
  /* Columna centrada (TopBar y página alineadas); colchón para la sombra */
  .content-inner {
    display: flex;
    flex-direction: column;
    flex: 1 0 auto;
    width: 100%;
    max-width: 1680px;
    margin-inline: auto;
    min-width: 0;
    padding: 0 clamp(var(--s-10), 3vw, var(--s-16)) clamp(var(--s-6), 2.5vw, var(--s-12));
  }
  /* La TopBar vive dentro de la columna: su padding horizontal lo pone el contenedor */
  .content-inner :global(.top.top) {
    padding-inline: 0;
  }
  /* Vistas que llenan el alto (semana, día, mes, horario, sesiones) */
  .cal-wrap {
    flex: 1 0 auto;
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  /* La transición {#key} no debe romper la cadena flex */
  .view-fill {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-height: 0;
  }
  .cal-wrap :global(.cal) {
    flex: 1;
  }
  .page-wrap {
    flex: 1 0 auto;
    min-height: 0;
  }
  /* Asistente: llena el alto exacto; scrollea solo la conversación interna */
  .assistant-wrap {
    display: flex;
    flex-direction: column;
    flex: 1 1 0;
    min-height: 0;
    overflow: hidden;
  }
  .fatal {
    position: fixed;
    left: 50%;
    bottom: 18px;
    transform: translateX(-50%);
    z-index: 120;
    display: flex;
    align-items: center;
    gap: var(--s-3);
    max-width: min(640px, calc(100vw - 40px));
    background: var(--surface);
    color: var(--danger);
    border: 1px solid color-mix(in srgb, var(--danger) 40%, transparent);
    border-radius: var(--r-card);
    box-shadow: var(--e3);
    padding: var(--s-2) var(--s-3);
    font-size: var(--fs-base);
    font-weight: 600;
  }
  .fatal button {
    border: none;
    background: var(--danger);
    color: #fff;
    border-radius: var(--r-control);
    padding: var(--s-1) var(--s-3);
    font-size: var(--fs-sm);
    font-weight: 600;
    cursor: pointer;
    flex-shrink: 0;
  }
  /* Toast global (nlToast): abajo al centro, fuera de la esquina de
     ContextualToast (abajo a la derecha). `margin-inline: auto` centra sin
     transform propio: Svelte fly anima transform y pisaría un translateX. */
  .nl-toast {
    position: fixed;
    left: 0;
    right: 0;
    bottom: 20px;
    margin-inline: auto;
    width: fit-content;
    max-width: min(520px, calc(100vw - 40px));
    z-index: 90;
    background: var(--surface);
    border: 1px solid var(--border);
    border-left: 3px solid var(--border);
    border-radius: var(--r-card);
    box-shadow: var(--e2);
    padding: var(--s-3) var(--s-4);
    color: var(--text-1);
    font-size: var(--fs-base);
    font-weight: 500;
  }
  .nl-toast.error {
    border-left-color: var(--danger);
  }
  :global([data-widget] html) {
    background: transparent;
  }
  :global([data-widget] body) {
    background: transparent;
  }
  :global([data-widget] .wp) {
    background: transparent;
    min-height: auto;
    padding: 0;
    display: block;
  }
  :global([data-widget] .toolbar) {
    display: none;
  }
</style>
