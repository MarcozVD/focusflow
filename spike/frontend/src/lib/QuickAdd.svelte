<script lang="ts">
  import { fade, scale } from "svelte/transition";
  import { categories, quickadd, createTaskFromText, nlBusy } from "./data.svelte";

  let text = $state("");
  let showPreview = $state(false);
  let flash = $state(false);
  let flashText = $state("Tarea creada");
  let inputEl: HTMLInputElement;

  $effect(() => {
    if (quickadd() > 0) inputEl?.focus();
  });

  interface Detect {
    label: string;
    color: string;
    id: string;
  }

  function findCat(kw: string): Detect | null {
    const map: [string, string, string][] = [
      ["pagar", "fin", "Finanzas"],
      ["factura", "trab", "Trabajo"],
      ["examen", "uni", "Universidad"],
      ["entregar", "uni", "Universidad"],
      ["proyecto", "trab", "Trabajo"],
      ["médico", "sal", "Salud"],
      ["gimnasio", "sal", "Salud"],
      ["cita", "per", "Personal"],
    ];
    for (const [k, id, name] of map) {
      if (kw.includes(k)) {
        const c = categories.find((x) => x.id === id)!;
        return { label: name, color: c.color, id };
      }
    }
    return null;
  }

  function detected(): Detect[] {
    if (!text.trim()) return [];
    const out: Detect[] = [];
    const t = text.toLowerCase();
    if (/\bmañana\b/.test(t)) out.push({ label: "Mañana", color: "#2563EB", id: "" });
    if (/\bel \d{1,2}\b/.test(t)) out.push({ label: "Fecha detectada", color: "#2563EB", id: "" });
    if (/\bprox(imo)?\s*lunes\b/.test(t)) out.push({ label: "Próximo lunes", color: "#2563EB", id: "" });
    if (/de \d{1,2}(:?\d{2})?\s*(a|pm|am)/.test(t) || /\ba las \d/.test(t))
      out.push({ label: "Horario", color: "#059669", id: "" });
    if (/\b(todos los lunes|cada mes|cada año|cada \d+ días)\b/.test(t))
      out.push({ label: "Repetición", color: "#7C3AED", id: "" });
    if (/\burgente\b/.test(t)) out.push({ label: "Prioridad alta", color: "#DC2626", id: "" });
    if (/\brecord(a|arme)\b/.test(t)) out.push({ label: "Recordatorio", color: "#F59E0B", id: "" });
    const c = findCat(t);
    if (c) out.push(c);
    return out.slice(0, 4);
  }

  async function confirm() {
    if (!text.trim() || nlBusy()) return;
    const input = text;
    const r = await createTaskFromText(input);
    if (r.ok) {
      flashText =
        r.source === "ai"
          ? "Tarea creada con la IA"
          : "Tarea creada (interpretación local)";
      text = "";
      showPreview = false;
      flash = true;
      setTimeout(() => (flash = false), 1600);
    } else {
      flashText = "No se pudo interpretar la tarea";
      flash = true;
      setTimeout(() => (flash = false), 1600);
    }
  }
</script>

