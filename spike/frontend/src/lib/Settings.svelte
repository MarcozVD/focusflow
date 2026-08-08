<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import {
    aiConfig,
    emailConfig,
    syncHistory,
    syncToday,
    lastSyncAt,
    nextSyncAt,
    syncRunning,
    syncProgress,
    syncSummary,
    loadAiConfig,
    loadEmailConfig,
    loadSyncStatus,
    syncNow,
    fmtMs,
    generalSettings,
    loadGeneralSettings,
    saveGeneralSettings,
    setUiPrefs,
    uiTheme,
    uiAccent,
    notifPrefs,
    loadNotifPrefs,
    saveNotifPrefs,
  } from "./data.svelte";

  const ACCENTS = ["#2563EB", "#7C3AED", "#EC4899", "#F59E0B", "#10B981", "#0EA5E9"];

  let curTheme = $state<"light" | "dark">("light");
  let curAccent = $state("#2563EB");

  $effect(() => {
    curTheme = uiTheme() === "dark" ? "dark" : "light";
    curAccent = uiAccent();
  });

  function pickTheme(t: "light" | "dark") {
    curTheme = t;
    setUiPrefs({ theme: t });
  }
  function pickAccent(c: string) {
    curAccent = c;
    setUiPrefs({ accent: c });
  }

  let saving = $state(false);
  let saved = $state("");
  let testResult = $state<null | { ok: boolean; latency_ms: number; error: string }>(null);
  let testing = $state(false);
  let verifyResult = $state<null | { ai: { ok: boolean; detail: string }; email: { ok: boolean; detail: string } }>(null);
  let verifying = $state(false);

  const todayFound = $derived(syncToday().reduce((n, h) => n + (h.items_found || 0), 0));
  const todayCreated = $derived(syncToday().reduce((n, h) => n + (h.items_processed || 0), 0));
  const todayErrors = $derived(syncToday().filter((h) => h.result === "error" || !!h.error).length);

  let aiEndpoint = $state("");
  let aiModel = $state("");
  let aiKey = $state("");
  let emailHost = $state("");
  let emailPort = $state(993);
  let emailUser = $state("");
  let emailPass = $state("");
  let emailSsl = $state(true);
  let emailEnabled = $state(false);
  let emailInterval = $state(8);
  let emailMaxAge = $state(7);
  let mailboxes = $state("");
  let senders = $state("");
  let domains = $state("");
  let keywords = $state("");
  let newTrusted = $state("");
  let trusted = $state<string[]>([]);

  let gStartWin = $state(false);
  let gStartMinimized = $state(false);
  let gCloseTray = $state(true);
  let gConflictStrict = $state(false);
  let gSaving = $state(false);

  let nEnabled = $state(true);
  let nQuietStart = $state("22:00");
  let nQuietEnd = $state("08:00");
  let nDailyCap = $state(5);
  let nFreeMinutes = $state(120);
  let nSaving = $state(false);
  let nError = $state("");

  $effect(() => {
    const p = notifPrefs();
    if (p) {
      nEnabled = p.enabled;
      nQuietStart = p.quiet_start;
      nQuietEnd = p.quiet_end;
      nDailyCap = p.daily_cap;
      nFreeMinutes = p.free_minutes;
    }
  });

  async function saveNotif() {
    nSaving = true;
    nError = "";
    const r = await saveNotifPrefs({
      enabled: nEnabled,
      quiet_start: nQuietStart,
      quiet_end: nQuietEnd,
      daily_cap: nDailyCap,
      free_minutes: nFreeMinutes,
    });
    if (!r.ok) nError = r.error ?? "error";
    await loadNotifPrefs();
    nSaving = false;
  }

  $effect(() => {
    const g = generalSettings();
    if (g) {
      gStartWin = g.start_with_windows;
      gStartMinimized = g.start_minimized;
      gCloseTray = g.close_to_tray_widget;
      gConflictStrict = g.conflict_strict;
    }
  });

  async function saveGeneral() {
    gSaving = true;
    try {
      const r = await saveGeneralSettings({
        startWithWindows: gStartWin,
        startMinimized: gStartMinimized,
        closeToTrayWidget: gCloseTray,
        conflictStrict: gConflictStrict,
      });
      await loadGeneralSettings();
      saved = r.ok ? "Ajustes de inicio guardados" : `Error: ${r.error}`;
    } finally {
      gSaving = false;
    }
  }

  $effect(() => {
    const c = aiConfig();
    if (c) {
      aiEndpoint = c.endpoint;
      aiModel = c.model;
    }
  });
  $effect(() => {
    const e = emailConfig();
    if (e) {
      emailHost = e.config.host;
      emailPort = e.config.port;
      emailUser = e.config.user;
      emailSsl = e.config.ssl;
      emailEnabled = e.enabled;
      emailInterval = e.interval_hours;
      emailMaxAge = e.max_age_days;
      mailboxes = e.config.mailboxes.join("\n");
      senders = e.config.filters.senders.join("\n");
      domains = e.config.filters.domains.join("\n");
      keywords = e.config.filters.keywords.join("\n");
      trusted = e.trusted;
    }
  });

  async function saveAi() {
    saving = true;
    saved = "";
    try {
      await invoke("ai_config_set", { endpoint: aiEndpoint, model: aiModel });
      if (aiKey) {
        await invoke("ai_set_key", { key: aiKey });
        aiKey = "";
      }
      await loadAiConfig();
      saved = "IA guardada";
    } catch (e) {
      saved = `Error: ${e}`;
    } finally {
      saving = false;
    }
  }

  async function clearKey() {
    try {
      await invoke("ai_clear_key");
      await loadAiConfig();
    } catch (e) {
      saved = `Error: ${e}`;
    }
  }

  async function testAi() {
    testing = true;
    testResult = null;
    try {
      const r = await invoke<{ ok: boolean; latency_ms: number; error: string }>("ai_test");
      testResult = r;
    } catch (e) {
      testResult = { ok: false, latency_ms: 0, error: String(e) };
    } finally {
      testing = false;
    }
  }

  async function saveEmail() {
    saving = true;
    saved = "";
    try {
      const config = {
        host: emailHost,
        port: Number(emailPort) || 993,
        user: emailUser,
        auth: "password",
        ssl: emailSsl,
        mailboxes: mailboxes.split("\n").map((s) => s.trim()).filter(Boolean),
        filters: {
          senders: senders.split("\n").map((s) => s.trim()).filter(Boolean),
          domains: domains.split("\n").map((s) => s.trim()).filter(Boolean),
          keywords: keywords.split("\n").map((s) => s.trim()).filter(Boolean),
        },
      };
      await invoke("email_config_set", {
        config,
        password: emailPass || null,
        enabled: emailEnabled,
        intervalHours: Number(emailInterval) || 8,
        maxAgeDays: Math.max(1, Number(emailMaxAge) || 7),
      });
      emailPass = "";
      await loadEmailConfig();
      saved = "Configuración de correo guardada";
    } catch (e) {
      saved = `Error: ${e}`;
    } finally {
      saving = false;
    }
  }

  async function addTrusted() {
    const t = newTrusted.trim();
    if (!t) return;
    try {
      await invoke("trusted_senders_add", { sender: t });
      newTrusted = "";
      await loadEmailConfig();
    } catch (e) {
      saved = `Error: ${e}`;
    }
  }

  async function removeTrusted(s: string) {
    await invoke("trusted_senders_remove", { sender: s });
    await loadEmailConfig();
  }

  async function verifyAll() {
    verifying = true;
    verifyResult = null;
    try {
      const r = await invoke<{ ai: { ok: boolean; detail: string }; email: { ok: boolean; detail: string } }>(
        "verify_connections",
      );
      verifyResult = r;
    } catch (e) {
      verifyResult = { ai: { ok: false, detail: String(e) }, email: { ok: false, detail: String(e) } };
    } finally {
      verifying = false;
    }
  }
