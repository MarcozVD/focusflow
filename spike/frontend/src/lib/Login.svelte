<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { signInWithGoogle, loadAuthStatus } from "./data.svelte";

  let busy = $state(false);
  let err = $state("");

  async function doSignIn() {
    if (busy) return;
    busy = true;
    err = "";
    try {
      const r = await signInWithGoogle();
      if (!r.ok) {
        err = r.error ?? "No se pudo conectar con Google.";
      } else {
        await loadAuthStatus();
      }
    } catch (e) {
      err = String(e);
    } finally {
      busy = false;
    }
  }

  // Guía de primeros pasos (incluye cómo obtener una cuenta de Google).
  // Misma URL whitelisteada en el comando open_website del backend.
  async function openGuide() {
    try {
      await invoke("open_website", { url: "https://flowfocus.site/legal/empezar.html" });
    } catch {
      // fallback directo si el comando falla
      window.open("https://accounts.google.com/signup", "_blank");
    }
  }
</script>

<div class="stage">
  <div class="card">
    <div class="logo">
      <img src="/icon.png" alt="Icono de FocusFlow" />
    </div>
    <h1>FocusFlow</h1>
    <p class="sub">
      Tus tareas, tu correo y tu agenda en un solo sitio. Inicia sesión con tu cuenta de Google
      para empezar.
    </p>
    <button class="btn google" onclick={doSignIn} disabled={busy}>
      <svg viewBox="0 0 48 48" width="20" height="20" aria-hidden="true">
        <path fill="#EA4335" d="M24 9.5c3.54 0 6.71 1.22 9.21 3.6l6.85-6.85C35.9 2.38 30.47 0 24 0 14.62 0 6.51 5.38 2.56 13.22l7.98 6.19C12.43 13.72 17.74 9.5 24 9.5z"/>
        <path fill="#4285F4" d="M46.98 24.55c0-1.57-.15-3.09-.38-4.55H24v9.02h12.94c-.58 2.96-2.26 5.48-4.78 7.18l7.73 6c4.51-4.18 7.09-10.36 7.09-17.65z"/>
        <path fill="#FBBC05" d="M10.53 28.59c-.48-1.45-.76-2.99-.76-4.59s.27-3.14.76-4.59l-7.98-6.19C.92 16.46 0 20.12 0 24c0 3.88.92 7.54 2.56 10.78l7.97-6.19z"/>
        <path fill="#34A853" d="M24 48c6.48 0 11.93-2.13 15.89-5.81l-7.73-6c-2.15 1.45-4.92 2.3-8.16 2.3-6.26 0-11.57-4.22-13.47-9.91l-7.98 6.19C6.51 42.62 14.62 48 24 48z"/>
      </svg>
      {busy ? "Abriendo navegador…" : "Iniciar sesión con Google"}
    </button>
    {#if err}
      <p class="err" role="alert">{err}</p>
    {/if}
    <p class="hint">
      Se abre tu navegador para autorizar. Tu cuenta de Google se usa solo para leer Gmail y
      sincronizar tu agenda. Sin sesión no puedes usar la app.
    </p>
    <p class="hint">
      ¿No tienes cuenta de Google? Se crea gratis en un minuto —
      <button class="linklike" onclick={openGuide}>ver guía para empezar</button>
      (o créala directamente en
      <button class="linklike" onclick={() => window.open("https://accounts.google.com/signup", "_blank")}>accounts.google.com/signup</button>).
    </p>
  </div>
</div>

<style>
  .stage {
    flex: 1;
    min-height: 0;
    display: grid;
    place-items: center;
    padding: var(--s-8);
    background: var(--bg);
  }
  .card {
    max-width: 420px;
    width: 100%;
    text-align: center;
    display: flex;
    flex-direction: column;
    align-items: center;
    /* tarjeta: gap y padding un escalón por encima (aire) */
    gap: var(--s-6);
    background: var(--surface);
    border: 1px solid var(--border, rgba(0, 0, 0, 0.08));
    border-radius: var(--r-lg);
    padding: var(--s-10);
    box-shadow: var(--shadow-raised-lg);
  }
  .logo {
    width: 64px;
    height: 64px;
    display: grid;
    place-items: center;
    border-radius: var(--r-md);
    background: var(--surface-2, var(--surface));
  }
  .logo img {
    width: 40px;
    height: 40px;
  }
  h1 {
    margin: 0;
    font-size: var(--fs-xl);
  }
  .sub {
    color: var(--text-dim, inherit);
    line-height: 1.5;
    margin: 0;
  }
  .btn.google {
    display: inline-flex;
    align-items: center;
    gap: var(--s-2);
    background: #fff;
    color: #1f1f1f;
    border: none;
    padding: var(--s-3) var(--s-6);
    border-radius: var(--r-full);
    box-shadow: var(--btn-shadow);
    font-weight: 600;
    cursor: pointer;
    transition: box-shadow var(--dur-fast) var(--ease-out);
  }
  .btn.google:hover {
    box-shadow: var(--btn-shadow-hover);
  }
  .btn.google:active {
    box-shadow: var(--btn-shadow-active);
  }
  .btn.google:disabled {
    opacity: 0.6;
    cursor: default;
  }
  .err {
    color: var(--danger, #dc2626);
    font-size: var(--fs-base);
  }
  .hint {
    font-size: var(--fs-sm);
    color: var(--text-dim, inherit);
    line-height: 1.5;
    margin: 0;
  }
  .linklike {
    background: none;
    border: none;
    padding: 0;
    font: inherit;
    font-size: var(--fs-sm);
    color: var(--primary);
    text-decoration: underline;
    cursor: pointer;
  }
  .linklike:hover {
    color: var(--primary-hover);
  }
</style>
