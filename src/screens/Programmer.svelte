<script lang="ts">
  // Select fixtures, set their intensity, colour, position and beam, and
  // record or recall presets. The main screen for operating the rig, laid out
  // as windows that can be moved, resized and hidden.

  import { onDestroy } from "svelte";
  import ColourPicker from "../components/ColourPicker.svelte";
  import Dialog from "../components/Dialog.svelte";
  import Fader from "../components/Fader.svelte";
  import PositionPad from "../components/PositionPad.svelte";
  import RecordCue from "../components/RecordCue.svelte";
  import Workspace from "../components/Workspace.svelte";
  import PresetPool from "../components/programmer/PresetPool.svelte";
  import ProgrammerSheet from "../components/programmer/ProgrammerSheet.svelte";
  import { api } from "../lib/api";
  import { attributeName, capabilityAt, fixtureColour, rgbCss, SWATCHES } from "../lib/format";
  import { loadLayout, saveLayout, type Grid, type Layout } from "../lib/layout";
  import { endSelectWindow, restartSelectWindow, tapMode } from "../lib/selection";
  import { attempt, show } from "../lib/show.svelte";
  import { RECORD_EVENT } from "../lib/surface.svelte";
  import type { Attribute, Channel, Family } from "../lib/types";

  const selection = $derived(show.programmer.selection);
  const selected = $derived(new Set(selection));
  const current = $derived(show.current);
  const program = show.program.bind(show);

  // --- Layout --------------------------------------------------------------

  const windows = [
    { id: "fixtures", title: "Fixtures" },
    { id: "groups", title: "Groups" },
    { id: "sheet", title: "Programmer" },
    { id: "presets-intensity", title: "Intensity presets" },
    { id: "presets-beam", title: "Beam presets" },
    { id: "presets-position", title: "Position presets" },
    { id: "presets-colour", title: "Colour presets" },
    { id: "intensity", title: "Intensity" },
    { id: "colour", title: "Colour" },
    { id: "position", title: "Position" },
    { id: "beam", title: "Beam" },
  ];
  const presetFamily = (id: string) => id.slice("presets-".length) as Family;

  const GRID: Grid = { cols: 24, rows: 12 };
  const DEFAULT: Layout = {
    fixtures: { x: 0, y: 0, w: 8, h: 5 },
    groups: { x: 8, y: 0, w: 5, h: 5 },
    sheet: { x: 13, y: 0, w: 11, h: 5 },
    "presets-intensity": { x: 0, y: 5, w: 2, h: 7 },
    "presets-beam": { x: 2, y: 5, w: 2, h: 7 },
    "presets-position": { x: 4, y: 5, w: 2, h: 7 },
    "presets-colour": { x: 6, y: 5, w: 2, h: 7 },
    intensity: { x: 8, y: 5, w: 3, h: 7 },
    colour: { x: 11, y: 5, w: 4, h: 7 },
    position: { x: 15, y: 5, w: 5, h: 7 },
    beam: { x: 20, y: 5, w: 4, h: 7 },
  };
  const KEY = "humlx.layout.programmer.v1";

  let layout = $state(loadLayout(KEY, DEFAULT, GRID));
  let editing = $state(false);
  let workspace: Workspace;
  const hidden = $derived(windows.filter((w) => layout[w.id]?.hidden));

  function setLayout(next: Layout) {
    layout = next;
    saveLayout(KEY, next);
  }

  // --- Selection -----------------------------------------------------------

  let editingGroups = $state(false);
  let namingGroup = $state(false);
  let groupName = $state("");

  function tapGroup(id: number) {
    if (editingGroups) program(() => api.deleteGroup(id));
    else program(() => api.selectGroup(id, tapMode("add")));
  }

  function tapFixture(id: number) {
    program(() => api.select([id], tapMode("toggle")));
  }

  function selectAll() {
    restartSelectWindow();
    program(() => api.select(show.patch.fixtures.map((f) => f.id), "set"));
  }

  function selectNone() {
    endSelectWindow();
    program(() => api.select([], "set"));
  }

  async function saveGroup() {
    const ok = await program(() => api.saveGroup(groupName.trim() || "Group", null));
    if (ok) namingGroup = false;
  }

  // --- Controls ------------------------------------------------------------

  let fine = $state(false);

  // The Position window shows either the XY pad or separate pan and tilt
  // faders; remembered per machine.
  const POSITION_MODE_KEY = "humlx.position.mode";
  let positionFaders = $state(readPositionMode());

  function readPositionMode() {
    try {
      return localStorage.getItem(POSITION_MODE_KEY) === "faders";
    } catch {
      return false;
    }
  }

  function setPositionFaders(on: boolean) {
    positionFaders = on;
    try {
      localStorage.setItem(POSITION_MODE_KEY, on ? "faders" : "pad");
    } catch {
      // Storage unavailable: the choice just lasts for this session.
    }
  }

  // Position faders run 0 to 1000 so each step is 0.1%.
  const POSITION_STEPS = 1000;
  const positionPercent = (v: number) => `${(v / 10).toFixed(1)}%`;
  let recordingCue = $state(false);

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

  // --- Highlight -----------------------------------------------------------

  // Puts the selection at full, white and tight with everything else out, to
  // find fixtures or check focus. Nothing is recorded, and turning it off
  // brings back whatever was on stage before.
  const highlight = $derived(show.live.highlight);

  function setHighlight(on: boolean) {
    // Update the button straight away rather than on the next live poll.
    show.live.highlight = on;
    return attempt(() => api.setHighlight(on));
  }

  // H toggles highlight, unless typing or a dialog is open.
  function keys(e: KeyboardEvent) {
    if (e.key.toLowerCase() !== "h" || e.ctrlKey || e.metaKey || e.altKey || e.repeat) return;
    if (namingGroup || recordingCue) return;
    const target = e.target as HTMLElement;
    if (target.closest("input, textarea, select, [role=dialog]")) return;
    e.preventDefault();
    setHighlight(!highlight);
  }

  // Record on the control surface opens the same dialog as the Record button.
  $effect(() => {
    const record = () => {
      if (!namingGroup && Object.keys(show.programmer.values).length > 0) recordingCue = true;
    };
    window.addEventListener(RECORD_EVENT, record);
    return () => window.removeEventListener(RECORD_EVENT, record);
  });

  // Leaving it on would keep the rest of the rig dark from other screens.
  onDestroy(() => {
    if (show.live.highlight) setHighlight(false);
  });

  const families: Record<string, Family> = { intensity: "intensity", colour: "colour", position: "position", beam: "beam" };
