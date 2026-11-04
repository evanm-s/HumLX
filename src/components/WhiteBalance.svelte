<script lang="ts">
  // White balance strip for bi-colour fixtures: warm on the left, cool on
  // the right. The value runs from 0 (warmest) to 1 (coolest).

  interface Props {
    value: number | undefined;
    /** The value is what's on stage from a cue, not in the programmer: drawn dimmer. */
    live?: boolean;
    onchange: (value: number) => void;
  }

  import { drag, type DragPoint } from "../lib/drag";

  let { value, live = false, onchange }: Props = $props();

  const STOPS = [
    { label: "Warm", value: 0 },
    { label: "Warmer", value: 0.25 },
    { label: "Neutral", value: 0.5 },
    { label: "Cooler", value: 0.75 },
    { label: "Cool", value: 1 },
  ];

  let strip: HTMLDivElement;

  // Dragging moves the balance relative to the finger, so a touch never makes
  // it jump. With no balance set yet it starts at neutral.
  let start: { x: number; value: number } | null = null;

  function down(event: DragPoint) {
    start = { x: event.clientX, value: value ?? 0.5 };
  }

  function move(event: DragPoint) {
    if (!start) return;
    onchange(Math.max(0, Math.min(1, start.value + (event.clientX - start.x) / strip.clientWidth)));
  }

  function key(event: KeyboardEvent) {
    const step = event.shiftKey ? 0.1 : 0.01;
    const delta = { ArrowRight: step, ArrowUp: step, ArrowLeft: -step, ArrowDown: -step }[event.key];
    if (delta == null) return;
    event.preventDefault();
    onchange(Math.max(0, Math.min(1, (value ?? 0.5) + delta)));
  }
</script>

<div class="balance" class:live>
  <div class="ends muted"><span>Warm</span><span>{value == null ? "Not set" : `${Math.round(value * 100)}% cool${live ? " on stage" : ""}`}</span><span>Cool</span></div>
  <div
    class="strip"
    bind:this={strip}
    role="slider"
    tabindex="0"
    aria-label="White balance"
    aria-valuemin={0}
    aria-valuemax={100}
    aria-valuenow={value == null ? undefined : Math.round(value * 100)}
    use:drag={{ start: down, move, end: () => (start = null) }}
    onkeydown={key}
  >
    {#if value != null}<div class="marker" style:left="{value * 100}%"></div>{/if}
  </div>
  <div class="stops">
    {#each STOPS as stop (stop.value)}
      <button aria-pressed={value != null && Math.abs(value - stop.value) < 0.005} onclick={() => onchange(stop.value)}>
        {stop.label}
      </button>
    {/each}
  </div>
</div>

<style>
  .balance {
    display: grid;
    gap: 8px;
    width: 100%;
  }

  .ends {
    display: flex;
    justify-content: space-between;
    font-size: 0.8rem;
  }

  .strip {
    position: relative;
    height: 72px;
    border-radius: var(--radius);
    /* Roughly 2700 K through 6500 K. */
    background: linear-gradient(to right, #ffa957, #ffd2a1, #fff4e8, #e9efff, #c9d9ff);
    box-shadow: 0 0 0 1px var(--line);
    touch-action: none;
    cursor: ew-resize;
  }

  .strip:focus-visible {
    outline: 2px solid var(--accent);
  }

  .live .marker {
    opacity: 0.5;
  }

  .marker {
    position: absolute;
    top: -4px;
    bottom: -4px;
    width: 12px;
    margin-left: -6px;
    border-radius: 2px;
    background: #fff;
    box-shadow: 0 0 0 2px #000;
    pointer-events: none;
  }

  .stops {
    display: grid;
    grid-template-columns: repeat(5, 1fr);
    gap: 6px;
  }

  .stops button {
    min-height: 44px;
    padding: 0 4px;
    font-size: 0.85rem;
  }


</style>
