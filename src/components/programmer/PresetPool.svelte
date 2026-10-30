<script lang="ts">
  // A pool of presets for one family. Tap to apply. Record (on the control
  // surface, or R) then tap a slot to store; Delete then tap a preset and
  // Enter to remove it.

  import Dialog from "../Dialog.svelte";
  import { api } from "../../lib/api";
  import { rgbCss, SWATCHES } from "../../lib/format";
  import { show } from "../../lib/show.svelte";
  import { surface } from "../../lib/surface.svelte";
  import type { Family, Preset } from "../../lib/types";

  interface Props {
    family: Family;
  }

  let { family }: Props = $props();

  const program = show.program.bind(show);
  const presets = $derived(show.programmer.presets.filter((p) => p.family === family));
  const selection = $derived(show.programmer.selection);

  const recording = $derived(surface.mode === "record");
  const deleting = $derived(surface.mode === "delete");
  let naming = $state<number | null>(null);
  let name = $state("");

  // Preset numbers are shared across families, so the next slot is the first unused one.
  const nextSlot = $derived.by(() => {
    const used = new Set(show.programmer.presets.map((p) => p.id));
    let id = 1;
    while (used.has(id)) id++;
    return id;
  });

  function sample(preset: Preset) {
    return preset.all ?? Object.values(preset.fixtures ?? {})[0] ?? {};
  }

  function defaultName(): string {
    const current = show.current;
    if (family === "intensity" && current.intensity != null) return `${Math.round(current.intensity * 100)}%`;
    if (family === "colour" && current.colour) {
      const c = current.colour;
      const match = SWATCHES.find(
        (s) => Math.abs(s.colour.r - c.r) + Math.abs(s.colour.g - c.g) + Math.abs(s.colour.b - c.b) < 0.15,
      );
      if (match) return match.name;
    }
    return { intensity: "Level", colour: "Colour", position: "Position", beam: "Beam", control: "Control" }[family];
  }

  function tap(preset: Preset | null, slot: number) {
    if (deleting) {
      if (preset) surface.pendingDelete = surface.pendingDelete === preset.id ? null : preset.id;
    } else if (recording || !preset) {
      name = preset?.name ?? defaultName();
      naming = slot;
    } else {
      program(() => api.applyPreset(preset.id));
    }
  }

  async function save() {
    if (naming == null) return;
    const slot = naming;
    const ok = await program(() => api.recordPreset(family, name.trim() || defaultName(), slot));
    if (ok) {
      naming = null;
      surface.setMode(null);
    }
  }
</script>

<div class="pool">
  {#if recording}<p class="hint rec">Tap a slot to store the {family} of the selection.</p>
  {:else if deleting && presets.length > 0}
    {@const doomed = presets.find((p) => p.id === surface.pendingDelete)}
    <p class="hint del">
      {#if doomed}Press <strong>Enter</strong> to delete {doomed.name}.{:else}Tap a preset to delete.{/if}
    </p>
  {/if}
  <div class="slots">
    {#each presets as p (p.id)}
      {@const s = sample(p)}
      <button
        class="slot"
        class:deleting
        class:doomed={deleting && surface.pendingDelete === p.id}
        class:recording
        disabled={!recording && !deleting && selection.length === 0}
        onclick={() => tap(p, p.id)}
      >
        {#if family === "colour"}
          <span class="chip" style:background={rgbCss(s.colour, "var(--line)")}></span>
        {:else if family === "intensity"}
          <span class="bar"><span style:width="{(s.intensity ?? 0) * 100}%"></span></span>
        {:else if family === "position"}
          <span class="mini-pad"
            ><span style:left="{(s.pan ?? 0.5) * 100}%" style:top="{(1 - (s.tilt ?? 0.5)) * 100}%"></span></span
          >
        {/if}
        <span class="slot-name">{p.name}</span>
      </button>
    {/each}
    {#if recording}
      <button class="slot empty-slot" onclick={() => tap(null, nextSlot)}>+ New</button>
    {/if}
  </div>
  {#if presets.length === 0 && !recording}
    <p class="hint">None yet. Set some {family}, press <strong>Record</strong> (R), then tap <strong>+ New</strong>.</p>
  {/if}
</div>

{#if naming != null}
  <Dialog title="Record {family} preset" onclose={() => (naming = null)}>
    <label class="field">
      Name
      <input bind:value={name} />
    </label>
    {#snippet actions()}
      <button onclick={() => (naming = null)}>Cancel</button>
      <button class="primary" onclick={save}>Save</button>
    {/snippet}
  </Dialog>
{/if}

<style>
  .pool {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .hint {
    margin: 0;
    font-size: 0.8rem;
    color: var(--muted);
  }

  .hint.rec,
  .hint.del {
    color: var(--danger);
  }

  .slots {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(110px, 1fr));
    gap: 6px;
  }

  .slot {
    display: grid;
    gap: 6px;
    align-content: end;
    padding: 8px;
    min-height: 64px;
    text-align: left;
  }

  .slot.recording {
    border-color: color-mix(in srgb, var(--danger) 60%, transparent);
  }

  .slot.deleting {
    border-color: color-mix(in srgb, var(--danger) 45%, transparent);
  }

  .slot.doomed {
    border-color: var(--danger);
    background: color-mix(in srgb, var(--danger) 25%, var(--panel-raised));
  }

  .slot-name {
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .empty-slot {
    border-style: dashed;
    color: var(--muted);
    align-content: center;
    text-align: center;
  }

  .chip {
    height: 24px;
    border-radius: 8px;
    box-shadow: inset 0 0 0 1px rgb(255 255 255 / 0.15);
  }

  .bar {
    height: 12px;
    border-radius: 6px;
    background: var(--bg);
    overflow: hidden;
  }

  .bar span {
    display: block;
    height: 100%;
    background: var(--accent);
  }

  .mini-pad {
    position: relative;
    height: 28px;
    border-radius: 8px;
    background: var(--bg);
  }

  .mini-pad span {
    position: absolute;
    width: 10px;
    height: 10px;
    margin: -5px 0 0 -5px;
    border-radius: 50%;
    background: var(--accent);
  }
</style>
