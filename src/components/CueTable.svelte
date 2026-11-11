<script lang="ts">
  // The cues of one list, with the live cue filled and the picked one outlined.
  // Compact for the Playback screen; full on the Cue Lists screen.

  import type { Snippet } from "svelte";
  import { status } from "../lib/cuelists.svelte";
  import type { CueList } from "../lib/types";

  interface Props {
    list: CueList;
    picked: number | null;
    compact?: boolean;
    onpick: (index: number | null) => void;
    empty?: Snippet;
  }

  let { list, picked, compact = false, onpick, empty }: Props = $props();
  const live = $derived(status(list.id));
</script>

<div class="table scroll" class:compact>
  {#each list.cues as cue, i (i)}
    <button
      class="cue"
      class:selected={picked === i}
      class:live={live?.current === i}
      onclick={() => onpick(picked === i ? null : i)}
    >
      <span class="state">{live?.current === i ? (live.fading ? "◐" : "▶") : ""}</span>
      <span class="num">{i + 1}</span>
      <span class="name">{cue.name}</span>
      {#if !compact}
        <span class="muted">
          {Object.keys(cue.values).length} fixtures{#if cue.effects?.length}<span
              class="fx"
              title={cue.effects.map((e) => e.name).join(", ")}>&nbsp;· FX {cue.effects.length}</span
            >{/if}
        </span>
      {:else if cue.effects?.length}
        <span class="fx">FX</span>
      {:else}
        <span></span>
      {/if}
      <span class="mode" class:follow={cue.mode === "follow"}>
        {#if cue.mode === "follow"}
          Follow {live?.current === i && live.followIn != null ? `in ${live.followIn.toFixed(1)}` : cue.follow}s
        {:else}
          Hold
        {/if}
      </span>
      <span class="fade" title={cue.fadeOut != null ? "Fade in / out" : "Fade"}
        >{cue.fade}{#if cue.fadeOut != null}<span class="out">/{cue.fadeOut}</span>{/if}s</span
      >
    </button>
  {:else}
    {@render empty?.()}
  {/each}
</div>

<style>
  .table {
    display: flex;
    flex-direction: column;
    gap: 4px;
    min-height: 0;
  }

  .cue {
    display: grid;
    grid-template-columns: 32px 56px 1fr auto 130px 100px;
    gap: 12px;
    align-items: center;
    text-align: left;
    min-height: 44px;
    flex-shrink: 0;
    border-color: transparent;
    background: transparent;
  }

  .compact .cue {
    grid-template-columns: 24px 44px 1fr auto 110px 76px;
    gap: 8px;
    min-height: 38px;
  }

  /* The live cue is filled in the playback colour; the picked one is outlined. */
  .cue.live {
    background: var(--f-playback);
    color: var(--accent-text);
  }

  .cue.live .muted,
  .cue.live .mode,
  .cue.live .fx,
  .cue.live .state {
    color: var(--accent-text);
  }

  .cue.selected {
    border-color: var(--accent);
  }

  .fx,
  .state {
    color: var(--f-playback);
  }

  .num {
    font-family: var(--mono);
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }

  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .mode {
    font-family: var(--mono);
    font-size: 0.9rem;
    color: var(--muted);
    font-variant-numeric: tabular-nums;
  }

  .mode.follow {
    color: var(--f-playback);
  }

  .fade {
    font-family: var(--mono);
    text-align: right;
    font-variant-numeric: tabular-nums;
    white-space: nowrap;
  }

  .fade .out {
    color: var(--muted);
  }

  .cue.live .fade .out {
    color: var(--accent-text);
  }
</style>
