<script lang="ts">
  import { fade } from "svelte/transition";
  import {
    planProposal,
    planBusy,
    planError,
    planAccept,
    planReject,
    cat,
    fmtDate,
    fmtMs,
    type PlanProposalView,
    type PlanItemView,
    type EditedPlan,
  } from "./data.svelte";

  interface EditableBlock {
    startMs: number;
    endMs: number;
  }

  const proposal = $derived(planProposal());
  let editing = $state(false);
  let blocks = $state<EditableBlock[][]>([]);
  let localError = $state("");

  $effect(() => {
    if (!proposal) return;
    const h = (e: KeyboardEvent) => {
      if (e.key === "Escape") cancel();
    };
    window.addEventListener("keydown", h);
    return () => window.removeEventListener("keydown", h);
  });

  const typeLabel: Record<string, string> = {
    Event: "Evento",
    Task: "Tarea",
    Deadline: "Vencimiento",
    Preparation: "Preparación",
    Availability: "Disponibilidad",
    Reminder: "Recordatorio",
    Constraint: "Restricción",
  };

  function totalSessions(p: PlanProposalView): number {
    return p.items.reduce((n, i) => n + i.sessions.length, 0);
  }

  function iso(ms: number): string {
    const d = new Date(ms);
    return d.toISOString().slice(0, 10);
  }
  function hm(ms: number): string {
    const d = new Date(ms);
    return `${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
  }
  function fromParts(date: string, time: string): number {
    const [y, m, d] = date.split("-").map(Number);
    const [h, mi] = time.split(":").map(Number);
    return new Date(y, m - 1, d, h, mi).getTime();
  }

  function startEdit() {
    if (!proposal) return;
    editing = true;
    localError = "";
    blocks = proposal.items.map((it) =>
      it.sessions.map((s) => ({ startMs: s.start_ms, endMs: s.end_ms })),
    );
  }
  function stopEdit() {
    editing = false;
    localError = "";
  }

  function addBlock(itemIdx: number) {
    const b = blocks[itemIdx];
    const last = b[b.length - 1];
    const startMs = last ? last.endMs : Date.now() + 86_400_000;
    b.push({ startMs, endMs: startMs + 3_600_000 });
    blocks = [...blocks];
  }
  function removeBlock(itemIdx: number, blockIdx: number) {
    blocks[itemIdx] = blocks[itemIdx].filter((_, i) => i !== blockIdx);
    blocks = [...blocks];
  }

  function editedPlan(): EditedPlan {
    return { items: blocks.map((b) => b.map((x) => ({ start_ms: x.startMs, end_ms: x.endMs }))) };
  }

  async function accept() {
    if (!proposal) return;
    localError = "";
    if (editing) {
      for (const b of blocks.flat()) {
        if (b.endMs <= b.startMs) {
          localError = "Un bloque termina antes de empezar. Corrígelo.";
          return;
        }
        if (b.endMs - b.startMs < 15 * 60_000) {
          localError = "Un bloque dura menos de 15 minutos.";
          return;
        }
      }
      const r = await planAccept(proposal.id, editedPlan());
      if (!r.ok && r.error) localError = r.error;
      return;
    }
    const r = await planAccept(proposal.id);
    if (!r.ok && r.error) localError = r.error;
  }

  async function cancel() {
    if (!proposal) return;
    await planReject(proposal.id);
  }

  function warn(it: PlanItemView): string {
    if (it.complete) return "";
    const done = `${Math.round(it.planned_min / 60 * 10) / 10}`;
    const need = `${Math.round(it.required_min / 60 * 10) / 10}`;
    const txt = `Tiempo insuficiente: se planificaron ${done} de ${need} horas.`;
    const notes = it.notes.filter((n) => !n.includes("no hay tiempo"));
    return notes.length ? `${txt} ${notes.join(" ")}` : txt;
  }
</script>

{#if proposal}
  <div class="overlay" onclick={cancel} transition:fade={{ duration: 140 }}></div>
  <div class="modal" transition:fade={{ duration: 160 }}>
    <header class="head">
      <div class="head-info">
        <span class="logo">
          <svg width="15" height="15" viewBox="0 0 24 24" fill="none">
            <path d="M13 2L4.5 13.5H11L9.5 22L19 9.5H12.5L13 2Z" stroke="currentColor" stroke-width="2" stroke-linejoin="round" />
          </svg>
        </span>
        <div>
          <strong>Plan sugerido</strong>
          <span class="sub">La IA propone un horario. Revísalo antes de aceptar.</span>
        </div>
      </div>
      <span class="chip src">{proposal.source === "ai" ? "IA" : "Local"}</span>
      <button class="x" onclick={cancel} aria-label="Cancelar">✕</button>
    </header>

    <div class="body">
      <p class="quote">“{proposal.text}”</p>

      {#if proposal.understanding.length > 0}
        <section class="sec">
          <h4>Entendí</h4>
          {#each proposal.understanding as u}
            <div class="under">
              <span class="chip" style="--c: {cat(u.category_id).color}">{typeLabel[u.intent_type] ?? u.intent_type}</span>
              <div class="under-main">
                <strong>{u.title}</strong>
                <span class="when">{u.when_label}</span>
              </div>
              <div class="hours">
                {#if u.prep_min > 0}<span class="h">Preparación: {Math.round(u.prep_min / 60 * 10) / 10} h</span>{/if}
                {#if u.task_min > 0}<span class="h">Tarea: {Math.round(u.task_min / 60 * 10) / 10} h</span>{/if}
                {#if u.deadline}<span class="h dl">Vence {fmtDate(u.deadline)}</span>{/if}
              </div>
            </div>
          {/each}
        </section>
      {/if}

      <section class="sec">
        <h4>Plan propuesto</h4>

        {#if proposal.items.length === 0}
          <div class="empty">
            <p>No hay nada que planificar en este texto.</p>
            <p class="sub">Añade una duración (“2 horas”) o un vencimiento (“el viernes”).</p>
          </div>
        {/if}

        {#each proposal.items as it, itemIdx (itemIdx)}
          <div class="plan-item">
            <div class="item-head">
              <div class="item-title">
                <span class="chip" style="--c: {cat(it.category_id).color}">{typeLabel[it.intent_type] ?? it.intent_type}</span>
                <strong>{it.title}</strong>
                <span class="req">{Math.round(it.required_min / 60 * 10) / 10} h</span>
              </div>
              {#if it.complete}
                <span class="ok">Completo</span>
              {:else}
                <span class="warn">Parcial</span>
              {/if}
            </div>

            {#if warn(it)}
              <p class="warn-note">⚠ {warn(it)}</p>
            {/if}

            {#if editing}
              {#each blocks[itemIdx] ?? [] as b, bIdx (bIdx)}
                <div class="edit-row">
                  <input type="date" value={iso(b.startMs)} onchange={(e) => { b.startMs = fromParts(e.currentTarget.value, hm(b.startMs)); blocks = [...blocks]; }} />
                  <input type="time" value={hm(b.startMs)} onchange={(e) => { b.startMs = fromParts(iso(b.startMs), e.currentTarget.value); blocks = [...blocks]; }} />
                  <span class="arr">→</span>
                  <input type="time" value={hm(b.endMs)} onchange={(e) => { b.endMs = fromParts(iso(b.endMs), e.currentTarget.value); blocks = [...blocks]; }} />
                  <button class="mini" onclick={() => removeBlock(itemIdx, bIdx)} title="Quitar bloque">✕</button>
                </div>
              {/each}
              <button class="link" onclick={() => addBlock(itemIdx)}>+ Añadir bloque</button>
            {:else}
              {#each it.sessions as s (s.start_ms)}
                <div class="sess">
                  <span class="dot {s.is_prep ? 'prep' : ''}"></span>
                  <span class="day">{fmtDate(s.start_ms)}</span>
                  <span class="range">{fmtMs(s.start_ms)} – {new Date(s.end_ms).toLocaleTimeString("es-ES", { hour: "2-digit", minute: "2-digit" })}</span>
                  {#if s.is_prep}
                    <span class="badge">prep</span>
                  {/if}
                </div>
              {/each}
            {/if}
          </div>
        {/each}
      </section>
    </div>

    {#if planError() || localError}
      <p class="err">⚠ {localError || planError()}</p>
    {/if}

    <footer class="foot">
      <span class="total">{totalSessions(proposal)} bloque{totalSessions(proposal) === 1 ? "" : "s"} en {proposal.items.length} tarea{proposal.items.length === 1 ? "" : "s"}</span>
      <div class="row">
        {#if editing}
          <button class="btn" onclick={stopEdit} disabled={planBusy()}>Listo</button>
        {:else}
          <button class="btn" onclick={startEdit} disabled={planBusy() || proposal.items.length === 0}>Editar</button>
        {/if}
        <button class="btn ghost" onclick={cancel} disabled={planBusy()}>Cancelar</button>
        <button class="btn primary" onclick={accept} disabled={planBusy() || proposal.items.length === 0}>
          {planBusy() ? "Procesando…" : "Aceptar plan"}
        </button>
      </div>
    </footer>
  </div>
{/if}

<style>
  .overlay {
    position: fixed;
    inset: 0;
    background: color-mix(in srgb, var(--bg) 55%, transparent);
    backdrop-filter: blur(3px);
    z-index: 90;
  }
  .modal {
    position: fixed;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    width: min(560px, calc(100vw - 32px));
    max-height: min(720px, calc(100vh - 48px));
    display: flex;
    flex-direction: column;
    background: var(--surface);
    border-radius: var(--r-lg);
    box-shadow: var(--e3);
    border: 1px solid var(--border);
    z-index: 91;
    overflow: hidden;
  }
  .head {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 14px 18px;
    border-bottom: 1px solid var(--border);
  }
  .head-info {
    display: flex;
    align-items: center;
    gap: 10px;
    min-width: 0;
    flex: 1;
  }
  .head-info strong {
    display: block;
    font-size: 15px;
  }
  .sub {
    font-size: 11.5px;
    color: var(--text-3);
  }
  .logo {
    display: grid;
    place-items: center;
    width: 28px;
    height: 28px;
    border-radius: 9px;
    background: var(--primary-soft);
    color: var(--primary);
    flex-shrink: 0;
  }
  .x {
    border: none;
    background: transparent;
    color: var(--text-3);
    font-size: 14px;
    cursor: pointer;
    padding: 6px;
    border-radius: 8px;
    flex-shrink: 0;
  }
  .x:hover {
    background: var(--surface-3);
    color: var(--text-1);
  }
  .body {
    overflow-y: auto;
    padding: 14px 18px;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }
  .quote {
    margin: 0;
    font-size: 12.5px;
    font-style: italic;
    color: var(--text-3);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .sec h4 {
    margin: 0 0 8px;
    font-size: 12px;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--text-3);
  }
  .under {
    display: flex;
    align-items: flex-start;
    gap: 10px;
    padding: 8px 10px;
    border-radius: 12px;
    background: var(--surface-2);
    margin-bottom: 6px;
  }
  .under-main {
    min-width: 0;
    flex: 1;
  }
  .under-main strong {
    display: block;
    font-size: 13.5px;
  }
  .when {
    font-size: 12px;
    color: var(--text-2);
  }
  .hours {
    display: flex;
    flex-direction: column;
    align-items: flex-end;
    gap: 2px;
    font-size: 11.5px;
    color: var(--text-3);
  }
  .h.dl {
    color: var(--danger);
    font-weight: 600;
  }
  .chip {
    font-size: 11px;
    font-weight: 600;
    white-space: nowrap;
    color: color-mix(in srgb, var(--c, var(--primary)) 60%, var(--text-1));
    background: color-mix(in srgb, var(--c, var(--primary)) 13%, var(--surface));
    border-radius: var(--r-full);
    padding: 3px 9px;
    border: 1px solid color-mix(in srgb, var(--c, var(--primary)) 30%, transparent);
  }
  .chip.src {
    --c: var(--primary);
    flex-shrink: 0;
  }
  .plan-item {
    border: 1px solid var(--border);
    border-radius: var(--r-md);
    padding: 10px 12px;
    margin-bottom: 8px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .item-head {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }
  .item-title {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }
  .item-title strong {
    font-size: 13.5px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }
  .req {
    font-size: 11.5px;
    color: var(--text-3);
    flex-shrink: 0;
  }
  .ok {
    font-size: 11px;
    font-weight: 700;
    color: var(--success);
    background: color-mix(in srgb, var(--success) 14%, var(--surface));
    border-radius: var(--r-full);
    padding: 2px 9px;
    flex-shrink: 0;
  }
  .warn {
    font-size: 11px;
    font-weight: 700;
    color: var(--warning);
    background: color-mix(in srgb, var(--warning) 14%, var(--surface));
    border-radius: var(--r-full);
    padding: 2px 9px;
    flex-shrink: 0;
  }
  .warn-note {
    margin: 0;
    font-size: 12px;
    color: var(--warning);
    background: color-mix(in srgb, var(--warning) 10%, var(--surface));
    border-radius: 10px;
    padding: 6px 10px;
  }
  .sess {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
    padding: 4px 0;
  }
  .dot {
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--primary);
    flex-shrink: 0;
  }
  .dot.prep {
    background: var(--warning);
  }
  .day {
    font-weight: 600;
    min-width: 92px;
  }
  .range {
    color: var(--text-2);
  }
  .badge {
    font-size: 10px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    color: var(--warning);
    border: 1px solid color-mix(in srgb, var(--warning) 40%, transparent);
    border-radius: 6px;
    padding: 1px 6px;
  }
  .edit-row {
    display: flex;
    align-items: center;
    gap: 6px;
  }
  .edit-row input {
    border: 1px solid var(--border);
    background: var(--surface-3);
    border-radius: 10px;
    padding: 6px 8px;
    color: var(--text-1);
    font-family: inherit;
    font-size: 12.5px;
    outline: none;
  }
  .edit-row input:focus {
    border-color: var(--primary);
  }
  .arr {
    color: var(--text-3);
  }
  .mini {
    border: none;
    background: var(--surface-3);
    color: var(--danger);
    border-radius: 8px;
    width: 26px;
    height: 26px;
    cursor: pointer;
    font-size: 12px;
    flex-shrink: 0;
  }
  .link {
    align-self: flex-start;
    border: none;
    background: transparent;
    color: var(--primary);
    font-size: 12.5px;
    font-weight: 600;
    cursor: pointer;
    padding: 2px 0;
    font-family: inherit;
  }
  .empty {
    background: var(--surface-2);
    border-radius: var(--r-md);
    padding: 14px;
    text-align: center;
  }
  .empty p {
    margin: 0;
    font-weight: 600;
    font-size: 13px;
  }
  .empty .sub {
    font-weight: 400;
  }
  .err {
    margin: 0;
    padding: 0 18px 8px;
    font-size: 12.5px;
    color: var(--danger);
    font-weight: 600;
  }
  .foot {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 10px;
    padding: 12px 18px;
    border-top: 1px solid var(--border);
    background: var(--surface-2);
  }
  .total {
    font-size: 12px;
    color: var(--text-3);
  }
  .row {
    display: flex;
    gap: 8px;
  }
  .btn {
    border: none;
    background: var(--surface-3);
    color: var(--text-1);
    border-radius: 12px;
    padding: 8px 16px;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
    font-family: inherit;
    transition: all var(--dur-fast) var(--ease-out);
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
    box-shadow: none;
    border: 1px solid var(--border);
  }
  .btn:disabled {
    opacity: 0.5;
    pointer-events: none;
  }
</style>
