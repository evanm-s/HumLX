<script lang="ts">
  // The show as it runs: a grid of look buttons, the selected cue list's cues,
  // and fader slots that the lists needed for the show are loaded into. Lists are built and kept on
  // the Cue Lists screen. Effects are made in the Programmer and recorded into
  // cues; the Effect size fader scales them all.

  import CueTable from "../components/CueTable.svelte";
  import Fader from "../components/Fader.svelte";
  import Dialog from "../components/Dialog.svelte";
  import ListPicker from "../components/ListPicker.svelte";
  import NumberField from "../components/NumberField.svelte";
  import PlaybackGrid from "../components/PlaybackGrid.svelte";
  import RecordCue from "../components/RecordCue.svelte";
  import { api } from "../lib/api";
  import { goKeys, master, selectedList, selectList, showScreen, status } from "../lib/cuelists.svelte";
  import { show } from "../lib/show.svelte";
  import { RECORD_EVENT } from "../lib/surface.svelte";

  const play = show.play.bind(show);

  const list = $derived(selectedList());
  const live = $derived(list ? status(list.id) : undefined);

  let cueIndex = $state<number | null>(null);
  let pickedFor = $state<number | null>(null);
  $effect(() => {
    if (list?.id !== pickedFor) {
      pickedFor = list?.id ?? null;
      cueIndex = null;
    }
    if (list && cueIndex != null && cueIndex >= list.cues.length) cueIndex = null;
  });

  // Fixtures with programmer values or effects, which Record would take.
  const programmed = $derived(
    new Set([...Object.keys(show.programmer.values).map(Number), ...show.programmer.effects.flatMap((e) => e.fixtures)]).size,
  );

  let dialog = $state<null | "record" | { slot: number } | { look: [number, number] }>(null);
  /** Record is armed: tap an empty grid button for a new look, or a look to add a cue. */
  let recording = $state(false);
  $effect(() => {
    if (programmed === 0) recording = false;
  });
  let lookName = $state("");
  let lookFade = $state<number | null>(2);
  // Where the record dialog records into, when not the selected list.
  let recordList = $state<number | null>(null);

  function openRecord(into: number | null) {
    recording = false;
    recordList = into;
    dialog = "record";
  }

  // Record on the control surface arms recording onto the grid, as the Record
  // button does; Record twice records into the selected list.
  $effect(() => {
    const record = (e: Event) => {
      if (dialog || programmed === 0) return;
      if ((e as CustomEvent<{ cue?: boolean }>).detail?.cue || recording) openRecord(list?.id ?? null);
      else recording = true;
    };
    window.addEventListener(RECORD_EVENT, record);
    return () => window.removeEventListener(RECORD_EVENT, record);
  });

  function keys(e: KeyboardEvent) {
    if (e.key === "Escape" && recording && !dialog) {
      e.preventDefault();
      recording = false;
      return;
    }
    goKeys(e);
  }

  function recordLook(page: number, button: number) {
    recording = false;
    lookName = "";
    dialog = { look: [page, button] };
  }

  async function saveLook(page: number, button: number) {
    const ok = await play(() => api.recordCue({ button: { page, button, name: lookName } }, lookName, Math.max(0, lookFade ?? 0), null));
    if (ok) dialog = null;
  }

  async function load(slot: number, id: number) {
    if (await play(() => api.setSlot(slot, id))) {
      dialog = null;
      selectList(id);
    }
  }
</script>

<svelte:window onkeydown={keys} />

