<script lang="ts">
  // Records the programmer as a cue, into a cue list picked here or a new one
  // named here, so there's no need to set up a cue list first.

  import Dialog from "./Dialog.svelte";
  import NumberField from "./NumberField.svelte";
  import { api } from "../lib/api";
  import { show } from "../lib/show.svelte";

  interface Props {
    /** The cue list to suggest, such as the one on screen. */
    list?: number | null;
    /** Inserts after this cue (0-based) when recording into `list`. */
    after?: number | null;
    onclose: () => void;
    onrecorded?: (list: number, index: number) => void;
  }

  let { list = null, after = null, onclose, onrecorded }: Props = $props();

  // With many lists, offer the suggested one, the target and those in fader
  // slots, plus any that match a search.
  let query = $state("");
  const many = show.playback.cueLists.length > 8;
  const lists = $derived.by(() => {
    const all = show.playback.cueLists;
    if (!many) return all;
    const q = query.trim().toLowerCase();
    const slots = new Set(show.playback.layout.slots);
    return all.filter((l) =>
      q ? l.name.toLowerCase().includes(q) || String(l.id) === q : l.id === list || l.id === target || slots.has(l.id),
    );
  });
  const programmed = $derived(Object.keys(show.programmer.values).length);
  const effects = $derived(show.programmer.effects.length);

  // The suggested list, else the newest one; null means a new cue list.
  function suggested() {
    const all = show.playback.cueLists;
    return all.some((l) => l.id === list) ? list : (all[all.length - 1]?.id ?? null);
  }
  let target = $state<number | null>(suggested());
  let listName = $state("");
  let name = $state("");
  let fade = $state<number | null>(2);
  /** Null: the same as the fade in. */
  let fadeOut = $state<number | null>(null);

  const into = $derived(show.playback.cueLists.find((l) => l.id === target));
  // Where the cue lands: after the picked cue in the suggested list, else at the end.
  const insertAfter = $derived(into && target === list ? after : null);
  const at = $derived(into ? Math.min(insertAfter == null ? into.cues.length : insertAfter + 1, into.cues.length) : 0);

  async function record() {
    const ok = await show.play(() =>
      api.recordCue(target == null ? { new: listName } : { list: target }, name, Math.max(0, fade ?? 0), insertAfter, fadeOut == null ? null : Math.max(0, fadeOut)),
    );
    if (!ok) return;
    const id = target ?? ok.cueLists[ok.cueLists.length - 1].id;
    onrecorded?.(id, at);
    onclose();
  }
</script>

<Dialog title="Record cue" {onclose}>
  <div class="field">
    Cue list
    {#if many}<input type="search" bind:value={query} placeholder="Search all cue lists" />{/if}
    <div class="segmented wrap">
      {#each lists as l (l.id)}
        <button aria-pressed={target === l.id} onclick={() => (target = l.id)}>
          {l.name} <span class="muted">· {l.cues.length}</span>
        </button>
      {/each}
      <button aria-pressed={target == null} onclick={() => (target = null)}>+ New cue list</button>
    </div>
  </div>
  {#if target == null}
    <label class="field">
      New cue list name
      <!-- svelte-ignore a11y_autofocus -->
      <input bind:value={listName} placeholder="e.g. Podcast" autofocus />
    </label>
  {/if}
  <label class="field">
    Cue name
    <input bind:value={name} placeholder={`Cue ${at + 1}`} />
  </label>
  <div class="fades">
    <NumberField label="Fade in (s)" fractional bind:value={fade} min={0} max={600} />
    <NumberField label="Fade out (s)" fractional bind:value={fadeOut} placeholder={String(fade ?? 0)} min={0} max={600} />
  </div>
  <p class="muted">
    {#if into}
      Becomes cue {at + 1} of {into.name}{at < into.cues.length ? "; later cues move down" : ""}.
    {:else}
      Starts a new cue list.
    {/if}
    Stores {programmed} fixtures{effects
      ? ` and ${effects} ${effects === 1 ? "effect" : "effects"} (effects carry on until a later cue sets those fixtures)`
      : ""}, then clears the Programmer. The stage doesn't change.
  </p>
  {#snippet actions()}
    <button onclick={onclose}>Cancel</button>
    <button class="primary" disabled={programmed === 0 && effects === 0} onclick={record}>Record</button>
  {/snippet}
</Dialog>

<style>
  .wrap {
    flex-wrap: wrap;
  }

  .fades {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: var(--gap);
  }
</style>
