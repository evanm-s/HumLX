<script lang="ts">
  // White balance strip for bi-colour fixtures: warm on the left, cool on
  // the right. The value runs from 0 (warmest) to 1 (coolest).

  interface Props {
    value: number | undefined;
    onchange: (value: number) => void;
  }

  let { value, onchange }: Props = $props();

  const STOPS = [
    { label: "Warm", value: 0 },
    { label: "Warmer", value: 0.25 },
    { label: "Neutral", value: 0.5 },
    { label: "Cooler", value: 0.75 },
    { label: "Cool", value: 1 },
  ];

  let strip: HTMLDivElement;
  let dragging = false;

  function pick(event: PointerEvent) {
    const rect = strip.getBoundingClientRect();
    onchange(Math.max(0, Math.min(1, (event.clientX - rect.left) / rect.width)));
  }

  function key(event: KeyboardEvent) {
    const step = event.shiftKey ? 0.1 : 0.01;
    const delta = { ArrowRight: step, ArrowUp: step, ArrowLeft: -step, ArrowDown: -step }[event.key];
    if (delta == null) return;
    event.preventDefault();
    onchange(Math.max(0, Math.min(1, (value ?? 0.5) + delta)));
  }
</script>

<div class="balance">
  <div class="ends muted"><span>Warm</span><span>{value == null ? "Not set" : `${Math.round(value * 100)}% cool`}</span><span>Cool</span></div>
  <div
    class="strip"
    bind:this={strip}
    role="slider"
    tabindex="0"
    aria-label="White balance"
    aria-valuemin={0}
    aria-valuemax={100}
    aria-valuenow={value == null ? undefined : Math.round(value * 100)}
    onpointerdown={(e) => {
      strip.setPointerCapture(e.pointerId);
      dragging = true;
      pick(e);
    }}
    onpointermove={(e) => dragging && pick(e)}
    onpointerup={() => (dragging = false)}
    onpointercancel={() => (dragging = false)}
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
    border-radius: 12px;
    /* Roughly 2700 K through 6500 K. */
    background: linear-gradient(to right, #ffa957, #ffd2a1, #fff4e8, #e9efff, #c9d9ff);
    box-shadow: 0 0 0 1px var(--line);
    touch-action: none;
    cursor: ew-resize;
  }

  .strip:focus-visible {
    outline: 2px solid var(--accent);
  }

  .marker {
    position: absolute;
    top: -4px;
    bottom: -4px;
    width: 12px;
    margin-left: -6px;
    border-radius: 6px;
    background: #fff;
    box-shadow:
      0 0 0 2px #000,
      0 4px 10px rgb(0 0 0 / 0.5);
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

  .stops button[aria-pressed="true"] {
    border-color: var(--accent);
    color: var(--accent);
  }
</style>
