<script lang="ts">
  // Select fixtures, set their intensity, colour, position and beam, and
  // record or recall presets. The main screen for operating the rig.

  import ColourPicker from "../components/ColourPicker.svelte";
  import Dialog from "../components/Dialog.svelte";
  import Fader from "../components/Fader.svelte";
  import PositionPad from "../components/PositionPad.svelte";
  import { api } from "../lib/api";
  import { attributeName, capabilityAt, fixtureColour, rgbCss } from "../lib/format";
  import { show } from "../lib/show.svelte";
  import type { Attribute, Channel, Family, Preset, Rgb } from "../lib/types";

  const tabs: { id: Family; label: string }[] = [
    { id: "intensity", label: "Intensity" },
    { id: "colour", label: "Colour" },
    { id: "position", label: "Position" },
    { id: "beam", label: "Beam" },
  ];
  let tab = $state<Family>("intensity");

  const selection = $derived(show.programmer.selection);
  const selected = $derived(new Set(selection));
  const current = $derived(show.current);
  const program = show.program.bind(show);

  // --- Selection -----------------------------------------------------------

  function tapFixture(id: number) {
    program(() => api.select([id], "toggle"));
  }

  function selectAll() {
    program(() => api.select(show.patch.fixtures.map((f) => f.id), "set"));
  }

  let editingGroups = $state(false);
  let naming = $state<null | { kind: "group" } | { kind: "preset"; family: Family; id: number }>(null);
  let name = $state("");

  function tapGroup(id: number) {
    if (editingGroups) program(() => api.deleteGroup(id));
    else program(() => api.selectGroup(id, "set"));
  }

  // --- Controls ------------------------------------------------------------

  const swatches: { name: string; colour: Rgb }[] = [
    { name: "Red", colour: { r: 1, g: 0, b: 0 } },
    { name: "Orange", colour: { r: 1, g: 0.45, b: 0 } },
    { name: "Amber", colour: { r: 1, g: 0.7, b: 0.1 } },
    { name: "Yellow", colour: { r: 1, g: 1, b: 0 } },
    { name: "Green", colour: { r: 0, g: 1, b: 0 } },
    { name: "Cyan", colour: { r: 0, g: 1, b: 1 } },
    { name: "Blue", colour: { r: 0, g: 0, b: 1 } },
    { name: "Lavender", colour: { r: 0.6, g: 0.45, b: 1 } },
    { name: "Magenta", colour: { r: 1, g: 0, b: 1 } },
    { name: "Pink", colour: { r: 1, g: 0.4, b: 0.7 } },
    { name: "Warm", colour: { r: 1, g: 0.85, b: 0.6 } },
    { name: "White", colour: { r: 1, g: 1, b: 1 } },
  ];

  let fine = $state(false);

  const BEAM: Attribute[] = [
    "shutter",
    "zoom",
    "focus",
    "iris",
    "frost",
    "gobo",
    "gobo_rotation",
    "prism",
    "color_wheel",
    "color_temperature",
    "pan_tilt_speed",
    "speed",
    "effect",
    "control",
  ];

  // Beam attributes the selected fixtures have, with the first fixture's
  // channel for showing its labelled ranges.
  const beamChannels = $derived.by(() => {
    const found = new Map<Attribute, Channel>();
    for (const id of selection) {
      const fixture = show.fixture(id);
      if (!fixture) continue;
      for (const channel of show.channels(fixture)) {
        if (!channel.fineOf && BEAM.includes(channel.attribute) && !found.has(channel.attribute)) {
          found.set(channel.attribute, channel);
        }
      }
    }
    return BEAM.filter((a) => found.has(a)).map((attribute) => ({ attribute, channel: found.get(attribute)! }));
  });

  // --- Presets ---------------------------------------------------------------

  let recording = $state(false);
  let deleting = $state(false);

  const presets = $derived(show.programmer.presets.filter((p) => p.family === tab));
  const nextSlot = $derived.by(() => {
    const used = new Set(show.programmer.presets.map((p) => p.id));
    let id = 1;
    while (used.has(id)) id++;
    return id;
  });

  function presetSample(preset: Preset) {
    return preset.all ?? Object.values(preset.fixtures ?? {})[0] ?? {};
  }

  function defaultName(family: Family): string {
    if (family === "intensity" && current.intensity != null) return `${Math.round(current.intensity * 100)}%`;
    if (family === "colour" && current.colour) {
      const c = current.colour;
      const match = swatches.find((s) => Math.abs(s.colour.r - c.r) + Math.abs(s.colour.g - c.g) + Math.abs(s.colour.b - c.b) < 0.15);
      if (match) return match.name;
    }
    return { intensity: "Level", colour: "Colour", position: "Position", beam: "Beam", control: "Control" }[family];
  }

  function tapPreset(preset: Preset | null, slot: number) {
    if (deleting && preset) {
      program(() => api.deletePreset(preset.id));
    } else if (recording || !preset) {
      name = preset?.name ?? defaultName(tab);
      naming = { kind: "preset", family: tab, id: slot };
    } else {
      program(() => api.applyPreset(preset.id));
    }
  }

  async function saveName() {
    const target = naming;
    if (!target) return;
    const value = name.trim() || (target.kind === "group" ? `Group` : defaultName(target.family));
    const ok =
      target.kind === "group"
        ? await program(() => api.saveGroup(value, null))
        : await program(() => api.recordPreset(target.family, value, target.id));
    if (ok) {
      naming = null;
      recording = false;
    }
  }