<div class="playback">
  <header class="row">
    <h1>Playback</h1>
    {#if programmed > 0}
      <span class="programmer-note">Programmer has values or effects for {programmed} fixtures, shown on top of playback</span>
      <button onclick={() => show.program(() => api.clearProgrammer(null))}>Clear programmer</button>
    {/if}
    <span class="spacer"></span>
    {#if recording}
      <span class="record-hint">Tap an empty button to save a new look, or a look to add a cue to it.</span>
      {#if list}<button onclick={() => openRecord(list.id)}>Record into {list.name}…</button>{/if}
      <button onclick={() => (recording = false)}>Cancel</button>
    {:else}
      <button class="record" disabled={programmed === 0} onclick={() => (recording = true)}>● Record</button>
    {/if}
    <button class="danger" onclick={() => play(() => api.releaseAll())}>Release all</button>
  </header>

  <PlaybackGrid {recording} onrecordlook={recordLook} onrecordinto={openRecord} />

  <!-- The selected cue list -->
  <section class="panel cues">
    {#if list}
      <div class="row">
        <span class="list-num">{list.id}</span>
        <h2>{list.name}</h2>
        <span class="muted">
          {live ? `On cue ${live.current + 1} of ${list.cues.length}` : `${list.cues.length} ${list.cues.length === 1 ? "cue" : "cues"} · not running`}
        </span>
        <span class="spacer"></span>
        <button class="ghost" onclick={() => showScreen("cuelists")}>Edit in Cue Lists ›</button>
      </div>
      <CueTable {list} compact picked={cueIndex} onpick={(i) => (cueIndex = i)}>
        {#snippet empty()}
          <p class="muted empty">No cues yet. Set a look in the Programmer, then tap <strong>Record cue</strong>.</p>
        {/snippet}
      </CueTable>
      <div class="row actions">
        <button class="go" disabled={list.cues.length === 0} onclick={() => play(() => api.go(list.id))}>GO</button>
        <button disabled={!live} onclick={() => play(() => api.back(list.id))}>Back</button>
        <button disabled={!live} onclick={() => play(() => api.releaseCueList(list.id))}>Release</button>
        <button disabled={cueIndex == null} onclick={() => play(() => api.goTo(list.id, cueIndex!))}>Go to cue</button>
        <span class="spacer"></span>
        <span class="muted keys">Space: GO · Shift+Space: Back</span>
      </div>
    {:else}
      <div class="empty muted">
        <p>
          Tap a fader slot's name to select its cue list, or pick one on
          <button class="ghost" onclick={() => showScreen("cuelists")}>Cue Lists</button>.
        </p>
      </div>
    {/if}
  </section>

  <!-- Playback faders -->
  <section class="panel strips scroll-x">
    <div class="strip effect-size" class:running={show.playback.effectSize < 1}>
      <div class="strip-name">Effect size</div>
      <div class="strip-cue muted">All effects</div>
      <div class="strip-body">
        <Fader
          max={100}
          value={Math.round(show.playback.effectSize * 100)}
          label="Size"
          colour="var(--f-playback)"
          active={show.playback.effectSize > 0}
          onchange={(v) => play(() => api.setEffectSize(v / 100))}
        />
      </div>
    </div>
    {#each [0, 1] as m (m)}
      {@const bpm = show.live.speedBpm[m]}
      {@const beat = (show.live.beats[m] % 1) < 0.2}
      <div class="strip speed">
        <div class="strip-name">Speed {m === 0 ? "A" : "B"} <span class="beat" class:on={beat}></span></div>
        <div class="strip-cue muted">Effects that follow it</div>
        <div class="speed-body">
          <div class="bpm"><strong>{Math.round(bpm)}</strong> BPM</div>
          <button class="tap" onclick={() => play(() => api.tapSpeed(m))}>Tap</button>
          <div class="row">
            <button aria-label="Slower by 1 BPM" onclick={() => play(() => api.setSpeedBpm(m, Math.round(bpm) - 1))}>−</button>
            <button aria-label="Faster by 1 BPM" onclick={() => play(() => api.setSpeedBpm(m, Math.round(bpm) + 1))}>+</button>
          </div>
          <div class="row">
            <button onclick={() => play(() => api.setSpeedBpm(m, bpm / 2))}>½</button>
            <button onclick={() => play(() => api.setSpeedBpm(m, bpm * 2))}>×2</button>
          </div>
        </div>
      </div>
    {/each}
    {#each show.playback.layout.slots as id, slot (slot)}
      {@const l = show.playback.cueLists.find((x) => x.id === id)}
      {#if l}
        {@const running = status(l.id)}
        {@const on = running != null && !running.releasing}
        <div
          class="strip slot"
          class:live={on}
          class:fading={on && running?.fading}
          class:releasing={running?.releasing}
          class:selected={l.id === list?.id}
        >
          <div class="slot-head">
            <button class="strip-name ghost" title="Select this cue list" onclick={() => selectList(l.id)}>
              <span class="slot-num">{slot + 1}</span>{l.name}
            </button>
            <button class="ghost unload" aria-label={`Take ${l.name} out of slot ${slot + 1}`} onclick={() => play(() => api.setSlot(slot, null))}>✕</button>
          </div>
          <div class="strip-cue" class:muted={!running}>
            {#if running}
              <span class="live-tag">{running.releasing ? "Releasing" : "Live"}</span>
              {running.current + 1} · {l.cues[running.current]?.name ?? ""}
            {:else}
              Not running
            {/if}
          </div>
          <div class="strip-body">
            <Fader
              max={100}
              value={Math.round(master(l.id) * 100)}
              label="Level"
              colour="var(--f-playback)"
              active={running != null}
              onchange={(v) => play(() => api.setMaster(l.id, v / 100))}
            />
            <div class="strip-buttons">
              <button class="go" disabled={l.cues.length === 0} onclick={() => (selectList(l.id), play(() => api.go(l.id)))}>GO</button>
              <button disabled={!running} onclick={() => play(() => api.back(l.id))}>Back</button>
              <button disabled={!running} onclick={() => play(() => api.releaseCueList(l.id))}>Release</button>
            </div>
          </div>
        </div>
      {:else}
        <button class="strip slot empty-slot" onclick={() => (dialog = { slot })}>
          <span class="slot-num">{slot + 1}</span>
          <span>+ Load list</span>
        </button>
      {/if}
    {/each}
  </section>
</div>

{#if dialog === "record"}
  {@const into = recordList ?? list?.id ?? null}
  <RecordCue
    list={into}
    after={into === list?.id ? cueIndex : null}
    onclose={() => (dialog = null)}
    onrecorded={(_, i) => (cueIndex = i)}
  />
{:else if dialog && "look" in dialog}
  {@const [page, button] = dialog.look}
  <Dialog title={`New look on button ${button + 1}`} onclose={() => (dialog = null)}>
    <label class="field">
      Name
      <input bind:value={lookName} placeholder="e.g. Blue wash" />
    </label>
    <NumberField label="Fade time (seconds)" fractional bind:value={lookFade} min={0} max={600} />
    <p class="muted">Saves the Programmer as a new one-cue list on this button. Tap the button to bring the look up, and again to release it.</p>
    {#snippet actions()}
      <button onclick={() => (dialog = null)}>Cancel</button>
      <button class="primary" onclick={() => saveLook(page, button)}>Save look</button>
    {/snippet}
  </Dialog>
{:else if dialog}
  {@const slot = dialog.slot}
  <ListPicker title={`Load a cue list into slot ${slot + 1}`} onpick={(id) => load(slot, id)} onclose={() => (dialog = null)} />
{/if}

<style>
  .playback {
    height: 100%;
    display: grid;
        grid-template-columns: minmax(0, 1fr) 640px;
    grid-template-rows: auto minmax(0, 1fr) 400px;
    gap: var(--gap);
  }

  header {
    grid-column: 1 / -1;
  }

  .record-hint {
    color: var(--danger, #e5484d);
    font-weight: 600;
  }

  .programmer-note {
    color: var(--warning);
    font-size: 0.9rem;
  }

  .panel {
    padding: 14px;
    min-height: 0;
  }

  .cues {
    display: grid;
    grid-template-rows: auto minmax(0, 1fr) auto;
    gap: 10px;
  }

  .list-num {
    font-family: var(--mono);
    font-size: 1.3rem;
    opacity: 0.6;
  }

  .strip.slot {
    width: auto;
    flex: 1 0 210px;
  }

  /* Live: the head turns solid playback orange (like a live grid button) and
     the whole strip gets an orange edge and tint. Selected stays a white
     outline drawn outside that edge, so the two never hide each other. */
  div.strip.slot {
    border-top: none;
    padding-top: 0;
    box-shadow: inset 0 0 0 2px transparent;
  }

  .strip.slot.live,
  .strip.slot.releasing {
    box-shadow: inset 0 0 0 2px var(--f-playback);
  }

  .strip.slot.live {
    background: color-mix(in srgb, var(--f-playback) 14%, var(--panel-raised));
  }

  .strip.slot.fading .slot-head {
    animation: fade 0.6s ease-in-out infinite alternate;
  }

  @keyframes fade {
    to {
      opacity: 0.55;
    }
  }

  .strip.slot.selected {
    outline: 3px solid var(--accent);
    outline-offset: 3px;
  }

  .slot-head {
    display: flex;
    align-items: center;
    gap: 4px;
    min-width: 0;
    margin: 0 -14px;
    padding: 6px 8px;
    border-radius: var(--radius) var(--radius) 0 0;
    background: var(--line);
  }

  .live .slot-head {
    background: var(--f-playback);
    color: var(--accent-text);
  }

  .live .slot-num {
    opacity: 0.75;
  }

  .releasing .slot-head {
    color: var(--f-playback);
  }

  .live-tag {
    display: inline-block;
    margin-right: 6px;
    padding: 1px 6px;
    border-radius: var(--radius);
    background: var(--f-playback);
    color: var(--accent-text);
    font-weight: 700;
    font-size: 0.75rem;
    letter-spacing: 0.08em;
    text-transform: uppercase;
  }

  .releasing .live-tag {
    background: transparent;
    color: var(--f-playback);
    box-shadow: inset 0 0 0 1px var(--f-playback);
  }

  .slot-head .strip-name {
    flex: 1;
    min-width: 0;
    text-align: left;
    padding: 4px 6px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .slot-num {
    font-family: var(--mono);
    opacity: 0.6;
    margin-right: 6px;
  }

  .unload {
    flex-shrink: 0;
    padding: 4px 8px;
  }

  .empty-slot {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 8px;
    text-align: center;
    border: 2px dashed var(--line);
    background: transparent;
    color: var(--muted);
  }

  .strip-buttons .go {
    font-size: 1.6rem;
  }

  .strip.effect-size {
    width: 140px;
  }

  .strip.speed {
    border-top-color: var(--f-playback);
    width: 170px;
  }

  .speed-body {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .speed-body .row {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 6px;
  }

  .bpm {
    text-align: center;
    font-family: var(--mono);
    font-variant-numeric: tabular-nums;
  }

  .bpm strong {
    font-size: 2rem;
  }

  .tap {
    border-color: var(--f-playback);
    flex: 1;
    min-height: 88px;
    font-size: 1.2rem;
    font-weight: 700;
  }

  .beat {
    display: inline-block;
    width: 10px;
    height: 10px;
    margin-left: 6px;
    border-radius: 50%;
    background: var(--line);
    vertical-align: middle;
  }

  .beat.on {
    background: var(--f-playback);
  }

  .empty {
    padding: 24px;
    text-align: center;
  }

  .actions {
    flex-wrap: wrap;
    gap: 10px;
  }

  .keys {
    font-size: 0.85rem;
  }

  .strips {
    grid-column: 1 / -1;
    display: flex;
    gap: var(--gap);
    overflow-x: auto;
  }

  .strip {
    display: grid;
    grid-template-rows: auto auto 1fr;
    gap: 6px;
    width: 300px;
    flex-shrink: 0;
    padding: 14px;
    border-radius: var(--radius);
    border-top: 4px solid var(--line);
    background: var(--panel-raised);
  }

  .strip.running {
    border-top-color: var(--f-playback);
  }

  .strip-name {
    font-weight: 700;
    font-size: 1.1rem;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }

  .strip-cue {
    font-size: 0.85rem;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .strip-body {
    display: flex;
    gap: 14px;
    min-height: 0;
  }

  .strip-buttons {
    flex: 1;
    display: grid;
    grid-template-rows: 2fr 1fr 1fr;
    gap: 10px;
  }

  .go {
    background: var(--f-playback);
    color: var(--accent-text);
    font-size: 2rem;
    font-weight: 700;
    letter-spacing: 0.08em;
  }

  @media (max-width: 2000px) {
    .playback {
      grid-template-rows: auto minmax(0, 1fr) 380px;
    }
  }
</style>
