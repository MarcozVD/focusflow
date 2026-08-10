<script lang="ts">
  import { fade, slide } from "svelte/transition";
  import {
    taskDetail,
    closeTaskDetail,
    categories,
    cat,
    completeTask,
    updateTaskDetail,
    deleteTask,
    duplicateTask,
    type Task,
  } from "./data.svelte";

  const detail = $derived(taskDetail());

  let eTitle = $state("");
  let eDesc = $state("");
  let eCat = $state("otr");
  let ePrio = $state("media");
  let eTags = $state("");
  let eSDate = $state("");
  let eSTime = $state("09:00");
  let eEDate = $state("");
  let eETime = $state("10:00");
  let eReminder = $state("");
  let eNotes = $state("");
  let eLinks = $state("");
  let eAllDay = $state(false);
  let saving = $state(false);
  let feedback = $state("");
  let confirmOpen = $state(false);

  function iso(d: Date): string {
    return `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")}`;
  }
  function hm(d: Date): string {
    return `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
  }

  $effect(() => {
    const t = detail;
    if (!t) return;
    eTitle = t.title;
    eDesc = t.description ?? "";
    eCat = t.categoryId;
    ePrio = t.priority;
    eTags = (t.tags ?? []).join(", ");
    eSDate = iso(t.start);
    eSTime = hm(t.start);
    eEDate = iso(t.end);
    eETime = hm(t.end);
    eReminder = t.reminderMinutes != null ? String(t.reminderMinutes) : "";
    eNotes = t.notes ?? "";
    eLinks = (t.links ?? []).join(", ");
    eAllDay = t.allDay ?? false;
    feedback = "";
  });

  function at(dateStr: string, timeStr: string): number {
    const [y, m, d] = dateStr.split("-").map(Number);
    const [hh, mm] = timeStr.split(":").map(Number);
    return new Date(y, m - 1, d, hh, mm).getTime();
  }

  async function save() {
    const t = detail;
    if (!t) return;
    saving = true;
    let startAt: number;
    let endAt: number;
    if (eAllDay) {
      const [y, m, d] = eSDate.split("-").map(Number);
      startAt = new Date(y, m - 1, d).getTime();
      endAt = startAt;
    } else {
      startAt = at(eSDate, eSTime);
      endAt = at(eEDate, eETime);
      if (endAt <= startAt) endAt = startAt + 3_600_000;
    }
    const r = await updateTaskDetail(t.id, {
      title: eTitle.trim() || t.title,
      description: eDesc,
      categoryId: eCat,
      priority: ePrio,
      startAt,
      endAt,
      allDay: eAllDay,
      tags: eTags.split(",").map((s) => s.trim()).filter(Boolean),
      notes: eNotes,
      links: eLinks.split(",").map((s) => s.trim()).filter(Boolean),
      reminderMinutes: eReminder ? Number(eReminder) : null,
    });
    saving = false;
    feedback = r.ok ? "Guardado ✓" : `Error: ${r.error ?? "desconocido"}`;
  }

  async function toggleDone() {
    const t = detail;
    if (!t) return;
    await completeTask(t.id);
    feedback = t.status === "completada" ? "Reabierta" : "Completada ✓";
  }

  async function duplicate() {
    const t = detail;
    if (!t) return;
    const r = await duplicateTask(t.id);
    if (r.ok) feedback = "Tarea duplicada ✓";
    else feedback = `Error: ${r.error}`;
  }

  async function remove() {
    confirmOpen = true;
  }

  async function confirmDelete() {
    const t = detail;
    if (!t) return;
    confirmOpen = false;
    const r = await deleteTask(t.id);
    if (r.ok) closeTaskDetail();
    else feedback = `Error: ${r.error}`;
  }
</script>

{#if detail}
  <div class="overlay" onclick={closeTaskDetail} transition:fade={{ duration: 140 }}></div>
  <aside class="drawer" transition:slide={{ duration: 200, axis: "x" }}>
    <header class="head">
      <div class="head-info">
        <span class="cat-dot" style="--c: {cat(detail.categoryId).color}"></span>
        <strong>Detalle de tarea</strong>
      </div>
      <button class="x" onclick={closeTaskDetail} aria-label="Cerrar">✕</button>
    </header>

    <div class="body">
      <label>Título
        <input class="t" bind:value={eTitle} placeholder="Título de la tarea" />
      </label>

      <label>Descripción
        <textarea bind:value={eDesc} rows="3" placeholder="Detalles de la tarea…"></textarea>
      </label>

      <div class="grid2">
        <label>Categoría
          <select bind:value={eCat}>
            {#each categories as c}
              <option value={c.id}>{c.name}</option>
            {/each}
          </select>
        </label>
        <label>Prioridad
          <select bind:value={ePrio}>
            <option value="alta">Alta</option>
            <option value="media">Media</option>
            <option value="baja">Baja</option>
          </select>
        </label>
      </div>

      <label class="check" title="Sin hora fija: ocupa el día completo">
        <input type="checkbox" bind:checked={eAllDay} />
        Todo el día
      </label>
      <div class="grid2">
        <label>Inicio — fecha
          <input type="date" bind:value={eSDate} />
        </label>
        <label>Inicio — hora
          <input type="time" bind:value={eSTime} disabled={eAllDay} />
        </label>
      </div>
      <div class="grid2">
        <label>Fin — fecha
          <input type="date" bind:value={eEDate} disabled={eAllDay} />
        </label>
        <label>Fin — hora
          <input type="time" bind:value={eETime} disabled={eAllDay} />
        </label>
      </div>

      <div class="grid2">
        <label>Recordatorio
          <select bind:value={eReminder}>
            <option value="">Sin recordatorio</option>
            <option value="5">5 min antes</option>
            <option value="10">10 min antes</option>
            <option value="30">30 min antes</option>
            <option value="60">1 h antes</option>
            <option value="120">2 h antes</option>
            <option value="1440">1 día antes</option>
          </select>
        </label>
        <label>Etiquetas
          <input type="text" bind:value={eTags} placeholder="examen, importante" />
        </label>
      </div>

      <label>Notas
        <textarea bind:value={eNotes} rows="2" placeholder="Notas personales…"></textarea>
      </label>

      <label>Enlaces
        <input type="text" bind:value={eLinks} placeholder="https://… (separados por coma)" />
      </label>
    </div>

    <footer class="foot">
      {#if feedback}
        <span class="fb">{feedback}</span>
      {/if}
      <div class="actions">
        <button class="btn ghost" onclick={toggleDone}>
          {detail.status === "completada" ? "Reabrir" : "Completar"}
        </button>
        <button class="btn ghost" onclick={duplicate}>Duplicar</button>
        <button class="btn danger" onclick={remove}>Eliminar</button>
        <button class="btn primary" onclick={save} disabled={saving}>
          {saving ? "Guardando…" : "Guardar"}
        </button>
      </div>
    </footer>
  </aside>

  {#if confirmOpen && detail}
    <div class="dlg-overlay" onclick={() => (confirmOpen = false)} transition:fade={{ duration: 120 }}>
      <div class="dlg" role="dialog" aria-modal="true" aria-labelledby="dlg-title"
        onclick={(e) => e.stopPropagation()}>
        <div class="dlg-icon" aria-hidden="true">
          <svg viewBox="0 0 24 24" width="22" height="22" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
            <path d="M3 6h18" />
            <path d="M8 6V4a1 1 0 0 1 1-1h6a1 1 0 0 1 1 1v2" />
            <path d="M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6" />
            <path d="M10 11v6M14 11v6" />
          </svg>
        </div>
        <h3 id="dlg-title">Eliminar tarea</h3>
        <p>¿Deseas eliminar esta tarea?</p>
        <p class="dlg-title-name">"{detail.title}"</p>
        <div class="dlg-actions">
          <button class="btn ghost" onclick={() => (confirmOpen = false)}>Cancelar</button>
          <button class="btn danger-solid" onclick={confirmDelete}>Eliminar</button>
        </div>
      </div>
    </div>
  {/if}
{/if}

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: rgba(15, 18, 24, 0.32);
    backdrop-filter: blur(2px);
    z-index: 90;
  }
  .drawer {
    position: fixed;
    top: 0;
    right: 0;
    bottom: 0;
    width: min(400px, 100vw);
    background: var(--surface);
    box-shadow: -12px 0 36px -8px rgba(31, 41, 55, 0.25);
    z-index: 95;
    display: flex;
    flex-direction: column;
    border-left: 1px solid var(--border);
  }
  .head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: var(--s-4) var(--s-5);
    border-bottom: 1px solid var(--border);
    flex-shrink: 0;
  }
  .head-info {
    display: flex;
    align-items: center;
    gap: 10px;
  }
  .head-info strong {
    font-size: 15px;
  }
  .cat-dot {
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: var(--c);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--c) 20%, transparent);
  }
  .x {
    width: 32px;
    height: 32px;
    border: none;
    background: var(--surface-2);
    color: var(--text-2);
    border-radius: 10px;
    font-size: 13px;
    transition: all var(--dur-fast) var(--ease-out);
  }
  .x:hover {
    color: var(--danger);
    background: var(--danger-bg);
  }
  .body {
    flex: 1;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: var(--s-3);
    padding: var(--s-4) var(--s-5);
  }
  label {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 11.5px;
    font-weight: 700;
    color: var(--text-3);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }
  label.check {
    flex-direction: row;
    align-items: center;
    gap: 8px;
    text-transform: none;
    letter-spacing: normal;
    font-size: 13px;
    font-weight: 600;
    color: var(--text-1);
    cursor: pointer;
    padding: 2px 0;
  }
  label.check input {
    width: auto;
  }
  input:disabled {
    opacity: 0.45;
    cursor: not-allowed;
  }
  input,
  select,
  textarea {
    border: 1px solid var(--border);
    background: var(--surface-3);
    border-radius: 11px;
    padding: 9px 12px;
    font-size: 13.5px;
    color: var(--text-1);
    font-family: inherit;
    outline: none;
    width: 100%;
    text-transform: none;
    letter-spacing: normal;
    font-weight: 400;
  }
  input:focus,
  select:focus,
  textarea:focus {
    border-color: var(--primary);
    box-shadow: 0 0 0 3px var(--primary-soft);
  }
  textarea {
    resize: vertical;
    font-weight: 400;
    text-transform: none;
  }
  .grid2 {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: var(--s-3);
  }
  .foot {
    border-top: 1px solid var(--border);
    padding: var(--s-4) var(--s-5);
    display: flex;
    flex-direction: column;
    gap: 8px;
    flex-shrink: 0;
  }
  .fb {
    font-size: 12px;
    font-weight: 600;
    color: var(--text-2);
  }
  .actions {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
  }
  .btn {
    border: none;
    background: var(--surface-3);
    color: var(--text-1);
    border-radius: 12px;
    padding: 9px 14px;
    font-size: 13px;
    font-weight: 600;
    transition: all var(--dur-fast) var(--ease-out);
    flex: 1;
  }
  .btn:hover {
    transform: translateY(-1px);
    box-shadow: var(--e1);
  }
  .btn.primary {
    background: var(--primary);
    color: #fff;
  }
  .btn.ghost {
    background: transparent;
    border: 1px solid var(--border);
  }
  .btn.danger {
    color: var(--danger);
  }
  .btn:disabled {
    opacity: 0.6;
    pointer-events: none;
  }
  .dlg-overlay {
    position: fixed;
    inset: 0;
    background: rgba(15, 18, 24, 0.4);
    backdrop-filter: blur(3px);
    z-index: 120;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 24px;
  }
  .dlg {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: 18px;
    box-shadow: 0 18px 48px -10px rgba(15, 18, 24, 0.45);
    padding: 24px 26px;
    width: min(320px, 100%);
    text-align: center;
    animation: pop var(--dur-fast) var(--ease-out);
  }
  @keyframes pop {
    from {
      opacity: 0;
      transform: scale(0.94) translateY(6px);
    }
    to {
      opacity: 1;
      transform: scale(1) translateY(0);
    }
  }
  .dlg-icon {
    width: 48px;
    height: 48px;
    border-radius: 50%;
    background: var(--danger-bg);
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 20px;
    margin: 0 auto 12px;
  }
  .dlg h3 {
    margin: 0 0 6px;
    font-size: 16px;
  }
  .dlg p {
    margin: 0 0 4px;
    font-size: 13px;
    color: var(--text-2);
  }
  .dlg-title-name {
    font-weight: 700;
    color: var(--text-1);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
  .dlg-actions {
    display: flex;
    gap: 10px;
    margin-top: 18px;
  }
  .btn.danger-solid {
    background: var(--danger);
    color: #fff;
  }
</style>
