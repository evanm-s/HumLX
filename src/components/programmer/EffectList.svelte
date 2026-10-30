<script lang="ts">
  // The effects of one kind, shown in place of a Programmer window's controls.
  // Add makes a new one on the selected fixtures; effects that include a
  // selected fixture are marked.

  import { api } from "../../lib/api";
  import { show } from "../../lib/show.svelte";
  import type { Effect, EffectKind } from "../../lib/types";
  import EffectDialog from "./EffectDialog.svelte";
  import { EFFECT_KINDS } from "./effects";

  interface Props {
    kind: EffectKind;
  }

  let { kind }: Props = $props();

  const effects = $derived(show.playback.effects.filter((e) => e.kind === kind));
  const selected = $derived(new Set(show.programmer.selection));
  let editing = $state<Effect | "new" | null>(null);
</script>

<div class="effects">
  <button class="add" disabled={selected.size === 0} onclick={() => (editing = "new")}>
    {selected.size === 0 ? "Select fixtures to add an effect" : `+ Add ${EFFECT_KINDS[kind].label.toLowerCase()} effect`}
  </button>
  {#each effects as e (e.id)}
    {@const on = show.live.runningEffects.includes(e.id)}
    <div class="effect" class:on class:mine={e.fixtures.some((f) => selected.has(f))}>
      <button class="toggle" aria-pressed={on} onclick={() => show.play(() => api.setEffectRunning(e.id, !on))}>
        {on ? "On" : "Off"}
      </button>
      <button class="text" onclick={() => (editing = e)}>
        <strong>{e.name}</strong>
        <span class="muted">{e.fixtures.length} fixtures · {e.rate} per second</span>
      </button>
    </div>
  {:else}
    <p class="muted">No {EFFECT_KINDS[kind].label.toLowerCase()} effects yet.</p>
  {/each}
</div>

{#if editing}
  <EffectDialog {kind} effect={editing === "new" ? undefined : editing} onclose={() => (editing = null)} />
{/if}

<style>
  .effects {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .effects p {
    margin: 0;
    font-size: 0.9rem;
  }

  .add {
    min-height: 44px;
    border-style: dashed;
  }

  .effect {
    display: grid;
    grid-template-columns: 64px 1fr;
    gap: 6px;
    flex-shrink: 0;
  }

  .toggle[aria-pressed="true"] {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--accent-text);
    font-weight: 700;
  }

  .text {
    display: grid;
    gap: 2px;
    min-width: 0;
    padding: 6px 10px;
    text-align: left;
  }

  .text strong,
  .text span {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .text span {
    font-size: 0.8rem;
  }

  /* Effects running on any of the selected fixtures. */
  .mine .text {
    border-color: var(--accent);
  }
</style>