</script>

<svelte:window onkeydown={keys} />

<div class="programmer">
  <header class="row">
    <h1>Programmer</h1>
    <span class="muted">
      {selection.length === 0 ? "Nothing selected" : `${selection.length} selected`}
    </span>
    <span class="spacer"></span>
    {#if editing}
      <span class="muted">Drag title bars to move, corners to resize.</span>
      {#each hidden as w (w.id)}
        <button onclick={() => workspace.restore(w.id)}>+ {w.title}</button>
      {/each}
      <button onclick={() => setLayout(structuredClone(DEFAULT))}>Reset layout</button>
      <button class="primary" onclick={() => (editing = false)}>Done</button>
    {:else}
      <button onclick={() => (editing = true)}>Edit layout</button>
      <button
        class="highlight"
        aria-pressed={highlight}
        title="Selected fixtures full, white and tight; everything else out (H)"
        onclick={() => setHighlight(!highlight)}
      >
        {highlight ? "Highlight on" : "Highlight"}
      </button>
      <button
        class="record"
        disabled={Object.keys(show.programmer.values).length === 0}
        onclick={() => (recordingCue = true)}
      >
        ● Record cue
      </button>
      <button class="danger" onclick={() => program(() => api.clearProgrammer(null))}>Clear all</button>
    {/if}
  </header>

  <Workspace bind:this={workspace} {windows} {layout} grid={GRID} {editing} onchange={setLayout} {body} {tools} />
</div>

{#snippet tools(id: string)}
  {#if id === "fixtures"}
    <button class="small" onclick={selectAll}>All</button>
    <button class="small" onclick={selectNone}>None</button>
  {:else if id === "groups" && show.programmer.groups.length > 0}
    <button class="small" class:warn={editingGroups} onclick={() => (editingGroups = !editingGroups)}>
      {editingGroups ? "Done" : "Edit"}
    </button>
  {:else if families[id]}
    <button class="small" onclick={() => program(() => api.clearProgrammer(families[id]))}>Clear</button>
  {/if}
{/snippet}

{#snippet noSelection()}
  <p class="muted">Select fixtures or a group first.</p>
{/snippet}

{#snippet body(id: string)}
  {#if id === "fixtures"}
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
  {:else if id === "groups"}
    {#if editingGroups}<p class="muted hint">Tap a group to delete it.</p>{/if}
    <div class="tiles">
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
          groupName = "";
          namingGroup = true;
        }}
      >
        + Save selection
      </button>
    </div>
  {:else if id === "sheet"}
    <ProgrammerSheet />
  {:else if id.startsWith("presets-")}
    <PresetPool family={presetFamily(id)} />
  {:else if selection.length === 0}
    {@render noSelection()}
  {:else if id === "intensity"}
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
  {:else if id === "colour"}
    <div class="colour">
      <ColourPicker colour={current.colour} onchange={(c) => program(() => api.setColour(c))} />
      <div class="swatches">
        {#each SWATCHES as s (s.name)}
          <button
            class="swatch"
            title={s.name}
            aria-label={s.name}
            style:background={rgbCss(s.colour)}
            onclick={() => program(() => api.setColour(s.colour))}
          ></button>
        {/each}
      </div>
    </div>
  {:else if id === "position"}
    <div class="position">
      <div class="segmented" role="group" aria-label="Position control">
        <button aria-pressed={!positionFaders} class:on={!positionFaders} onclick={() => setPositionFaders(false)}>
          Pad
        </button>
        <button aria-pressed={positionFaders} class:on={positionFaders} onclick={() => setPositionFaders(true)}>
          Faders
        </button>
      </div>
      {#if positionFaders}
        <div class="pos-faders">
          {#each [{ axis: "pan", label: "Pan", level: current.pan }, { axis: "tilt", label: "Tilt", level: current.tilt }] as f (f.axis)}
            <Fader
              wide
              {fine}
              max={POSITION_STEPS}
              format={positionPercent}
              value={Math.round((f.level ?? 0.5) * POSITION_STEPS)}
              label={f.label}
              detail={f.level == null ? "Not set" : undefined}
              active={f.level != null}
              onchange={(v) =>
                program(() =>
                  f.axis === "pan" ? api.setPosition(v / POSITION_STEPS, null) : api.setPosition(null, v / POSITION_STEPS),
                )}
            />
          {/each}
        </div>
      {:else}
        <PositionPad
          pan={current.pan}
          tilt={current.tilt}
          {fine}
          onchange={(pan, tilt) => program(() => api.setPosition(pan, tilt))}
        />
      {/if}
      <div class="pos-buttons">
        <button aria-pressed={fine} class:on={fine} onclick={() => (fine = !fine)}>Fine</button>
        <button onclick={() => program(() => api.setPosition(0.5, 0.5))}>Centre</button>
        <button onclick={() => program(() => api.setPosition(0.5, null))}>Centre pan</button>
        <button onclick={() => program(() => api.setPosition(null, 0.5))}>Centre tilt</button>
      </div>
    </div>
  {:else if id === "beam"}
    <div class="beam">
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
{/snippet}

{#if namingGroup}
  <Dialog title="Save group" onclose={() => (namingGroup = false)}>
    <label class="field">
      Name
      <input bind:value={groupName} />
    </label>
    <p class="muted">Saves the {selection.length} selected fixtures, in the order you picked them.</p>
    {#snippet actions()}
      <button onclick={() => (namingGroup = false)}>Cancel</button>
      <button class="primary" onclick={saveGroup}>Save</button>
    {/snippet}
  </Dialog>
{/if}

{#if recordingCue}
  <RecordCue onclose={() => (recordingCue = false)} />
{/if}

<style>
  .programmer {
    height: 100%;
    display: grid;
    grid-template-rows: auto 1fr;
    gap: var(--gap);
    min-height: 0;
  }

  .highlight[aria-pressed="true"] {
    background: var(--accent);
    border-color: var(--accent);
    color: #fff;
  }

  .hint {
    margin: 0 0 10px;
    font-size: 0.85rem;
  }

  .tiles {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(210px, 1fr));
    gap: 8px;
  }

  .tile {
    display: grid;
    grid-template-columns: 36px 1fr auto;
    align-items: center;
    gap: 6px;
    padding: 0 12px;
    min-height: 64px;
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

  .group {
    display: grid;
    text-align: left;
    padding: 10px 14px;
    min-height: 64px;
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

  .on {
    border-color: var(--accent);
    color: var(--accent);
    background: color-mix(in srgb, var(--accent) 15%, var(--panel-raised));
  }

  /* Controls size themselves to their window (the window body is a size container). */
  .intensity {
    display: flex;
    gap: 12px;
    height: 100cqh;
    justify-content: center;
  }

  .intensity :global(.fader) {
    height: 100%;
  }

  .levels {
    display: flex;
    flex-direction: column;
    gap: 8px;
    flex: 0 1 110px;
    min-width: 72px;
  }

  .levels button {
    flex: 1;
    min-height: 44px;
    max-height: 80px;
    padding: 0 6px;
  }

  .colour,
  .position {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 12px;
  }

  .colour :global(.disc) {
    width: min(100cqw, 100cqh - 120px);
    flex-shrink: 0;
  }

  .position :global(.pad) {
    width: min(100cqw, 100cqh - 170px);
    flex-shrink: 0;
  }

  .swatches {
    display: grid;
    grid-template-columns: repeat(6, 1fr);
    gap: 6px;
    width: 100%;
  }

  .swatch {
    min-height: 48px;
    padding: 0;
    box-shadow: inset 0 0 0 1px rgb(255 255 255 / 0.15);
  }

  .segmented {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 6px;
    width: 100%;
  }

  .segmented button {
    min-height: 40px;
  }

  .pos-faders {
    display: flex;
    justify-content: center;
    gap: 24px;
    width: 100%;
    height: calc(100cqh - 176px);
    min-height: 0;
  }

  .pos-buttons {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 6px;
    width: 100%;
  }

  .pos-buttons button {
    min-height: 52px;
    padding: 0 6px;
  }

  .beam {
    display: flex;
    gap: 12px;
    height: 100cqh;
  }

  .beam :global(.fader) {
    flex-shrink: 0;
  }
</style>
