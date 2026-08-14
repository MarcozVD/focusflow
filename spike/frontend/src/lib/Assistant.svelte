<script lang="ts">
  import { tick } from "svelte";
  import { fade } from "svelte/transition";
  import PlanProposal from "./PlanProposal.svelte";
  import {
    assistantThread,
    assistantBusy,
    assistantError,
    assistantRetry,
    assistantTurn,
    assistantActionAccept,
    assistantActionReject,
    planProposal,
    planResult,
    planAccept,
    planReject,
    takeAssistantDraft,
    cat,
    fmtDate,
    fmtMs,
    tasks as tasksStore,
    openTaskDetail,
    openTaskRemote,
    type AssistantTurn,
    type PlanProposalView,
    type TaskRefView,
  } from "./data.svelte";

  const thread = $derived(assistantThread());
  const busy = $derived(assistantBusy());
  const err = $derived(assistantError());
  const retryMsg = $derived(assistantRetry());
  const activeProposal = $derived(planProposal());

  let input = $state("");
  let pendingAction = $state<number | null>(null);
  // Resultado de ACCIONES (no planes): local al componente. Los planes usan el
  // estado global `planResult` del store, que persiste al re-montar la pestaña.
  let appliedActions: Record<number, string> = $state({});

  // ---- auto-scroll del hilo ----
  // El usuario está "pegado" abajo si el scroll está a menos de umbral del
  // final. Solo entonces se auto-desplaza al llegar mensajes nuevos; si está
  // leyendo mensajes anteriores no se le roba el scroll y aparece el badge.
  let threadEl = $state<HTMLDivElement | null>(null);
  let pinned = $state(true);
  let unseen = $state(0);
  const PIN_EPS = 64; // px de margen respecto al fondo

  function nearBottom(): boolean {
    const el = threadEl;
    if (!el) return true;
    return el.scrollHeight - el.scrollTop - el.clientHeight <= PIN_EPS;
  }

  function scrollToBottom(behavior: ScrollBehavior = "auto") {
    const el = threadEl;
    if (!el) return;
    el.scrollTo({ top: el.scrollHeight, behavior });
    pinned = true;
    unseen = 0;
  }

  function onThreadScroll() {
    if (!threadEl) return;
    pinned = nearBottom();
    if (pinned) unseen = 0;
  }

  // Reacciona a cualquier cambio del hilo (mensaje nuevo, respuesta larga,
  // plan/acción renderizada) o del estado de carga (aparece/desaparece el
  // indicador de typing, que cambia la altura). `tick()` espera a que Svelte
  // aplique el DOM; si no, el scrollTo mediría la altura anterior.
  $effect(() => {
    const n = thread.length;
    const b = busy;
    if (threadEl) {
      if (pinned) {
        void tick().then(() => scrollToBottom("smooth"));
      } else if (b || n > 0) {
        unseen += 1;
      }
    }
  });

  $effect(() => {
    const draft = takeAssistantDraft();
    if (draft) send(draft);
  });

  const QUICK = [
    "¿Qué debería hacer hoy?",
    "¿Tengo tiempo para estudiar esta semana?",
    "¿Cuál es mi tarea más urgente?",
    "Organiza mi semana",
    "Voy atrasado con el estudio",
  ];

  async function send(text?: string) {
    const t = (text ?? input).trim();
    if (!t || busy) return;
    input = "";
    // el mensaje del usuario se pushea al hilo de forma síncrona al entrar en
    // assistantTurn; esperamos a que Svelte lo pinte y bajamos al hilo nuevo.
    const p = assistantTurn(t);
    await tick();
    scrollToBottom();
    await p;
  }

  async function acceptAction(id: number) {
    pendingAction = id;
    try {
      const summary = await assistantActionAccept(id);
      appliedActions[id] = summary;
    } catch (e) {
      appliedActions[id] = `Error: ${e}`;
    } finally {
      pendingAction = null;
    }
  }

  async function rejectAction(id: number) {
    try {
      await assistantActionReject(id);
      appliedActions[id] = "rechazada";
    } catch (e) {
      appliedActions[id] = `Error: ${e}`;
    }
  }

  async function acceptPlan(proposal: PlanProposalView) {
    // planAccept registra el resultado en el store global (planResult) y
    // refresca el calendario; el render lo lee de ahí.
    await planAccept(proposal.id).catch((e) => console.error("acceptPlan", e));
  }

  async function rejectPlan(proposal: PlanProposalView) {
    await planReject(proposal.id).catch((e) => console.error("rejectPlan", e));
  }

  function fmtWhen(s: number | null, e: number | null): string {
    if (!s) return "sin horario";
    return `${fmtDate(s)} ${e && e > s ? fmtMs(s) + "–" + fmtMs(e) : fmtMs(s)}`;
  }

  const TASKS_META: Record<string, { label: string; cls: string }> = {
    URGENT: { label: "URGENTE", cls: "urgent" },
    IMPORTANT: { label: "IMPORTANTE", cls: "important" },
    NORMAL: { label: "NORMAL", cls: "normal" },
  };

  /** Abre una tarea real completa desde el store; si aún no está en caché,
   *  usa el comando remoto (como el widget). */
  function openTaskRef(ref: TaskRefView) {
    const full = tasksStore().find((t) => t.id === ref.id);
    if (full) openTaskDetail(full);
    else openTaskRemote(ref.id);
  }

  function taskRefsOf(turn: AssistantTurn): TaskRefView[] {
    return turn.type === "Answer" && turn.tasks ? turn.tasks : [];
  }

  const actionLabel: Record<string, string> = {
    complete: "Marcar completada",
    reschedule: "Reagendar",
    create_event: "Crear evento",
    cancel_proposal: "Cancelar propuesta",
  };

  function msgId(i: number, m: { role: string; at: number }): string {
    return `${m.role}-${i}-${m.at}`;
  }
