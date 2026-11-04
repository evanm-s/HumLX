<script lang="ts">
  // Make an effect on the selected fixtures, or change one. Pick a shape, then
  // set speed, size and spread; the rest sits under More, and the steps
  // themselves under Edit steps. The shapes on offer come from the window it
  // was opened from.

  import Dialog from "../Dialog.svelte";
  import { api } from "../../lib/api";
  import { show } from "../../lib/show.svelte";
  import type { Direction, Effect, EffectFamily, Pattern, Rgb, Shape, Speed } from "../../lib/types";
  import EffectPreview from "./EffectPreview.svelte";
  import StepEditor from "./StepEditor.svelte";
  import { FAMILY_LABELS, PATTERNS, recipe, SHAPES, shapesOf, type Step } from "./effects";

  interface Props {
    family: EffectFamily;
    effect?: Effect;
    onclose: () => void;
  }

  let { family, effect, onclose }: Props = $props();

  const shapes = $derived(shapesOf(family));
  const directions: Direction[] = ["forward", "backward", "bounce"];
  const SPEEDS: [Speed, string][] = [
    ["own", "Own"],
    ["a", "Speed A"],
    ["b", "Speed B"],
  ];
  const MULTIPLIERS: [number, string][] = [
    [0.25, "¼"],
    [0.5, "½"],
    [1, "×1"],
    [2, "×2"],
    [4, "×4"],
  ];
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
  const NAMES = ["Red", "Orange", "Yellow", "Green", "Cyan", "Blue", "Magenta", "White"];
  // The colours to pick steps from: the basics, then the colour presets.
  const palette = $derived([
    ...PALETTE.map((colour, i) => ({ name: NAMES[i], colour })),
    ...show.programmer.presets
      .filter((p) => p.family === "colour")
      .map((p) => ({ name: p.name, colour: p.all?.colour ?? Object.values(p.fixtures ?? {})[0]?.colour }))
      .filter((p): p is { name: string; colour: Rgb } => !!p.colour),
  ]);

  // Seeded once from the effect being edited; the dialog is remade per open.
  // svelte-ignore state_referenced_locally
  const startShape: Shape = effect?.shape ?? shapesOf(family)[0];
  // svelte-ignore state_referenced_locally
  const start = effect ?? { ...recipe(startShape), width: SHAPES[startShape].width ?? 1 };
  // svelte-ignore state_referenced_locally
  let draft = $state({
    name: effect?.name ?? "",
    shape: startShape,
    bpm: Math.round((effect?.rate ?? 1) * 60),
    size: effect?.size ?? 1,
    spread: effect?.spread ?? (start.fan ? 0 : 1),
    pattern: (effect?.pattern ?? "inOrder") as Pattern,
    direction: (effect?.direction ?? "forward") as Direction,
    width: start.width,
    speed: (effect?.speed ?? "own") as Speed,
    multiplier: effect?.multiplier ?? 1,
    transition: effect?.transition ?? SHAPES[startShape].transition,
    steps: structuredClone($state.snapshot(start.steps)) as Step[],
    layering: start.layering,
    curve: start.curve,
    wrap: start.wrap,
    fan: start.fan,
  });
  let more = $state(false);
  let editing = $state(false);
  const colourList = $derived(SHAPES[draft.shape].colours);
  // Live tempo of the speed master the effect follows, which tapping can change.
  const masterBpm = $derived(draft.speed === "own" ? 0 : show.live.speedBpm[draft.speed === "a" ? 0 : 1]);

  const preview = $derived({
    steps: draft.steps,
    layering: draft.layering,
    curve: draft.curve,
    wrap: draft.wrap,
    fan: draft.fan,
    transition: draft.transition,
    width: draft.width,
    direction: draft.direction,
    pattern: draft.pattern,
    rate: draft.speed === "own" ? draft.bpm / 60 : (masterBpm * draft.multiplier) / 60,
    size: draft.size,
    spread: draft.spread,
  });
  // svelte-ignore state_referenced_locally
  const count = effect?.fixtures.length ?? show.programmer.selection.length;

  function pick(shape: Shape) {
    if (shape === draft.shape) return;
    // Colours carry over between the shapes that take a list of them.
    const colours = colourList ? draft.steps.flatMap((s) => (s.colour ? [s.colour] : [])) : [];
    const made = recipe(shape, $state.snapshot(colours) as Rgb[]);
    if (made.fan !== draft.fan) draft.spread = made.fan ? 0 : 1;
    Object.assign(draft, made, {
      shape,
      transition: SHAPES[shape].transition,
      width: SHAPES[shape].width ?? 1,
    });
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
      speed: draft.speed,
      multiplier: draft.multiplier,
      transition: draft.transition,
      steps: $state.snapshot(draft.steps),
      layering: draft.layering,
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
  <EffectPreview fx={preview} {family} {count} />

  {#if colourList}
    <div class="colours">
      Colours
      <StepEditor
        {family}
        {palette}
        noun="Colour"
        min={colourList.min}
        max={colourList.max}
        steps={draft.steps}
        onchange={(steps) => (draft.steps = steps)}
      />
    </div>
  {/if}

  <div class="field">
    Speed from
    <div class="segmented">
      {#each SPEEDS as [sp, label] (sp)}
        <button aria-pressed={draft.speed === sp} onclick={() => (draft.speed = sp)}>{label}</button>
      {/each}
    </div>
  </div>
  {#if draft.speed === "own"}
    <label class="field">
      Speed: {draft.bpm} BPM
      <input type="range" min="3" max="240" step="1" bind:value={draft.bpm} />
    </label>
  {:else}
    <div class="field">
      Speed: {Math.round(masterBpm * draft.multiplier)} BPM ({draft.speed === "a" ? "Speed A" : "Speed B"} is {Math.round(masterBpm)})
      <div class="segmented">
        {#each MULTIPLIERS as [m, label] (m)}
          <button aria-pressed={draft.multiplier === m} onclick={() => (draft.multiplier = m)}>{label}</button>
        {/each}
      </div>
    </div>
  {/if}
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
    <div class="field">
      Layering
      <div class="segmented">
        <button aria-pressed={draft.layering === "onTop"} onclick={() => (draft.layering = "onTop")}>
          On top of the look
        </button>
        <button aria-pressed={draft.layering === "replace"} onclick={() => (draft.layering = "replace")}>
          Replace the look
        </button>
      </div>
    </div>
  {/if}

  {#if !colourList}
    <button class="ghost more" aria-expanded={editing} onclick={() => (editing = !editing)}>
      {editing ? "▾" : "▸"} Edit steps ({draft.steps.length})
    </button>
    {#if editing}
      <StepEditor {family} {palette} steps={draft.steps} onchange={(steps) => (draft.steps = steps)} />
    {/if}
  {/if}

  <label class="field">
    Name
    <input bind:value={draft.name} placeholder={SHAPES[draft.shape].label} />
  </label>
  {#if effect}
    <p class="muted">On {effect.fixtures.length} fixtures. Record a cue to keep it.</p>
  {:else}
    <p class="muted">
      On the {show.programmer.selection.length} selected fixtures, in pick order. Record a cue to keep it.
    </p>
  {/if}
  {#snippet actions()}
    {#if effect}<button class="danger" onclick={remove}>Remove</button>{/if}
    <span class="spacer"></span>
    <button onclick={onclose}>Cancel</button>
    <button class="primary" onclick={save}>{effect ? "Save" : "Add effect"}</button>
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

  .colours {
    display: grid;
    gap: 8px;
    color: var(--muted);
    font-size: 0.9rem;
  }

  .more {
    align-self: flex-start;
  }

  p {
    margin: 0;
  }
</style>
