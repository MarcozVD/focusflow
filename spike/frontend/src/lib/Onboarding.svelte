<script lang="ts">
  import { onMount } from "svelte";
  import { fade } from "svelte/transition";
  import gsap from "gsap";
  import { invoke } from "@tauri-apps/api/core";
  import {
    onboarding,
    completeOnboarding,
    authUser,
    signInWithGoogle,
    loadAuthStatus,
  } from "./data.svelte";

  const GOOGLE_STEPS: string[] = [
    "Pulsa «Iniciar sesión con Google»: se abre tu navegador.",
    "Elige la cuenta y acepta los permisos de lectura de Gmail.",
    "Vuelve a FocusFlow: la cuenta queda conectada automáticamente.",
  ];

  const VALID_EMAIL = /^[^\s@]+@[^\s@]+\.[^\s@]{2,}$/;

  type AiPresetId = "groq" | "opencode" | "other";

  interface AiPreset {
    label: string;
    endpoint: string;
    model: string;
    name: string;
    tip: string;
    steps: string[];
  }

  const AI_PRESETS: Record<AiPresetId, AiPreset> = {
    groq: {
      label: "Groq · recomendado",
      endpoint: "https://api.groq.com/openai/v1",
      model: "openai/gpt-oss-120b",
      name: "Groq — gratis y muy rápida",
      tip: "La opción gratuita más rápida (hardware LPU): entiende tus tareas al instante. 1.000 peticiones al día gratis, sin tarjeta. La clave va incrustada en la app.",
      steps: [
        "Elige el proveedor (o deja el recomendado).",
        "Endpoint y Modelo ya vienen rellenados: no hace falta tocarlos.",
        "Pulsa Continuar y la IA queda lista.",
      ],
    },
    opencode: {
      label: "OpenCode Zen",
      endpoint: "https://opencode.ai/zen/v1",
      model: "big-pickle",
      name: "OpenCode Zen",
      tip: "Modelos gratuitos dentro de OpenCode. Pueden ir lentos o saturarse (límite de peticiones). La clave va incrustada en la app.",
      steps: [
        "Elige OpenCode Zen.",
        "Endpoint y Modelo ya vienen rellenados (hay otros modelos gratis: kimi-k3…).",
        "Pulsa Continuar y la IA queda lista.",
      ],
    },
    other: {
      label: "Otro",
      endpoint: "",
      model: "",
      name: "Otro proveedor",
      tip: "Cualquier API compatible con OpenAI chat completions funciona: Gemini (AI Studio), Cerebras, OpenRouter…",
      steps: [
        "Copia el endpoint base de tu proveedor (suele terminar en /v1).",
        "Escribe el nombre exacto del modelo que quieras usar.",
        "Pulsa Continuar y la IA queda lista.",
      ],
    },
  };

  let step = $state(1);
  let reduced = $state(
    typeof window !== "undefined" ? window.matchMedia("(prefers-reduced-motion: reduce)").matches : false,
  );
  let stageEl = $state<HTMLElement | null>(null);

  let googleBusy = $state(false);
  let googleErr = $state("");

  let aiEndpoint = $state(AI_PRESETS.groq.endpoint);
  let aiModel = $state(AI_PRESETS.groq.model);
  let aiPreset = $state<AiPresetId>("groq");

  let aiState = $state<"idle" | "loading" | "ok" | "error" | "skip">("idle");
  let aiDetail = $state("");
  let aiRaw = $state("");
  let fieldErr = $state<{ endpoint?: string; model?: string }>({});
  let busy = $state(false);

  function pickAiPreset(p: AiPresetId) {
    aiPreset = p;
    if (p !== "other") {
      aiEndpoint = AI_PRESETS[p].endpoint;
      aiModel = AI_PRESETS[p].model;
    }
    fieldErr = {};
  }

  onMount(() => {
    loadAuthStatus();
    const o = onboarding();
    if (o) {
      const ep = (o.ai.effective_endpoint || "").toLowerCase();
      if (ep) {
        // configuración existente: manda sobre los valores por defecto
        aiEndpoint = o.ai.effective_endpoint;
        aiModel = o.ai.effective_model;
        if (ep.includes("groq")) aiPreset = "groq";
        else if (ep.includes("opencode")) aiPreset = "opencode";
        else aiPreset = "other";
      }
    }
  });

  // Entrada escalonada de las cards (GSAP). Se respeta prefers-reduced-motion:
  // con reduce, las cards aparecen sin animación (solo el fade CSS global).
  $effect(() => {
    const s = step;
    if (reduced || !stageEl) return;
    const ctx = gsap.context(() => {
      if (s === 1) {
        gsap.from(".values > li", {
          y: 14, opacity: 0, duration: 0.45, ease: "power2.out", stagger: 0.09, delay: 0.05,
        });
        gsap.from(".hero .actions", {
          y: 10, opacity: 0, duration: 0.35, ease: "power2.out", delay: 0.3,
        });
      } else {
        gsap.from(".setup .head", {
          y: 10, opacity: 0, duration: 0.3, ease: "power2.out", delay: 0.05,
        });
        gsap.from(".guide .g-card", {
          y: 16, opacity: 0, duration: 0.4, ease: "power2.out", stagger: 0.1, delay: 0.15,
        });
        gsap.from(".form fieldset", {
          y: 16, opacity: 0, duration: 0.4, ease: "power2.out", stagger: 0.12, delay: 0.35,
        });
      }
    }, stageEl);
    return () => ctx.revert();
  });

  const aiFilled = $derived(aiEndpoint.trim() !== "" || aiModel.trim() !== "");

  function validateFields(): boolean {
    const errs: typeof fieldErr = {};
    if (aiFilled) {
      if (!aiEndpoint.trim()) errs.endpoint = "Escribe el endpoint de la API.";
      if (!aiModel.trim()) errs.model = errs.model || "Escribe el nombre del modelo.";
    }
    fieldErr = errs;
    return Object.keys(errs).length === 0;
  }

  async function doGoogleSignIn() {
    googleBusy = true;
    googleErr = "";
    try {
      const r = await signInWithGoogle();
      if (!r.ok) googleErr = r.error ?? "no se pudo conectar con Google";
      else await loadAuthStatus();
    } catch (e) {
      googleErr = String(e);
    } finally {
      googleBusy = false;
    }
  }

  function humanError(raw: string, kind: "ai" | "email"): string {
    const s = raw.toLowerCase();
    if (s.includes("535") || s.includes("authentication") || s.includes("invalid credentials") || s.includes("logon"))
      return "No pudimos autenticar tu cuenta. Revisa la contraseña: Gmail y Outlook exigen una contraseña de aplicación, no la normal.";
    if (s.includes("dns") || s.includes("resolve"))
      return "No encontramos el servidor de correo. Revisa el host (por ejemplo, imap.gmail.com).";
    if (s.includes("tls"))
      return "La conexión segura falló. Revisa que el puerto y la opción SSL sean correctos.";
    if (s.includes("tcp") || s.includes("refused") || s.includes("timeout") || s.includes("timed out"))
      return "No pudimos conectarnos. Comprueba tu conexión a internet o que el puerto esté abierto.";
    if (s.includes("401") || s.includes("403") || s.includes("unauthorized") || s.includes("api key") || s.includes("apikey"))
      return "La clave de API no es válida o venció.";
    if (s.includes("404") || s.includes("endpoint") || s.includes("not found"))
      return "No encontramos ese endpoint. Revisa la URL completa de la API.";
    if (kind === "ai") return "La comprobación de la IA falló. Revisa el endpoint, el modelo y la clave.";
    return "No pudimos conectar con tu correo. Revisa los datos e inténtalo de nuevo.";
  }

  async function continueSetup() {
    if (busy) return;
    if (!validateFields()) {
      aiState = "idle";
      return;
    }
    busy = true;
    aiState = aiFilled ? "loading" : "skip";
    aiDetail = "";
    aiRaw = "";

    try {
      if (aiFilled) {
        await invoke("ai_config_set", { endpoint: aiEndpoint.trim(), model: aiModel.trim() });
      }
      const r = await invoke<{ ai: { ok: boolean; detail: string }; email: { ok: boolean; detail: string } }>(
        "verify_connections",
      );
      if (aiFilled) {
        aiState = r.ai.ok ? "ok" : "error";
        aiDetail = r.ai.detail;
        aiRaw = r.ai.ok ? "" : r.ai.detail;
      }
      if (aiFilled && aiState === "error") {
        return;
      }
      await completeOnboarding();
    } catch (e) {
      const msg = String(e);
      if (aiFilled) {
        aiState = "error";
        aiDetail = humanError(msg, "ai");
        aiRaw = msg;
      } else {
        // sin IA configurada, un fallo de verify no debe bloquear el onboarding
        await completeOnboarding();
      }
    } finally {
      busy = false;
    }
  }

  function fieldClass(st: "idle" | "loading" | "ok" | "error" | "skip", filled = false) {
    if (busy) return "inp disabled";
    if (st === "ok") return "inp ok";
    if (st === "error") return "inp err";
    return filled ? "inp filled" : "inp";
  }
