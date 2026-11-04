<script lang="ts">
  // Every cue list in the show: find one in the library on the left, then
  // build and edit its cues on the right. The Playback screen only shows the
  // lists loaded into its fader slots and button grid.

  import CueTable from "../components/CueTable.svelte";
  import Dialog from "../components/Dialog.svelte";
  import NumberField from "../components/NumberField.svelte";
  import RecordCue from "../components/RecordCue.svelte";
  import { api } from "../lib/api";
  import { goKeys, placements, selectedList, selectList, showScreen, status } from "../lib/cuelists.svelte";
  import { show } from "../lib/show.svelte";
  import { RECORD_EVENT } from "../lib/surface.svelte";
  import { updateCue } from "../lib/updatecue.svelte";
  import type { CueMode } from "../lib/types";

  const play = show.play.bind(show);

  // Library: search, sort and filter.
  type Filter = "all" | "slots" | "grid" | "unplaced";
  let query = $state("");
  let sort = $state<"number" | "name">("number");
  let filter = $state<Filter>("all");

  const lists = $derived.by(() => {
    const q = query.trim().toLowerCase();
    const { slots, pages } = show.playback.layout;
    const onGrid = new Set(pages.flatMap((p) => p.buttons));
    const inSlot = new Set(slots);
    return show.playback.cueLists
      .filter((l) => !q || l.name.toLowerCase().includes(q) || String(l.id) === q)
      .filter((l) =>
        filter === "slots"
          ? inSlot.has(l.id)
          : filter === "grid"
            ? onGrid.has(l.id)
            : filter === "unplaced"
              ? !inSlot.has(l.id) && !onGrid.has(l.id)
              : true,
      )
      .sort((a, b) => (sort === "name" ? a.name.localeCompare(b.name) || a.id - b.id : a.id - b.id));
  });

  const list = $derived(selectedList());

  // The cue picked in the editor.
  let cueIndex = $state<number | null>(null);
  let pickedFor = $state<number | null>(null);
  $effect(() => {
    // A different list starts with no cue picked.
    if (list?.id !== pickedFor) {
      pickedFor = list?.id ?? null;
      cueIndex = null;
    }
    if (list && cueIndex != null && cueIndex >= list.cues.length) cueIndex = null;
  });

  const live = $derived(list ? status(list.id) : undefined);

  // While a cue is picked here, Update (the NX-K key or U) updates that cue.
  $effect(() => {
    updateCue.picked = list && cueIndex != null ? { list: list.id, index: cueIndex } : null;
    return () => (updateCue.picked = null);
  });
  const programmed = $derived(
    new Set([...Object.keys(show.programmer.values).map(Number), ...show.programmer.effects.flatMap((e) => e.fixtures)]).size,
  );

  // Dialogs
  let dialog = $state<null | "record" | "edit" | "new" | "rename" | "slot">(null);
  let draftName = $state("");
  let draftFade = $state<number | null>(2);
  let draftMode = $state<CueMode>("hold");
  let draftFollow = $state<number | null>(5);
  let confirmDelete = $state<"cue" | "list" | null>(null);

  function pick(index: number | null) {
    cueIndex = index;
    confirmDelete = null;
  }

  function openEdit() {
    if (!list || cueIndex == null) return;
    const cue = list.cues[cueIndex];
    draftName = cue.name;
    draftFade = cue.fade;
    draftMode = cue.mode;
    // A new follow cue starts with its fade time, so it moves on once the fade is done.
    draftFollow = cue.follow > 0 ? cue.follow : cue.fade > 0 ? cue.fade : 5;
    dialog = "edit";
  }

  async function saveCue() {
    if (!list || cueIndex == null) return;
    const fade = Math.max(0, draftFade ?? 0);
    const follow = draftMode === "follow" ? Math.max(0, draftFollow ?? 0) : null;
    const ok = await play(() => api.editCue(list.id, cueIndex!, draftName, fade, draftMode, follow));
    if (ok) dialog = null;
  }

  async function deleteCue() {
    if (!list || cueIndex == null) return;
    if (confirmDelete !== "cue") return void (confirmDelete = "cue");
    await play(() => api.deleteCue(list.id, cueIndex!));
    confirmDelete = null;
  }

  async function deleteList() {
    if (!list) return;
    if (confirmDelete !== "list") return void (confirmDelete = "list");
    await play(() => api.deleteCueList(list.id));
    confirmDelete = null;
  }

  async function newList() {
    if (await play(() => api.newCueList(draftName))) dialog = null;
  }

  async function rename() {
    if (!list) return;
    if (await play(() => api.renameCueList(list.id, draftName))) dialog = null;
  }

  async function loadInto(slot: number) {
    if (!list) return;
    if (await play(() => api.setSlot(slot, list.id))) dialog = null;
  }

  // Record on the control surface opens the same dialog as the Record button.
  $effect(() => {
    const record = () => {
      if (!dialog && programmed > 0) dialog = "record";
    };
    window.addEventListener(RECORD_EVENT, record);
    return () => window.removeEventListener(RECORD_EVENT, record);
  });