</script>

<div class="ast">
  <div class="head">
    <div>
      <h2>Asistente</h2>
      <p class="hint">
        Pregunta sobre tu tiempo o pide cambios: nada se modifica sin tu aprobación.
      </p>
    </div>
  </div>

  <div class="chips">
    {#each QUICK as q}
      <button class="chip" onclick={() => send(q)} disabled={busy}>{q}</button>
    {/each}
  </div>

  <div class="thread" bind:this={threadEl} onscroll={onThreadScroll}>
    {#each thread as m, i (msgId(i, m))}
      {#if m.role === "user"}
        <div class="msg user">{m.text}</div>
      {:else}
        <div class="msg ai" transition:fade={{ duration: 160 }}>
          {#if m.turn?.type === "Answer"}
            <p class="answer">{m.turn.text}</p>
            {#if taskRefsOf(m.turn).length > 0}
              <div class="task-refs">
                {#each ["URGENT", "IMPORTANT", "NORMAL"] as level}
                  {@const refs = taskRefsOf(m.turn).filter((r) => r.level === level)}
                  {#if refs.length > 0}
                    <div class="task-ref-group">
                      <p class="task-ref-level {TASKS_META[level].cls}">
                        {TASKS_META[level].label}
                      </p>
                      {#each refs as r}
                        <div class="task-ref">
                          <div class="task-ref-body">
                            <p class="task-ref-title">{r.title}</p>
                            <p class="task-ref-meta">
                              {#if r.end_ms}
                                Vence: {fmtDate(r.end_ms)}
                                {r.all_day ? "" : " · " + fmtMs(r.end_ms)}
                              {/if}
                              {#if r.priority && r.priority !== "media"}
                                <span class="task-ref-prio prio-{r.priority}">
                                  {r.priority === "alta" ? "Prioridad: alta" : "Prioridad: " + r.priority}
                                </span>
                              {/if}
                            </p>
                          </div>
                          <button class="btn ghost small" onclick={() => openTaskRef(r)}>
                            Abrir tarea
                          </button>
                        </div>
                      {/each}
                    </div>
                  {/if}
                {/each}
              </div>
            {/if}
          {:else if m.turn?.type === "Nothing"}
            <p class="answer muted">{m.turn.text}</p>
          {:else if m.turn?.type === "Plan"}
            {@const r = planResult(m.turn.proposal.id)}
            {#if r?.ok}
              <div class="plan-done">Hecho: {r.text}</div>
            {:else if activeProposal && activeProposal.id === m.turn.proposal.id}
              <PlanProposal />
              {#if r && !r.ok}
                <div class="plan-done error">Error al aceptar: {r.text}</div>
              {/if}
            {:else}
              <div class="card mini">
                <div class="mini-row">
                  <span class="k">Propuesta de plan</span>
                  <span class="mini-text">{m.turn.proposal.text}</span>
                </div>
                <div class="mini-rows">
                  {#each m.turn.proposal.items as it}
                    <div class="mini-row">
                      <span class="mini-title">{it.title}</span>
                      <span class="mini-meta">
                        {it.sessions.length} sesión{it.sessions.length === 1 ? "" : "es"}
                      </span>
                    </div>
                  {/each}
                </div>
                <div class="row">
                  <button class="btn primary" onclick={() => acceptPlan(m.turn!.proposal)}>Aceptar</button>
                  <button class="btn" onclick={() => rejectPlan(m.turn!.proposal)}>Descartar</button>
                </div>
              </div>
            {/if}
            <p class="note">{m.turn.note}</p>
          {:else if m.turn?.type === "Action"}
            {@const a = m.turn.action}
            <div class="card mini">
              <div class="mini-row">
                <span class="k">{actionLabel[a.kind] ?? a.kind}</span>
                <span class="mini-title">{a.title || a.task_title}</span>
              </div>
              <p class="summary">{a.summary}</p>
              {#if a.start_ms}
                <p class="when">{fmtWhen(a.start_ms, a.end_ms)}</p>
              {/if}
              {#if appliedActions[a.proposal_id]}
                <div class="plan-done">Hecho: {appliedActions[a.proposal_id]}</div>
              {:else}
                <div class="row">
                  <button class="btn primary" onclick={() => acceptAction(a.proposal_id)} disabled={pendingAction === a.proposal_id}>
                    {pendingAction === a.proposal_id ? "Aplicando…" : "Confirmar"}
                  </button>
                  <button class="btn ghost" onclick={() => rejectAction(a.proposal_id)}>Descartar</button>
                </div>
              {/if}
            </div>
          {/if}
        </div>
      {/if}
    {/each}

    {#if busy}
      <div class="msg ai typing">Analizando tu calendario<span class="dots">…</span></div>
    {/if}

    {#if err}
      <p class="error">{err}</p>
    {/if}

    {#if retryMsg && !busy}
      <button class="retry" onclick={() => send(retryMsg)}>Reintentar</button>
    {/if}

    {#if unseen > 0}
      <button class="goto-last" onclick={() => scrollToBottom("smooth")}>
        Ir al último mensaje
      </button>
    {/if}
  </div>

  <div class="composer">
    <input
      class="t"
      bind:value={input}
      placeholder="Pregunta o pide algo: ¿tengo tiempo hoy? · organiza mi semana · marca la tarea X como hecha"
      onkeydown={(e) => e.key === "Enter" && send()}
      disabled={busy}
    />
    <button class="btn primary" onclick={() => send()} disabled={busy || !input.trim()}>Enviar</button>
  </div>
  <p class="foot">
    El asistente solo propone: ninguna tarea se crea, mueve ni completa sin tu confirmación.
  </p>
</div>

<style>
  .ast {
    max-width: 780px;
    margin: 0 auto;
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }
  .head h2 {
    font-size: 20px;
    margin: 0 0 4px;
  }
  .hint,
  .foot {
    color: var(--text-3);
    font-size: 13px;
    margin: 0;
  }
  .chips {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
    margin: var(--s-3) 0 var(--s-4);
  }
  .chip {
    border: 1px solid var(--border);
    background: var(--surface);
    color: var(--text-2);
    border-radius: var(--r-full);
    padding: 6px 14px;
    font-size: 12.5px;
    font-weight: 600;
    cursor: pointer;
    transition: all var(--dur-fast) var(--ease-out);
  }
  .chip:hover {
    border-color: var(--primary);
    color: var(--primary);
  }
  .chip:disabled {
    opacity: 0.5;
    pointer-events: none;
  }
  .thread {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    gap: var(--s-3);
    padding: var(--s-2) var(--s-2) var(--s-4);
  }
  .msg {
    max-width: 88%;
    border-radius: 16px;
    padding: 12px 16px;
    font-size: 14px;
    line-height: 1.5;
  }
  .msg.user {
    align-self: flex-end;
    background: var(--primary);
    color: #fff;
    border-bottom-right-radius: 4px;
  }
  .msg.ai {
    align-self: flex-start;
    background: var(--surface);
    box-shadow: var(--e1);
    border-bottom-left-radius: 4px;
  }
  .msg.ai.typing {
    color: var(--text-3);
  }
  .goto-last {
    align-self: center;
    margin: 10px auto 0;
    border: 1px solid var(--border);
    border-radius: var(--r-full);
    background: var(--surface-2);
    color: var(--text-2);
    padding: 6px 14px;
    font-size: 12.5px;
    font-weight: 600;
    cursor: pointer;
    box-shadow: var(--e1);
    transition: all var(--dur-fast) var(--ease-out);
  }
  .goto-last:hover {
    border-color: var(--primary);
    color: var(--primary);
  }
  .task-refs {
    margin-top: 8px;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .task-ref-group {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .task-ref-level {
    margin: 0;
    font-size: 11px;
    font-weight: 800;
    letter-spacing: 0.08em;
    padding: 3px 10px;
    border-radius: var(--r-full);
    width: fit-content;
  }
  .task-ref-level.urgent {
    color: var(--danger);
    background: color-mix(in srgb, var(--danger) 12%, transparent);
  }
  .task-ref-level.important {
    color: var(--warning);
    background: color-mix(in srgb, var(--warning) 14%, transparent);
  }
  .task-ref-level.normal {
    color: var(--text-2);
    background: var(--surface-2);
  }
  .task-ref {
    display: flex;
    align-items: center;
    gap: 10px;
    border: 1px solid var(--border);
    border-left-width: 3px;
    border-radius: var(--r-md);
    padding: 8px 12px;
    background: var(--surface-2);
    transition: border-color var(--dur-fast) var(--ease-out);
  }
  .task-ref.urgent {
    border-left-color: var(--danger);
  }
  .task-ref.important {
    border-left-color: var(--warning);
  }
  .task-ref.normal {
    border-left-color: var(--border);
  }
  .task-ref-body {
    flex: 1;
    min-width: 0;
  }
  .task-ref-title {
    margin: 0;
    font-size: 13.5px;
    font-weight: 600;
    color: var(--text-1);
  }
  .task-ref-meta {
    margin: 2px 0 0;
    font-size: 12px;
    color: var(--text-3);
  }
  .task-ref-prio {
    margin-left: 8px;
    font-weight: 600;
  }
  .task-ref-prio.prio-alta {
    color: var(--danger);
  }
  .task-ref-prio.prio-baja {
    color: var(--text-3);
  }
  .btn.small {
    padding: 5px 10px;
    font-size: 12px;
    white-space: nowrap;
  }
  .answer {
    margin: 0;
    white-space: pre-line;
  }
  .answer.muted {
    color: var(--text-3);
  }
  .card.mini {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .mini-row {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
  }
  .k {
    font-size: 11px;
    font-weight: 700;
    color: var(--primary);
    background: var(--primary-soft);
    border-radius: var(--r-full);
    padding: 3px 10px;
  }
  .mini-title {
    font-weight: 700;
    font-size: 13.5px;
  }
  .mini-meta {
    margin-left: auto;
    font-size: 12px;
    color: var(--text-3);
  }
  .summary,
  .note {
    margin: 0;
    font-size: 13px;
    color: var(--text-2);
  }
  .note {
    font-size: 12px;
    color: var(--text-3);
    font-style: italic;
  }
  .when {
    margin: 0;
    font-size: 12.5px;
    color: var(--text-2);
    font-weight: 600;
  }
  .plan-done {
    font-size: 13px;
    font-weight: 600;
    color: var(--success);
  }
  .error {
    color: var(--danger);
    font-size: 13px;
    margin: 0;
  }
  .retry {
    border: none;
    background: var(--accent);
    color: #fff;
    font-size: 13px;
    font-weight: 600;
    padding: 7px 14px;
    border-radius: 10px;
    cursor: pointer;
    margin-top: 8px;
  }
  .retry:hover {
    filter: brightness(1.08);
  }
  .row {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
    margin-top: 6px;
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
  .composer {
    display: flex;
    gap: 8px;
    padding: var(--s-3) 0 var(--s-2);
    border-top: 1px solid var(--border);
  }
  .t {
    flex: 1;
    border: 1px solid var(--border);
    background: var(--surface-3);
    border-radius: 14px;
    padding: 10px 14px;
    font-size: 14px;
    color: var(--text-1);
    font-family: inherit;
    outline: none;
  }
  .t:focus {
    border-color: var(--primary);
  }
  .foot {
    text-align: center;
  }
  .dots {
    animation: blink 1.2s infinite;
  }
  @keyframes blink {
    50% {
      opacity: 0.2;
    }
  }
</style>
