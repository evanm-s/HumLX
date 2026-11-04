<script lang="ts">
  // The Playback screen's button grid, like Onyx's Playbacks: each button runs
  // a cue list, usually a one-cue look. Tap to play and select it; tap a live
  // look again to release it. Pages hold more buttons. Edit layout moves,
  // swaps, assigns and clears buttons.

  import Dialog from "./Dialog.svelte";
  import ListPicker from "./ListPicker.svelte";
  import { api } from "../lib/api";
  import { selectList, status } from "../lib/cuelists.svelte";
  import { show } from "../lib/show.svelte";
  import { GRID_COLUMNS, GRID_ROWS } from "../lib/types";

  interface Props {
    /** Record is armed: the next tap records onto a button. */
    recording: boolean;
    /** An empty button tapped while recording: make a new look there. */
    onrecordlook: (page: number, button: number) => void;
    /** A filled button tapped while recording: add a cue to its list. */
    onrecordinto: (list: number) => void;
  }

  let { recording, onrecordlook, onrecordinto }: Props = $props();

  const play = show.play.bind(show);
  const layout = $derived(show.playback.layout);

  let page = $state(0);
  $effect(() => {
    if (page >= layout.pages.length) page = layout.pages.length - 1;
  });
  const buttons = $derived(layout.pages[page]?.buttons ?? []);

  let editing = $state(false);
  /** In Edit layout, the button picked to move, swap, assign or clear. */
  let picked = $state<number | null>(null);
  let dialog = $state<null | { assign: number } | "page" | "newPage">(null);
  let draftName = $state("");
  let confirmDeletePage = $state(false);

  const listOf = (id: number | null) => (id == null ? undefined : show.playback.cueLists.find((l) => l.id === id));

  function tap(i: number) {
    const id = buttons[i];
    const list = listOf(id);
    if (recording) {
      if (list) onrecordinto(list.id);
      else onrecordlook(page, i);
      return;
    }
    if (editing) {
      if (picked == null) {
        picked = i;
        if (list) selectList(list.id);
      } else if (picked === i) {
        picked = null;
      } else {
        const from = picked;
        picked = null;
        play(() => api.swapGridButtons([page, from], [page, i]));
      }
      return;
    }
    if (!list) {
      dialog = { assign: i };
      return;
    }
    if (list.cues.length === 0) return void selectList(list.id);
    // Selects it, then a live one-cue look fades off; anything else GOes,
    // fading out the looks it replaces.
    show.playback.layout.selected = list.id;
    play(() => api.tapCueList(list.id));
  }

  async function assign(button: number, list: number | null) {
    if (await play(() => api.setGridButton(page, button, list))) {
      dialog = null;
      picked = null;
    }
  }

  async function savePage() {
    const ok =
      dialog === "newPage"
        ? await play(() => api.addGridPage(draftName))
        : await play(() => api.renameGridPage(page, draftName));
    if (!ok) return;
    if (dialog === "newPage") page = ok.layout.pages.length - 1;
    dialog = null;
  }

  async function deletePage() {
    if (!confirmDeletePage) return void (confirmDeletePage = true);
    confirmDeletePage = false;
    await play(() => api.deleteGridPage(page));
    page = Math.max(0, page - 1);
  }

  function pageLabel(i: number) {
    const name = layout.pages[i]?.name;
    return name ? `${i + 1} · ${name}` : `Page ${i + 1}`;
  }
</script>

