<script lang="ts">
  import { onMount } from "svelte";
  import { listen } from "@tauri-apps/api/event";
  import Widget from "./Widget.svelte";
  import { applySavedTheme, init } from "./data.svelte";

  onMount(() => {
    init();
    applySavedTheme();
    const un = listen("theme:changed", (e) => {
      const v = e.payload;
      if (v === "dark" || v === "light") {
        document.documentElement.dataset.theme = v;
      }
    });
    return () => {
      un.then((f) => f());
    };
  });
</script>

<main class="wp">
  <Widget />
</main>

<style>
  .wp {
    min-height: 100vh;
    background: transparent;
    display: flex;
    flex-direction: column;
    align-items: stretch;
    padding: 0;
  }
  :global([data-widget] body) {
    background: transparent;
  }
  :global([data-widget] .wp) {
    background: transparent;
    min-height: auto;
    padding: 0;
  }
</style>
