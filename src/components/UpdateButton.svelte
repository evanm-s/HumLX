<script lang="ts">
  // Shown in the side rail when a new version is out.

  import Dialog from "./Dialog.svelte";
  import { updates } from "../lib/updates.svelte";

  let open = $state(false);
</script>

{#if updates.available}
  <button class="update" onclick={() => (open = true)}>
    Update<br /><span>v{updates.available.version}</span>
  </button>
{/if}

{#if open && updates.available}
  <Dialog title="Update HumLX" onclose={() => updates.progress == null && (open = false)}>
    <p>
      Version <strong>{updates.available.version}</strong> is available. You have {updates.version ?? "an older version"}.
    </p>
    {#if updates.available.body}<p class="notes">{updates.available.body}</p>{/if}
    <p class="muted">
      HumLX closes, installs the update and opens again, which takes a minute. Lighting output stops while it
      restarts, so update between sessions, not during one. The show is kept.
    </p>
    {#if updates.progress != null}
      <div class="bar"><span style:width="{updates.progress * 100}%"></span></div>
    {/if}
    {#if updates.error}<p class="bad">Update failed: {updates.error}</p>{/if}
    {#snippet actions()}
      <button disabled={updates.progress != null} onclick={() => (open = false)}>Later</button>
      <button class="primary" disabled={updates.progress != null} onclick={() => updates.install()}>
        {updates.progress != null ? "Installing…" : "Update and restart"}
      </button>
    {/snippet}
  </Dialog>
{/if}

<style>
  .update {
    width: 100%;
    min-height: 64px;
    border-color: var(--ok);
    color: var(--ok);
    font-weight: 600;
    line-height: 1.2;
  }

  .update span {
    font-size: 0.75rem;
    font-weight: 400;
  }

  .notes {
    white-space: pre-wrap;
    background: var(--bg);
    border-radius: 8px;
    padding: 12px;
  }

  .bar {
    height: 10px;
    border-radius: 5px;
    background: var(--bg);
    overflow: hidden;
  }

  .bar span {
    display: block;
    height: 100%;
    background: var(--accent);
  }

  .bad {
    color: var(--danger);
  }
</style>
