<script lang="ts">
  import { DAYS_ES, MONTHS_ES, tasks, cat, type Task } from "./data.svelte";

  let {
    view,
    date,
    onSelectDate,
  }: { view: "mes" | "semana" | "dia"; date: Date; onSelectDate: (d: Date) => void } = $props();

  const PX_HOUR = 56;
  const START_HOUR = 7;

  function startOfDay(d: Date): Date {
    const x = new Date(d);
    x.setHours(0, 0, 0, 0);
    return x;
  }

  function weekDays(anchor: Date): Date[] {
    const s = startOfDay(anchor);
    const dow = (s.getDay() + 6) % 7; // lunes = 0
    s.setDate(s.getDate() - dow);
    return Array.from({ length: 7 }, (_, i) => {
      const d = new Date(s);
      d.setDate(s.getDate() + i);
      return d;
    });
  }

  const days = $derived(view === "semana" ? weekDays(date) : [startOfDay(date)]);
  const isToday = $derived((d: Date) => d.toDateString() === new Date().toDateString());

  function dayTasks(d: Date): Task[] {
    return tasks.filter((t) => {
      const s = t.start;
      return s.getFullYear() === d.getFullYear() && s.getMonth() === d.getMonth() && s.getDate() === d.getDate();
    });
  }

  function topOf(t: Task): number {
    const mins = t.start.getHours() * 60 + t.start.getMinutes();
    return (mins - START_HOUR * 60) * (PX_HOUR / 60);
  }
  function nowTop(): number {
    const n = new Date();
    const mins = n.getHours() * 60 + n.getMinutes();
    return (mins - START_HOUR * 60) * (PX_HOUR / 60);
  }
  function heightOf(t: Task): number {
    const durMin = Math.max(30, (t.end.getTime() - t.start.getTime()) / 60000);
    return durMin * (PX_HOUR / 60);
  }

  const hours = Array.from({ length: 14 }, (_, i) => START_HOUR + i);

  // ---- mes ----
  function monthCells(): { d: Date; inMonth: boolean }[] {
    const first = new Date(date.getFullYear(), date.getMonth(), 1);
    const start = startOfDay(first);
    const offset = (start.getDay() + 6) % 7;
    start.setDate(start.getDate() - offset);
    return Array.from({ length: 42 }, (_, i) => {
      const d = new Date(start);
      d.setDate(start.getDate() + i);
      return { d, inMonth: d.getMonth() === date.getMonth() };
    });
  }

  function hourLabel(i: number): string {
    const h = i < 12 ? (i === 0 ? 12 : i) : i - 12;
    const ampm = i < 12 ? "a" : "p";
    return `${h} ${ampm}`;
  }
</script>

