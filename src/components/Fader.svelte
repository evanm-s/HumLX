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
  >
    <div class="fill" style:height="{fill}%"></div>
    <div class="handle" style:bottom="{fill}%"></div>
  </div>
  <div class="label" title={label}>{label}</div>
  {#if sublabel}<div class="sublabel">{sublabel}</div>{/if}
  <div class="detail" title={detail}>{detail ?? ""}</div>
</div>

<style>
  .fader {
    display: grid;
    grid-template-rows: auto 1fr auto auto auto;
    justify-items: center;
    gap: 8px;
    width: 104px;
    min-height: 0;
  }

  .wide {
    width: 180px;
  }

  .wide .track {
    width: 150px;
    border-radius: 20px;
  }

  .value {
    font-variant-numeric: tabular-nums;
    font-size: 1.1rem;
    font-weight: 600;
  }

  .active .value {
    color: var(--accent);
  }

  .track {
    position: relative;
    width: 76px;
    height: 100%;
    min-height: 200px;
    border-radius: 14px;
    background: var(--bg);
    border: 1px solid var(--line);
    overflow: hidden;
    touch-action: none;
    cursor: ns-resize;
  }

  .track:focus-visible {
    outline: 2px solid var(--accent);
  }

  .fill {
    position: absolute;
    inset: auto 0 0 0;
    background: linear-gradient(to top, color-mix(in srgb, var(--colour) 35%, transparent), var(--colour));
    opacity: 0.85;
  }

  .handle {
    position: absolute;
    left: 6px;
    right: 6px;
    height: 6px;
    margin-bottom: -3px;
    border-radius: 3px;
    background: var(--text);
    box-shadow: 0 0 0 2px var(--bg);
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
