<script lang="ts">
  // The steps of an effect, one numbered chip each. Tap a step to change it
  // with the control for the effect's family: a level, a colour, a point on
  // the pan and tilt pad (which draws the whole path), or a beam setting.

  import ColourPicker from "../ColourPicker.svelte";
  import { attributeName, rgbCss } from "../../lib/format";
  import type { Attribute, EffectFamily, Rgb } from "../../lib/types";
  import type { Step } from "./effects";
  import { drag, type DragPoint } from "../../lib/drag";

  interface Props {
    family: EffectFamily;
    steps: Step[];
    /** Swatches to pick a colour step from, such as the colour presets. */
    palette: { name: string; colour: Rgb }[];
    /** The fewest and most steps allowed. */
    min?: number;
    max?: number;
    /** What a step is called, such as "Colour" for a colour list. */
    noun?: string;
    onchange: (steps: Step[]) => void;
  }

  let { family, steps, palette, min = 2, max = 32, noun = "Step", onchange }: Props = $props();

  let picked = $state(0);
  const index = $derived(Math.min(picked, steps.length - 1));
  const step = $derived(steps[index] ?? {});
  const beamKeys = $derived(Object.keys(step.other ?? {}) as Attribute[]);

  function set(change: Partial<Step>) {
    onchange(steps.map((s, i) => (i === index ? { ...s, ...change } : s)));
  }

  function add() {
    const copy = structuredClone($state.snapshot(step)) as Step;
    onchange([...steps.slice(0, index + 1), copy, ...steps.slice(index + 1)]);
    picked = index + 1;
  }

  function remove() {
    onchange(steps.filter((_, i) => i !== index));
    picked = Math.max(0, index - 1);
  }

  function shift(by: -1 | 1) {
    const to = index + by;
    if (to < 0 || to >= steps.length) return;
    const next = [...steps];
    [next[index], next[to]] = [next[to], next[index]];
    onchange(next);
    picked = to;
  }

  // The pad places a step where it is touched; up is higher tilt.
  let area = $state<HTMLDivElement>();
  function place(event: DragPoint) {
    if (!area) return;
    const rect = area.getBoundingClientRect();
    const clamp = (v: number) => Math.round(Math.max(0, Math.min(1, v)) * 1000) / 1000;
    const pan = clamp((event.clientX - rect.left) / rect.width);
    const tilt = clamp(1 - (event.clientY - rect.top) / rect.height);
    // Keep to the axes the step moves, so a pan sweep stays a pan sweep.
    const both = step.pan == null && step.tilt == null;
    set({
      ...(step.pan != null || both ? { pan } : {}),
      ...(step.tilt != null || both ? { tilt } : {}),
    });
  }
  const point = (s: Step) => `${(s.pan ?? 0.5) * 100},${(1 - (s.tilt ?? 0.5)) * 100}`;

  const chipStyle = (s: Step) => {
    if (family === "colour") return `background: ${rgbCss(s.colour ?? { r: 1, g: 1, b: 1 })}`;
    if (family === "intensity") {
      const l = Math.round((s.intensity ?? 1) * 100);
      return `background: linear-gradient(to top, var(--f-intensity) ${l}%, transparent ${l}%)`;
    }
    return "";
  };
</script>