<div class="cal" class:week={view === "semana"} class:day={view === "dia"}>
  {#if view === "mes"}
    <div class="month-head">
      {#each Array.from({ length: 7 }, (_, i) => i) as i}
        <span>{DAYS_ES[(i + 1) % 7]}</span>
      {/each}
    </div>
    <div class="month-grid">
      {#each monthCells() as { d, inMonth } (d.toDateString())}
        <button
          class="cell {inMonth ? '' : 'outside'} {isToday(d) ? 'today' : ''}"
          onclick={() => onSelectDate(d)}
        >
          <span class="daynum">{d.getDate()}</span>
          <div class="chips">
            {#each dayTasks(d).slice(0, 3) as t}
              <span
                class="minichip {t.status === 'completada' ? 'done' : ''}"
                style="--c: {cat(t.categoryId).color}"
              >
                {t.title}
              </span>
            {/each}
            {#if dayTasks(d).length > 3}
              <span class="more">+{dayTasks(d).length - 3} más</span>
            {/if}
          </div>
        </button>
      {/each}
    </div>
  {:else}
    <div class="week-head">
      <span class="gutter-spacer"></span>
      {#each days as d (d.toDateString())}
        <button class="day-head {isToday(d) ? 'today' : ''}" onclick={() => onSelectDate(d)}>
          <span class="dow">{DAYS_ES[d.getDay()]}</span>
          <span class="num">{d.getDate()}</span>
        </button>
      {/each}
    </div>
    <div class="week-body">
      <div class="gutter">
        {#each hours as h}
          <span class="hour">{hourLabel(h)}</span>
        {/each}
      </div>
      {#each days as d (d.toDateString())}
        <div class="day-col {isToday(d) ? 'today' : ''}">
          {#each hours as h}
            <div class="slot"></div>
          {/each}
          <div class="now-line" style="top: {nowTop()}px"></div>
          {#each dayTasks(d) as t}
            <div
              class="evt {t.status === 'completada' ? 'done' : ''}"
              style="top: {topOf(t)}px; height: {heightOf(t)}px; --c: {cat(t.categoryId).color}"
            >
              <span class="evt-time">{t.start.getHours().toString().padStart(2, "0")}:{t.start.getMinutes().toString().padStart(2, "0")}</span>
              <span class="evt-title">{t.title}</span>
            </div>
          {/each}
        </div>
      {/each}
    </div>
  {/if}
</div>

<style>
  .cal {
    background: var(--surface);
    border-radius: var(--r-lg);
    box-shadow: var(--shadow-raised);
    overflow: hidden;
    min-width: 0;
  }
  .cal.week {
    display: flex;
    flex-direction: column;
    height: 100%;
  }

  /* ---------- mes ---------- */
  .month-head {
    display: grid;
    grid-template-columns: repeat(7, 1fr);
    padding: var(--s-4) var(--s-4) var(--s-2);
    font-size: 11px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--text-3);
    text-align: center;
  }
  .month-grid {
    display: grid;
    grid-template-columns: repeat(7, 1fr);
    grid-auto-rows: minmax(84px, 1fr);
    gap: 8px;
    padding: 0 var(--s-4) var(--s-4);
  }
  .cell {
    background: var(--surface-2);
    border: none;
    border-radius: var(--r-md);
    padding: 6px 8px;
    text-align: left;
    display: flex;
    flex-direction: column;
    gap: 4px;
    transition: transform var(--dur-fast) var(--ease-out), box-shadow var(--dur-fast) var(--ease-out);
    overflow: hidden;
  }
  .cell:hover {
    transform: translateY(-1px);
    box-shadow: var(--e1);
  }
  .cell.outside {
    opacity: 0.4;
  }
  .cell.today {
    box-shadow: inset 0 0 0 2px var(--primary-soft-2);
  }
  .daynum {
    font-size: 13px;
    font-weight: 600;
    color: var(--text-2);
    width: 26px;
    height: 26px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border-radius: var(--r-full);
  }
  .cell.today .daynum {
    background: var(--primary);
    color: #fff;
  }
  .chips {
    display: flex;
    flex-direction: column;
    gap: 2px;
    overflow: hidden;
  }
  .minichip {
    font-size: 11px;
    font-weight: 500;
    color: color-mix(in srgb, var(--c) 60%, var(--text-1));
    background: color-mix(in srgb, var(--c) 13%, var(--surface));
    border-radius: 8px;
    padding: 2px 7px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .minichip.done {
    text-decoration: line-through;
    opacity: 0.55;
  }
  .more {
    font-size: 10px;
    color: var(--text-3);
    padding: 0 4px;
  }

  /* ---------- semana / día ---------- */
  .week-head {
    display: flex;
    padding: var(--s-4) var(--s-4) var(--s-3);
    gap: 6px;
  }
  .gutter-spacer {
    width: 52px;
    flex-shrink: 0;
  }
  .day-head {
    flex: 1;
    border: none;
    background: transparent;
    border-radius: var(--r-md);
    padding: 6px 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
    transition: background var(--dur-fast) var(--ease-out);
  }
  .day-head:hover {
    background: var(--surface-2);
  }
  .day-head.today .num {
    background: var(--primary);
    color: #fff;
  }
  .dow {
    font-size: 11px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--text-3);
  }
  .num {
    font-size: 15px;
    font-weight: 700;
    font-variant-numeric: tabular-nums;
    width: 30px;
    height: 30px;
    display: inline-flex;
    align-items: center;
    justify-content: center;
    border-radius: var(--r-full);
    color: var(--text-1);
  }

  .week-body {
    flex: 1;
    display: flex;
    overflow-y: auto;
    padding: 0 var(--s-4) var(--s-4);
    gap: 6px;
  }
  .gutter {
    width: 52px;
    flex-shrink: 0;
    position: relative;
  }
  .hour {
    position: absolute;
    left: 0;
    font-size: 11px;
    font-weight: 600;
    color: var(--text-3);
    transform: translateY(-6px);
    font-variant-numeric: tabular-nums;
  }
  .day-col {
    flex: 1;
    position: relative;
    border-radius: var(--r-md);
    min-width: 0;
  }
  .day-col.today {
    box-shadow: inset 0 0 0 2px var(--primary-soft-2);
  }
  .slot {
    height: 56px;
    border-top: 1px solid var(--border);
    margin-left: 2px;
    margin-right: 2px;
  }
  .now-line {
    position: absolute;
    left: 2px;
    right: 2px;
    height: 2px;
    background: var(--primary);
    border-radius: var(--r-full);
    z-index: 2;
    pointer-events: none;
  }
  .now-line::after {
    content: "";
    position: absolute;
    left: -3px;
    top: -3px;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--primary);
  }
  .evt {
    position: absolute;
    left: 6px;
    right: 6px;
    background: color-mix(in srgb, var(--c) 13%, var(--surface));
    border-left: 4px solid var(--c);
    border-radius: var(--r-sm);
    padding: 4px 8px;
    display: flex;
    flex-direction: column;
    gap: 1px;
    overflow: hidden;
    z-index: 1;
    box-shadow: var(--shadow-inset-sm);
    transition: transform var(--dur-fast) var(--ease-out);
  }
  .evt:hover {
    transform: translateY(-1px) scale(1.01);
  }
  .evt.done {
    opacity: 0.55;
  }
  .evt.done .evt-title {
    text-decoration: line-through;
  }
  .evt-time {
    font-size: 10px;
    font-weight: 700;
    color: var(--text-2);
    font-variant-numeric: tabular-nums;
  }
  .evt-title {
    font-size: 12px;
    font-weight: 600;
    color: var(--text-1);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
</style>
