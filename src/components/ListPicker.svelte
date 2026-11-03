<script lang="ts">
  // Picks a cue list from the whole library, searching by name or number.

  import Dialog from "./Dialog.svelte";
  import { placements } from "../lib/cuelists.svelte";
  import { show } from "../lib/show.svelte";

  interface Props {
    title: string;
    onpick: (list: number) => void;
    onclose: () => void;
  }

  let { title, onpick, onclose }: Props = $props();

  let query = $state("");
  const lists = $derived.by(() => {
    const q = query.trim().toLowerCase();
    return show.playback.cueLists
      .filter((l) => !q || l.name.toLowerCase().includes(q) || String(l.id) === q)
      .sort((a, b) => a.id - b.id);
  });
</script>

<Dialog {title} {onclose}>
  <input class="search" type="search" bind:value={query} placeholder="Search by name or number" />
  <div class="lists">
    {#each lists as l (l.id)}
      {@const where = placements(l.id)}
      <button onclick={() => onpick(l.id)}>
        <span class="num">{l.id}</span>
        <span class="name">{l.name}</span>
        <span class="muted">{l.cues.length} {l.cues.length === 1 ? "cue" : "cues"}{where.length ? ` · ${where.join(", ")}` : ""}</span>
      </button>
    {:else}
      <p class="muted">{show.playback.cueLists.length ? "No cue lists match." : "There are no cue lists yet."}</p>
    {/each}
  </div>
</Dialog>

<style>
  .search {
    width: 100%;
    margin-bottom: 10px;
  }

  .lists {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-width: 520px;
  }

  .lists button {
    display: grid;
    grid-template-columns: 48px 1fr auto;
    gap: 12px;
    text-align: left;
    align-items: center;
  }

  .num {
    font-family: var(--mono);
    font-variant-numeric: tabular-nums;
    opacity: 0.7;
  }

  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }
</style>
