<script lang="ts">
  // The Update prompt: says which cue the programmer will be merged into and
  // what that changes, then does it. Enter or Update again confirms, Esc closes.

  import { untrack } from "svelte";
  import Dialog from "./Dialog.svelte";
  import { api } from "../lib/api";
  import { show } from "../lib/show.svelte";
  import { currentCue, runningCues, updateCue, type UpdateTarget } from "../lib/updatecue.svelte";
  import type { Values } from "../lib/types";

  interface Props {
    /** A cue picked on the Cue Lists page; otherwise the current cue. */
    cue: UpdateTarget | null;
  }

  let { cue }: Props = $props();

  // Fixed when the prompt opens, so a GO behind it can't change what was agreed.
  const running = runningCues();
  const named = untrack(() => cue);
  let target = $state<UpdateTarget | null>(named ?? currentCue());
  const choosing = named == null && running.length > 1;

  const listOf = (id: number) => show.playback.cueLists.find((l) => l.id === id);
  const list = $derived(target ? listOf(target.list) : undefined);
  const into = $derived(list && target ? list.cues[target.index] : undefined);
  const isCurrent = $derived(target != null && running.some((r) => r.list === target!.list && r.index === target!.index));

  const values = $derived(show.programmer.values);
  const effects = $derived(show.programmer.effects.length);
  const fixtures = $derived(Object.keys(values));
  const added = $derived(into ? fixtures.filter((f) => !into.values[f]).length : 0);

  const FAMILIES: [string, (v: Values) => boolean][] = [
    ["intensity", (v) => v.intensity != null],
    ["colour", (v) => v.colour != null || Object.keys(v.other ?? {}).some((a) => a.startsWith("colour"))],
    ["position", (v) => v.pan != null || v.tilt != null],
    ["beam", (v) => Object.keys(v.other ?? {}).some((a) => !a.startsWith("colour"))],
  ];
  const families = $derived(FAMILIES.filter(([, has]) => Object.values(values).some(has)).map(([name]) => name));

  const plural = (n: number, one: string, many = `${one}s`) => `${n} ${n === 1 ? one : many}`;
  const cueName = (list: number, index: number) => {
    const c = listOf(list)?.cues[index];
    return `Cue ${index + 1}${c?.name && c.name !== `Cue ${index + 1}` ? ` “${c.name}”` : ""}`;
  };

  async function update() {
    if (!target || !into) return;
    const { list, index } = target;
    if (await show.play(() => api.updateCue(list, index))) updateCue.close();
  }

  // Enter confirms; Esc is handled by the dialog.
  function keys(e: KeyboardEvent) {
    if (e.key !== "Enter" || e.defaultPrevented || e.repeat) return;
    e.preventDefault();
    update();
  }
</script>

<svelte:window onkeydown={keys} />

<Dialog title="Update cue" onclose={() => updateCue.close()}>
  {#if choosing}
    <div class="field">
      Running cue lists
      <div class="segmented wrap">
        {#each running as r (r.list)}
          <button aria-pressed={target?.list === r.list} onclick={() => (target = r)}>
            {listOf(r.list)?.name} <span class="muted">· {cueName(r.list, r.index)}</span>
          </button>
        {/each}
      </div>
      <span class="muted">
        {target ? "Updates the selected list. Tap another to change." : "None of these is selected. Pick one."}
      </span>
    </div>
  {/if}

  {#if into && list && target}
    <p class="what">
      Update <strong>{cueName(target.list, target.index)}</strong> of <strong>{list.name}</strong>{isCurrent ? " (running now)" : ""}?
    </p>
    <p>
      Merges {plural(fixtures.length, "fixture")}{families.length ? ` (${families.join(", ")})` : ""}{effects
        ? ` and ${plural(effects, "effect")}`
        : ""} into the cue{added ? `, adding ${plural(added, "fixture")}` : ""}. Everything else in the cue stays.
    </p>
    <p class="muted">
      Then clears the programmer{isCurrent ? "; the stage doesn't change" : ""}.
    </p>
  {:else if !choosing}
    <p>
      {running.length === 0
        ? "No cue is running. GO the cue to change, or pick it on the Cue Lists page."
        : "That cue no longer exists."}
    </p>
  {/if}

  {#snippet actions()}
    <button onclick={() => updateCue.close()}>Cancel</button>
    <button class="primary" disabled={!into} onclick={update}>Update</button>
  {/snippet}
</Dialog>

<style>
  .what {
    font-size: 1.15em;
  }
</style>
