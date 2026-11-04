<script lang="ts">
  // Select fixtures, set their intensity, colour, position and beam, add
  // effects to them, and record or recall presets. The main screen for operating the rig, laid out
  // as windows that can be moved, resized and hidden.

  import { onDestroy } from "svelte";
  import EffectList from "../components/programmer/EffectList.svelte";
  import { EFFECT_WINDOWS, pathOf, SHAPES } from "../components/programmer/effects";
  import ColourPicker from "../components/ColourPicker.svelte";
  import Dialog from "../components/Dialog.svelte";
  import Fader from "../components/Fader.svelte";
  import PositionPad from "../components/PositionPad.svelte";
  import RecordCue from "../components/RecordCue.svelte";
  import StageWindow from "../components/StageWindow.svelte";
  import WhiteBalance from "../components/WhiteBalance.svelte";
  import Workspace from "../components/Workspace.svelte";
  import PresetPool from "../components/programmer/PresetPool.svelte";
  import ProgrammerSheet from "../components/programmer/ProgrammerSheet.svelte";
  import { api } from "../lib/api";
  import { attributeColour, attributeName, capabilityAt, fixtureColour, rgbCss, SWATCHES } from "../lib/format";
  import { nearestWheelSlot } from "../lib/wheel";
  import { loadLayout, saveLayout, type Grid, type Layout } from "../lib/layout";
  import { endSelectWindow, restartSelectWindow, tapSelectMode } from "../lib/selection";
  import { attempt, show } from "../lib/show.svelte";
  import { RECORD_EVENT, surface } from "../lib/surface.svelte";
  import { sheetPicks } from "../lib/sheet.svelte";
  import { updateCue } from "../lib/updatecue.svelte";
  import type { Attribute, Channel, Family, Fixture, Rgb } from "../lib/types";

  const selection = $derived(show.programmer.selection);
  const selected = $derived(new Set(selection));
  const current = $derived(show.current);
  const program = show.program.bind(show);

  // Fixtures in ID order, split into runs of the same type (profile and
  // mode). Each run starts on its own row like Onyx, so a type patched again
  // later (say dimmers 17-18 after the movers) gets its own row in ID order.
  const fixtureTypes = $derived.by(() => {
    const runs: Fixture[][] = [];
    let lastKey: string | undefined;
    for (const f of [...show.patch.fixtures].sort((a, b) => a.id - b.id)) {
      const key = `${f.profile}\u0000${f.mode}`;
      if (key === lastKey) runs[runs.length - 1].push(f);
      else runs.push([f]);
      lastKey = key;
    }
    return runs;
  });

  // --- Layout --------------------------------------------------------------

  const windows = [
    { id: "fixtures", title: "Fixtures" },
    { id: "groups", title: "Groups" },
    { id: "sheet", title: "Programmer" },
    { id: "presets-intensity", title: "Intensity presets", tone: "var(--f-intensity)" },
    { id: "presets-beam", title: "Beam presets", tone: "var(--f-beam)" },
    { id: "presets-position", title: "Position presets", tone: "var(--f-position)" },
    { id: "presets-colour", title: "Colour presets", tone: "var(--f-colour)" },
    { id: "intensity", title: "Intensity", tone: "var(--f-intensity)" },
    { id: "colour", title: "Colour", tone: "var(--f-colour)" },
    { id: "position", title: "Position", tone: "var(--f-position)" },
    { id: "beam", title: "Beam", tone: "var(--f-beam)" },
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
    else {
      const fixtures = show.programmer.groups.find((g) => g.id === id)?.fixtures ?? [];
      program(() => api.selectGroup(id, tapSelectMode(fixtures, selection)));
    }
  }

  function tapFixture(id: number) {
    program(() => api.select([id], tapSelectMode([id], selection)));
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
  // The stage plan pops out over everything, big enough to aim precisely.
  let stageOpen = $state(false);

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

  // --- Effects -------------------------------------------------------------

  // The Intensity, Colour and Position windows can swap their controls for
  // the effects of their family.
  let effectsShown = $state<Record<string, boolean>>({});
  const runningOfFamily = (id: string) =>
    show.programmer.effects.filter((e) => SHAPES[e.shape]?.family === EFFECT_WINDOWS[id]).length;

  // The path of the position effect on the first selected fixture, drawn on the pad.
  const positionPath = $derived.by(() => {
    const first = selection[0];
    const fx = show.programmer.effects.find((e) => SHAPES[e.shape]?.family === "position" && e.fixtures.includes(first));
    if (!fx) return undefined;
    return pathOf(fx, fx.fixtures.indexOf(first), fx.fixtures.length, { pan: current.pan, tilt: current.tilt });
  });

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

  // --- Colour ------------------------------------------------------------

  // The colour window shows either the picker or a fader per emitter.
  const COLOUR_MODE_KEY = "humlx.programmer.colourFaders";
  let colourFaders = $state(readColourFaders());

  function readColourFaders() {
    try {
      return localStorage.getItem(COLOUR_MODE_KEY) === "true";
    } catch {
      return false;
    }
  }

  function setColourFaders(on: boolean) {
    colourFaders = on;
    try {
      localStorage.setItem(COLOUR_MODE_KEY, String(on));
    } catch {
      // Only a convenience; the toggle still works for this session.
    }
  }

  const EMITTERS: Attribute[] = [
    "red",
    "green",
    "blue",
    "white",
    "warm_white",
    "cool_white",
    "amber",
    "lime",
    "uv",
    "cyan",
    "magenta",
    "yellow",
  ];
  const WHITES: Attribute[] = ["white", "warm_white", "cool_white"];

  // Colour channels the selected fixtures have, with the first fixture's
  // channel for each, for its resting level and labelled ranges.
  const colourChannels = $derived.by(() => {
    const found = new Map<Attribute, Channel>();
    for (const id of selection) {
      const fixture = show.fixture(id);
      if (!fixture) continue;
      for (const channel of show.channels(fixture)) {
        if (!channel.fineOf && !found.has(channel.attribute)) found.set(channel.attribute, channel);
      }
    }
    return found;
  });
  const colourHas = (attribute: Attribute) => colourChannels.has(attribute);
  // Anything that can make a colour other than white uses the colour disc.
  const canMix = $derived(
    (["red", "green", "blue", "cyan", "magenta", "yellow", "color_wheel"] as Attribute[]).some(colourHas),
  );
  // Bi-colour fixtures: a white balance channel, or warm and cool emitters.
  const canBalance = $derived(
    colourHas("color_temperature") || (colourHas("warm_white") && colourHas("cool_white") && !colourHas("red")),
  );
  // Which windows have anything to control for the selection. Like Beam, a
  // window whose attributes none of the selected fixtures have says so
  // instead of offering controls that would do nothing; a mixed selection
  // gets the controls if any fixture can use them.
  const MIXERS: Attribute[] = ["red", "green", "blue", "white", "warm_white", "cool_white", "cyan", "magenta", "yellow"];
  // Fixtures without a dimmer take their intensity from their colour mix.
  const hasIntensity = $derived(colourHas("intensity") || MIXERS.some(colourHas));
  const hasColour = $derived([...EMITTERS, "color_temperature", "color_wheel"].some((a) => colourHas(a as Attribute)));
  const hasPosition = $derived(colourHas("pan") || colourHas("tilt"));
  // Colour wheels get a fader too, so wheel-only fixtures are not left out.
  const colourFaderList = $derived(
    [...EMITTERS, "color_wheel" as Attribute, "color_temperature" as Attribute]
      .filter(colourHas)
      .map((attribute) => ({ attribute, channel: colourChannels.get(attribute)! })),
  );

  // The level the first selected fixture's emitter gets from the picked
  // colour, mirroring how the engine splits a colour into its channels.
  function emitterFromColour(attribute: Attribute): number | undefined {
    const colour = current.colour;
    const fixture = show.fixture(selection[0] ?? null);
    if (!colour || !fixture) return undefined;
    const channels = show.channels(fixture).filter((c) => !c.fineOf);
    const own = new Set(channels.map((c) => c.attribute));
    if (attribute === "color_wheel") {
      // The engine only turns the wheel when the fixture cannot mix the colour.
      if (["red", "green", "blue", "cyan", "magenta", "yellow"].some((a) => own.has(a as Attribute))) return undefined;
      const wheel = channels.find((c) => c.attribute === "color_wheel");
      const slot = wheel && nearestWheelSlot(wheel, colour);
      return slot == null ? undefined : slot / 255;
    }
    const scale = own.has("intensity") ? 1 : (current.intensity ?? 0);
    const [r, g, b] = [colour.r * scale, colour.g * scale, colour.b * scale];
    if (own.has("red") || own.has("green") || own.has("blue")) {
      const white = WHITES.some((a) => own.has(a)) ? Math.min(r, g, b) : 0;
      const levels: Partial<Record<Attribute, number>> = { red: r - white, green: g - white, blue: b - white };
      return WHITES.includes(attribute) ? white : (levels[attribute] ?? 0);
    }
    if (attribute === "cyan") return 1 - r;
    if (attribute === "magenta") return 1 - g;
    if (attribute === "yellow") return 1 - b;
    return undefined;
  }

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

  // Keys, unless typing or a dialog is open: H toggles highlight, L is Load,
  // and R and Delete work like the control surface's Record and Delete.
  // Delete (or Backspace) takes out what's picked on the programmer sheet.
  function keys(e: KeyboardEvent) {
    if (e.ctrlKey || e.metaKey || e.altKey || e.repeat || e.defaultPrevented) return;
    if (namingGroup || recordingCue) return;
    const target = e.target as HTMLElement;
    if (target.closest("input, textarea, select, [role=dialog]")) return;
    const key = e.key.toLowerCase();
    if (key === "h") setHighlight(!highlight);
    else if (key === "r") surface.press("record");
    else if (key === "l") return void (e.preventDefault(), surface.press("load"));
    else if (key === "delete") surface.press("delete");
    else if (key === "backspace" && sheetPicks.count > 0) surface.press("delete");
    else if (key === "enter" && surface.mode) surface.confirm();
    else return;
    e.preventDefault();
    surface.disarmClear();
  }

  // Record (on the control surface, or R) arms record mode so a preset slot
  // can be tapped; pressed again, or followed by Enter, it records a cue like
  // the Record cue button.
  $effect(() => {
    const record = (e: Event) => {
      if (namingGroup || recordingCue) return;
      if (!(e as CustomEvent<{ cue?: boolean }>).detail?.cue) return surface.setMode("record");
      surface.setMode(null);
      if (Object.keys(show.programmer.values).length > 0 || show.programmer.effects.length > 0) recordingCue = true;
    };
    window.addEventListener(RECORD_EVENT, record);
    return () => window.removeEventListener(RECORD_EVENT, record);
  });

  // Leaving it on would keep the rest of the rig dark from other screens.
  onDestroy(() => {
    surface.setMode(null);
    sheetPicks.clear();
    if (show.live.highlight) setHighlight(false);
  });

  const sameColour = (a: Rgb | undefined, b: Rgb) =>
    a != null && Math.abs(a.r - b.r) < 0.01 && Math.abs(a.g - b.g) < 0.01 && Math.abs(a.b - b.b) < 0.01;

  const families: Record<string, Family> = { intensity: "intensity", colour: "colour", position: "position", beam: "beam" };
</script>

<svelte:window onkeydown={keys} />

<div class="programmer">
  <header class="row">
    <h1>Programmer</h1>
    <span class="muted">
      {selection.length === 0 ? "Nothing selected" : `${selection.length} selected`}
    </span>
    {#if surface.mode === "record"}
      <span class="mode">Record: tap a preset slot, or Enter to record a cue. Esc cancels.</span>
    {:else if surface.mode === "delete"}
      <span class="mode">Delete: tap a preset, then Enter. Esc cancels.</span>
    {:else if sheetPicks.count > 0}
      <span class="mode">{sheetPicks.count} picked on the sheet: Delete takes them out. Esc unpicks.</span>
    {/if}
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
        disabled={Object.keys(show.programmer.values).length === 0 && show.programmer.effects.length === 0}
        onclick={() => (recordingCue = true)}
      >
        ● Record cue
      </button>
      <button
        title="Merge the programmer into the cue running now (U)"
        disabled={Object.keys(show.programmer.values).length === 0 && show.programmer.effects.length === 0}
        onclick={() => updateCue.open()}
      >
        Update
      </button>
      <button
        class="load"
        class:armed={surface.loadLoadsAll && show.programmer.selection.length > 0}
        title="Load what's on stage from running cues into the programmer: the selected fixtures, and Load again for everything (L)"
        onclick={() => surface.press("load")}
      >
        {surface.loadLoadsAll ? "Load all" : "Load"}
      </button>
      <button
        class="danger"
        class:armed={surface.clearClears}
        title="Clear deselects, and Clear again clears the programmer (Esc)"
        onclick={() => surface.press("clear")}
      >
        {surface.clearClears ? "Clear all" : show.programmer.selection.length > 0 ? "Deselect" : "Clear"}
      </button>
    {/if}
  </header>

  {#if stageOpen}<StageWindow onclose={() => (stageOpen = false)} />{/if}
  <Workspace bind:this={workspace} {windows} {layout} grid={GRID} {editing} onchange={setLayout} {body} {tools} />
</div>

{#snippet tools(id: string)}
  {#if id === "fixtures"}
    <button class="small" onclick={selectAll}>All</button>
    <button class="small" onclick={selectNone}>None</button>
  {:else if id === "sheet"}
    {#if sheetPicks.count > 0}
      <button class="small" onclick={() => sheetPicks.clear()}>Unpick</button>
      <button class="small danger" title="Take the picked values out of the programmer (Delete)" onclick={() => surface.press("delete")}>
        Delete {sheetPicks.count}
      </button>
    {/if}
  {:else if id === "groups" && show.programmer.groups.length > 0}
    <button class="small" class:warn={editingGroups} onclick={() => (editingGroups = !editingGroups)}>
      {editingGroups ? "Done" : "Edit"}
    </button>
  {:else if families[id]}
    <!-- The view toggle goes first so FX and Clear stay put when it hides. -->
    {#if id === "colour" && !effectsShown.colour && hasColour}
      <button class="small" onclick={() => setColourFaders(!colourFaders)}>
        {colourFaders ? "Picker" : "Faders"}
      </button>
    {/if}
    {#if EFFECT_WINDOWS[id]}
      {@const running = runningOfFamily(id)}
      <button
        class="small"
        class:on={effectsShown[id]}
        aria-pressed={effectsShown[id] ?? false}
        title="Effects on {id}"
        onclick={() => (effectsShown[id] = !effectsShown[id])}
      >
        FX{running > 0 ? ` ${running}` : ""}
      </button>
    {/if}
    <button
      class="small"
      title={show.programmer.selection.length > 0 ? `Clear ${id} from the selected fixtures` : `Clear ${id} from every fixture`}
      onclick={() =>
        program(() =>
          api.clearProgrammer(families[id], show.programmer.selection.length > 0 ? [...show.programmer.selection] : null),
        )}>Clear</button
    >
  {/if}
{/snippet}

{#snippet noSelection()}
  <p class="muted">Select fixtures or a group first.</p>
{/snippet}

{#snippet body(id: string)}
  {#if id === "fixtures"}
    <div class="tiles">
      {#each fixtureTypes as type, i (type[0].id)}
        {#if i > 0}<span class="type-break" aria-hidden="true"></span>{/if}
        {#each type as f (f.id)}
          <button class="tile" title={f.name} aria-pressed={selected.has(f.id)} onclick={() => tapFixture(f.id)}>
            <span class="num" style:color={fixtureColour(f.id)}>{f.id}</span>
            <span class="name">{f.name}</span>
            {#if show.programmer.values[f.id]}<span class="dot" aria-label="Has programmer values"></span>{/if}
          </button>
        {/each}
      {:else}
        <p class="muted">Patch some fixtures first.</p>
      {/each}
    </div>
  {:else if id === "groups"}
    {#if editingGroups}<p class="muted hint">Tap a group to delete it.</p>{/if}
    <div class="tiles">
      {#each show.programmer.groups as g (g.id)}
        {@const picked = g.fixtures.filter((f) => selected.has(f)).length}
        <button
          class="group"
          class:deleting={editingGroups}
          aria-pressed={picked === 0 ? false : picked === g.fixtures.length ? true : "mixed"}
          onclick={() => tapGroup(g.id)}
        >
          <span>{g.name}</span>
          <span class="muted">
            {picked > 0 && picked < g.fixtures.length
              ? `${picked} of ${g.fixtures.length} selected`
              : `${g.fixtures.length} fixtures`}
          </span>
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
  {:else if EFFECT_WINDOWS[id] && effectsShown[id]}
    <EffectList family={EFFECT_WINDOWS[id]} />
  {:else if selection.length === 0}
    {@render noSelection()}
  {:else if id === "intensity" && !hasIntensity}
    <p class="muted">The selected fixtures have no intensity control.</p>
  {:else if id === "colour" && !hasColour}
    <p class="muted">The selected fixtures have no colour controls.</p>
  {:else if id === "position" && !hasPosition}
    <p class="muted">The selected fixtures have no position controls.</p>
  {:else if id === "intensity"}
    <div class="intensity">
      <Fader
        wide
        max={100}
        colour="var(--f-intensity)"
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
  {:else if id === "colour" && colourFaders}
    <div class="faders">
      {#each colourFaderList as { attribute, channel } (attribute)}
        {@const set = current.other?.[attribute]}
        {@const derived = attribute === "color_temperature" ? undefined : emitterFromColour(attribute)}
        <!-- Emitters not set directly show what the picked colour gives them. -->
        {@const value = Math.round(set != null ? set * 255 : derived != null ? derived * 255 : (channel.default ?? 0))}
        <Fader
          {value}
          label={attributeName(attribute)}
          sublabel={set != null ? undefined : derived != null ? "From colour" : "Not set"}
          detail={capabilityAt(channel.capabilities, value)}
          colour={attributeColour(attribute)}
          active={set != null}
          onchange={(v) => program(() => api.setAttribute(attribute, v / 255))}
        />
      {:else}
        <p class="muted">The selected fixtures have no colour channels.</p>
      {/each}
    </div>
  {:else if id === "colour"}
    <div class="colour" class:with-balance={canMix && canBalance}>
      {#if canMix || !canBalance}
        <ColourPicker colour={current.colour} onchange={(c) => program(() => api.setColour(c))} />
        <div class="swatches">
          {#each SWATCHES as s (s.name)}
            <button
              class="swatch"
              aria-pressed={sameColour(current.colour, s.colour)}
              onclick={() => program(() => api.setColour(s.colour))}
            >
              <span class="chip" style:background={rgbCss(s.colour)}></span>
              <span>{s.name}</span>
            </button>
          {/each}
        </div>
      {/if}
      {#if canBalance}
        <WhiteBalance
          value={current.other?.color_temperature}
          onchange={(v) => program(() => api.setAttribute("color_temperature", v))}
        />
      {/if}
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
              colour="var(--f-position)"
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
          path={positionPath}
          onchange={(pan, tilt) => program(() => api.setPosition(pan, tilt))}
        />
      {/if}
      <div class="pos-buttons">
        <button aria-pressed={fine} class:on={fine} onclick={() => (fine = !fine)}>Fine</button>
        <button onclick={() => program(() => api.setPosition(0.5, 0.5))}>Centre</button>
        <button onclick={() => program(() => api.setPosition(0.5, null))}>Centre pan</button>
        <button onclick={() => program(() => api.setPosition(null, 0.5))}>Centre tilt</button>
        <button onclick={() => (stageOpen = true)}>Stage</button>
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
          colour="var(--f-beam)"
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
  /* The next Load loads every fixture. */
  button.load.armed {
    background: var(--accent);
    color: var(--accent-text);
    font-weight: 600;
  }

  /* The next Clear clears the programmer. */
  button.danger.armed {
    background: var(--danger);
    color: var(--bg);
    font-weight: 600;
  }

  .programmer {
    height: 100%;
    display: grid;
    grid-template-rows: auto 1fr;
    gap: var(--gap);
    min-height: 0;
  }

  .mode {
    padding: 6px 12px;
    border-radius: var(--radius);
    background: var(--danger);
    color: var(--bg);
    font-weight: 600;
    font-size: 0.9rem;
  }

  .hint {
    margin: 0 0 10px;
    font-size: 0.85rem;
  }

  .tiles {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(132px, 1fr));
    gap: 6px;
  }

  /* Ends the row so the next fixture type starts on a new one. */
  .type-break {
    grid-column: 1 / -1;
  }

  .tile {
    display: grid;
    grid-template-columns: 2ch 1fr auto;
    align-items: center;
    gap: 6px;
    padding: 0 10px;
    min-height: 90px;
    text-align: left;
  }

  .num {
    font-family: var(--mono);
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }

  /* Selected tiles are white, so the fixture's own colour would wash out. */
  .tile[aria-pressed="true"] .num {
    color: var(--accent-text) !important;
  }

  /* Long names wrap onto a second line, then cut off with an ellipsis. */
  .name {
    display: -webkit-box;
    -webkit-box-orient: vertical;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    overflow: hidden;
    overflow-wrap: anywhere;
  }

  .dot {
    width: 8px;
    height: 8px;
    background: var(--f-intensity);
  }

  .tile[aria-pressed="true"] .dot {
    background: var(--accent-text);
  }

  .group {
    display: grid;
    text-align: left;
    padding: 6px 10px;
    min-height: 48px;
    align-content: center;
  }

  /* Selected like a fixture tile when all its fixtures are selected, and
     with a dashed outline when only some are. */
  .group[aria-pressed="true"] .muted {
    color: #4a5260;
  }

  .group[aria-pressed="mixed"] {
    border-color: var(--accent);
    border-style: dashed;
  }

  .group span:last-child {
    font-size: 0.8rem;
  }

  .group.add {
    border-color: var(--line);
    border-style: dashed;
    background: transparent;
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
    background: var(--accent);
    color: var(--accent-text);
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
    min-height: 36px;
    max-height: 64px;
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
    width: min(100cqw, 100cqh - 150px);
    flex-shrink: 0;
  }

  .colour.with-balance :global(.disc) {
    width: min(100cqw, 100cqh - 344px);
  }

  .position :global(.pad) {
    width: min(100cqw, 100cqh - 150px);
    flex-shrink: 0;
  }

  /* A chip of the colour above its name, like a gel swatch. */
  .swatches {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(84px, 1fr));
    gap: 6px;
    width: 100%;
  }

  .swatch {
    display: grid;
    gap: 4px;
    min-height: 0;
    padding: 6px;
    text-align: left;
    font-size: 0.8rem;
    letter-spacing: 0.03em;
    text-transform: uppercase;
    overflow: hidden;
  }

  .swatch span:last-child {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .chip {
    height: 22px;
    border-radius: 2px;
    box-shadow: inset 0 0 0 1px rgb(0 0 0 / 0.4);
  }

  .swatch[aria-pressed="true"] {
    background: color-mix(in srgb, var(--f-colour) 22%, var(--panel-raised));
    border-color: var(--f-colour);
    color: #ff9fd6;
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
    height: calc(100cqh - 156px);
    min-height: 0;
  }

  .pos-buttons {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 6px;
    width: 100%;
  }

  .pos-buttons button {
    min-height: 40px;
    padding: 0 6px;
  }

  .beam,
  .faders {
    display: flex;
    gap: 12px;
    height: 100cqh;
  }

  .beam :global(.fader),
  .faders :global(.fader) {
    flex-shrink: 0;
  }

  /* Narrower faders so RGBW and white balance fit the colour window. */
  .faders {
    gap: 8px;
    overflow-x: auto;
  }

  .faders :global(.fader) {
    width: 64px;
  }

  .faders :global(.track) {
    width: 50px;
  }
</style>
