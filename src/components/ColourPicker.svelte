<script lang="ts">
  // Hue and saturation disc: hue goes round the edge, the centre is white.

  import { hsvToRgb, rgbCss, rgbToHsv } from "../lib/format";
  import type { Rgb } from "../lib/types";

  interface Props {
    colour: Rgb | undefined;
    onchange: (colour: Rgb) => void;
  }

  let { colour, onchange }: Props = $props();

  let disc: HTMLDivElement;
  let dragging = false;

  const hsv = $derived(colour ? rgbToHsv(colour) : null);
  // Marker position as a fraction of the radius.
  const marker = $derived(
    hsv
      ? {
          x: Math.sin((hsv.h * Math.PI) / 180) * hsv.s,
          y: -Math.cos((hsv.h * Math.PI) / 180) * hsv.s,
        }
      : null,
  );

  function pick(event: PointerEvent) {
    const rect = disc.getBoundingClientRect();
    const radius = rect.width / 2;
    const dx = event.clientX - rect.left - radius;
    const dy = event.clientY - rect.top - radius;
    // 0° at the top, clockwise, matching the conic gradient.
    const hue = ((Math.atan2(dx, -dy) * 180) / Math.PI + 360) % 360;
    const saturation = Math.min(1, Math.hypot(dx, dy) / radius);
    onchange(hsvToRgb(hue, saturation));
  }
</script>

<div
  class="disc"
  bind:this={disc}
  role="slider"
  tabindex="0"
  aria-label="Colour"
  aria-valuenow={hsv ? Math.round(hsv.h) : undefined}
  onpointerdown={(e) => {
    disc.setPointerCapture(e.pointerId);
    dragging = true;
    pick(e);
  }}
  onpointermove={(e) => dragging && pick(e)}
  onpointerup={() => (dragging = false)}
  onpointercancel={() => (dragging = false)}
>
  {#if marker && colour}
    <div
      class="marker"
      style:left="{50 + marker.x * 50}%"
      style:top="{50 + marker.y * 50}%"
      style:background={rgbCss(colour)}
    ></div>
  {/if}
</div>

<style>
  .disc {
    position: relative;
    aspect-ratio: 1;
    width: 100%;
    max-width: 640px;
    border-radius: 50%;
    background:
      radial-gradient(circle closest-side, #fff 0%, rgb(255 255 255 / 0) 100%),
      conic-gradient(#f00, #ff0, #0f0, #0ff, #00f, #f0f, #f00);
    touch-action: none;
    cursor: crosshair;
    box-shadow: 0 0 0 1px var(--line);
  }

  .marker {
    position: absolute;
    width: 44px;
    height: 44px;
    margin: -22px 0 0 -22px;
    border-radius: 50%;
    border: 4px solid #fff;
    box-shadow:
      0 0 0 2px #000,
      0 6px 16px rgb(0 0 0 / 0.5);
    pointer-events: none;
  }
</style>