</script>

<div class="programmer">
  <header class="row">
    <h1>Programmer</h1>
    <span class="muted">
      {selection.length === 0 ? "Nothing selected" : `${selection.length} selected`}
    </span>
    <span class="spacer"></span>
    <button class="danger" onclick={() => program(() => api.clearProgrammer(null))}>Clear all</button>
  </header>

  <!-- Selection -->
  <section class="panel select scroll" aria-label="Selection">
    <div class="row">
      <h2>Fixtures</h2>
      <span class="spacer"></span>
      <button onclick={selectAll}>All</button>
      <button onclick={() => program(() => api.select([], "set"))}>None</button>
    </div>
    <div class="tiles">
      {#each show.patch.fixtures as f (f.id)}
        <button class="tile" aria-pressed={selected.has(f.id)} onclick={() => tapFixture(f.id)}>
          <span class="num" style:color={fixtureColour(f.id)}>{f.id}</span>
          <span class="name">{f.name}</span>
          {#if show.programmer.values[f.id]}<span class="dot" aria-label="Has programmer values"></span>{/if}
        </button>
      {:else}
        <p class="muted">Patch some fixtures first.</p>
      {/each}
    </div>

    <div class="row">
      <h2>Groups</h2>
      <span class="spacer"></span>
      {#if show.programmer.groups.length > 0}
        <button aria-pressed={editingGroups} class:warn={editingGroups} onclick={() => (editingGroups = !editingGroups)}>
          {editingGroups ? "Done" : "Edit"}
        </button>
      {/if}
    </div>
    {#if editingGroups}<p class="muted hint">Tap a group to delete it.</p>{/if}
    <div class="groups">
      {#each show.programmer.groups as g (g.id)}
        <button class="group" class:deleting={editingGroups} onclick={() => tapGroup(g.id)}>
          <span>{g.name}</span>
          <span class="muted">{g.fixtures.length} fixtures</span>
        </button>
      {/each}
      <button
        class="group add"
        disabled={selection.length === 0}
        onclick={() => {
          name = "";
          naming = { kind: "group" };
        }}
      >
        + Save selection
      </button>
    </div>
  </section>

  <!-- Controls -->
  <section class="panel controls">
    <div class="row">
      <div class="segmented tabs" role="tablist">
        {#each tabs as t (t.id)}
          <button role="tab" aria-selected={tab === t.id} onclick={() => (tab = t.id)}>
            {t.label}
          </button>
        {/each}
      </div>
      <span class="spacer"></span>
      <button onclick={() => program(() => api.clearProgrammer(tab))}>Clear {tab}</button>
    </div>

    {#if selection.length === 0}
      <div class="empty muted">Select fixtures or a group on the left to start.</div>
    {:else if tab === "intensity"}
      <div class="intensity">
        <Fader
          wide
          max={100}
          value={Math.round((current.intensity ?? 0) * 100)}
          label="Intensity"
          detail={current.intensity == null ? "Not set" : `${Math.round(current.intensity * 100)}%`}
          active={current.intensity != null}
          onchange={(v) => program(() => api.setIntensity(v / 100))}
        />
        <div class="levels">
          {#each [100, 75, 50, 25, 10, 0] as level (level)}
            <button onclick={() => program(() => api.setIntensity(level / 100))}>
              {level === 100 ? "Full" : level === 0 ? "Out" : `${level}%`}
            </button>
          {/each}
        </div>
      </div>
    {:else if tab === "colour"}
      <div class="colour">
        <ColourPicker colour={current.colour} onchange={(c) => program(() => api.setColour(c))} />
        <div class="swatches">
          {#each swatches as s (s.name)}
            <button class="swatch" onclick={() => program(() => api.setColour(s.colour))}>
              <span class="chip" style:background={rgbCss(s.colour)}></span>
              {s.name}
            </button>
          {/each}
        </div>
      </div>
    {:else if tab === "position"}
      <div class="position">
        <PositionPad
          pan={current.pan}
          tilt={current.tilt}
          {fine}
          onchange={(pan, tilt) => program(() => api.setPosition(pan, tilt))}
        />
        <div class="levels">
          <button aria-pressed={fine} class:on={fine} onclick={() => (fine = !fine)}>Fine</button>
          <button onclick={() => program(() => api.setPosition(0.5, 0.5))}>Centre</button>
          <button onclick={() => program(() => api.setPosition(0.5, null))}>Centre pan</button>
          <button onclick={() => program(() => api.setPosition(null, 0.5))}>Centre tilt</button>
        </div>
      </div>
    {:else}
      <div class="beam scroll-x">
        {#each beamChannels as { attribute, channel } (attribute)}
          {@const set = current.other?.[attribute]}
          <!-- Unset channels show the level they are resting at. -->
          {@const value = set == null ? channel.default : Math.round(set * 255)}
          <Fader
            {value}
            label={attributeName(attribute)}
            sublabel={set == null ? "Not set" : undefined}
            detail={capabilityAt(channel.capabilities, value)}
            active={set != null}
            onchange={(v) => program(() => api.setAttribute(attribute, v / 255))}
          />
        {:else}
          <p class="muted">The selected fixtures have no beam controls.</p>
        {/each}
      </div>
    {/if}
  </section>

  <!-- Presets -->
  <section class="panel presets scroll">
    <div class="row">
      <h2>{tabs.find((t) => t.id === tab)?.label} presets</h2>
      <span class="spacer"></span>
    </div>
    <div class="row">
      <button
        class="record"
        aria-pressed={recording}
        class:on={recording}
        onclick={() => {
          recording = !recording;
          deleting = false;
        }}
      >
        ● Record
      </button>
      <button
        aria-pressed={deleting}
        class:warn={deleting}
        onclick={() => {
          deleting = !deleting;
          recording = false;
        }}
      >
        Delete
      </button>
    </div>
    <p class="muted hint">
      {#if recording}Tap a slot to store the {tab} of the selected fixtures.
      {:else if deleting}Tap a preset to delete it.
      {:else}Tap a preset to apply it to the selection.{/if}
    </p>
    <div class="slots">
      {#each presets as p (p.id)}
        {@const sample = presetSample(p)}
        <button
          class="slot"
          class:deleting
          class:recording
          disabled={!recording && !deleting && selection.length === 0}
          onclick={() => tapPreset(p, p.id)}
        >
          {#if p.family === "colour"}
            <span class="chip big" style:background={rgbCss(sample.colour, "var(--line)")}></span>
          {:else if p.family === "intensity"}
            <span class="bar"><span style:width="{(sample.intensity ?? 0) * 100}%"></span></span>
          {:else if p.family === "position"}
            <span class="mini-pad"
              ><span style:left="{(sample.pan ?? 0.5) * 100}%" style:top="{(1 - (sample.tilt ?? 0.5)) * 100}%"></span></span
            >
          {/if}
          <span class="slot-name">{p.name}</span>
          <span class="slot-id muted">{p.id}</span>
        </button>
      {/each}
      {#if recording}
        <button class="slot empty-slot" onclick={() => tapPreset(null, nextSlot)}>+ New</button>
      {/if}
    </div>
    {#if presets.length === 0 && !recording}
      <p class="muted">No {tab} presets yet. Set some values, tap <strong>Record</strong>, then tap a slot.</p>
    {/if}
  </section>
</div>

{#if naming}
  <Dialog title={naming.kind === "group" ? "Save group" : "Record preset"} onclose={() => (naming = null)}>
    <label class="field">
      Name
      <!-- svelte-ignore a11y_autofocus -->
      <input bind:value={name} autofocus onkeydown={(e) => e.key === "Enter" && saveName()} />
    </label>
    {#if naming.kind === "group"}
      <p class="muted">Saves the {selection.length} selected fixtures, in the order you picked them.</p>
    {/if}
    {#snippet actions()}
      <button onclick={() => (naming = null)}>Cancel</button>
      <button class="primary" onclick={saveName}>Save</button>
    {/snippet}
  </Dialog>
{/if}

<style>
  .programmer {
    height: 100%;
    display: grid;
    grid-template-columns: 480px 1fr 440px;
    grid-template-rows: auto 1fr;
    gap: var(--gap);
  }

  header {
    grid-column: 1 / -1;
  }

  .panel {
    padding: 20px;
    min-height: 0;
  }

  .select,
  .presets {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  .hint {
    margin: 0;
    font-size: 0.85rem;
  }

  .tiles {
    display: grid;
    grid-template-columns: repeat(2, 1fr);
    gap: 8px;
  }

  .tile {
    display: grid;
    grid-template-columns: 40px 1fr auto;
    align-items: center;
    gap: 6px;
    padding: 0 12px;
    min-height: 68px;
    text-align: left;
  }

  .tile[aria-pressed="true"] {
    border-color: var(--accent);
    background: color-mix(in srgb, var(--accent) 22%, var(--panel-raised));
  }

  .num {
    font-weight: 700;
    font-variant-numeric: tabular-nums;
  }

  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .dot {
    width: 10px;
    height: 10px;
    border-radius: 50%;
    background: var(--accent);
  }

  .groups {
    display: grid;
    grid-template-columns: repeat(2, 1fr);
    gap: 8px;
  }

  .group {
    display: grid;
    text-align: left;
    padding: 10px 14px;
    min-height: 68px;
    align-content: center;
  }

  .group span:last-child {
    font-size: 0.8rem;
  }

  .group.add {
    border-style: dashed;
    text-align: center;
    color: var(--muted);
  }

  .deleting {
    border-color: var(--danger) !important;
  }

  .warn {
    border-color: var(--danger);
    color: var(--danger);
  }

  .controls {
    display: grid;
    grid-template-rows: auto 1fr;
    gap: 20px;
  }

  .tabs button {
    min-width: 150px;
    font-size: 1.05rem;
  }

  .empty {
    display: grid;
    place-items: center;
    font-size: 1.1rem;
  }

  .intensity,
  .position {
    display: flex;
    gap: 40px;
    min-height: 0;
    justify-content: center;
    align-items: stretch;
  }

  .position {
    align-items: flex-start;
  }

  .intensity :global(.fader) {
    height: 100%;
  }

  .levels {
    display: flex;
    flex-direction: column;
    gap: 10px;
    width: 200px;
  }

  .levels button {
    min-height: 72px;
    font-size: 1.1rem;
  }

  .on {
    border-color: var(--accent);
    color: var(--accent);
    background: color-mix(in srgb, var(--accent) 15%, var(--panel-raised));
  }

  .colour {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 260px;
    gap: 32px;
    align-items: start;
    justify-items: center;
    min-height: 0;
  }

  /* Keep the disc and pad square and inside the panel on any screen height. */
  .colour :global(.disc),
  .position :global(.pad) {
    width: min(640px, calc(100vh - 300px), 100%);
    flex-shrink: 0;
  }

  .swatches {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
    width: 100%;
  }

  .swatch {
    display: flex;
    gap: 10px;
    align-items: center;
    padding: 0 12px;
    min-height: 64px;
  }

  .chip {
    width: 28px;
    height: 28px;
    border-radius: 8px;
    flex-shrink: 0;
    box-shadow: inset 0 0 0 1px rgb(255 255 255 / 0.15);
  }

  .chip.big {
    width: 100%;
    height: 44px;
  }

  .beam {
    display: flex;
    gap: 12px;
    min-height: 0;
    overflow-x: auto;
  }

  .record.on {
    border-color: var(--danger);
    color: #fff;
    background: var(--danger);
  }

  .slots {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 10px;
  }

  .slot {
    position: relative;
    display: grid;
    gap: 8px;
    align-content: end;
    padding: 12px;
    min-height: 110px;
    text-align: left;
  }

  .slot.recording {
    border-color: color-mix(in srgb, var(--danger) 60%, transparent);
  }

  .slot-name {
    font-weight: 600;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .slot-id {
    position: absolute;
    top: 8px;
    right: 10px;
    font-size: 0.75rem;
  }

  .empty-slot {
    border-style: dashed;
    color: var(--muted);
    align-content: center;
    text-align: center;
  }

  .bar {
    height: 14px;
    border-radius: 7px;
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
    height: 44px;
    border-radius: 8px;
    background: var(--bg);
  }

  .mini-pad span {
    position: absolute;
    width: 12px;
    height: 12px;
    margin: -6px 0 0 -6px;
    border-radius: 50%;
    background: var(--accent);
  }

  /* Narrower side columns when the window is smaller than the studio screen. */
  @media (max-width: 2000px) {
    .programmer {
      grid-template-columns: 360px 1fr 340px;
    }
    .tabs button {
      min-width: 110px;
    }
    .slots {
      grid-template-columns: repeat(2, 1fr);
    }
  }
</style>
