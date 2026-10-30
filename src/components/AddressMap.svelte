<script lang="ts">
  // All 512 channels of the universe, coloured by the fixture that uses them.

  import { fixtureColour } from "../lib/format";

  interface Props {
    occupancy: (number | null)[];
    /** The selected fixture, or several. */
    selected?: number | null | number[];
    levels?: number[];
    onselect?: (fixture: number | null, address: number) => void;
  }

  let { occupancy, selected = null, levels, onselect }: Props = $props();
  const isSelected = $derived((owner: number) => (Array.isArray(selected) ? selected.includes(owner) : owner === selected));
</script>

<div class="map" role="grid" aria-label="DMX address map">
  {#each occupancy as owner, index (index)}
    {@const start = owner != null && occupancy[index - 1] !== owner}
    <button
      class="cell"
      class:used={owner != null}
      class:start
      class:selected={owner != null && isSelected(owner)}
      style:--colour={owner != null ? fixtureColour(owner) : undefined}
      style:--level={levels ? levels[index] / 255 : 0}
      title={owner != null ? `Address ${index + 1}: fixture ${owner}` : `Address ${index + 1}: free`}
      onclick={() => onselect?.(owner, index + 1)}
    >
      {#if start}<span>{owner}</span>{:else if (index + 1) % 32 === 1}<span class="muted">{index + 1}</span>{/if}
    </button>
  {/each}
</div>

<style>
  .map {
    display: grid;
    grid-template-columns: repeat(32, 1fr);
    gap: 3px;
  }

  .cell {
    min-height: 0;
    height: clamp(14px, 1.7vh, 24px);
    padding: 0;
    border-radius: 4px;
    border: none;
    background: var(--bg);
    font-size: 0.7rem;
    position: relative;
    overflow: hidden;
  }

  .cell.used {
    background: color-mix(in srgb, var(--colour) 30%, var(--panel));
  }

  .cell.used::after {
    content: "";
    position: absolute;
    inset: auto 0 0 0;
    height: calc(var(--level) * 100%);
    background: color-mix(in srgb, var(--colour) 60%, transparent);
    pointer-events: none;
  }

  .cell.start {
    box-shadow: inset 3px 0 0 var(--colour);
  }

  .cell.selected {
    outline: 2px solid var(--text);
    outline-offset: -2px;
  }

  .cell span {
    position: relative;
    z-index: 1;
    font-weight: 600;
  }
</style>