<section class="panel grid-panel" class:recording class:editing>
  <div class="row bar">
    <div class="segmented pages">
      {#each layout.pages as _, i (i)}
        <button aria-pressed={page === i} onclick={() => ((page = i), (picked = null), (confirmDeletePage = false))}
          >{pageLabel(i)}</button
        >
      {/each}
    </div>
    {#if editing}
      <button
        class="ghost"
        onclick={() => {
          draftName = "";
          dialog = "newPage";
        }}>+ Page</button
      >
      <button
        class="ghost"
        onclick={() => {
          draftName = layout.pages[page]?.name ?? "";
          dialog = "page";
        }}>Rename page</button
      >
      {#if layout.pages.length > 1}
        <button class="ghost danger" onclick={deletePage}>{confirmDeletePage ? "Tap again to delete page" : "Delete page"}</button>
      {/if}
    {/if}
    <span class="spacer"></span>
    <button aria-pressed={editing} onclick={() => ((editing = !editing), (picked = null))}>
      {editing ? "Done" : "Edit layout"}
    </button>
  </div>

  {#if editing}
    <div class="row hint">
      {#if picked == null}
        <span class="muted">Tap a button, then another to swap.</span>
      {:else}
        <span>Button {picked + 1} picked. Tap another to swap, or:</span>
        <button onclick={() => (dialog = { assign: picked! })}>Assign list…</button>
        <button disabled={buttons[picked] == null} onclick={() => assign(picked!, null)}>Clear button</button>
        <button class="ghost" onclick={() => (picked = null)}>Cancel</button>
      {/if}
    </div>
  {/if}

  <div class="grid" style:--cols={GRID_COLUMNS} style:--rows={GRID_ROWS}>
    {#each buttons as id, i (i)}
      {@const list = listOf(id)}
      {@const running = list ? status(list.id) : undefined}
      {@const live = running?.releasing ? undefined : running}
      <button
        class="cell"
        class:empty={!list}
        class:live={live != null}
        class:fading={live?.fading}
        class:releasing={running?.releasing}
        class:selected={list != null && list.id === layout.selected}
        class:picked={editing && picked === i}
        onclick={() => tap(i)}
      >
        {#if list}
          <span class="name">{list.name}</span>
          <span class="meta">
            {#if list.cues.length > 1}{live ? live.current + 1 : "–"}/{list.cues.length}{:else if list.cues.length === 0}empty{/if}
          </span>
        {:else if recording}
          <span class="plus">● Record here</span>
        {:else}
          <span class="plus">+</span>
        {/if}
      </button>
    {/each}
  </div>
</section>

{#if dialog === "page" || dialog === "newPage"}
  <Dialog title={dialog === "newPage" ? "New grid page" : "Rename grid page"} onclose={() => (dialog = null)}>
    <label class="field">
      Name
      <input bind:value={draftName} placeholder="e.g. Podcast looks" />
    </label>
    {#snippet actions()}
      <button onclick={() => (dialog = null)}>Cancel</button>
      <button class="primary" onclick={savePage}>{dialog === "newPage" ? "Add page" : "Save"}</button>
    {/snippet}
  </Dialog>
{:else if dialog}
  {@const button = dialog.assign}
  <ListPicker title={`Put a cue list on button ${button + 1}`} onpick={(id) => assign(button, id)} onclose={() => (dialog = null)} />
{/if}

<style>
  .grid-panel {
    padding: 14px;
    min-height: 0;
    display: grid;
    grid-template-rows: auto auto minmax(0, 1fr);
    gap: 10px;
  }

  .grid-panel:not(.editing) {
    grid-template-rows: auto minmax(0, 1fr);
  }

  .bar {
    gap: 8px;
    flex-wrap: wrap;
  }

  .pages {
    flex-wrap: wrap;
  }

  .hint {
    gap: 8px;
    flex-wrap: wrap;
  }

  .grid {
    display: grid;
    grid-template-columns: repeat(var(--cols), minmax(0, 1fr));
    grid-template-rows: repeat(var(--rows), minmax(0, 1fr));
    gap: 6px;
    min-height: 0;
  }

  .cell {
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    align-items: stretch;
    text-align: left;
    padding: 8px 10px;
    min-height: 0;
    min-width: 0;
    overflow: hidden;
    border-top: 4px solid var(--line);
    background: var(--panel-raised);
  }

  .cell .name {
    font-weight: 700;
    line-height: 1.15;
    overflow: hidden;
    display: -webkit-box;
    -webkit-line-clamp: 3;
    line-clamp: 3;
    -webkit-box-orient: vertical;
    overflow-wrap: anywhere;
  }

  .cell .meta {
    font-family: var(--mono);
    font-size: 0.85rem;
    color: var(--muted);
    text-align: right;
  }

  .cell.empty {
    background: transparent;
    border: 1px dashed var(--line);
    align-items: center;
    justify-content: center;
    color: var(--muted);
  }

  .recording .cell.empty {
    border-color: var(--danger, #e5484d);
    color: var(--danger, #e5484d);
  }

  /* Live looks are filled in the playback colour, and pulse while fading. */
  .cell.live {
    background: var(--f-playback);
    border-top-color: var(--f-playback);
    color: var(--accent-text);
  }

  .cell.live .meta {
    color: var(--accent-text);
  }

  .cell.fading {
    animation: fade 0.6s ease-in-out infinite alternate;
  }

  @keyframes fade {
    to {
      opacity: 0.65;
    }
  }

  /* Fading out: an outline in the playback colour until it's gone. */
  .cell.releasing {
    border-top-color: var(--f-playback);
    box-shadow: inset 0 0 0 2px var(--f-playback);
  }

  .cell.selected {
    outline: 3px solid var(--accent);
    outline-offset: -3px;
  }

  .cell.picked {
    outline: 3px dashed var(--accent);
    outline-offset: -3px;
  }

  .plus {
    font-size: 1rem;
    opacity: 0.7;
  }
</style>
