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

  .toast {
    text-align: left;
    padding: 16px 20px;
    height: auto;
    background: var(--panel-raised);
    border-left: 6px solid var(--muted);
    box-shadow: 0 12px 30px rgb(0 0 0 / 0.4);
  }

  .error {
    border-left-color: var(--danger);
  }

  .warning {
    border-left-color: var(--warning);
  }
</style>
