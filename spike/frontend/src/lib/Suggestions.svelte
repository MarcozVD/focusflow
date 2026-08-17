<script lang="ts">
  import {
    suggestions as suggestionsStore,
    tasks as tasksStore,
    categories,
    cat,
    fmtDate,
    fmtMs,
    suggestionAccept,
    suggestionReject,
    suggestionEdit,
    suggestionMerge,
    suggestionRevert,
    suggestionDelete,
    syncNow,
    syncRunning,
    type Suggestion,
    KIND_LABELS,
  } from "./data.svelte";

  const suggestions = $derived(suggestionsStore());
  const tasks = $derived(
    [...tasksStore()].sort((a, b) => (a.start_at ?? 0) - (b.start_at ?? 0)),
  );

  let editing = $state<number | null>(null);
  let mergeFor = $state<number | null>(null);
  let mergeTask = $state("");

  let eTitle = $state("");
  let eCat = $state("otr");
  let ePrio = $state("media");
  let eDate = $state("");
  let eStart = $state("");
  let eEnd = $state("");
  let eDesc = $state("");

  function iso(ms: number): string {
    const d = new Date(ms);
    return d.toISOString().slice(0, 10);
  }
  function hm(ms: number): string {
    const d = new Date(ms);
    return `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
  }

  function startEdit(s: Suggestion) {
    editing = s.id;
    eTitle = s.title;
    eCat = s.category_id;
    ePrio = s.priority;
    eDate = s.start_at ? iso(s.start_at) : iso(Date.now() + 86_400_000);
    eStart = s.start_at ? hm(s.start_at) : "09:00";
    eEnd = s.end_at ? hm(s.end_at) : "10:00";
    eDesc = s.description;
  }

  function startMerge(s: Suggestion) {
    // siempre se elige la tarea destino; si el aviso ya marca una (duplicado
    // detectado), queda preseleccionada y se puede cambiar por cualquier otra
    mergeFor = s.id;
    const suggested = s.dedupe_task_id;
    mergeTask =
      suggested != null && tasks.some((t) => t.id === suggested) ? String(suggested) : "";
  }

  async function saveEdit() {
    if (!editing) return;
    const [y, m, d] = eDate.split("-").map(Number);
    const [sh, sm] = eStart.split(":").map(Number);
    const [eh, em] = eEnd.split(":").map(Number);
    const startAt = new Date(y, m - 1, d, sh, sm).getTime();
    let endAt = new Date(y, m - 1, d, eh, em).getTime();
    if (endAt < startAt) endAt = startAt + 3_600_000;
    await suggestionEdit(editing, {
      title: eTitle,
      categoryId: eCat,
      priority: ePrio,
      startAt,
      endAt,
      description: eDesc,
      allDay: startAt === endAt,
    });
    editing = null;
  }

  async function doMerge() {
    if (!mergeFor || !mergeTask) return;
    await suggestionMerge(mergeFor, Number(mergeTask));
    mergeFor = null;
    mergeTask = "";
  }

  const statusLabel: Record<string, string> = {
    pending: "Pendiente",
    accepted: "Aceptada",
    rejected: "Rechazada",
    merged: "Fusionada",
    auto_approved: "Auto-aprobada",
  };

  const settled = $derived(
    (s: Suggestion) => s.status === "accepted" || s.status === "rejected" || s.status === "merged" || s.status === "auto_approved",
  );

  function remainMs(s: Suggestion): string {
    const remain = Math.max(0, s.updated_at + 3_600_000 - Date.now());
    const min = Math.ceil(remain / 60_000);
    return min <= 1 ? "1 min" : `${min} min`;
  }
</script>

<div class="sug">
  <div class="head">
    <div class="head-left">
      <h2>Eventos detectados</h2>
      <p class="hint">
        Eventos extraídos de tus correos por la IA. Revisa antes de añadirlos al calendario.
      </p>
    </div>
    <button class="btn check-now" onclick={syncNow} disabled={syncRunning()}>
      {syncRunning() ? "Comprobando…" : "Comprobar correo ahora"}
    </button>
  </div>

  {#if suggestions.length === 0}
    <div class="empty">
      <p>Sin eventos detectados todavía.</p>
      <p class="sub">
        Conecta tu correo en Ajustes y la app te avisará aquí cuando encuentre tareas, exámenes o reuniones.
      </p>
    </div>
  {/if}

  {#each suggestions as s (s.id)}
    {#if editing === s.id}
      <div class="card edit">
        <input class="t" bind:value={eTitle} placeholder="Título" />
        <textarea bind:value={eDesc} placeholder="Descripción" rows="2"></textarea>
        <div class="grid">
          <label>Fecha
            <input type="date" bind:value={eDate} />
          </label>
          <label>Inicio
            <input type="time" bind:value={eStart} />
          </label>
          <label>Fin
            <input type="time" bind:value={eEnd} />
          </label>
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
        <div class="row">
          <button class="btn primary" onclick={saveEdit}>Guardar</button>
          <button class="btn" onclick={() => (editing = null)}>Cancelar</button>
        </div>
      </div>
    {:else if mergeFor === s.id}
      <div class="card edit">
        <p class="mt">Fusionar <strong>{s.title}</strong> con una tarea existente:</p>
        <select class="t" bind:value={mergeTask}>
          <option value="">Selecciona la tarea…</option>
          {#each tasks as t (t.id)}
            <option value={t.id}>
              {t.title}{t.start_at ? ` · ${fmtDate(t.start_at)}` : ""}{s.dedupe_task_id === t.id ? " (sugerida)" : ""}
            </option>
          {/each}
        </select>
        <div class="row">
          <button class="btn primary" onclick={doMerge} disabled={!mergeTask}>Fusionar</button>
          <button class="btn" onclick={() => (mergeFor = null)}>Cancelar</button>
        </div>
      </div>
    {:else}
      <div class="card">
        <div class="top">
          <span class="kind"><span class="kdot" aria-hidden="true"></span>{KIND_LABELS[s.kind] ?? "Evento"}</span>
          <span class="status {s.status}">{statusLabel[s.status] ?? s.status}</span>
          {#if s.source_sender}
            <span class="sender">{s.source_sender}</span>
          {/if}
          <span class="conf">confianza {Math.round(s.confidence * 100)}%</span>
        </div>
        <h3>{s.title}</h3>
        {#if s.description}
          <p class="desc">{s.description}</p>
        {/if}
        {#if s.reason}
          <p class="reason">{s.reason}</p>
        {/if}
        <div class="meta">
          <span class="chip" style="--c: {cat(s.category_id).color}">
            {cat(s.category_id).name}
          </span>
          {#if s.kind === "deadline" && s.deadline_at}
            <span class="deadline">Vence {fmtDate(s.deadline_at)} {fmtMs(s.deadline_at)}</span>
          {:else if s.kind === "availability" && s.start_at && s.end_at}
            <span class="range">{fmtDate(s.start_at)} → {fmtDate(s.end_at)}</span>
          {:else if s.start_at}
            <span>{fmtDate(s.start_at)} · {fmtMs(s.start_at)}</span>
          {/if}
          {#if s.prep_min > 0}
            <span class="prep">prep {s.prep_min} min</span>
          {/if}
          {#if s.priority === "alta"}
            <span class="prio">Prioridad alta</span>
          {/if}
        </div>
        {#if s.dedupe_note}
          <p class="dupe">Aviso: {s.dedupe_note}</p>
        {/if}
        {#if s.status === "pending"}
          <div class="row">
            <button class="btn primary" onclick={() => suggestionAccept(s.id)}>Aceptar</button>
            <button class="btn" onclick={() => startEdit(s)}>Editar</button>
            <button class="btn" onclick={() => startMerge(s)}>Fusionar</button>
            <button class="btn danger" onclick={() => suggestionReject(s.id)}>Rechazar</button>
            <button class="btn danger ghost" title="Eliminar definitivamente" onclick={() => suggestionDelete(s.id)}>
              Borrar
            </button>
          </div>
        {:else if settled(s)}
          <div class="row settled-row">
            <span class="settled-note">
              Esta acción estará disponible {remainMs(s)} · puedes revertirla o editarla.
            </span>
            <span class="spacer"></span>
            <button class="btn" onclick={() => startEdit(s)}>Editar</button>
            <button class="btn ghost" onclick={() => suggestionRevert(s.id)}>Revertir</button>
            <button class="btn danger ghost" onclick={() => suggestionDelete(s.id)}>Borrar</button>
          </div>
        {/if}
      </div>
    {/if}
  {/each}
</div>

<style>
  .sug {
    max-width: 760px;
    display: flex;
    flex-direction: column;
    gap: var(--s-4);
    padding-bottom: var(--s-8);
  }
  .head h2 {
    font-size: 20px;
    margin: 0 0 4px;
  }
  .head {
    display: flex;
    align-items: flex-start;
    justify-content: space-between;
    gap: var(--s-4);
    flex-wrap: wrap;
  }
  .head-left {
    min-width: 0;
  }
  .check-now {
    flex-shrink: 0;
  }
  .hint,
  .sub {
    color: var(--text-3);
    font-size: 13px;
    margin: 0;
  }
  .empty {
    background: var(--surface-2);
    border-radius: var(--r-lg);
    padding: var(--s-8);
    text-align: center;
  }
  .empty p:first-child {
    font-weight: 600;
  }
  .card {
    background: var(--surface);
    border-radius: var(--r-lg);
    box-shadow: var(--shadow-raised);
    padding: var(--s-5);
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .card.edit {
    box-shadow: var(--shadow-raised), inset 0 0 0 2px var(--primary-soft-2);
    border: none;
  }
  .top {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 12px;
    color: var(--text-3);
  }
  .status {
    font-weight: 700;
    font-size: 11px;
    padding: 3px 10px;
    border-radius: var(--r-full);
    background: var(--surface-3);
    color: var(--text-2);
  }
  .kind {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-weight: 700;
    font-size: 11px;
    padding: 3px 10px;
    border-radius: var(--r-full);
    background: var(--primary-soft);
    color: var(--primary);
  }
  .kind .kdot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    background: currentColor;
    flex-shrink: 0;
  }
  .status.pending {
    background: var(--primary-soft);
    color: var(--primary);
  }
  .status.auto_approved {
    background: color-mix(in srgb, var(--success) 15%, var(--surface));
    color: var(--success);
  }
  .status.accepted {
    background: color-mix(in srgb, var(--success) 15%, var(--surface));
    color: var(--success);
  }
  .status.merged {
    background: color-mix(in srgb, var(--primary) 15%, var(--surface));
    color: var(--primary);
  }
  .status.rejected {
    background: color-mix(in srgb, var(--danger) 12%, var(--surface));
    color: var(--danger);
    opacity: 1;
  }
  .settled-row {
    align-items: center;
    background: var(--surface-2);
    border-radius: 12px;
    padding: 8px 10px;
    margin-top: 2px;
  }
  .settled-note {
    font-size: 12px;
    color: var(--text-3);
  }
  .spacer {
    flex: 1;
  }
  .btn.ghost {
    background: transparent;
    box-shadow: none;
    border: none;
  }
  .btn.ghost:hover {
    background: var(--surface-2);
    transform: none;
  }
  .conf {
    margin-left: auto;
  }
  h3 {
    margin: 0;
    font-size: 15px;
  }
  .desc,
  .reason {
    margin: 0;
    font-size: 13px;
    color: var(--text-2);
  }
  .reason {
    font-style: italic;
  }
  .meta {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 12.5px;
    color: var(--text-2);
    flex-wrap: wrap;
  }
  .chip {
    font-weight: 600;
    color: color-mix(in srgb, var(--c) 60%, var(--text-1));
    background: color-mix(in srgb, var(--c) 13%, var(--surface));
    border-radius: var(--r-full);
    padding: 3px 10px;
  }
  .prio {
    color: var(--danger);
    font-weight: 600;
  }
  .deadline,
  .range {
    font-weight: 600;
  }
  .prep {
    color: var(--text-3);
  }
  .dupe {
    margin: 0;
    font-size: 12px;
    color: var(--warning);
  }
  .row {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
    margin-top: 4px;
  }
  .btn {
    border: none;
    background: var(--surface-2);
    color: var(--text-1);
    border-radius: 12px;
    padding: 8px 16px;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
    transition: all var(--dur-fast) var(--ease-out);
  }
  .btn:hover {
    background: var(--surface-3);
  }
  .btn:active {
    transform: scale(0.98);
  }
  .btn.primary {
    background: var(--primary);
    color: #fff;
  }
  .btn.primary:hover {
    background: var(--primary-hover);
  }
  .btn.danger {
    color: var(--danger);
  }
  .btn.danger:hover {
    background: var(--danger-bg);
  }
  .btn:disabled {
    opacity: 0.5;
    pointer-events: none;
  }
  .t {
    width: 100%;
    border: none;
    background: var(--surface-3);
    box-shadow: var(--shadow-inset-sm);
    border-radius: 12px;
    padding: 10px 14px;
    font-size: 14px;
    color: var(--text-1);
    font-family: inherit;
    outline: none;
    transition: box-shadow var(--dur-fast) var(--ease-out);
  }
  .t:focus {
    box-shadow: var(--shadow-inset-sm), inset 0 0 0 2px var(--primary-soft-2);
  }
  textarea {
    resize: vertical;
    font-family: inherit;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(130px, 1fr));
    gap: 10px;
  }
  .grid label {
    display: flex;
    flex-direction: column;
    gap: 4px;
    font-size: 12px;
    color: var(--text-3);
    font-weight: 600;
  }
  .grid input,
  .grid select {
    border: none;
    background: var(--surface-3);
    box-shadow: var(--shadow-inset-sm);
    border-radius: 10px;
    padding: 8px 10px;
    color: var(--text-1);
    font-family: inherit;
    outline: none;
    transition: box-shadow var(--dur-fast) var(--ease-out);
  }
  .grid input:focus,
  .grid select:focus {
    box-shadow: var(--shadow-inset-sm), inset 0 0 0 2px var(--primary-soft-2);
  }
  .mt {
    margin: 0;
    font-size: 13px;
    color: var(--text-2);
  }
</style>
