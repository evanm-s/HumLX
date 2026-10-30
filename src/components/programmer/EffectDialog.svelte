<script lang="ts">
  // Make an effect on the selected fixtures, or change one. The kind comes
  // from the window it was opened from: dimmer for Intensity, rainbow for
  // Colour, circle for Position.

  import Dialog from "../Dialog.svelte";
  import { api } from "../../lib/api";
  import { show } from "../../lib/show.svelte";
  import type { Effect, EffectKind, Wave } from "../../lib/types";
  import { EFFECT_KINDS } from "./effects";

  interface Props {
    kind: EffectKind;
    effect?: Effect;
    onclose: () => void;
  }

  let { kind, effect, onclose }: Props = $props();

  const info = $derived(EFFECT_KINDS[kind]);
  const waves: Wave[] = ["sine", "square", "saw"];
  // Seeded once from the effect being edited; the dialog is remade per open.
  // svelte-ignore state_referenced_locally
  let draft = $state({
    name: effect?.name ?? "",
    wave: effect?.wave ?? "sine",
    rate: effect?.rate ?? 1,
    size: effect?.size ?? 1,
    spread: effect?.spread ?? 1,
  });

  async function save() {
    const name = draft.name.trim() || info.label;
    const ok = await show.play(() => api.saveEffect({ ...draft, kind, name }, effect?.id ?? null));
    if (ok) onclose();
  }

  async function remove() {
    if (!effect) return;
    const ok = await show.play(() => api.deleteEffect(effect.id));
    if (ok) onclose();
  }
</script>

<Dialog title={effect ? `Edit ${info.label.toLowerCase()} effect` : `New ${info.label.toLowerCase()} effect`} {onclose}>
  <p class="muted">{info.hint}</p>
  {#if kind === "dimmer"}
    <div class="field">
      Shape
      <div class="segmented">
        {#each waves as w (w)}
          <button aria-pressed={draft.wave === w} onclick={() => (draft.wave = w)}>
            {w.charAt(0).toUpperCase() + w.slice(1)}
          </button>
        {/each}
      </div>
    </div>
  {/if}
  <label class="field">
    Speed: {draft.rate.toFixed(2)} per second
    <input type="range" min="0.05" max="4" step="0.05" bind:value={draft.rate} />
  </label>
  {#if kind !== "rainbow"}
    <label class="field">
      Size: {Math.round(draft.size * 100)}%
      <input type="range" min="0" max="1" step="0.01" bind:value={draft.size} />
    </label>
  {/if}
  <label class="field">
    Spread: {Math.round(draft.spread * 100)}% {draft.spread === 0 ? "(all together)" : ""}
    <input type="range" min="0" max="1" step="0.01" bind:value={draft.spread} />
  </label>
  <label class="field">
    Name
    <input bind:value={draft.name} placeholder={info.label} />
  </label>
  {#if effect}
    <p class="muted">Runs on {effect.fixtures.length} fixtures.</p>
  {:else}
    <p class="muted">Runs on the {show.programmer.selection.length} selected fixtures, in the order you picked them.</p>
  {/if}
  {#snippet actions()}
    {#if effect}<button class="danger" onclick={remove}>Delete</button>{/if}
    <span class="spacer"></span>
    <button onclick={onclose}>Cancel</button>
    <button class="primary" onclick={save}>{effect ? "Save" : "Create and run"}</button>
  {/snippet}
</Dialog>
