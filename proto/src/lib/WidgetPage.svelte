<script lang="ts">
  import Widget from "./Widget.svelte";

  let expanded = $state(false);
  let close = $state(false);
</script>

<main class="wp" class:close={close}>
  <div class="toolbar">
    <button class="tl" onclick={() => (expanded = !expanded)}>
      {expanded ? "Compactar" : "Expandir"}
    </button>
    <button class="tl" onclick={() => (close = !close)}>Cerrar</button>
  </div>
  <div class="slot" class:expanded={expanded}>
    {#if !close}
      <Widget compact={!expanded} />
    {/if}
  </div>
</main>

<style>
  .wp {
    min-height: 100vh;
    background: var(--bg);
    display: flex;
    align-items: center;
    justify-content: center;
    gap: var(--s-6);
    padding: var(--s-8);
  }
  .toolbar {
    position: absolute;
    top: 12px;
    left: 12px;
    display: flex;
    gap: 8px;
  }
  .tl {
    border: none;
    background: var(--surface);
    border-radius: 12px;
    box-shadow: var(--e1);
    padding: 8px 14px;
    font-size: 13px;
    font-weight: 600;
    color: var(--text-2);
    transition: all var(--dur-fast) var(--ease-out);
  }
  .tl:hover {
    color: var(--primary);
  }
  .slot {
    transition: transform var(--dur-slow) var(--ease-out), opacity var(--dur-slow) var(--ease-out);
    transform-origin: bottom left;
  }
  .slot.expanded {
    transform: scale(1.08);
  }
  .wp.close {
    justify-content: flex-start;
  }
</style>
