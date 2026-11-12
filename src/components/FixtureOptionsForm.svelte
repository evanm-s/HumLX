<script lang="ts">
  // Output limits and orientation for one fixture, or several set the same.
  import NumberField from "./NumberField.svelte";
  import { MAX_COLUMNS, type FixtureOptions } from "../lib/types";

  interface Props {
    options: FixtureOptions;
    /** Show the pan and tilt controls. */
    moves: boolean;
    /** Show the virtual intensity switch: colour mixing, no dimmer channel. */
    dimmerless?: boolean;
    /** Show the Resolume OSC target and column names. */
    resolume?: boolean;
    onchange?: () => void;
  }

  let { options = $bindable(), moves, dimmerless = false, resolume = false, onchange }: Props = $props();

  function set(changes: Partial<FixtureOptions>) {
    options = { ...options, ...changes };
    onchange?.();
  }

  function setColumnCount(count: number) {
    const columns = options.columns.slice(0, count);
    while (columns.length < count) columns.push("");
    set({ columns });
  }

  function nameColumn(index: number, name: string) {
    set({ columns: options.columns.map((c, i) => (i === index ? name : c)) });
  }

  const PAN_STEPS = [-90, 0, 90, 180];
  const TILT_STEPS = [-90, 0, 90];
</script>

<div class="options">
  {#if resolume}
    <h3>Resolume</h3>
    <div class="pair">
      <label class="field">
        Resolume IP
        <input value={options.oscHost} onchange={(e) => set({ oscHost: e.currentTarget.value.trim() })} />
      </label>
      <NumberField label="OSC port" value={options.oscPort} min={1} max={65535} onchange={(v) => set({ oscPort: v ?? 7000 })} />
    </div>
    <NumberField label="Columns" value={options.columns.length} min={1} max={MAX_COLUMNS} onchange={(v) => setColumnCount(v ?? 8)} />
    <div class="names">
      {#each options.columns as name, i (i)}
        <input placeholder={`Column ${i + 1}`} value={name} onchange={(e) => nameColumn(i, e.currentTarget.value)} />
      {/each}
    </div>
    <p class="muted hint">Intensity is the composition master. Turn on OSC input in Resolume.</p>
  {/if}

  <h3>Limits and orientation</h3>
  <div class="pair">
    <NumberField
      label="Intensity limit (%)"
      value={options.intensityLimit}
      min={0}
      max={100}
      onchange={(v) => set({ intensityLimit: v ?? 100 })}
    />
    <p class="muted hint">
      {options.intensityLimit < 100
        ? `Full sends ${options.intensityLimit}%; lower levels scale.`
        : "Full sends full."}
    </p>
  </div>

  {#if dimmerless}
    <div class="pair">
      <div class="segmented">
        <button
          aria-pressed={options.virtualIntensity}
          onclick={() => set({ virtualIntensity: !options.virtualIntensity })}>Virtual intensity</button
        >
      </div>
      <p class="muted hint">
        {options.virtualIntensity ? "No dimmer channel; intensity scales the colour." : "Colour sets brightness; intensity does nothing."}
      </p>
    </div>
  {/if}

  {#if moves}
    <div class="segmented">
      <button aria-pressed={options.swapPanTilt} onclick={() => set({ swapPanTilt: !options.swapPanTilt })}
        >Swap pan/tilt</button
      >
      <button aria-pressed={options.invertPan} onclick={() => set({ invertPan: !options.invertPan })}
        >Invert pan</button
      >
      <button aria-pressed={options.invertTilt} onclick={() => set({ invertTilt: !options.invertTilt })}
        >Invert tilt</button
      >
    </div>
    <div class="pair">
      <div class="offset">
        <NumberField
          label="Pan offset (°)"
          value={options.panOffset}
          min={-options.panRange}
          max={options.panRange}
          onchange={(v) => set({ panOffset: v ?? 0 })}
        />
        <div class="steps">
          {#each PAN_STEPS as step (step)}
            <button aria-pressed={options.panOffset === step} onclick={() => set({ panOffset: step })}
              >{step > 0 ? `+${step}` : step}</button
            >
          {/each}
        </div>
      </div>
      <div class="offset">
        <NumberField
          label="Tilt offset (°)"
          value={options.tiltOffset}
          min={-options.tiltRange}
          max={options.tiltRange}
          onchange={(v) => set({ tiltOffset: v ?? 0 })}
        />
        <div class="steps">
          {#each TILT_STEPS as step (step)}
            <button aria-pressed={options.tiltOffset === step} onclick={() => set({ tiltOffset: step })}
              >{step > 0 ? `+${step}` : step}</button
            >
          {/each}
        </div>
      </div>
    </div>
    <div class="pair">
      <NumberField
        label="Pan range (°)"
        value={options.panRange}
        min={1}
        max={1000}
        onchange={(v) => set({ panRange: v ?? 540, panOffset: Math.max(-(v ?? 540), Math.min(v ?? 540, options.panOffset)) })}
      />
      <NumberField
        label="Tilt range (°)"
        value={options.tiltRange}
        min={1}
        max={1000}
        onchange={(v) => set({ tiltRange: v ?? 270, tiltOffset: Math.max(-(v ?? 270), Math.min(v ?? 270, options.tiltOffset)) })}
      />
    </div>
    <p class="muted hint">
      Applied at output; cues and presets don't change. Range is the fixture's full travel from its manual.
    </p>
  {/if}
</div>

<style>
  .options {
    display: flex;
    flex-direction: column;
    gap: 12px;
    border-top: 1px solid var(--line);
    padding-top: 12px;
  }

  h3 {
    margin: 0;
    font-size: 1rem;
  }

  .pair {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: var(--gap);
    align-items: end;
  }

  .names {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(140px, 1fr));
    gap: 6px;
  }

  .hint {
    margin: 0;
    font-size: 0.85rem;
  }

  .offset {
    display: grid;
    gap: 6px;
  }

  .steps {
    display: grid;
    grid-auto-flow: column;
    grid-auto-columns: 1fr;
    gap: 4px;
  }

  .steps button {
    min-height: 36px;
    padding: 0 4px;
    font-variant-numeric: tabular-nums;
  }

  .steps button[aria-pressed="true"] {
    background: var(--accent);
    color: var(--accent-text);
  }
</style>
