<script lang="ts">
  import { onMount } from "svelte";
  import { fade } from "svelte/transition";
  import { invoke } from "@tauri-apps/api/core";
  import { onboarding, completeOnboarding } from "./data.svelte";

  type ProviderId = "gmail" | "outlook" | "yahoo" | "other";

  interface Provider {
    label: string;
    host: string;
    port: number;
    steps: string[];
    tip: string;
  }

  const PROVIDERS: Record<ProviderId, Provider> = {
    gmail: {
      label: "Gmail",
      host: "imap.gmail.com",
      port: 993,
      steps: [
        "En tu cuenta de Google activa la verificación en dos pasos (Seguridad → Verificación en 2 pasos).",
        "Crea una contraseña de aplicación: Seguridad → Contraseñas de aplicaciones.",
        "Copia la contraseña de 16 caracteres y pégala aquí.",
      ],
      tip: "FocusFlow leerá tu bandeja, detectará fechas, horas y compromisos, y te propondrá añadirlos al calendario. La contraseña se guarda cifrada en Windows.",
    },
    outlook: {
      label: "Outlook / Hotmail",
      host: "outlook.office365.com",
      port: 993,
      steps: [
        "Activa la verificación en dos pasos de tu cuenta Microsoft (si aún no la tienes).",
        "Crea una contraseña de aplicación en la configuración de seguridad de Microsoft.",
        "Copia esa contraseña y pégala aquí.",
      ],
      tip: "FocusFlow leerá tu bandeja, detectará fechas, horas y compromisos, y te propondrá añadirlos al calendario. La contraseña se guarda cifrada en Windows.",
    },
    yahoo: {
      label: "Yahoo",
      host: "imap.mail.yahoo.com",
      port: 993,
      steps: [
        "Genera una contraseña de aplicación en la configuración de seguridad de Yahoo.",
        "Copia la contraseña de aplicación generada.",
        "Pégala aquí: tu contraseña normal no sirve para IMAP.",
      ],
      tip: "FocusFlow leerá tu bandeja, detectará fechas, horas y compromisos, y te propondrá añadirlos al calendario. La contraseña se guarda cifrada en Windows.",
    },
    other: {
      label: "Otro servidor",
      host: "",
      port: 993,
      steps: [
        "Escribe el servidor IMAP de tu proveedor (host y puerto suelen aparecer en su ayuda).",
        "Usa tu dirección de correo completa como usuario.",
        "Si tu proveedor exige TLS (recomendado), deja SSL activado.",
      ],
      tip: "Cualquier servidor IMAP funciona. FocusFlow nunca guarda tu contraseña en la base de datos: va cifrada al administrador de credenciales de Windows.",
    },
  };

  const VALID_EMAIL = /^[^\s@]+@[^\s@]+\.[^\s@]{2,}$/;

  let step = $state(1);
  let reduced = $state(false);

  let provider = $state<ProviderId>("gmail");
  let emailUser = $state("");
  let emailPass = $state("");
  let host = $state("imap.gmail.com");
  let port = $state(993);
  let useSsl = $state(true);

  let aiEndpoint = $state("");
  let aiModel = $state("");
  let aiKey = $state("");

  let emailState = $state<"idle" | "loading" | "ok" | "error" | "skip">("idle");
  let aiState = $state<"idle" | "loading" | "ok" | "error" | "skip">("idle");
  let emailDetail = $state("");
  let aiDetail = $state("");
  let emailRaw = $state("");
  let aiRaw = $state("");
  let fieldErr = $state<{ user?: string; pass?: string; host?: string; key?: string; endpoint?: string }>({});
  let busy = $state(false);

  function detectProvider(h: string): ProviderId {
    const l = h.toLowerCase();
    if (l.includes("gmail")) return "gmail";
    if (l.includes("outlook") || l.includes("office365") || l.includes("hotmail")) return "outlook";
    if (l.includes("yahoo")) return "yahoo";
    return "other";
  }

  onMount(() => {
    reduced = window.matchMedia("(prefers-reduced-motion: reduce)").matches;
    const o = onboarding();
    if (o) {
      const em = o.email;
      if (em) {
        provider = detectProvider(em.host);
        host = em.host;
        port = em.port ?? 993;
        useSsl = em.ssl ?? true;
        emailUser = em.user || "";
      }
      aiEndpoint = o.ai.endpoint;
      aiModel = o.ai.model;
    }
  });

  function pickProvider(p: ProviderId) {
    provider = p;
    host = PROVIDERS[p].host;
    port = PROVIDERS[p].port;
    fieldErr = {};
  }

  function setHostManual(v: string) {
    host = v;
    provider = v ? detectProvider(v) : provider;
  }

  const emailFilled = $derived(emailUser.trim() !== "" || emailPass !== "");
  const aiFilled = $derived(aiEndpoint.trim() !== "" || aiModel.trim() !== "" || aiKey !== "");

  function validateFields(): boolean {
    const errs: typeof fieldErr = {};
    if (emailFilled) {
      if (!emailUser.trim()) errs.user = "Escribe tu dirección de correo.";
      else if (!VALID_EMAIL.test(emailUser.trim())) errs.user = "Esa dirección no parece válida.";
      if (!emailPass) errs.pass = "Escribe la contraseña (para Gmail y Outlook, la contraseña de aplicación).";
      if (!host.trim()) errs.host = "Escribe el servidor IMAP.";
    }
    if (aiFilled) {
      if (!aiEndpoint.trim()) errs.endpoint = "Escribe el endpoint de la API.";
      if (!aiModel.trim()) errs.endpoint = errs.endpoint || "Escribe el nombre del modelo.";
      if (!aiKey && !onboarding()?.ai.has_key) errs.key = "Escribe la clave de API.";
    }
    fieldErr = errs;
    return Object.keys(errs).length === 0;
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
      emailState = "idle";
      aiState = "idle";
      return;
    }
    busy = true;
    emailState = emailFilled ? "loading" : "skip";
    aiState = aiFilled ? "loading" : "skip";
    emailDetail = "";
    aiDetail = "";
    emailRaw = "";
    aiRaw = "";

    try {
      if (aiFilled) {
        await invoke("ai_config_set", { endpoint: aiEndpoint.trim(), model: aiModel.trim() });
        if (aiKey) await invoke("ai_set_key", { key: aiKey });
      }
      if (emailFilled) {
        await invoke("email_config_set", {
          config: {
            host: host.trim(),
            port: Number(port) || 993,
            user: emailUser.trim(),
            auth: "password",
            ssl: useSsl,
            mailboxes: ["INBOX"],
            filters: { senders: [], domains: [], keywords: [] },
          },
          password: emailPass,
          enabled: true,
          intervalHours: 8,
          maxAgeDays: 7,
        });
      }
      const r = await invoke<{ ai: { ok: boolean; detail: string }; email: { ok: boolean; detail: string } }>(
        "verify_connections",
      );
      if (aiFilled) {
        aiState = r.ai.ok ? "ok" : "error";
        aiDetail = r.ai.detail;
        aiRaw = r.ai.ok ? "" : r.ai.detail;
      }
      if (emailFilled) {
        emailState = r.email.ok ? "ok" : "error";
        emailDetail = r.email.detail;
        emailRaw = r.email.ok ? "" : r.email.detail;
      }
      const failed = (emailFilled && emailState === "error") || (aiFilled && aiState === "error");
      if (!failed) {
        await completeOnboarding();
      }
    } catch (e) {
      const msg = String(e);
      if (emailFilled) {
        emailState = "error";
        emailDetail = humanError(msg, "email");
        emailRaw = msg;
      }
      if (aiFilled) {
        aiState = "error";
        aiDetail = humanError(msg, "ai");
        aiRaw = msg;
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
  <div class="stage" in:fade={{ duration: reduced ? 0 : 500 }}>
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
          <span class="vi" aria-hidden="true">✉</span>
          <div>
            <strong>Correo conectado</strong>
            <p>Las fechas de tus correos llegan solas como sugerencias.</p>
          </div>
        </li>
        <li>
          <span class="vi" aria-hidden="true">✦</span>
          <div>
            <strong>IA para planear</strong>
            <p>Describe una tarea en lenguaje natural y recibe un plan con horarios.</p>
          </div>
        </li>
        <li>
          <span class="vi" aria-hidden="true">🛡</span>
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
  <div class="stage" in:fade={{ duration: reduced ? 0 : 350 }}>
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
            <h2>{PROVIDERS[provider].label}</h2>
            <p class="g-tip">{PROVIDERS[provider].tip}</p>
            <ol class="g-steps">
              {#each PROVIDERS[provider].steps as s}
                <li>{s}</li>
              {/each}
            </ol>
          </div>
        </aside>

        <form class="form" onsubmit={(e) => { e.preventDefault(); continueSetup(); }}>
          <fieldset>
            <legend>Correo (IMAP)</legend>

            <div class="frow">
              <span class="lbl" id="ob-provider">Proveedor</span>
              <div class="pills" role="group" aria-labelledby="ob-provider">
                {#each Object.entries(PROVIDERS) as [id, p]}
                  <button
                    type="button"
                    class="pill {provider === id ? 'on' : ''}"
                    onclick={() => pickProvider(id as ProviderId)}
                    aria-pressed={provider === id}
                  >
                    {p.label}
                  </button>
                {/each}
              </div>
            </div>

            <div class="frow">
              <label class="lbl" for="ob-user">Dirección de correo</label>
              <input
                id="ob-user"
                type="email"
                class={fieldClass(emailState, emailUser !== "")}
                placeholder="tucorreo@ejemplo.com"
                bind:value={emailUser}
                autocomplete="username"
                disabled={busy}
                aria-invalid={!!fieldErr.user}
              />
              {#if fieldErr.user}
                <p class="ferr">{fieldErr.user}</p>
              {/if}
            </div>

            <div class="frow">
              <label class="lbl" for="ob-pass">Contraseña o contraseña de aplicación</label>
              <input
                id="ob-pass"
                type="password"
                class={fieldClass(emailState, emailUser !== "")}
                placeholder="••••••••••••••••"
                bind:value={emailPass}
                autocomplete="current-password"
                disabled={busy}
                aria-invalid={!!fieldErr.pass}
              />
              {#if fieldErr.pass}
                <p class="ferr">{fieldErr.pass}</p>
              {/if}
            </div>

            <div class="frow srv">
              <div class="srv-field">
                <label class="lbl" for="ob-host">Servidor</label>
                <input
                  id="ob-host"
                  type="text"
                  class={fieldClass(emailState, emailUser !== "")}
                  bind:value={host}
                  oninput={(e) => setHostManual(e.currentTarget.value)}
                  disabled={busy}
                  aria-invalid={!!fieldErr.host}
                />
                {#if fieldErr.host}
                  <p class="ferr">{fieldErr.host}</p>
                {/if}
              </div>
              <div class="srv-field small">
                <label class="lbl" for="ob-port">Puerto</label>
                <input id="ob-port" type="number" class="inp" bind:value={port} disabled={busy} />
              </div>
              <label class="check">
                <input type="checkbox" bind:checked={useSsl} disabled={busy} />
                SSL
              </label>
            </div>

            {#if emailState === "ok"}
              <p class="fok" role="status">✓ {emailDetail || "Correo conectado"}</p>
            {:else if emailState === "error"}
              <div class="ferrbox" role="alert">
                <p>{emailDetail}</p>
                {#if emailRaw}
                  <details>
                    <summary>Ver detalles</summary>
                    <pre>{emailRaw}</pre>
                  </details>
                {/if}
              </div>
            {:else if emailState === "skip"}
              <p class="fskip">Omitido: puedes conectarlo después desde Ajustes.</p>
            {:else if emailState === "loading"}
              <p class="fwait">Comprobando correo…</p>
            {/if}
          </fieldset>

          <fieldset>
            <legend>Asistente con IA</legend>
            <p class="fs-hint">
              El asistente entiende lenguaje natural y prepara planes de estudio, trabajo o
              personales.
            </p>

            <div class="frow">
              <label class="lbl" for="ob-endpoint">Endpoint de la API</label>
              <input
                id="ob-endpoint"
                type="text"
                class={fieldClass(aiState, aiEndpoint !== "" || aiModel !== "" || aiKey !== "")}
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
                class={fieldClass(aiState, aiEndpoint !== "" || aiModel !== "" || aiKey !== "")}
                bind:value={aiModel}
                placeholder="gpt-4o-mini"
                disabled={busy}
              />
            </div>

            <div class="frow">
              <label class="lbl" for="ob-key">
                Clave de API{onboarding()?.ai.has_key ? " (ya guardada, déjala en blanco)" : ""}
              </label>
              <input
                id="ob-key"
                type="password"
                class={fieldClass(aiState, aiEndpoint !== "" || aiModel !== "" || aiKey !== "")}
                placeholder={onboarding()?.ai.has_key ? "•••••••• (guardada)" : "sk-…"}
                bind:value={aiKey}
                autocomplete="off"
                disabled={busy}
                aria-invalid={!!fieldErr.key}
              />
              {#if fieldErr.key}
                <p class="ferr">{fieldErr.key}</p>
              {/if}
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
              Las claves van cifradas al administrador de credenciales de Windows. Nada se guarda
              en la base de datos.
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
  }
  .stage::before {
    content: "";
    position: absolute;
    inset: 0;
    z-index: -1;
    pointer-events: none;
    background:
      radial-gradient(720px 420px at 18% -8%, color-mix(in srgb, var(--primary) 14%, transparent), transparent 62%),
      radial-gradient(640px 400px at 92% 110%, color-mix(in srgb, #8b5cf6 12%, transparent), transparent 60%);
  }

  .hero {
    margin: auto;
    max-width: 620px;
    text-align: center;
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: var(--s-5);
  }
  .logo {
    width: 76px;
    height: 76px;
    display: grid;
    place-items: center;
    border-radius: 20px;
    background: var(--surface);
    box-shadow: var(--e2);
    border: 1px solid var(--border);
  }
  h1 {
    margin: 0;
    font-size: 34px;
    line-height: 1.16;
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
    font-size: 15.5px;
    line-height: 1.6;
    color: var(--text-2);
    max-width: 52ch;
  }
  .values {
    list-style: none;
    margin: var(--s-2) 0 0;
    padding: 0;
    display: flex;
    flex-direction: column;
    gap: var(--s-3);
    width: 100%;
    max-width: 460px;
    text-align: left;
  }
  .values li {
    display: flex;
    gap: 14px;
    align-items: flex-start;
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--r-md);
    padding: 14px 16px;
    box-shadow: var(--e1);
  }
  .vi {
    width: 34px;
    height: 34px;
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
    margin: 2px 0 0;
    font-size: 12.5px;
    color: var(--text-3);
    line-height: 1.45;
  }
  .actions {
    display: flex;
    gap: 12px;
    align-items: center;
    margin-top: var(--s-2);
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
    border: 1px solid var(--border);
    background: var(--surface);
    color: var(--text-2);
    font-size: 16px;
    cursor: pointer;
    transition: all var(--dur-fast) var(--ease-out);
  }
  .back:hover {
    color: var(--primary);
    border-color: var(--primary);
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
  .guide .g-card {
    background: var(--surface);
    border: 1px solid var(--border);
    border-radius: var(--r-lg);
    box-shadow: var(--e1);
    padding: var(--s-6);
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  .g-card h2 {
    margin: 0;
    font-size: 17px;
    color: var(--text-1);
  }
  .g-tip {
    margin: 0;
    font-size: 13px;
    line-height: 1.55;
    color: var(--text-2);
    background: var(--primary-soft);
    border-radius: var(--r-sm);
    padding: 10px 12px;
  }
  .g-steps {
    margin: 4px 0 0;
    padding-left: 20px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    font-size: 13px;
    line-height: 1.5;
    color: var(--text-2);
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
    border: 1px solid var(--border);
    border-radius: var(--r-lg);
    background: var(--surface);
    box-shadow: var(--e1);
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
    border: 1px solid var(--border);
    background: var(--surface-3);
    border-radius: 10px;
    padding: 10px 12px;
    font-size: 13.5px;
    color: var(--text-1);
    font-family: inherit;
    outline: none;
    transition: border-color var(--dur-fast) var(--ease-out), box-shadow var(--dur-fast) var(--ease-out);
  }
  .inp:focus {
    border-color: var(--primary);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--primary) 18%, transparent);
  }
  .inp.filled {
    border-color: color-mix(in srgb, var(--text-3) 55%, var(--border));
  }
  .inp.ok {
    border-color: var(--success);
  }
  .inp.ok:focus {
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--success) 18%, transparent);
  }
  .inp.err {
    border-color: var(--danger);
  }
  .inp.err:focus {
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--danger) 18%, transparent);
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
    border: 1px solid var(--border);
    background: var(--surface-3);
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
    border-color: color-mix(in srgb, var(--primary) 45%, transparent);
  }
  .pill.on {
    background: var(--primary-soft);
    border-color: var(--primary);
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
  .check input {
    accent-color: var(--primary);
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
    border: 1px solid var(--border);
    background: var(--surface);
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
    border-color: color-mix(in srgb, var(--primary) 45%, transparent);
    color: var(--primary);
  }
  .btn.primary {
    background: var(--primary);
    border-color: var(--primary);
    color: #fff;
  }
  .btn.primary:hover {
    background: var(--primary-hover);
  }
  .btn.primary:active {
    background: var(--primary-active);
  }
  .btn.ghost {
    background: transparent;
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