</script>

{#if step === 1}
  <div class="stage" bind:this={stageEl} in:fade={{ duration: reduced ? 0 : 500 }}>
    <div class="hero">
      <div class="logo">
        <img src="/icon.png" alt="Icono de FocusFlow" width="48" height="48" />
      </div>
      <h1>Tu calendario, al día. <span>Sin copiar ni pegar.</span></h1>
      <p class="lead">
        FocusFlow lee tu correo y tu agenda, detecta fechas, horas y compromisos, y te propone
        añadirlos al calendario. Tú solo apruebas. Dos pasos y listo.
      </p>
      <ul class="values">
        <li>
          <span class="vi" aria-hidden="true">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
              <rect x="3" y="5" width="18" height="14" rx="2" />
              <path d="m3 7 9 6 9-6" />
            </svg>
          </span>
          <div>
            <strong>Correo conectado</strong>
            <p>Las fechas de tus correos llegan solas como sugerencias.</p>
          </div>
        </li>
        <li>
          <span class="vi" aria-hidden="true">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linejoin="round">
              <path d="M12 3l1.9 5.1L19 10l-5.1 1.9L12 17l-1.9-5.1L5 10l5.1-1.9L12 3z" />
              <path d="M19 15l.9 2.1L22 18l-2.1.9L19 21l-.9-2.1L16 18l2.1-.9L19 15z" />
            </svg>
          </span>
          <div>
            <strong>IA para planear</strong>
            <p>Describe una tarea en lenguaje natural y recibe un plan con horarios.</p>
          </div>
        </li>
        <li>
          <span class="vi" aria-hidden="true">
            <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round">
              <path d="M12 3l7 3v5c0 4.4-3 8.4-7 10-4-1.6-7-5.6-7-10V6l7-3z" />
              <path d="m9.5 12 1.8 1.8 3.4-3.6" />
            </svg>
          </span>
          <div>
            <strong>Privado por diseño</strong>
            <p>Tus claves se guardan cifradas en Windows, nunca en la base de datos.</p>
          </div>
        </li>
      </ul>
      <div class="actions">
        <button class="btn primary" onclick={() => (step = 2)} autofocus>
          Comenzar <span class="arrow">→</span>
        </button>
        <button class="btn ghost" onclick={() => completeOnboarding()}>Omitir y explorar</button>
      </div>
      <p class="est">Te tomará unos 2 minutos. Puedes saltar cualquier paso.</p>
    </div>
  </div>
{:else}
  <div class="stage" bind:this={stageEl} in:fade={{ duration: reduced ? 0 : 350 }}>
    <div class="setup">
      <div class="head">
        <button class="back" onclick={() => (step = 1)} aria-label="Volver">←</button>
        <div>
          <p class="kicker">Paso 2 de 2</p>
          <h1>Conecta tus servicios</h1>
        </div>
      </div>
      <div class="grid">
        <aside class="guide">
          <div class="g-card">
            <h2>Cuenta de Google</h2>
            <p class="g-tip">
              Conecta tu cuenta para sincronizar Gmail (OAuth2): no necesitas servidores ni contraseñas.
            </p>
            <ol class="g-steps">
              {#each GOOGLE_STEPS as s}
                <li>{s}</li>
              {/each}
            </ol>
          </div>
          <div class="g-card">
            <h2>{AI_PRESETS[aiPreset].name}</h2>
            <p class="g-tip">{AI_PRESETS[aiPreset].tip}</p>
            <ol class="g-steps">
              {#each AI_PRESETS[aiPreset].steps as s}
                <li>{s}</li>
              {/each}
            </ol>
          </div>
        </aside>

        <form class="form" onsubmit={(e) => { e.preventDefault(); continueSetup(); }}>
          <fieldset>
            <legend>Cuenta de Google</legend>
            <p class="fs-hint">
              Conecta tu cuenta para sincronizar Gmail (IMAP con OAuth2). Se abre tu navegador
              para autorizar; al volver, la cuenta queda conectada.
            </p>

            {#if authUser()}
              <p class="fok" role="status">✓ Conectado: {authUser()!.name} ({authUser()!.email})</p>
            {:else}
              <div class="frow">
                <button
                  type="button"
                  class="btn primary big"
                  onclick={doGoogleSignIn}
                  disabled={googleBusy}
                >
                  {googleBusy ? "Abriendo navegador…" : "Iniciar sesión con Google"}
                </button>
              </div>
              {#if googleErr}
                <div class="ferrbox" role="alert">
                  <p>{googleErr}</p>
                </div>
              {/if}
            {/if}
          </fieldset>

          <fieldset>
            <legend>Asistente con IA</legend>
            <p class="fs-hint">
              El asistente entiende lenguaje natural y prepara planes de estudio, trabajo o
              personales.
            </p>

            <div class="frow">
              <span class="lbl" id="ob-ai-preset">Proveedor de IA</span>
              <div class="pills" role="group" aria-labelledby="ob-ai-preset">
                {#each Object.entries(AI_PRESETS) as [id, p]}
                  <button
                    type="button"
                    class="pill {aiPreset === id ? 'on' : ''}"
                    onclick={() => pickAiPreset(id as AiPresetId)}
                    aria-pressed={aiPreset === id}
                  >
                    {p.label}
                  </button>
                {/each}
              </div>
            </div>

            <div class="frow">
              <label class="lbl" for="ob-endpoint">Endpoint de la API</label>
              <input
                id="ob-endpoint"
                type="text"
                class={fieldClass(aiState, aiEndpoint !== "" || aiModel !== "")}
                bind:value={aiEndpoint}
                placeholder="https://…/v1"
                disabled={busy}
                aria-invalid={!!fieldErr.endpoint}
              />
              {#if fieldErr.endpoint}
                <p class="ferr">{fieldErr.endpoint}</p>
              {/if}
            </div>

            <div class="frow">
              <label class="lbl" for="ob-model">Modelo</label>
              <input
                id="ob-model"
                type="text"
                class={fieldClass(aiState, aiEndpoint !== "" || aiModel !== "")}
                bind:value={aiModel}
                placeholder="gpt-4o-mini"
                disabled={busy}
              />
            </div>

            {#if aiState === "ok"}
              <p class="fok" role="status">✓ IA conectada{aiDetail ? ` — ${aiDetail}` : ""}</p>
            {:else if aiState === "error"}
              <div class="ferrbox" role="alert">
                <p>{aiDetail}</p>
                {#if aiRaw}
                  <details>
                    <summary>Ver detalles</summary>
                    <pre>{aiRaw}</pre>
                  </details>
                {/if}
              </div>
            {:else if aiState === "skip"}
              <p class="fskip">Omitido: puedes configurarlo después desde Ajustes.</p>
            {:else if aiState === "loading"}
              <p class="fwait">Comprobando IA…</p>
            {/if}
          </fieldset>

          <div class="foot">
            <button type="submit" class="btn primary big" disabled={busy}>
              {busy ? "Comprobando…" : "Continuar →"}
            </button>
            <p class="sec">
              La clave de IA va incrustada en la app. Tus tokens de Google quedan en tu disco local.
            </p>
          </div>
        </form>
      </div>
    </div>
  </div>
{/if}

<style>
  .stage {
    flex: 1;
    min-height: 0;
    overflow-y: auto;
    display: flex;
    flex-direction: column;
    padding: var(--s-8);
    position: relative;
    isolation: isolate;
    /* fondo limpio: el neumorfismo vive en las superficies, sin degradados de color */
    background: var(--bg);
  }

  .hero {
    margin: auto;
    max-width: 620px;
    text-align: center;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--s-6);
  }
  .logo {
    width: 88px;
    height: 88px;
    display: grid;
    place-items: center;
    border-radius: 24px;
    background: var(--surface);
    box-shadow: var(--shadow-raised-lg);
  }
  .logo img {
    width: 56px;
    height: 56px;
  }
  h1 {
    margin: 0;
    font-size: 38px;
    line-height: 1.14;
    letter-spacing: -0.03em;
    font-weight: 800;
    color: var(--text-1);
    text-wrap: balance;
  }
  h1 span {
    color: var(--primary);
  }
  .lead {
    margin: 0;
    font-size: 16px;
    line-height: 1.65;
    color: var(--text-2);
    max-width: 54ch;
  }
  .values {
    list-style: none;
    margin: var(--s-2) 0 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: 12px;
    width: 100%;
    max-width: 480px;
    text-align: left;
  }
  .values li {
    display: flex;
    gap: 14px;
    align-items: flex-start;
    background: var(--surface);
    border: none;
    border-radius: var(--r-md);
    padding: 16px 18px;
    box-shadow: var(--shadow-raised);
    transition: transform var(--dur-fast) var(--ease-out), box-shadow var(--dur-fast) var(--ease-out);
  }
  .values li:hover {
    transform: translateY(-1px);
    box-shadow: var(--e1);
  }
  .vi {
    width: 36px;
    height: 36px;
    flex-shrink: 0;
    display: grid;
    place-items: center;
    border-radius: 10px;
    background: var(--primary-soft);
    color: var(--primary);
    font-size: 16px;
  }
  .values strong {
    font-size: 13.5px;
    color: var(--text-1);
  }
  .values p {
    margin: 3px 0 0;
    font-size: 12.5px;
    color: var(--text-3);
    line-height: 1.55;
  }
  .actions {
    display: flex;
    gap: 12px;
    align-items: center;
    margin-top: var(--s-3);
  }
  .arrow {
    font-size: 15px;
  }
  .est {
    margin: 0;
    font-size: 12px;
    color: var(--text-3);
  }

  .setup {
    margin: auto;
    width: 100%;
    max-width: 960px;
    display: flex;
    flex-direction: column;
    gap: var(--s-6);
  }
  .head {
    display: flex;
    align-items: center;
    gap: 14px;
  }
  .back {
    width: 38px;
    height: 38px;
    border-radius: var(--r-full);
    border: none;
    background: var(--surface-2);
    color: var(--text-2);
    font-size: 16px;
    cursor: pointer;
    transition: all var(--dur-fast) var(--ease-out);
  }
  .back:hover {
    color: var(--primary);
    background: var(--surface-3);
  }
  .back:active {
    transform: scale(0.96);
  }
  .back:focus-visible {
    outline: 2px solid var(--primary);
    outline-offset: 2px;
  }
  .kicker {
    margin: 0 0 2px;
    font-size: 10.5px;
    font-weight: 800;
    letter-spacing: 0.09em;
    text-transform: uppercase;
    color: var(--primary);
  }
  .head h1 {
    font-size: 26px;
  }

  .grid {
    display: grid;
    grid-template-columns: 45fr 55fr;
    gap: var(--s-6);
    align-items: start;
  }
  .guide {
    display: flex;
    flex-direction: column;
    gap: var(--s-5);
    min-width: 0;
  }
  .guide .g-card {
    background: var(--surface);
    border: none;
    border-radius: var(--r-lg);
    box-shadow: var(--shadow-raised);
    padding: 24px 26px;
    display: flex;
    flex-direction: column;
    gap: 16px;
    /* altura mínima compartida CORREO/IA: sin salto al cargar la configuración */
    min-height: 300px;
  }
  .g-card h2 {
    margin: 0;
    font-size: 18px;
    letter-spacing: -0.01em;
    color: var(--text-1);
  }
  .g-tip {
    margin: 0;
    font-size: 13px;
    line-height: 1.65;
    color: var(--text-2);
    background: var(--primary-soft);
    border-radius: var(--r-sm);
    padding: 12px 14px;
  }
  .g-steps {
    margin: 2px 0 0;
    padding-left: 22px;
    display: flex;
    flex-direction: column;
    gap: 12px;
    font-size: 13px;
    line-height: 1.6;
    color: var(--text-2);
  }
  .g-steps li {
    padding-left: 4px;
  }
  .g-steps li::marker {
    color: var(--primary);
    font-weight: 700;
  }

  .form {
    display: flex;
    flex-direction: column;
    gap: var(--s-5);
  }
  fieldset {
    border: none;
    border-radius: var(--r-lg);
    background: var(--surface);
    box-shadow: var(--shadow-raised);
    padding: var(--s-6);
    display: flex;
    flex-direction: column;
    gap: 14px;
    margin: 0;
  }
  legend {
    padding: 0 10px;
    font-size: 14.5px;
    font-weight: 800;
    color: var(--text-1);
  }
  .fs-hint {
    margin: 0;
    font-size: 12.5px;
    color: var(--text-3);
    line-height: 1.5;
  }
  .frow {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .lbl {
    font-size: 12px;
    font-weight: 700;
    color: var(--text-2);
  }
  .inp {
    border: 1px solid transparent;
    background: var(--surface-3);
    border-radius: 10px;
    padding: 10px 12px;
    font-size: 13.5px;
    color: var(--text-1);
    font-family: inherit;
    outline: none;
    box-shadow: var(--shadow-inset-sm);
    transition: border-color var(--dur-fast) var(--ease-out), box-shadow var(--dur-fast) var(--ease-out);
  }
  .inp:focus {
    border-color: var(--primary);
    box-shadow: var(--shadow-inset-sm), inset 0 0 0 2px var(--primary-soft-2);
  }
  .inp.filled {
    border-color: color-mix(in srgb, var(--text-3) 55%, var(--border));
  }
  .inp.ok {
    border-color: var(--success);
  }
  .inp.ok:focus {
    box-shadow: var(--shadow-inset-sm), inset 0 0 0 2px color-mix(in srgb, var(--success) 45%, transparent);
  }
  .inp.err {
    border-color: var(--danger);
  }
  .inp.err:focus {
    box-shadow: var(--shadow-inset-sm), inset 0 0 0 2px color-mix(in srgb, var(--danger) 45%, transparent);
  }
  .inp.disabled {
    opacity: 0.55;
  }
  input:disabled,
  button:disabled {
    cursor: not-allowed;
  }
  .ferr {
    margin: 0;
    font-size: 12px;
    color: var(--danger);
  }
  .pills {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
  }
  .pill {
    border: none;
    background: var(--surface-2);
    color: var(--text-2);
    border-radius: var(--r-full);
    padding: 6px 14px;
    font-size: 12.5px;
    font-weight: 600;
    font-family: inherit;
    cursor: pointer;
    transition: all var(--dur-fast) var(--ease-out);
  }
  .pill:hover {
    color: var(--primary);
    background: var(--surface-3);
  }
  .pill.on {
    background: var(--primary-soft);
    box-shadow: inset 0 0 0 2px var(--primary-soft-2);
    color: var(--primary);
  }
  .pill:focus-visible {
    outline: 2px solid var(--primary);
    outline-offset: 2px;
  }
  .srv {
    display: grid;
    grid-template-columns: 1fr 96px auto;
    gap: 10px;
    align-items: end;
  }
  .srv-field {
    display: flex;
    flex-direction: column;
    gap: 6px;
    min-width: 0;
  }
  .check {
    display: flex;
    align-items: center;
    gap: 7px;
    font-size: 12.5px;
    font-weight: 600;
    color: var(--text-2);
    padding-bottom: 11px;
    user-select: none;
  }

  .fok {
    margin: 0;
    font-size: 13px;
    font-weight: 700;
    color: var(--success);
    background: var(--success-bg);
    border-radius: 10px;
    padding: 9px 12px;
  }
  .ferrbox {
    border: 1px solid color-mix(in srgb, var(--danger) 35%, transparent);
    background: var(--danger-bg);
    border-radius: 10px;
    padding: 10px 12px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .ferrbox p {
    margin: 0;
    font-size: 12.5px;
    color: var(--danger);
    line-height: 1.5;
  }
  .ferrbox details summary {
    cursor: pointer;
    font-size: 11.5px;
    font-weight: 700;
    color: var(--text-2);
    user-select: none;
  }
  .ferrbox details summary:hover {
    color: var(--primary);
  }
  .ferrbox pre {
    margin: 4px 0 0;
    font-size: 11px;
    white-space: pre-wrap;
    word-break: break-all;
    color: var(--text-2);
    background: var(--surface);
    border-radius: 8px;
    padding: 8px 10px;
    max-height: 120px;
    overflow-y: auto;
  }
  .fskip {
    margin: 0;
    font-size: 12.5px;
    color: var(--text-3);
    background: var(--surface-2);
    border-radius: 10px;
    padding: 9px 12px;
  }
  .fwait {
    margin: 0;
    font-size: 12.5px;
    color: var(--text-2);
    display: flex;
    align-items: center;
    gap: 8px;
  }
  .fwait::before {
    content: "";
    width: 13px;
    height: 13px;
    border-radius: 50%;
    border: 2px solid var(--border);
    border-top-color: var(--primary);
    animation: spin 0.8s linear infinite;
  }
  @keyframes spin {
    to { transform: rotate(360deg); }
  }

  .foot {
    display: flex;
    align-items: center;
    gap: 16px;
    flex-wrap: wrap;
  }
  .btn {
    border: none;
    background: var(--surface-2);
    color: var(--text-1);
    border-radius: var(--r-full);
    padding: 9px 22px;
    font-size: 13.5px;
    font-weight: 700;
    font-family: inherit;
    cursor: pointer;
    transition: all var(--dur-fast) var(--ease-out);
  }
  .btn:hover {
    color: var(--primary);
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
    color: #fff;
  }
  .btn.primary:active {
    background: var(--primary-active);
  }
  .btn.ghost {
    background: transparent;
    box-shadow: none;
  }
  .btn.big {
    padding: 11px 30px;
    font-size: 14.5px;
  }
  .btn:disabled {
    opacity: 0.55;
    cursor: not-allowed;
  }
  .btn:focus-visible {
    outline: 2px solid var(--primary);
    outline-offset: 2px;
  }
  .sec {
    margin: 0;
    font-size: 11.5px;
    color: var(--text-3);
    line-height: 1.5;
    max-width: 34ch;
  }

  @media (max-width: 860px) {
    .grid {
      grid-template-columns: 1fr;
    }
    .stage {
      padding: var(--s-5);
    }
    h1 {
      font-size: 27px;
    }
  }

  @media (prefers-reduced-motion: reduce) {
    .fwait::before {
      animation: none;
    }
    .inp,
    .btn,
    .pill,
    .back {
      transition: none;
    }
  }
</style>