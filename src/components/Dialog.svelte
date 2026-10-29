<script lang="ts">
  import type { Snippet } from "svelte";

  interface Props {
    title: string;
    onclose: () => void;
    children: Snippet;
    actions?: Snippet;
  }

  let { title, onclose, children, actions }: Props = $props();
</script>

<div class="backdrop" role="presentation" onpointerdown={(e) => e.target === e.currentTarget && onclose()}>
  <div class="dialog panel" role="dialog" aria-modal="true" aria-label={title}>
    <header class="row">
      <h2>{title}</h2>
      <span class="spacer"></span>
      <button class="ghost" aria-label="Close" onclick={onclose}>✕</button>
    </header>
    <div class="body scroll">{@render children()}</div>
    {#if actions}<footer class="row">{@render actions()}</footer>{/if}
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgb(0 0 0 / 0.6);
    display: grid;
    place-items: center;
    z-index: 10;
  }

  .dialog {
    width: min(760px, calc(100vw - 48px));
    max-height: calc(100vh - 96px);
    display: grid;
    grid-template-rows: auto 1fr auto;
    padding: 24px;
    gap: 20px;
    box-shadow: 0 30px 80px rgb(0 0 0 / 0.5);
  }

  .body {
    display: grid;
    gap: 18px;
    align-content: start;
  }

  footer {
    justify-content: flex-end;
  }
</style>
