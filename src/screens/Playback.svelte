<script lang="ts">
  // Cue lists: record the programmer as cues, run them with GO, and mix them
  // on playback faders. Effects are made in the Programmer's Intensity, Colour
  // and Position windows.

  import Dialog from "../components/Dialog.svelte";
  import Fader from "../components/Fader.svelte";
  import NumberField from "../components/NumberField.svelte";
  import RecordCue from "../components/RecordCue.svelte";
  import { api } from "../lib/api";
  import { show } from "../lib/show.svelte";
  import { RECORD_EVENT, surface } from "../lib/surface.svelte";

  const play = show.play.bind(show);

  let listId = $state<number | null>(null);
  $effect(() => {
    if (!show.playback.cueLists.some((l) => l.id === listId)) listId = show.playback.cueLists[0]?.id ?? null;
  });
  const list = $derived(show.playback.cueLists.find((l) => l.id === listId));
  // Next and Last on the control surface run the list chosen here.
  $effect(() => {
    surface.cueList = listId;
  });

  let cueIndex = $state<number | null>(null);
  $effect(() => {
    if (list && cueIndex != null && cueIndex >= list.cues.length) cueIndex = null;
  });

  const status = (id: number) => show.live.playbacks.find((p) => p.list === id);
  // Masters come from the live poll too, since OSC can move them.
  const master = (id: number) =>
    (show.live.masters.find(([l]) => l === id) ?? show.playback.masters.find(([l]) => l === id))?.[1] ?? 1;
  const programmed = $derived(Object.keys(show.programmer.values).length);

  // Dialogs
  let dialog = $state<null | "record" | "edit" | "list">(null);
  let draftName = $state("");
  let draftFade = $state<number | null>(2);
  let confirmDelete = $state(false);

  function openEdit() {
    if (!list || cueIndex == null) return;
    const cue = list.cues[cueIndex];
    draftName = cue.name;
    draftFade = cue.fade;
    dialog = "edit";
  }

  async function saveCue() {
    if (!list || cueIndex == null) return;
    const ok = await play(() => api.editCue(list.id, cueIndex!, draftName, Math.max(0, draftFade ?? 0)));
    if (ok) dialog = null;
  }

  // Space is GO and Shift+Space is Back for the cue list on screen, unless
  // typing or a dialog is open.
  function keys(e: KeyboardEvent) {
    if (e.key !== " " || dialog || !list || e.repeat) return;
    const target = e.target as HTMLElement;
    if (target.closest("input, textarea, select, [role=dialog]")) return;
    e.preventDefault();
    const id = list.id;
    play(() => (e.shiftKey ? api.back(id) : api.go(id)));
  }

  // Record on the control surface opens the same dialog as the Record button.
  $effect(() => {
    const record = () => {
      if (!dialog && programmed > 0) dialog = "record";
    };
    window.addEventListener(RECORD_EVENT, record);
    return () => window.removeEventListener(RECORD_EVENT, record);
  });

  function recorded(id: number, index: number) {
    listId = id;
    cueIndex = index;
  }

  async function newList() {
    const ok = await play(() => api.newCueList(draftName));
    if (ok) {
      listId = ok.cueLists[ok.cueLists.length - 1]?.id ?? null;
      dialog = null;
    }
  }

  async function deleteCue() {
    if (!list || cueIndex == null) return;
    if (!confirmDelete) {
      confirmDelete = true;
      return;
    }
    await play(() => api.deleteCue(list.id, cueIndex!));
    confirmDelete = false;
  }
</script>

<svelte:window onkeydown={keys} />

