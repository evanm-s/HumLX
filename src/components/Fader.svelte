<script lang="ts">
  // A vertical touch fader. Dragging moves the level relative to where the
  // finger started, so touching the fader never makes the level jump.

  interface Props {
    value: number;
    label: string;
    sublabel?: string;
    detail?: string;
    colour?: string;
    max?: number;
    active?: boolean;
    /** A wider fader, for the main intensity control. */
    wide?: boolean;
    /** Slows dragging to a tenth of the speed, for precise moves. */
    fine?: boolean;
    /** How to show the level above the fader; defaults to the raw value. */
    format?: (value: number) => string;
    onchange: (value: number) => void;
  }

  let { value, label, sublabel, detail, colour = "var(--accent)", max = 255, active = false, wide = false, fine = false, format, onchange }: Props =
    $props();

  let track: HTMLDivElement;
  let dragging = $state(false);
  let local = $state(0);
  let startY = 0;
  let startValue = 0;

  // While dragging, show the finger's level rather than the polled one.
  const shown = $derived(dragging ? local : value);
  const fill = $derived((shown / max) * 100);

  function down(event: PointerEvent) {
    track.setPointerCapture(event.pointerId);
    dragging = true;
    startY = event.clientY;
    startValue = value;
    local = value;
  }

  function move(event: PointerEvent) {
    if (!dragging) return;
    const height = track.clientHeight;
    const scale = fine ? 0.1 : 1;
    const next = Math.round(startValue + ((startY - event.clientY) / height) * max * scale);
    const clamped = Math.max(0, Math.min(max, next));
    if (clamped !== local) {
      local = clamped;
      onchange(clamped);
    }
  }

  function up() {
    dragging = false;
  }

  function key(event: KeyboardEvent) {
    const step = event.shiftKey ? 10 : 1;
    const delta = { ArrowUp: step, ArrowRight: step, ArrowDown: -step, ArrowLeft: -step, PageUp: 25, PageDown: -25 }[
      event.key
    ];
    const target = event.key === "Home" ? 0 : event.key === "End" ? max : delta != null ? value + delta : null;
    if (target == null) return;
    event.preventDefault();
    onchange(Math.max(0, Math.min(max, target)));
  }
</script>

<div class="fader" class:active class:wide>
  <div class="value">{format ? format(shown) : shown}</div>
  <div
    class="track"
    bind:this={track}
    role="slider"
    tabindex="0"
    aria-label={label}
    aria-valuemin={0}
    aria-valuemax={max}
    aria-valuenow={shown}
    onpointerdown={down}
    onpointermove={move}
    onpointerup={up}
    onpointercancel={up}
    onkeydown={key}
    style:--colour={colour}
    style:--level={fill / 100}
  >
    <div class="groove"></div>
    <div class="fill"></div>
    <div class="tick top"></div>
    <div class="tick mid"></div>
    <div class="tick low"></div>
    <div class="handle"></div>
  </div>
  <div class="label" title={label}>{label}</div>
  {#if sublabel}<div class="sublabel">{sublabel}</div>{/if}
  <div class="detail" title={detail}>{detail ?? ""}</div>
</div>

<style>
  /* Drawn like a console fader: a slot with a groove, the level filling the
     groove in the fader's colour, and a flat cap with a line across it. */
  .fader {
    display: grid;
    grid-template-rows: auto 1fr auto auto auto;
    justify-items: center;
    gap: 8px;
    width: 88px;
    min-height: 0;
    --pad: 22px;
    --cap: 40px;
  }

  .wide {
    width: 140px;
    --cap: 54px;
  }

  .wide .track {
    width: 110px;
  }

  .value {
    font-family: var(--mono);
    font-variant-numeric: tabular-nums;
    font-size: 1.1rem;
    font-weight: 600;
  }

  .wide .value {
    font-size: 2rem;
    line-height: 1.1;
  }

  .active .value {
    color: var(--colour);
  }

  .track {
    position: relative;
    width: 64px;
    height: 100%;
    min-height: 160px;
    border-radius: var(--radius);
    background: #0a0c0f;
    box-shadow: inset 0 0 0 1px var(--line);
    touch-action: none;
    cursor: ns-resize;
  }

  .track:focus-visible {
    outline: 2px solid var(--accent);
  }

  .groove,
  .fill {
    position: absolute;
    left: calc(50% - 3px);
    width: 6px;
    bottom: var(--pad);
  }

  .groove {
    top: var(--pad);
    background: var(--line);
  }

  .fill {
    height: calc((100% - 2 * var(--pad)) * var(--level));
    background: var(--colour);
  }

  .tick {
    position: absolute;
    left: 8px;
    right: 8px;
    height: 2px;
    background:
      linear-gradient(90deg, #3a4150 0 calc(50% - 10px), transparent calc(50% - 10px) calc(50% + 10px), #3a4150 calc(50% + 10px));
  }
  .tick.top {
    top: calc(var(--pad) - 1px);
  }
  .tick.mid {
    top: calc(50% - 1px);
  }
  .tick.low {
    bottom: calc(var(--pad) - 1px);
  }

  .handle {
    position: absolute;
    left: 6px;
    right: 6px;
    height: var(--cap);
    bottom: calc(var(--pad) + (100% - 2 * var(--pad)) * var(--level) - var(--cap) / 2);
    border-radius: var(--radius);
    background: #3a404c;
    pointer-events: none;
  }

  .handle::after {
    content: "";
    position: absolute;
    left: 6px;
    right: 6px;
    top: calc(50% - 2px);
    height: 4px;
    background: var(--colour);
  }

  .label {
    font-weight: 600;
    font-size: 0.85rem;
    text-align: center;
    width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .sublabel,
  .detail {
    color: var(--muted);
    font-size: 0.75rem;
    text-align: center;
    width: 100%;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    height: 1.35em;
  }
</style>
