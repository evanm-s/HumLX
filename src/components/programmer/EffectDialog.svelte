<script lang="ts">
  // Make an effect on the selected fixtures, or change one. Pick a shape, then
  // set speed, size and spread; the rest sits under More. The shapes on offer
  // come from the window it was opened from.

  import Dialog from "../Dialog.svelte";
  import { api } from "../../lib/api";
  import { show } from "../../lib/show.svelte";
  import type { Direction, Effect, EffectFamily, Pattern, Rgb, Shape } from "../../lib/types";
  import { FAMILY_LABELS, PATTERNS, SHAPES, shapesOf } from "./effects";

  interface Props {
    family: EffectFamily;
    effect?: Effect;
    onclose: () => void;
  }

  let { family, effect, onclose }: Props = $props();

  const shapes = $derived(shapesOf(family));
  const directions: Direction[] = ["forward", "backward", "bounce"];
  const PALETTE: Rgb[] = [
    { r: 1, g: 0, b: 0 },
    { r: 1, g: 0.5, b: 0 },
    { r: 1, g: 1, b: 0 },
    { r: 0, g: 1, b: 0 },
    { r: 0, g: 1, b: 1 },
    { r: 0, g: 0, b: 1 },
    { r: 1, g: 0, b: 1 },
    { r: 1, g: 1, b: 1 },
  ];
  const css = (c: Rgb) => `rgb(${c.r * 255} ${c.g * 255} ${c.b * 255})`;
  const same = (a: Rgb | undefined, b: Rgb) => !!a && a.r === b.r && a.g === b.g && a.b === b.b;

  // Seeded once from the effect being edited; the dialog is remade per open.
  // svelte-ignore state_referenced_locally
  const startShape: Shape = effect?.shape ?? shapesOf(family)[0];
  // svelte-ignore state_referenced_locally
  let draft = $state({
    name: effect?.name ?? "",
    shape: startShape,
    bpm: Math.round((effect?.rate ?? 1) * 60),
    size: effect?.size ?? 1,
    spread: effect?.spread ?? 1,
    pattern: (effect?.pattern ?? "inOrder") as Pattern,
    direction: (effect?.direction ?? "forward") as Direction,
    width: effect?.width ?? 1,
    transition: effect?.transition ?? SHAPES[startShape].transition,
    colours: [effect?.steps[0]?.colour ?? PALETTE[0], effect?.steps[1]?.colour ?? PALETTE[5]],
  });
  let more = $state(false);

  function pick(shape: Shape) {
    if (shape === draft.shape) return;
    draft.shape = shape;
    draft.transition = SHAPES[shape].transition;
  }

  async function save() {
    const name = draft.name.trim() || SHAPES[draft.shape].label;
    const settings = {
      name,
      shape: draft.shape,
      rate: draft.bpm / 60,
      size: draft.size,
      spread: draft.spread,
      pattern: draft.pattern,
      direction: draft.direction,
      width: draft.width,
      transition: draft.transition,
      colours: draft.shape === "swap" ? $state.snapshot(draft.colours) : undefined,
    };
    const ok = await show.play(() => api.saveEffect(settings, effect?.id ?? null));
    if (ok) onclose();
  }

  async function remove() {
    if (!effect) return;
    const ok = await show.play(() => api.deleteEffect(effect.id));
    if (ok) onclose();
  }
</script>

<Dialog title={effect ? `Edit ${FAMILY_LABELS[family]} effect` : `New ${FAMILY_LABELS[family]} effect`} {onclose}>
  <div class="shapes" role="group" aria-label="Shape">
    {#each shapes as s (s)}
      <button class="shape" aria-pressed={draft.shape === s} title={SHAPES[s].hint} onclick={() => pick(s)}>
        <svg viewBox="0 0 28 20" aria-hidden="true"><path d={SHAPES[s].icon} /></svg>
        {SHAPES[s].label}
      </button>
    {/each}
  </div>
  <p class="muted">{SHAPES[draft.shape].hint}.</p>

  {#if draft.shape === "swap"}
    {#each [0, 1] as i (i)}
      <div class="field">
        Colour {i + 1}
        <div class="swatches">
          {#each PALETTE as c, j (j)}
            <button
              class="swatch"
              style:background={css(c)}
              aria-label="Colour {i + 1}: {css(c)}"
              aria-pressed={same(draft.colours[i], c)}
              onclick={() => (draft.colours[i] = c)}
            ></button>
          {/each}
        </div>
      </div>
    {/each}
  {/if}

  <label class="field">
    Speed: {draft.bpm} BPM
    <input type="range" min="3" max="240" step="1" bind:value={draft.bpm} />
  </label>
  <label class="field">
    Size: {Math.round(draft.size * 100)}%
    <input type="range" min="0" max="1" step="0.01" bind:value={draft.size} />
  </label>
  <label class="field">
    Spread: {Math.round(draft.spread * 100)}% {draft.spread === 0 ? "(all together)" : ""}
    <input type="range" min="0" max="1" step="0.01" bind:value={draft.spread} />
  </label>
  <div class="field">
    Pattern
    <div class="segmented">
      {#each Object.entries(PATTERNS) as [p, info] (p)}
        <button
          aria-pressed={draft.pattern === p}
          title={info.hint}
          disabled={draft.spread === 0}
          onclick={() => (draft.pattern = p as Pattern)}
        >
          {info.label}
        </button>
      {/each}
    </div>
  </div>

  <button class="ghost more" aria-expanded={more} onclick={() => (more = !more)}>{more ? "▾" : "▸"} More</button>
  {#if more}
    <div class="field">
      Direction
      <div class="segmented">
        {#each directions as d (d)}
          <button aria-pressed={draft.direction === d} onclick={() => (draft.direction = d)}>
            {d.charAt(0).toUpperCase() + d.slice(1)}
          </button>
        {/each}
      </div>
    </div>
    <label class="field">
      Fade between steps: {Math.round(draft.transition * 100)}% {draft.transition === 0 ? "(snap)" : ""}
      <input type="range" min="0" max="1" step="0.01" bind:value={draft.transition} />
    </label>
    <label class="field">
      Width: {Math.round(draft.width * 100)}% of each cycle
      <input type="range" min="0.05" max="1" step="0.01" bind:value={draft.width} />
    </label>
  {/if}

  <label class="field">
    Name
    <input bind:value={draft.name} placeholder={SHAPES[draft.shape].label} />
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

<style>
  .shapes {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(96px, 1fr));
    gap: 6px;
  }

  .shape {
    display: grid;
    justify-items: center;
    gap: 4px;
    min-height: 64px;
    padding: 8px 4px;
    font-size: 0.85rem;
  }

  .shape svg {
    width: 36px;
    height: 26px;
    fill: none;
    stroke: currentColor;
    stroke-width: 1.6;
    stroke-linecap: round;
    stroke-linejoin: round;
  }

  .shape[aria-pressed="true"] {
    background: var(--accent);
    border-color: var(--accent);
    color: var(--accent-text);
    font-weight: 700;
  }

  .swatches {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    padding: 5px;
  }

  .swatch {
    width: 40px;
    min-width: 40px;
    height: 40px;
    padding: 0;
  }

  .swatch[aria-pressed="true"] {
    outline: 3px solid var(--accent);
    outline-offset: 2px;
  }

  .more {
    align-self: flex-start;
  }

  p {
    margin: 0;
  }
</style>
