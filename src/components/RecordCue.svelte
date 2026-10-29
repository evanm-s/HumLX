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

  const lists = $derived(show.playback.cueLists);
  const programmed = $derived(Object.keys(show.programmer.values).length);

  // The suggested list, else the newest one; null means a new cue list.
  function suggested() {
    const all = show.playback.cueLists;
    return all.some((l) => l.id === list) ? list : (all[all.length - 1]?.id ?? null);
  }
  let target = $state<number | null>(suggested());
  let listName = $state("");
  let name = $state("");
  let fade = $state<number | null>(2);

  const into = $derived(lists.find((l) => l.id === target));
  // Where the cue lands: after the picked cue in the suggested list, else at the end.
  const insertAfter = $derived(into && target === list ? after : null);
  const at = $derived(into ? Math.min(insertAfter == null ? into.cues.length : insertAfter + 1, into.cues.length) : 0);

  async function record() {
    const ok = await show.play(() =>
      api.recordCue(target == null ? { new: listName } : { list: target }, name, Math.max(0, fade ?? 0), insertAfter),
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
  <NumberField label="Fade time (seconds)" bind:value={fade} min={0} max={600} />
  <p class="muted">
    {#if into}
      Becomes cue {at + 1} of {into.name}{at < into.cues.length ? ", and the cues after it move down one" : ""}.
    {:else}
      Starts a new cue list with this as its first cue.
    {/if}
    It stores what the Programmer is setting on {programmed} fixtures. The Programmer is then cleared and the cue list
    goes to the new cue, so the stage stays the same.
  </p>
  {#snippet actions()}
    <button onclick={onclose}>Cancel</button>
    <button class="primary" disabled={programmed === 0} onclick={record}>Record</button>
  {/snippet}
</Dialog>

<style>
  .wrap {
    flex-wrap: wrap;
  }
</style>
