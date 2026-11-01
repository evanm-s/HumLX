<script lang="ts">
  // A live picture of an effect: one dot per fixture, in the order they were
  // picked, running as the engine would run it.

  import type { EffectFamily } from "../../lib/types";
  import { previewAt, type PreviewEffect } from "./effects";

  interface Props {
    fx: PreviewEffect;
    family: EffectFamily;
    count: number;
  }

  let { fx, family, count }: Props = $props();

  const shown = $derived(Math.min(Math.max(count, 1), 24));
  let elapsed = $state(0);

  $effect(() => {
    const start = performance.now();
    let frame = requestAnimationFrame(function tick(now) {
      elapsed = (now - start) / 1000;
      frame = requestAnimationFrame(tick);
    });
    return () => cancelAnimationFrame(frame);
  });

  const css = (c: { r: number; g: number; b: number }, level = 1) =>
    `rgb(${c.r * level * 255} ${c.g * level * 255} ${c.b * level * 255})`;
</script>

<div class="preview" aria-label="Preview of the effect on {count} fixtures" role="img">
  {#each { length: shown } as _, i (i)}
    {@const v = previewAt(fx, i, shown, elapsed)}
    {#if family === "position"}
      <div class="pad">
        <span class="dot small" style:left="{v.pan * 100}%" style:top="{(1 - v.tilt) * 100}%"></span>
      </div>
    {:else}
      <span
        class="dot"
        style:background={family === "colour" ? css(v.colour) : css({ r: 1, g: 0.78, b: 0.4 }, v.level)}
      ></span>
    {/if}
  {/each}
</div>

<style>
  .preview {
    display: flex;
    flex-wrap: wrap;
    gap: 8px;
    padding: 10px;
    border-radius: 8px;
    background: #000;
    min-height: 48px;
    align-items: center;
  }

  .dot {
    width: 28px;
    height: 28px;
    border-radius: 50%;
    border: 1px solid #333;
  }

  .pad {
    position: relative;
    width: 44px;
    height: 44px;
    border: 1px solid #333;
    border-radius: 6px;
  }

  .dot.small {
    position: absolute;
    width: 10px;
    height: 10px;
    margin: -5px 0 0 -5px;
    background: var(--accent);
    border: none;
  }
</style>
