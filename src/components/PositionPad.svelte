<script lang="ts">
  // Pan and tilt pad. Dragging moves relative to the finger so a touch never
  // makes a moving head swing; "Fine" slows the movement for precise focus.

  interface Props {
    pan: number | undefined;
    tilt: number | undefined;
    fine: boolean;
    /** The value is what's on stage from a cue, not in the programmer: drawn dimmer. */
    live?: boolean;
    /** A position effect's path to draw, if one is running. */
    path?: { pan: number; tilt: number }[];
    onchange: (pan: number, tilt: number) => void;
  }

  import { drag, type DragPoint } from "../lib/drag";

  let { pan, tilt, fine, path, live = false, onchange }: Props = $props();

  let pad: HTMLDivElement;
  let start: { x: number; y: number; pan: number; tilt: number } | null = null;

  const x = $derived(pan ?? 0.5);
  const y = $derived(tilt ?? 0.5);

  function down(event: DragPoint) {
    start = { x: event.clientX, y: event.clientY, pan: x, tilt: y };
  }

  function move(event: DragPoint) {
    if (!start) return;
    const scale = fine ? 0.1 : 1;
    const clamp = (v: number) => Math.max(0, Math.min(1, v));
    onchange(
      clamp(start.pan + ((event.clientX - start.x) / pad.clientWidth) * scale),
      // Up on the pad is up in the air: higher tilt.
      clamp(start.tilt - ((event.clientY - start.y) / pad.clientHeight) * scale),
    );
  }
</script>

<div
  class="pad"
  class:unset={pan == null && tilt == null}
  class:live
  bind:this={pad}
  role="application"
  aria-label="Pan and tilt"
  use:drag={{ start: down, move, end: () => (start = null) }}
>
  <div class="axis h"></div>
  <div class="axis v"></div>
  {#if path?.length}
    <svg class="path" viewBox="0 0 100 100" preserveAspectRatio="none" aria-hidden="true">
      <polygon points={path.map((p) => `${p.pan * 100},${(1 - p.tilt) * 100}`).join(" ")} />
    </svg>
  {/if}
  <div class="cross h" style:top="{(1 - y) * 100}%"></div>
  <div class="cross v" style:left="{x * 100}%"></div>
  <div class="dot" style:left="{x * 100}%" style:top="{(1 - y) * 100}%"></div>
  <span class="label pan">Pan {Math.round(x * 100)}%</span>
  <span class="label tilt">Tilt {Math.round(y * 100)}%</span>
</div>

<style>
  /* A grid with fine crosshairs through a ring, in the position colour. */
  .pad {
    position: relative;
    aspect-ratio: 1;
    width: 100%;
    max-width: 640px;
    border-radius: var(--radius);
    background:
      linear-gradient(#232833 1px, transparent 1px) 0 0 / 12.5% 12.5%,
      linear-gradient(90deg, #232833 1px, transparent 1px) 0 0 / 12.5% 12.5%,
      #0a0c0f;
    box-shadow: inset 0 0 0 1px var(--line);
    touch-action: none;
    overflow: hidden;
  }

  .path {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    pointer-events: none;
  }
  .path polygon {
    fill: none;
    stroke: color-mix(in srgb, var(--f-position) 70%, transparent);
    stroke-width: 2;
    stroke-dasharray: 6 5;
    vector-effect: non-scaling-stroke;
  }

  .axis,
  .cross {
    position: absolute;
    pointer-events: none;
  }
  .axis {
    background: #343b48;
  }
  .cross {
    background: color-mix(in srgb, var(--f-position) 45%, transparent);
  }
  .h {
    left: 0;
    right: 0;
    height: 1px;
  }
  .v {
    top: 0;
    bottom: 0;
    width: 1px;
  }
  .axis.h {
    top: 50%;
  }
  .axis.v {
    left: 50%;
  }

  .dot {
    position: absolute;
    width: 48px;
    height: 48px;
    margin: -24px 0 0 -24px;
    box-sizing: border-box;
    border-radius: 50%;
    border: 3px solid var(--f-position);
    pointer-events: none;
  }

  .unset .dot,
  .unset .cross {
    opacity: 0.4;
  }

  /* Showing a cue's position rather than one in the programmer. */
  .live .dot {
    border-style: dashed;
    opacity: 0.65;
  }

  .label {
    position: absolute;
    font-family: var(--mono);
    font-size: 0.85rem;
    color: var(--f-position);
    font-variant-numeric: tabular-nums;
  }
  .label.pan {
    bottom: 10px;
    right: 14px;
  }
  .label.tilt {
    top: 10px;
    left: 14px;
  }
</style>
