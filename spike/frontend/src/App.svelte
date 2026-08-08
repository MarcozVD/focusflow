<script lang="ts">
  import { onMount } from "svelte";
  import { fade } from "svelte/transition";
  import { getCurrentWindow } from "@tauri-apps/api/window";
  import { listen } from "@tauri-apps/api/event";
  import TitleBar from "./lib/TitleBar.svelte";
  import TopBar from "./lib/TopBar.svelte";
  import Sidebar from "./lib/Sidebar.svelte";
  import Calendar from "./lib/Calendar.svelte";
  import Agenda from "./lib/Agenda.svelte";
  import WidgetPage from "./lib/WidgetPage.svelte";
  import Suggestions from "./lib/Suggestions.svelte";
  import Assistant from "./lib/Assistant.svelte";
  import Settings from "./lib/Settings.svelte";
  import { init, loadSuggestions, loadAiConfig, loadEmailConfig, loadSyncStatus, loadGeneralSettings, loadNotifPrefs, ensureRange, taskDetail, openTaskDetail, closeTaskDetail, applySavedTheme, loadUiPrefs, applyUiPrefs, tasks, aiConfig, setAssistantDraft } from "./lib/data.svelte.ts";
  import TaskDrawer from "./lib/TaskDrawer.svelte";
  import ContextualToast from "./lib/ContextualToast.svelte";

  let view = $state<"mes" | "semana" | "dia" | "agenda" | "sugerencias" | "asistente" | "ajustes">("semana");
  let date = $state(new Date());
  let hash = $state(window.location.hash);
  let isWidget = $state(false);

  applySavedTheme();

  onMount(() => {
    init();
    loadSuggestions();
    loadAiConfig();
    loadEmailConfig();
    loadSyncStatus();
    loadUiPrefs();
    loadNotifPrefs();
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
    const un2 = listen("nav:agenda", () => {
      view = "agenda";
    });
    const un3 = listen("nav:assistant", () => {
      view = "asistente";
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

  function setView(v: "mes" | "semana" | "dia" | "agenda" | "sugerencias" | "asistente" | "ajustes") {
    view = v;
    if (v === "sugerencias") loadSuggestions();
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
    else d.setDate(d.getDate() + dir * 7);
    date = d;
  }
  function goToday() {
    date = new Date();
  }
  function selectDate(d: Date) {
    date = d;
    view = "dia";
  }

  $effect(() => {
    if (view === "agenda" || view === "sugerencias" || view === "asistente" || view === "ajustes") return;
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
{:else}
  <div class="app">
    <TitleBar />
    <div class="body">
      <Sidebar {view} {setView} {navigate} />
      <main class="content">
        <TopBar {date} {view} {navigate} {goToday} />
        {#if view === "agenda"}
          <Agenda />
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
            <Settings />
          </div>
        {:else}
          <div class="cal-wrap">
            {#key view + date.toDateString()}
              <div transition:fade={{ duration: 160 }}>
                <Calendar {view} {date} onSelectDate={selectDate} />
              </div>
            {/key}
          </div>
        {/if}
      </main>
    </div>
    {#if taskDetail()}
      <TaskDrawer />
    {/if}
    {#if !isWidget}
      <ContextualToast onplan={planFromNotif} />
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
  .content {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
  }
  .cal-wrap {
    flex: 1;
    padding: 0 var(--s-8) var(--s-8);
    min-height: 0;
    display: flex;
    flex-direction: column;
    /* La vista mes tiene altura fija: si la resolución no alcanza, el scroll
       aparece SOLO dentro del área del calendario (sidebar/header fijos). */
    overflow-y: auto;
  }
  .cal-wrap :global(.cal) {
    flex: 1;
  }
  .page-wrap {
    flex: 1;
    overflow-y: auto;
    padding: 0 var(--s-8) var(--s-8);
    min-height: 0;
  }
  .assistant-wrap {
    display: flex;
    flex-direction: column;
    overflow-y: auto;
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