<div class="editor">
  <div class="chips" role="listbox" aria-label="{noun}s">
    {#each steps as s, i (i)}
      <button
        class="chip"
        class:light={family === "colour" || family === "intensity"}
        role="option"
        aria-selected={i === index}
        aria-label="{noun} {i + 1}"
        style={chipStyle(s)}
        onclick={() => (picked = i)}
      >
        <span>{i + 1}</span>
      </button>
    {/each}
  </div>

  {#if family === "intensity"}
    <label class="field">
      Level of step {index + 1}: {Math.round((step.intensity ?? 1) * 100)}%
      <input
        type="range"
        min="0"
        max="1"
        step="0.01"
        value={step.intensity ?? 1}
        oninput={(e) => set({ intensity: +e.currentTarget.value })}
      />
    </label>
  {:else if family === "colour"}
    <div class="colour">
      <div class="picker">
        <ColourPicker colour={step.colour} onchange={(colour) => set({ colour })} />
      </div>
      <div class="swatches" role="group" aria-label="Colours to pick from">
        {#each palette as p, j (j)}
          <button
            class="swatch"
            style:background={rgbCss(p.colour)}
            title={p.name}
            aria-label={p.name}
            onclick={() => set({ colour: p.colour })}
          ></button>
        {/each}
      </div>
    </div>
  {:else if family === "position"}
    <div
      class="pad"
      role="application"
      aria-label="Where step {index + 1} points"
      use:drag={{ start: place, move: place }}
    >
      <div class="area" bind:this={area}>
        <svg viewBox="0 0 100 100" preserveAspectRatio="none" aria-hidden="true">
          <polygon points={steps.map(point).join(" ")} />
        </svg>
        {#each steps as s, i (i)}
          <span
            class="point"
            class:on={i === index}
            style:left="{(s.pan ?? 0.5) * 100}%"
            style:top="{(1 - (s.tilt ?? 0.5)) * 100}%"
          >
            {#if i === index}{i + 1}{/if}
          </span>
        {/each}
      </div>
    </div>
    <p class="muted">Tap to move step {index + 1}. Centre is the fixture's own position.</p>
  {:else}
    {#each beamKeys as key (key)}
      <label class="field">
        {attributeName(key)} at step {index + 1}: {Math.round((step.other?.[key] ?? 0) * 100)}%
        <input
          type="range"
          min="0"
          max="1"
          step="0.01"
          value={step.other?.[key] ?? 0}
          oninput={(e) => set({ other: { ...step.other, [key]: +e.currentTarget.value } })}
        />
      </label>
    {/each}
  {/if}

  <div class="actions">
    <button onclick={() => shift(-1)} disabled={index === 0} aria-label="Move {noun.toLowerCase()} earlier">◀</button>
    <button onclick={() => shift(1)} disabled={index === steps.length - 1} aria-label="Move {noun.toLowerCase()} later">
      ▶
    </button>
    <span class="spacer"></span>
    <button onclick={add} disabled={steps.length >= max}>Add {noun.toLowerCase()}</button>
    <button onclick={remove} disabled={steps.length <= min}>Remove {noun.toLowerCase()} {index + 1}</button>
  </div>
</div>

<style>
  .editor {
    display: grid;
    gap: 10px;
  }

  .chips {
    display: flex;
    flex-wrap: wrap;
    gap: 6px;
  }

  .chip {
    min-width: 44px;
    width: 44px;
    height: 44px;
    padding: 0;
    font-family: var(--mono);
    font-size: 0.85rem;
  }

  .chip.light span {
    padding: 1px 5px;
    border-radius: 4px;
    background: rgb(0 0 0 / 0.6);
    color: #fff;
  }

  .chip[aria-selected="true"] {
    outline: 3px solid var(--accent);
    outline-offset: 2px;
  }

  .colour {
    display: grid;
    grid-template-columns: minmax(140px, 200px) 1fr;
    gap: 12px;
    align-items: start;
  }

  /* Room for the picker's marker, which overhangs the disc. */
  .picker {
    padding: 22px;
  }

  .swatches {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }

  .swatch {
    width: 40px;
    min-width: 40px;
    height: 40px;
    padding: 0;
  }

  .pad {
    position: relative;
    aspect-ratio: 1;
    width: 100%;
    max-width: 320px;
    border-radius: var(--radius);
    background:
      linear-gradient(#232833 1px, transparent 1px) 0 0 / 12.5% 12.5%,
      linear-gradient(90deg, #232833 1px, transparent 1px) 0 0 / 12.5% 12.5%,
      #0a0c0f;
    box-shadow: inset 0 0 0 1px var(--line);
    touch-action: none;
    overflow: hidden;
  }

  /* Inset so points at the very edge of the range stay in view. */
  .area {
    position: absolute;
    inset: 14px;
    pointer-events: none;
  }

  .pad svg {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    pointer-events: none;
  }

  .pad polygon {
    fill: none;
    stroke: color-mix(in srgb, var(--f-position) 60%, transparent);
    stroke-width: 1.5;
    vector-effect: non-scaling-stroke;
  }

  .point {
    position: absolute;
    display: grid;
    place-items: center;
    width: 22px;
    height: 22px;
    margin: -11px 0 0 -11px;
    border-radius: 50%;
    background: #0a0c0f;
    border: 2px solid var(--f-position);
    color: var(--f-position);
    font-family: var(--mono);
    font-size: 0.7rem;
    pointer-events: none;
  }

  .point:not(.on) {
    width: 12px;
    height: 12px;
    margin: -6px 0 0 -6px;
    font-size: 0;
  }

  .point.on {
    background: var(--f-position);
    color: #0a0c0f;
  }

  .actions {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
  }

  .spacer {
    flex: 1;
  }

  p {
    margin: 0;
  }
</style>
