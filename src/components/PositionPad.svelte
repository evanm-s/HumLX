<script lang="ts">
  // Pan and tilt pad. Dragging moves relative to the finger so a touch never
  // makes a moving head swing; "Fine" slows the movement for precise focus.

  interface Props {
    pan: number | undefined;
    tilt: number | undefined;
    fine: boolean;
    onchange: (pan: number, tilt: number) => void;
  }

  let { pan, tilt, fine, onchange }: Props = $props();

  let pad: HTMLDivElement;
  let start: { x: number; y: number; pan: number; tilt: number } | null = null;

  const x = $derived(pan ?? 0.5);
  const y = $derived(tilt ?? 0.5);

  function down(event: PointerEvent) {
    pad.setPointerCapture(event.pointerId);
    start = { x: event.clientX, y: event.clientY, pan: x, tilt: y };
  }

  function move(event: PointerEvent) {
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
  bind:this={pad}
  role="application"
  aria-label="Pan and tilt"
  onpointerdown={down}
  onpointermove={move}
  onpointerup={() => (start = null)}
  onpointercancel={() => (start = null)}
>
  <div class="axis h"></div>
  <div class="axis v"></div>
  <div class="dot" style:left="{x * 100}%" style:top="{(1 - y) * 100}%"></div>
  <span class="label pan">Pan {Math.round(x * 100)}%</span>
  <span class="label tilt">Tilt {Math.round(y * 100)}%</span>
</div>

<style>
  .pad {
    position: relative;
    aspect-ratio: 1;
    width: 100%;
    max-width: 640px;
    border-radius: 16px;
    background:
      linear-gradient(var(--line) 1px, transparent 1px) 0 0 / 10% 10%,
      linear-gradient(90deg, var(--line) 1px, transparent 1px) 0 0 / 10% 10%,
      var(--bg);
    border: 1px solid var(--line);
    touch-action: none;
    overflow: hidden;
  }

  .axis {
    position: absolute;
    background: color-mix(in srgb, var(--accent) 40%, transparent);
  }
  .axis.h {
    left: 0;
    right: 0;
    top: 50%;
    height: 2px;
  }
  .axis.v {
    top: 0;
    bottom: 0;
    left: 50%;
    width: 2px;
  }

  .dot {
    position: absolute;
    width: 48px;
    height: 48px;
    margin: -24px 0 0 -24px;
    border-radius: 50%;
    background: var(--accent);
    box-shadow: 0 0 0 6px color-mix(in srgb, var(--accent) 30%, transparent);
    pointer-events: none;
  }

  .unset .dot {
    background: var(--muted);
    box-shadow: none;
  }

  .label {
    position: absolute;
    font-size: 0.85rem;
    color: var(--muted);
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