<div class="qa-wrap">
  <div class="qa {showPreview ? 'expanded' : ''}">
    <svg class="bolt" width="18" height="18" viewBox="0 0 24 24" fill="none">
      <path d="M13 2L4.5 13.5H11L9.5 22L19 9.5H12.5L13 2Z" stroke="currentColor" stroke-width="2" stroke-linejoin="round" />
    </svg>
    <input
      bind:this={inputEl}
      bind:value={text}
      placeholder="Escribe tu tarea…  “Mañana estudiar cálculo de 3pm a 5pm”"
      onkeydown={(e) => {
        if (e.key === "Enter") {
          if (showPreview) {
            confirm();
          } else if (text.trim()) {
            showPreview = true;
          }
        }
        if (e.key === "Escape") showPreview = false;
      }}
      oninput={() => (showPreview = showPreview && detected().length > 0)}
    />
    <kbd>Ctrl⇧Espacio</kbd>
  </div>

  {#if showPreview && detected().length > 0}
    <div class="preview" transition:scale={{ duration: 200, easing: (t: number) => 1 - Math.pow(1 - t, 3) }}>
      <div class="chips">
        {#each detected() as d}
          <span class="chip" style="--c: {d.color}">{d.label}</span>
        {/each}
      </div>
      <button class="create" onclick={confirm} disabled={nlBusy()}>Crear tarea</button>
    </div>
  {/if}

  {#if flash}
    <div class="toast" transition:fade={{ duration: 150 }}>
      <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
        <path d="M2 6.5L4.5 9L10 3" stroke="white" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" />
      </svg>
      {flashText}
    </div>
  {/if}
</div>

<style>
  .qa-wrap {
    position: relative;
    width: min(520px, 100%);
  }
  .qa {
    display: flex;
    align-items: center;
    gap: 10px;
    background: var(--surface-3);
    border-radius: 15px;
    height: 44px;
    padding: 0 var(--s-4);
    box-shadow: var(--shadow-inset);
    border: 1px solid var(--border);
    transition: border-color var(--dur-fast) var(--ease-out), box-shadow var(--dur-fast) var(--ease-out);
  }
  .qa:focus-within {
    border-color: var(--primary);
    box-shadow: var(--shadow-inset), 0 0 0 3px var(--primary-soft);
  }
  .qa.expanded {
    border-radius: 15px 15px 10px 10px;
  }
  .bolt {
    color: var(--primary);
    flex-shrink: 0;
    display: block;
  }
  input {
    flex: 1;
    min-width: 0;
    border: none;
    background: transparent;
    outline: none;
    font-size: 13.5px;
    line-height: 1.4;
    color: var(--text-1);
    font-family: inherit;
    height: 100%;
  }
  input::placeholder {
    color: var(--text-3);
    font-size: 13px;
  }
  kbd {
    font-size: 10px;
    font-weight: 600;
    font-family: inherit;
    color: var(--text-3);
    background: var(--surface);
    border-radius: 7px;
    padding: 2px 7px;
    box-shadow: var(--e1);
    white-space: nowrap;
    flex-shrink: 0;
    letter-spacing: 0.02em;
  }
  .preview {
    position: absolute;
    top: calc(100% + 8px);
    left: 0;
    right: 0;
    background: var(--surface);
    border-radius: var(--r-md);
    box-shadow: var(--e3);
    border: 1px solid var(--border);
    padding: var(--s-3) var(--s-4);
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: var(--s-3);
    z-index: 30;
  }
  .chips {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
  }
  .chip {
    font-size: 11px;
    font-weight: 600;
    color: color-mix(in srgb, var(--c) 60%, var(--text-1));
    background: color-mix(in srgb, var(--c) 13%, var(--surface));
    border-radius: var(--r-full);
    padding: 4px 10px;
    border: 1px solid color-mix(in srgb, var(--c) 30%, transparent);
    white-space: nowrap;
  }
  .create {
    background: var(--primary);
    color: #fff;
    border: none;
    border-radius: 12px;
    padding: 8px 16px;
    font-size: 13px;
    font-weight: 600;
    font-family: inherit;
    box-shadow: 0 4px 10px -2px color-mix(in srgb, var(--primary) 55%, transparent);
    transition: all var(--dur-fast) var(--ease-out);
    flex-shrink: 0;
  }
  .create:hover {
    background: var(--primary-hover);
    transform: translateY(-1px);
  }
  .create:active {
    background: var(--primary-active);
    transform: translateY(0);
    box-shadow: var(--shadow-inset);
  }
  .create:disabled {
    opacity: 0.6;
    pointer-events: none;
  }
  .toast {
    position: fixed;
    bottom: 28px;
    left: 50%;
    transform: translateX(-50%);
    display: flex;
    align-items: center;
    gap: 8px;
    background: var(--surface);
    border-left: 3px solid var(--success);
    border-radius: var(--r-md);
    box-shadow: var(--e2);
    padding: 10px 18px;
    font-size: 13px;
    font-weight: 600;
    color: var(--text-1);
    z-index: 60;
    max-width: 90vw;
  }
  .toast svg {
    background: var(--success);
    border-radius: 50%;
    padding: 2px;
    flex-shrink: 0;
  }
</style>