</script>

<svelte:window onkeydown={goKeys} />

<div class="cuelists">
  <!-- Library -->
  <section class="panel library">
    <div class="row">
      <h1>Cue Lists</h1>
      <span class="spacer"></span>
      <button
        onclick={() => {
          draftName = "";
          dialog = "new";
        }}>+ New</button
      >
    </div>
    <input type="search" bind:value={query} placeholder="Search by name or number" />
    <div class="row filters">
      <div class="segmented">
        {#each [["all", "All"], ["slots", "In slots"], ["grid", "On grid"], ["unplaced", "Unplaced"]] as [id, label] (id)}
          <button aria-pressed={filter === id} onclick={() => (filter = id as Filter)}>{label}</button>
        {/each}
      </div>
      <span class="spacer"></span>
      <div class="segmented">
        <button aria-pressed={sort === "number"} onclick={() => (sort = "number")}>No.</button>
        <button aria-pressed={sort === "name"} onclick={() => (sort = "name")}>A–Z</button>
      </div>
    </div>
    <div class="lists scroll">
      {#each lists as l (l.id)}
        {@const where = placements(l.id)}
        <button class="item" class:selected={l.id === list?.id} class:running={status(l.id) != null} onclick={() => selectList(l.id)}>
          <span class="num">{l.id}</span>
          <span class="name">{l.name}</span>
          <span class="count muted">{l.cues.length}</span>
          <span class="where">
            {#each where as w (w)}<span class="chip">{w}</span>{/each}
          </span>
        </button>
      {:else}
        <p class="muted empty">
          {show.playback.cueLists.length
            ? "No cue lists match."
            : "No cue lists yet. Tap + New, or Record a look."}
        </p>
      {/each}
    </div>
    <div class="muted foot">{show.playback.cueLists.length} cue lists</div>
  </section>

  <!-- Editor for the selected list -->
  <section class="panel editor">
    {#if list}
      <header class="row">
        <span class="list-num">{list.id}</span>
        <h2>{list.name}</h2>
        <span class="muted">{list.cues.length} {list.cues.length === 1 ? "cue" : "cues"}</span>
        {#each placements(list.id) as w (w)}<span class="chip">{w}</span>{/each}
        <span class="spacer"></span>
        <button
          onclick={() => {
            draftName = list.name;
            dialog = "rename";
          }}>Rename</button
        >
        <button onclick={() => play(() => api.duplicateCueList(list.id))}>Duplicate</button>
        <button onclick={() => (dialog = "slot")}>Load into slot…</button>
        <button class="danger" onclick={deleteList}>{confirmDelete === "list" ? "Tap again to delete list" : "Delete list"}</button>
      </header>

      <div class="row options">
        <button
          aria-pressed={list.releaseOnNextGo}
          onclick={() => play(() => api.setCueListOptions(list.id, !list.releaseOnNextGo, list.solo))}
          >Release on next go</button
        >
        <button aria-pressed={list.solo} onclick={() => play(() => api.setCueListOptions(list.id, list.releaseOnNextGo, !list.solo))}
          >Solo</button
        >
        <span class="muted">
          {#if list.solo}
            GO fades out all other lists.
          {:else if list.releaseOnNextGo}
            GO fades out other lists set to release on next go.
          {:else}
            Layers on top; other lists' GO leaves it running.
          {/if}
        </span>
      </div>

      <CueTable {list} picked={cueIndex} onpick={pick}>
        {#snippet empty()}
          <p class="muted empty">No cues yet. Set a look, then tap <strong>Record cue</strong>.</p>
        {/snippet}
      </CueTable>

      <div class="row actions">
        <button class="record" disabled={programmed === 0} onclick={() => (dialog = "record")}>
          ● Record cue{cueIndex != null && cueIndex < list.cues.length - 1 ? ` after ${cueIndex + 1}` : ""}
        </button>
        <button
          title="Merge programmer into the picked or running cue (U)"
          disabled={(cueIndex == null && !live) || programmed === 0}
          onclick={() => updateCue.open({ list: list.id, index: cueIndex ?? live!.current })}
        >
          Update cue{cueIndex == null && live ? ` ${live.current + 1}` : ""}
        </button>
        <button disabled={cueIndex == null} onclick={openEdit}>Edit cue</button>
        <button class="danger" disabled={cueIndex == null} onclick={deleteCue}>
          {confirmDelete === "cue" ? "Tap again to delete" : "Delete cue"}
        </button>
        <span class="spacer"></span>
        <button disabled={cueIndex == null} onclick={() => play(() => api.goTo(list.id, cueIndex!))}>Go to cue</button>
        <button class="go" disabled={list.cues.length === 0} onclick={() => play(() => api.go(list.id))}>GO</button>
        <button disabled={!live} onclick={() => play(() => api.back(list.id))}>Back</button>
        <button disabled={!live} onclick={() => play(() => api.releaseCueList(list.id))}>Release</button>
      </div>
      <div class="row hint muted">
        <span>Space: GO · Shift+Space: Back</span>
        <span class="spacer"></span>
        <button class="ghost" onclick={() => showScreen("playback")}>Go to Playback ›</button>
      </div>
    {:else}
      <div class="empty muted">
        <p>Pick a cue list on the left.</p>
      </div>
    {/if}
  </section>
</div>

{#if dialog === "record"}
  <RecordCue list={list?.id ?? null} after={cueIndex} onclose={() => (dialog = null)} onrecorded={(_, i) => (cueIndex = i)} />
{:else if dialog === "edit"}
  <Dialog title="Edit cue" onclose={() => (dialog = null)}>
    <label class="field">
      Name
      <input bind:value={draftName} placeholder="e.g. Walk in" />
    </label>
    <NumberField label="Fade time (seconds)" fractional bind:value={draftFade} min={0} max={600} />
    <div class="field">
      Type
      <div class="segmented">
        <button aria-pressed={draftMode === "hold"} onclick={() => (draftMode = "hold")}>Hold</button>
        <button aria-pressed={draftMode === "follow"} onclick={() => (draftMode = "follow")}>Follow</button>
      </div>
    </div>
    {#if draftMode === "follow"}
      <NumberField label="Follow after (seconds)" fractional bind:value={draftFollow} min={0} max={3600} />
      <p class="muted">
        Counted from GO, including the fade. On the last cue, loops to the first.
      </p>
    {:else}
      <p class="muted">Waits for the next GO.</p>
    {/if}
    {#snippet actions()}
      <button onclick={() => (dialog = null)}>Cancel</button>
      <button class="primary" onclick={saveCue}>Save</button>
    {/snippet}
  </Dialog>
{:else if dialog === "new" || dialog === "rename"}
  <Dialog title={dialog === "new" ? "New cue list" : "Rename cue list"} onclose={() => (dialog = null)}>
    <label class="field">
      Name
      <input bind:value={draftName} placeholder="e.g. Podcast" />
    </label>
    {#snippet actions()}
      <button onclick={() => (dialog = null)}>Cancel</button>
      <button class="primary" onclick={dialog === "new" ? newList : rename}>{dialog === "new" ? "Create" : "Save"}</button>
    {/snippet}
  </Dialog>
{:else if dialog === "slot" && list}
  <Dialog title={`Load ${list.name} into a fader slot`} onclose={() => (dialog = null)}>
    <div class="slots">
      {#each show.playback.layout.slots as s, i (i)}
        {@const current = show.playback.cueLists.find((l) => l.id === s)}
        <button aria-pressed={s === list.id} onclick={() => loadInto(i)}>
          <strong>Slot {i + 1}</strong>
          <span class="muted">{current ? current.name : "Empty"}</span>
        </button>
      {/each}
    </div>
    <p class="muted">Replaces the slot's current list (it stays in Cue Lists).</p>
  </Dialog>
{/if}

<style>
  .cuelists {
    height: 100%;
    display: grid;
    grid-template-columns: minmax(420px, 32%) 1fr;
    gap: var(--gap);
  }

  .panel {
    padding: 14px;
    min-height: 0;
  }

  .library {
    display: grid;
    grid-template-rows: auto auto auto minmax(0, 1fr) auto;
    gap: 10px;
  }

  .filters {
    flex-wrap: wrap;
    gap: 8px;
  }

  .lists {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .item {
    display: grid;
    grid-template-columns: 48px 1fr auto;
    grid-template-areas: "num name count" "num where where";
    column-gap: 10px;
    row-gap: 2px;
    align-items: center;
    text-align: left;
    flex-shrink: 0;
    min-height: 52px;
    border-color: transparent;
    background: transparent;
    border-left: 4px solid transparent;
  }

  .item.running {
    border-left-color: var(--f-playback);
  }

  .item.selected {
    border-color: var(--accent);
    border-left-width: 4px;
  }

  .item .num {
    grid-area: num;
    font-family: var(--mono);
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }

  .item .name {
    grid-area: name;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .item .count {
    grid-area: count;
    font-family: var(--mono);
  }

  .item .where {
    grid-area: where;
    display: flex;
    gap: 4px;
    flex-wrap: wrap;
  }

  .item .where:empty {
    display: none;
  }

  .chip {
    font-size: 0.75rem;
    padding: 1px 6px;
    border-radius: 4px;
    border: 1px solid var(--f-playback);
    color: var(--f-playback);
    white-space: nowrap;
  }

  .foot {
    font-size: 0.85rem;
  }

  .editor {
    display: grid;
    grid-template-rows: auto auto minmax(0, 1fr) auto auto;
    gap: 10px;
  }

  .editor header {
    flex-wrap: wrap;
    gap: 10px;
  }

  .options {
    gap: 10px;
    flex-wrap: wrap;
  }

  .options button[aria-pressed="true"] {
    background: var(--f-playback);
    border-color: var(--f-playback);
    color: var(--accent-text);
  }

  .list-num {
    font-family: var(--mono);
    font-size: 1.4rem;
    opacity: 0.6;
  }

  .actions {
    flex-wrap: wrap;
    gap: 10px;
  }

  .go {
    background: var(--f-playback);
    color: var(--accent-text);
    font-weight: 700;
    letter-spacing: 0.08em;
    min-width: 110px;
  }

  .hint {
    font-size: 0.85rem;
  }

  .empty {
    padding: 24px;
    text-align: center;
  }

  .slots {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 8px;
    min-width: 640px;
    margin-bottom: 10px;
  }

  .slots button {
    display: grid;
    gap: 4px;
    min-height: 72px;
  }
</style>
