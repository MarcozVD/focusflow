<script lang="ts">
  import TopBar from "./lib/TopBar.svelte";
  import Sidebar from "./lib/Sidebar.svelte";
  import Calendar from "./lib/Calendar.svelte";
  import Agenda from "./lib/Agenda.svelte";
  import WidgetPage from "./lib/WidgetPage.svelte";

  let view = $state("semana");
  let date = $state(new Date());
  let hash = $state(window.location.hash);

  if (hash === "#/dark") document.documentElement.dataset.theme = "dark";

  window.addEventListener("hashchange", () => (hash = window.location.hash));
  window.addEventListener("keydown", (e) => {
    if (e.key === "Escape" && hash === "#/widget") window.location.hash = "";
  });

  function setView(v: string) {
    if (v === "agenda") view = v;
    else view = v;
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
  function showWidget() {
    window.location.hash = "#/widget";
  }
</script>

{#if hash === "#/widget"}
  <WidgetPage />
{:else}
  <div class="app">
    <Sidebar {view} {setView} {navigate} {showWidget} />
    <main class="content">
      <TopBar {date} {view} {navigate} {goToday} />
      {#if view === "agenda"}
        <Agenda />
      {:else}
        <div class="cal-wrap">
          <Calendar {view} {date} onSelectDate={selectDate} />
        </div>
      {/if}
    </main>
  </div>
{/if}

<style>
  .app {
    display: flex;
    height: 100vh;
    overflow: hidden;
  }
  .content {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
  }
  .cal-wrap {
    flex: 1;
    padding: 0 var(--s-8) var(--s-8);
    min-height: 0;
    display: flex;
    flex-direction: column;
  }
  .cal-wrap :global(.cal) {
    flex: 1;
  }
</style>