</script>

<div class="set">
  <section>
    <h2>Apariencia</h2>
    <p class="hint">
      El tema y el color de acento se guardan y se aplican a la vez en la app y en el widget.
    </p>
    <div class="row">
      <button class="btn {curTheme === 'light' ? 'primary' : ''}" onclick={() => pickTheme("light")}>Claro</button>
      <button class="btn {curTheme === 'dark' ? 'primary' : ''}" onclick={() => pickTheme("dark")}>Oscuro</button>
    </div>
    <div class="accents">
      {#each ACCENTS as c}
        <button
          class="swatch {curAccent.toLowerCase() === c.toLowerCase() ? 'on' : ''}"
          style="--sw: {c}"
          onclick={() => pickAccent(c)}
          aria-label={`Acento ${c}`}
          title={c}
        >
          {#if curAccent.toLowerCase() === c.toLowerCase()}
            <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
              <path d="M2 6.5L4.5 9L10 3" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" />
            </svg>
          {/if}
        </button>
      {/each}
    </div>
  </section>

  <section>
    <h2>Verificación rápida</h2>
    <p class="hint">
      Comprueba de golpe la conexión con la API de IA (OpenCode Zen) y con tu servidor de correo.
      Guarda la configuración antes de verificar.
    </p>
    <div class="row">
      <button class="btn primary" onclick={verifyAll} disabled={verifying}>
        {verifying ? "Verificando…" : "Verificar ya"}
      </button>
    </div>
    {#if verifyResult}
      <div class="vline {verifyResult.ai.ok ? 'ok' : 'err'}">
        <span class="vdot">{verifyResult.ai.ok ? "✓" : "✗"}</span>
        <span class="vname">IA</span>
        <span class="vdetail">{verifyResult.ai.detail}</span>
      </div>
      <div class="vline {verifyResult.email.ok ? 'ok' : 'err'}">
        <span class="vdot">{verifyResult.email.ok ? "✓" : "✗"}</span>
        <span class="vname">Correo</span>
        <span class="vdetail">{verifyResult.email.detail}</span>
      </div>
    {/if}
  </section>

  <section>
    <h2>Asistente IA</h2>
    <p class="hint">
      Proveedor intercambiable (OpenCode Zen, OpenAI, Anthropic, Gemini…). API compatible con
      chat completions. Las claves se guardan cifradas en Windows Credential Manager.
    </p>
    <details class="guide">
      <summary>Paso a paso: cómo sacar la clave de API</summary>
      <ol class="steps">
        <li>
          Crea una cuenta en el proveedor de IA que quieras usar
          (OpenAI: <code>platform.openai.com</code> · OpenCode Zen: <code>opencode.ai</code>).
        </li>
        <li>
          Entra en <strong>API keys</strong>:
          OpenAI → <code>platform.openai.com/api-keys</code> · OpenCode Zen → sección
          <em>API keys</em> de tu panel.
        </li>
        <li>
          Pulsa <strong>Create new secret key</strong> (o <em>Generate key</em>).
        </li>
        <li>
          Ponle un nombre (por ejemplo «FocusFlow») y pulsa <strong>Crear</strong>.
        </li>
        <li>
          <strong>Copia la clave</strong>: empieza por <code>sk-…</code> y solo se muestra
          una vez. Si la pierdes, crea otra.
        </li>
        <li>
          Pégala aquí en <em>Clave de API</em> y pulsa <strong>Guardar IA</strong>.
        </li>
        <li>
          En <em>Endpoint</em> pon <code>https://api.openai.com/v1</code> (o el de tu proveedor)
          y en <em>Modelo</em> uno válido (por ejemplo <code>gpt-4o-mini</code>).
        </li>
        <li>
          Pulsa <strong>Probar conexión</strong> para confirmar que funciona.
        </li>
      </ol>
    </details>
    <div class="grid">
      <label>Endpoint (base URL)
        <input type="text" bind:value={aiEndpoint} placeholder="https://…/v1" />
      </label>
      <label>Modelo
        <input type="text" bind:value={aiModel} placeholder="modelo-ia" />
      </label>
    </div>
    <div class="grid">
      <label>Clave de API {aiConfig()?.has_key ? "(guardada ✓)" : "(no hay clave)"}
        <input type="password" bind:value={aiKey} placeholder="••••••••" />
      </label>
    </div>
    <div class="row">
      <button class="btn primary" onclick={saveAi} disabled={saving}>Guardar IA</button>
      <button class="btn" onclick={testAi} disabled={testing || !aiConfig()?.has_key}>
        {testing ? "Probando…" : "Probar conexión"}
      </button>
      {#if aiConfig()?.has_key}
        <button class="btn danger" onclick={clearKey}>Eliminar clave</button>
      {/if}
    </div>
    {#if testResult}
      <p class="test {testResult.ok ? 'ok' : 'err'}">
        {testResult.ok
          ? `Conexión OK en ${testResult.latency_ms} ms`
          : `Fallo: ${testResult.error}`}
      </p>
    {/if}
    {#if aiConfig() && aiConfig()!.effective_endpoint === ""}
      <p class="warn">
        Endpoint efectivo vacío. Si usas variables de entorno, reinicia la app tras configurarlas.
      </p>
    {/if}
  </section>

  <section>
    <h2>Correo electrónico</h2>
    <p class="hint">
      IMAP seguro (Gmail, Outlook, universidad…). Para Gmail usa una contraseña de aplicación.
      La contraseña se guarda cifrada en Windows Credential Manager — nunca en disco.
      La IA revisa los correos nuevos cada {emailInterval} h en segundo plano.
      Solo se revisan correos de los últimos {emailMaxAge} días (el avance queda registrado y no se repite).
    </p>
    <details class="guide">
      <summary>Paso a paso: cómo sacar la contraseña de aplicación (Gmail)</summary>
      <ol class="steps">
        <li>
          Requisito: tu cuenta de Google debe tener la
          <strong>verificación en 2 pasos</strong> activada
          (<code>myaccount.google.com/security</code> → <em>Verificación en 2 pasos</em>).
        </li>
        <li>
          Entra en <code>myaccount.google.com/apppasswords</code>
          (te pedirá confirmar tu contraseña).
        </li>
        <li>
          En <em>Nombre de la aplicación</em> escribe «FocusFlow» y pulsa <strong>Crear</strong>.
        </li>
        <li>
          Google te muestra una <strong>contraseña de 16 letras</strong> (por ejemplo
          <code>abcd efgh ijkl mnop</code>). Cópiala.
        </li>
        <li>
          Pégala aquí en <em>Contraseña de aplicación</em> (los espacios no importan) y pulsa
          <strong>Guardar correo</strong>.
        </li>
        <li>
          Deja <em>Servidor IMAP</em> en <code>imap.gmail.com</code>, puerto <code>993</code> y
          <em>SSL: Sí</em>.
        </li>
        <li>
          Pulsa <strong>Comprobar ahora</strong> para verificar la conexión.
        </li>
      </ol>
      <p class="hint">
        ¿No usas Gmail? Outlook: <code>outlook.office365.com</code> con tu contraseña normal.
        Universidades: usa el servidor IMAP que te den y, si piden contraseña de aplicación,
        sácala del panel de tu institución.
      </p>
    </details>
    <label class="check">
      <input type="checkbox" bind:checked={emailEnabled} />
      Revisar correo automáticamente
    </label>
    <div class="grid">
      <label>Servidor IMAP
        <input type="text" bind:value={emailHost} placeholder="imap.gmail.com" />
      </label>
      <label>Puerto
        <input type="number" bind:value={emailPort} />
      </label>
      <label>SSL
        <select bind:value={emailSsl}>
          <option value={true}>Sí (993)</option>
          <option value={false}>No (143)</option>
        </select>
      </label>
    </div>
    <div class="grid">
      <label>Usuario
        <input type="text" bind:value={emailUser} placeholder="tucorreo@gmail.com" />
      </label>
      <label>Contraseña de aplicación {emailConfig()?.has_password ? "(guardada ✓)" : ""}
        <input type="password" bind:value={emailPass} placeholder="••••••••" />
      </label>
      <label>Frecuencia de revisión (horas)
        <input type="number" min="1" bind:value={emailInterval} />
      </label>
      <label>Revisar solo correos de los últimos (días)
        <input type="number" min="1" max="90" bind:value={emailMaxAge} />
      </label>
    </div>
    <div class="grid">
      <label>Carpetas / etiquetas (una por línea)
        <textarea bind:value={mailboxes} rows="3" placeholder="INBOX"></textarea>
      </label>
      <label>Solo remitentes (una por línea)
        <textarea bind:value={senders} rows="3" placeholder="profesor@universidad.edu"></textarea>
      </label>
      <label>Solo dominios (una por línea)
        <textarea bind:value={domains} rows="3" placeholder="universidad.edu"></textarea>
      </label>
      <label>Solo palabras clave (una por línea)
        <textarea bind:value={keywords} rows="3" placeholder="examen, entrega, reunión"></textarea>
      </label>
    </div>
    <div class="row">
      <button class="btn primary" onclick={saveEmail} disabled={saving}>Guardar correo</button>
      <button class="btn primary-solid" onclick={syncNow} disabled={syncRunning()}>
        {syncRunning() ? "Comprobando…" : "Comprobar ahora"}
      </button>
    </div>

    {#if syncRunning()}
      <div class="sync-run">
        <div class="progress-track">
          <div
            class="progress-bar"
            style="width: {syncProgress() && syncProgress()!.total > 0
              ? Math.max(6, (syncProgress()!.processed / syncProgress()!.total) * 100)
              : 8}%"
          ></div>
        </div>
        <p class="hint">
          {syncProgress()
            ? `Revisando correos de ${syncProgress()!.mailbox}: ${syncProgress()!.processed}/${syncProgress()!.total} analizados con IA…`
            : "Conectando y revisando correos nuevos…"}
        </p>
      </div>
    {/if}

    {#if syncSummary()}
      <div class="sync-summary">
        {#if syncSummary()!.error}
          <p class="sum-err">Error: {syncSummary()!.error}</p>
        {:else}
          <div class="sum-grid">
            <div class="sum-item">
              <strong>{syncSummary()!.total_found}</strong>
              <span>correos revisados</span>
            </div>
            <div class="sum-item">
              <strong>{syncSummary()!.total_suggestions}</strong>
              <span>eventos detectados</span>
            </div>
            <div class="sum-item">
              <strong>{syncSummary()!.mailboxes.length}</strong>
              <span>bandejas</span>
            </div>
          </div>
          <p class="hint">
            Revisa el panel Sugerencias para aceptar, fusionar o rechazar los nuevos eventos.
          </p>
        {/if}
      </div>
    {/if}

    <h3>Remitentes de confianza (aprobación automática)</h3>
    <p class="hint">
      Los eventos de estos remitentes se añaden al calendario sin pasar por la bandeja.
    </p>
    <div class="row">
      <input class="t" type="text" bind:value={newTrusted} placeholder="profesor@universidad.edu" />
      <button class="btn" onclick={addTrusted}>Añadir</button>
    </div>
    {#if trusted.length > 0}
      <div class="tags">
        {#each trusted as t}
          <span class="tag">
            {t}
            <button onclick={() => removeTrusted(t)} title="Quitar">×</button>
          </span>
        {/each}
      </div>
    {/if}
  </section>

  <section>
    <h2>Inicio y bandeja</h2>
    <p class="hint">
      Controla cómo se comporta FocusFlow al iniciar Windows, al arrancar y al cerrar la ventana.
    </p>
    <label class="check">
      <input type="checkbox" bind:checked={gStartWin} />
      Iniciar FocusFlow al iniciar Windows
    </label>
    {#if gStartWin && !generalSettings()?.autostart_actual}
      <p class="warn">La entrada de inicio aún no existe (guarda los cambios para crearla).</p>
    {/if}
    <label class="check">
      <input type="checkbox" bind:checked={gStartMinimized} />
      Al abrir la app, empezar en segundo plano y mostrar solo el widget
    </label>
    <label class="check">
      <input type="checkbox" bind:checked={gCloseTray} />
      Al cerrar la ventana, minimizar a la bandeja y abrir el widget
    </label>
    <label class="check">
      <input type="checkbox" bind:checked={gConflictStrict} />
      Bloquear movimientos que solapan otra tarea (si no, se permiten con aviso)
    </label>
    <div class="row">
      <button class="btn primary" onclick={saveGeneral} disabled={gSaving}>
        {gSaving ? "Guardando…" : "Guardar ajustes de inicio"}
      </button>
    </div>
  </section>

  <section>
    <h2>Notificaciones contextuales</h2>
    <p class="hint">
      FocusFlow te avisa solo cuando hay algo útil: vencimientos, tareas atrasadas, conflictos de
      horario o tiempo libre para preparar. Sin spam: hay cadencia por tipo y tope diario.
    </p>
    <label class="check">
      <input type="checkbox" bind:checked={nEnabled} />
      Notificaciones contextuales activadas
    </label>
    <div class="row">
      <label>
        <span class="lbl">Horario de silencio (desde)</span>
        <input class="t" type="time" bind:value={nQuietStart} disabled={!nEnabled} />
      </label>
      <label>
        <span class="lbl">Hasta</span>
        <input class="t" type="time" bind:value={nQuietEnd} disabled={!nEnabled} />
      </label>
    </div>
    <div class="row">
      <label>
        <span class="lbl">Tope diario</span>
        <input class="t" type="number" min="1" max="20" bind:value={nDailyCap} disabled={!nEnabled} />
      </label>
      <label>
        <span class="lbl">Tiempo libre mínimo para sugerir (min)</span>
        <input class="t" type="number" min="30" max="600" step="15" bind:value={nFreeMinutes} disabled={!nEnabled} />
      </label>
    </div>
    {#if nError}
      <p class="warn">{nError}</p>
    {/if}
    <div class="row">
      <button class="btn primary" onclick={saveNotif} disabled={nSaving}>
        {nSaving ? "Guardando…" : "Guardar notificaciones"}
      </button>
    </div>
  </section>

  <section>
    <h2>Sincronización de Hoy</h2>
    <p class="hint">
      Resumen de la última sincronización del día de hoy. El historial completo queda aparte.
    </p>
    <div class="stats">
      <div class="stat">
        <span class="k">Última sincronización</span>
        <span class="v">{lastSyncAt() ? fmtMs(lastSyncAt()) : "aún no hoy"}</span>
      </div>
      <div class="stat">
        <span class="k">Correos revisados hoy</span>
        <span class="v">{todayFound}</span>
      </div>
      <div class="stat">
        <span class="k">Nuevas tareas hoy</span>
        <span class="v">{todayCreated}</span>
      </div>
      <div class="stat">
        <span class="k">Errores hoy</span>
        <span class="v">{todayErrors} {todayErrors > 0 ? "⚠" : ""}</span>
      </div>
      <div class="stat">
        <span class="k">Próxima sincronización</span>
        <span class="v">{nextSyncAt() ? fmtMs(nextSyncAt()) : "—"}</span>
      </div>
    </div>
    {#if syncRunning()}
      <p class="hint">Sincronizando…</p>
    {/if}
    {#if todayErrors > 0}
      <div class="errbox">
        {#each syncToday() as h (h.id)}
          {#if h.error || h.result === "error"}
            <p>{h.source}: {h.error || h.result}</p>
          {/if}
        {/each}
      </div>
    {/if}
    {#if syncToday().length === 0 && !syncRunning()}
      <p class="hint">Sin sincronizaciones registradas hoy.</p>
    {/if}
    <details>
      <summary>Historial completo</summary>
      {#if syncHistory().length > 0}
        {#each syncHistory() as h (h.id)}
          <div class="syncrow">
            <span class="src">{h.source}</span>
            <span class="res {h.result === 'ok' ? 'ok' : h.result === 'error' ? 'err' : ''}">{h.result}</span>
            <span class="when">{fmtMs(h.started_at)}</span>
            <span class="when">encontrados {h.items_found} · procesados {h.items_processed}</span>
          </div>
        {/each}
      {:else}
        <p class="hint">Sin entradas.</p>
      {/if}
    </details>
  </section>

  {#if saved}
    <p class="saved">{saved}</p>
  {/if}
</div>

<style>
  .set {
    max-width: 760px;
    display: flex;
    flex-direction: column;
    gap: var(--s-6);
    padding-bottom: var(--s-8);
  }
  section {
    background: var(--surface);
    border-radius: var(--r-lg);
    box-shadow: var(--e1);
    padding: var(--s-6);
    display: flex;
    flex-direction: column;
    gap: 12px;
  }
  h2 {
    margin: 0;
    font-size: 17px;
  }
  h3 {
    margin: 8px 0 0;
    font-size: 14px;
  }
  .hint {
    margin: 0;
    font-size: 12.5px;
    color: var(--text-3);
    line-height: 1.5;
  }
  .guide {
    margin: 6px 0 4px;
    border: 1px solid var(--border);
    border-radius: var(--r-md);
    background: var(--surface-2);
    padding: 8px 12px;
    font-size: 12.5px;
  }
  .guide summary {
    cursor: pointer;
    font-weight: 600;
    color: var(--text-2);
    user-select: none;
  }
  .guide summary:hover {
    color: var(--primary);
  }
  .steps {
    margin: 8px 0 4px;
    padding-left: 18px;
    display: flex;
    flex-direction: column;
    gap: 5px;
    color: var(--text-1);
    line-height: 1.45;
  }
  .steps code {
    background: var(--surface-3);
    border-radius: 4px;
    padding: 1px 5px;
    font-size: 11.5px;
  }
  .grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(200px, 1fr));
    gap: 10px;
  }
  label {
    display: flex;
    flex-direction: column;
    gap: 5px;
    font-size: 12px;
    font-weight: 600;
    color: var(--text-2);
  }
  .check {
    flex-direction: row;
    align-items: center;
    gap: 8px;
  }
  input,
  select,
  textarea {
    border: 1px solid var(--border);
    background: var(--surface-3);
    border-radius: 10px;
    padding: 9px 12px;
    font-size: 13.5px;
    color: var(--text-1);
    font-family: inherit;
    outline: none;
  }
  input:focus,
  select:focus,
  textarea:focus {
    border-color: var(--primary);
  }
  textarea {
    resize: vertical;
  }
  .row {
    display: flex;
    gap: 8px;
    flex-wrap: wrap;
    align-items: center;
  }
  .lbl {
    display: block;
    font-size: 12px;
    color: var(--text-2);
    margin-bottom: 4px;
  }
  .accents {
    display: flex;
    gap: 10px;
    flex-wrap: wrap;
  }
  .swatch {
    width: 30px;
    height: 30px;
    border-radius: 50%;
    border: none;
    background: var(--sw);
    display: inline-flex;
    align-items: center;
    justify-content: center;
    color: #fff;
    cursor: pointer;
    transition: transform var(--dur-fast) var(--ease-out), box-shadow var(--dur-fast) var(--ease-out);
    box-shadow: 0 2px 6px -2px color-mix(in srgb, var(--sw) 55%, transparent);
  }
  .swatch:hover {
    transform: scale(1.12);
  }
  .swatch.on {
    box-shadow: 0 0 0 3px var(--surface), 0 0 0 5px var(--sw);
    transform: scale(1.08);
  }
  .btn {
    border: none;
    background: var(--surface-3);
    color: var(--text-1);
    border-radius: 12px;
    padding: 9px 18px;
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
  .btn.primary-solid {
    background: var(--primary);
    color: #fff;
    box-shadow: 0 4px 12px -2px color-mix(in srgb, var(--primary) 50%, transparent);
  }
  .btn.primary-solid:hover {
    background: var(--primary-hover);
  }
  .btn.danger {
    color: var(--danger);
  }
  .btn:disabled {
    opacity: 0.5;
    pointer-events: none;
  }
  .sync-run {
    background: var(--surface-3);
    border-radius: var(--r-md);
    padding: var(--s-4);
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .progress-track {
    height: 8px;
    border-radius: var(--r-full);
    background: var(--surface-2);
    box-shadow: var(--shadow-inset-sm);
    overflow: hidden;
  }
  .progress-bar {
    height: 100%;
    border-radius: var(--r-full);
    background: linear-gradient(90deg, var(--primary), color-mix(in srgb, var(--primary) 60%, var(--success)));
    transition: width var(--dur-base) var(--ease-out);
  }
  .sync-summary {
    background: color-mix(in srgb, var(--success) 8%, var(--surface));
    border: 1px solid color-mix(in srgb, var(--success) 25%, transparent);
    border-radius: var(--r-md);
    padding: var(--s-4);
    display: flex;
    flex-direction: column;
    gap: 8px;
  }
  .sum-grid {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(110px, 1fr));
    gap: 10px;
  }
  .sum-item {
    background: var(--surface);
    border-radius: 12px;
    padding: var(--s-3);
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
    box-shadow: var(--e1);
  }
  .sum-item strong {
    font-size: 22px;
    font-weight: 700;
    color: var(--primary);
    font-variant-numeric: tabular-nums;
  }
  .sum-item span {
    font-size: 11px;
    color: var(--text-3);
    font-weight: 600;
    text-align: center;
  }
  .sum-err {
    margin: 0;
    font-size: 13px;
    color: var(--danger);
    font-weight: 600;
  }
  .t {
    flex: 1;
    min-width: 200px;
  }
  .test,
  .saved,
  .warn {
    margin: 0;
    font-size: 13px;
    font-weight: 600;
  }
  .test.ok,
  .saved {
    color: var(--success);
  }
  .test.err,
  .warn {
    color: var(--warning);
  }
  .tags {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
  }
  .tag {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    background: var(--surface-3);
    border-radius: var(--r-full);
    padding: 5px 12px;
    font-size: 12px;
    font-weight: 600;
  }
  .tag button {
    border: none;
    background: transparent;
    color: var(--text-3);
    font-size: 14px;
    cursor: pointer;
  }
  .syncrow {
    display: flex;
    gap: 10px;
    align-items: center;
    font-size: 12.5px;
    padding: 6px 0;
    border-bottom: 1px solid var(--border);
    flex-wrap: wrap;
  }
  .src {
    font-weight: 700;
  }
  .res {
    font-weight: 700;
    padding: 2px 8px;
    border-radius: var(--r-full);
    background: var(--surface-3);
    font-size: 11px;
  }
  .res.ok {
    color: var(--success);
    background: var(--success-bg);
  }
  .res.err {
    color: var(--danger);
    background: var(--danger-bg);
  }
  .when {
    color: var(--text-3);
  }
  .errtext {
    color: var(--danger);
  }
  .stats {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(150px, 1fr));
    gap: 10px;
  }
  .stat {
    background: var(--surface-2);
    border-radius: var(--r-md);
    padding: 12px 14px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }
  .stat .k {
    font-size: 10px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.07em;
    color: var(--text-3);
  }
  .stat .v {
    font-size: 15px;
    font-weight: 700;
    color: var(--text-1);
  }
  .errbox {
    background: var(--danger-bg);
    border: 1px solid color-mix(in srgb, var(--danger) 35%, transparent);
    border-radius: var(--r-md);
    padding: 10px 14px;
  }
  .errbox p {
    margin: 2px 0;
    font-size: 12px;
    color: var(--danger);
  }
  details {
    border-top: 1px solid var(--border);
    padding-top: 10px;
  }
  details summary {
    cursor: pointer;
    font-size: 12.5px;
    font-weight: 700;
    color: var(--text-2);
    user-select: none;
  }
  details summary:hover {
    color: var(--primary);
  }
  .vline {
    display: flex;
    align-items: center;
    gap: 10px;
    font-size: 13px;
    padding: 8px 12px;
    border-radius: 10px;
    background: var(--surface-3);
  }
  .vline.ok {
    color: var(--success);
  }
  .vline.err {
    color: var(--danger);
  }
  .vdot {
    font-weight: 800;
    font-size: 14px;
  }
  .vname {
    font-weight: 700;
  }
  .vdetail {
    color: var(--text-2);
  }
</style>
