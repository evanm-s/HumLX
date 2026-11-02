<script lang="ts">
  // The programmer's effects of one family, shown in place of a Programmer
  // window's controls. Add makes a new one on the selected fixtures; they run
  // straight away and record into cues with the programmer's values.

  import { api } from "../../lib/api";
  import { show } from "../../lib/show.svelte";
  import type { Effect, EffectFamily } from "../../lib/types";
  import EffectDialog from "./EffectDialog.svelte";
  import { FAMILY_LABELS, PATTERNS, SHAPES } from "./effects";

  interface Props {
    family: EffectFamily;
  }

  let { family }: Props = $props();

  const effects = $derived(show.programmer.effects.filter((e) => SHAPES[e.shape]?.family === family));
  const selected = $derived(new Set(show.programmer.selection));
  let editing = $state<Effect | "new" | null>(null);

  const MULTIPLIER_TEXT: Record<number, string> = { 0.25: " ¼", 0.5: " ½", 1: "", 2: " ×2", 4: " ×4" };
  const speedText = (e: Effect) =>
    e.speed === "own"
      ? `${Math.round(e.rate * 60)} BPM`
      : `Speed ${e.speed.toUpperCase()}${MULTIPLIER_TEXT[e.multiplier] ?? ` ×${e.multiplier}`}`;
</script>

<div class="effects">
  <button class="add" disabled={selected.size === 0} onclick={() => (editing = "new")}>
    {selected.size === 0 ? "Select fixtures to add an effect" : `+ Add ${FAMILY_LABELS[family]} effect`}
  </button>
  {#each effects as e (e.id)}
    <div class="effect" class:mine={e.fixtures.some((f) => selected.has(f))}>
      <button class="text" onclick={() => (editing = e)}>
        <strong>{e.name}</strong>
        <span class="muted"
          >{e.name !== SHAPES[e.shape]?.label ? `${SHAPES[e.shape]?.label} · ` : ""}{e.fixtures.length} fixtures · {speedText(e)}{e.spread > 0 && e.pattern !== "inOrder" ? ` · ${PATTERNS[e.pattern].label}` : ""}</span
        >
      </button>
      <button class="remove" aria-label="Remove {e.name}" onclick={() => show.play(() => api.deleteEffect(e.id))}>✕</button>
    </div>
  {:else}
    <p class="muted">No {FAMILY_LABELS[family]} effects in the programmer. Effects in cues show on stage but not here.</p>
  {/each}
</div>

{#if editing}
  <EffectDialog {family} effect={editing === "new" ? undefined : editing} onclose={() => (editing = null)} />
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
    border-color: var(--line);
    border-style: dashed;
    background: transparent;
  }

  .effect {
    display: grid;
    grid-template-columns: 1fr 48px;
    gap: 6px;
    flex-shrink: 0;
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