<div class="playback">
  <header class="row">
    <h1>Playback</h1>
    {#if programmed > 0}
      <span class="programmer-note">Programmer has values for {programmed} fixtures, shown on top of playback</span>
      <button onclick={() => show.program(() => api.clearProgrammer(null))}>Clear programmer</button>
    {/if}
    <span class="spacer"></span>
    <button class="record" disabled={programmed === 0} onclick={() => (dialog = "record")}>
      ● Record cue{list && cueIndex != null && cueIndex < list.cues.length - 1 ? ` after ${cueIndex + 1}` : ""}
    </button>
    <button class="danger" onclick={() => play(() => api.releaseAll())}>Release all</button>
  </header>

  <!-- Cue list editor -->
  <section class="panel cues">
    <div class="row">
      <div class="segmented">
        {#each show.playback.cueLists as l (l.id)}
          <button aria-pressed={l.id === listId} onclick={() => ((listId = l.id), (cueIndex = null))}
            ><span class="list-num">{l.id}</span> {l.name}</button
          >
        {/each}
        <button
          class="ghost"
          onclick={() => {
            draftName = "";
            dialog = "list";
          }}>+ New cue list</button
        >
      </div>
    </div>

    {#if list}
      <div class="table scroll">
        {#each list.cues as cue, i (i)}
          {@const live = status(list.id)}
          <button
            class="cue"
            class:selected={cueIndex === i}
            class:live={live?.current === i}
            onclick={() => ((cueIndex = cueIndex === i ? null : i), (confirmDelete = false))}
          >
            <span class="state">{live?.current === i ? (live.fading ? "◐" : "▶") : ""}</span>
            <span class="num">{i + 1}</span>
            <span class="name">{cue.name}</span>
            <span class="muted">{Object.keys(cue.values).length} fixtures</span>
            <span class="fade">{cue.fade}s</span>
          </button>
        {:else}
          <p class="muted empty">
            No cues yet. Set a look in the Programmer, then tap <strong>Record cue</strong>.
          </p>
        {/each}
      </div>
      <div class="row actions">
        <button disabled={cueIndex == null || programmed === 0} onclick={() => play(() => api.updateCue(list.id, cueIndex!))}>
          Update cue
        </button>
        <button disabled={cueIndex == null} onclick={() => play(() => api.goTo(list.id, cueIndex!))}>Go to cue</button>
        <button disabled={cueIndex == null} onclick={openEdit}>Name and fade</button>
        <button class="danger" disabled={cueIndex == null} onclick={deleteCue}>
          {confirmDelete ? "Tap again to delete" : "Delete cue"}
        </button>
        <span class="spacer"></span>
        <span class="muted keys">Space: GO · Shift+Space: Back</span>
      </div>
    {:else}
      <div class="empty muted">
        <p>No cue lists yet. Set a look in the Programmer, then tap <strong>Record cue</strong> to start one.</p>
      </div>
    {/if}
  </section>

  <!-- Playback faders -->
  <section class="panel strips scroll-x">
    {#each show.playback.cueLists as l (l.id)}
      {@const live = status(l.id)}
      <div class="strip" class:running={live != null}>
        <div class="strip-name">{l.name}</div>
        <div class="strip-cue muted">
          {live ? `${live.current + 1} · ${l.cues[live.current]?.name ?? ""}` : "Not running"}
        </div>
        <div class="strip-body">
          <Fader
            max={100}
            value={Math.round(master(l.id) * 100)}
            label="Level"
            active={live != null}
            onchange={(v) => play(() => api.setMaster(l.id, v / 100))}
          />
          <div class="strip-buttons">
            <button class="go" disabled={l.cues.length === 0} onclick={() => play(() => api.go(l.id))}>GO</button>
            <button disabled={!live} onclick={() => play(() => api.back(l.id))}>Back</button>
            <button disabled={!live} onclick={() => play(() => api.releaseCueList(l.id))}>Release</button>
          </div>
        </div>
      </div>
    {:else}
      <p class="muted">Each cue list gets a fader here.</p>
    {/each}
  </section>
</div>

{#if dialog === "record"}
  <RecordCue list={listId} after={cueIndex} onclose={() => (dialog = null)} onrecorded={recorded} />
{:else if dialog === "edit"}
  <Dialog title="Name and fade" onclose={() => (dialog = null)}>
    <label class="field">
      Name
      <input bind:value={draftName} placeholder="e.g. Walk in" />
    </label>
    <NumberField label="Fade time (seconds)" fractional bind:value={draftFade} min={0} max={600} />
    {#snippet actions()}
      <button onclick={() => (dialog = null)}>Cancel</button>
      <button class="primary" onclick={saveCue}>Save</button>
    {/snippet}
  </Dialog>
{:else if dialog === "list"}
  <Dialog title="New cue list" onclose={() => (dialog = null)}>
    <label class="field">
      Name
      <input bind:value={draftName} placeholder="e.g. Podcast" />
    </label>
    {#snippet actions()}
      <button onclick={() => (dialog = null)}>Cancel</button>
      <button class="primary" onclick={newList}>Create</button>
    {/snippet}
  </Dialog>
{/if}

<style>
  .playback {
    height: 100%;
    display: grid;
        grid-template-rows: auto minmax(0, 1fr) 400px;
    gap: var(--gap);
  }

  header {
    grid-column: 1 / -1;
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
    grid-template-rows: auto 1fr auto;
    gap: 10px;
  }

  .table {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .cue {
    display: grid;
    grid-template-columns: 32px 56px 1fr auto 90px;
    gap: 12px;
    align-items: center;
    text-align: left;
    min-height: 44px;
    flex-shrink: 0;
    border-color: transparent;
    background: transparent;
  }

  .cue.live {
    background: color-mix(in srgb, var(--ok) 12%, transparent);
  }

  .cue.selected {
    border-color: var(--accent);
    background: var(--panel-raised);
  }

  .state {
    color: var(--ok);
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

  .fade {
    text-align: right;
    font-variant-numeric: tabular-nums;
  }

  .empty {
    padding: 24px;
    display: grid;
    place-items: center;
  }

  .actions {
    flex-wrap: wrap;
    gap: 10px;
  }

  .keys {
    font-size: 0.85rem;
  }

  .list-num {
    opacity: 0.6;
    font-variant-numeric: tabular-nums;
    margin-right: 4px;
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
    border: 1px solid var(--line);
    background: var(--panel-raised);
  }

  .strip.running {
    border-color: var(--ok);
  }

  .strip-name {
    font-weight: 700;
    font-size: 1.1rem;
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
    background: var(--ok);
    border-color: var(--ok);
    color: #04140b;
    font-size: 1.6rem;
    font-weight: 800;
  }

  @media (max-width: 2000px) {
    .playback {
      grid-template-rows: auto minmax(0, 1fr) 380px;
    }
  }
</style>
