<script lang="ts">
  import { dismiss, toasts } from "../lib/show.svelte";
</script>

<div class="toasts" aria-live="polite">
  {#each toasts as toast (toast.id)}
    <button class="toast {toast.kind}" onclick={() => dismiss(toast.id)}>{toast.message}</button>
  {/each}
</div>

<style>
  .toasts {
    position: fixed;
    right: 24px;
    bottom: 24px;
    display: grid;
    gap: 10px;
    z-index: 20;
    max-width: 560px;
  }

  /* Flat blocks: errors red, warnings yellow, everything else grey. */
  .toast {
    text-align: left;
    padding: 16px 20px;
    height: auto;
    background: #2c333f;
  }

  .error {
    background: var(--danger);
    color: var(--accent-text);
  }

  .warning {
    background: var(--warning);
    color: var(--accent-text);
  }
</style>
