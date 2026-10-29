<script lang="ts">
  // A screen of windows on a grid. While editing, drag a window's title bar to
  // move it, drag its corner to resize it, or hide it. Windows snap to the grid
  // and can't overlap.

  import type { Snippet } from "svelte";
  import { fits, freeSpot, type Grid, type Layout, type Rect } from "../lib/layout";

  interface Props {
    windows: { id: string; title: string }[];
    layout: Layout;
    grid: Grid;
    editing: boolean;
    /** A window's contents. */
    body: Snippet<[string]>;
    /** Buttons for a window's title bar, shown when not editing. */
    tools?: Snippet<[string]>;
    onchange: (layout: Layout) => void;
  }

  let { windows, layout, grid, editing, body, tools, onchange }: Props = $props();

  const MIN = { w: 2, h: 2 };
  let area: HTMLDivElement;

  interface Drag {
    id: string;
    mode: "move" | "resize";
    pointer: number;
    startX: number;
    startY: number;
    origin: Rect;
    rect: Rect;
  }
  let drag = $state<Drag | null>(null);
  const ok = $derived(drag != null && fits(layout, drag.id, drag.rect, grid));

  function cell() {
    const gap = parseFloat(getComputedStyle(area).columnGap) || 0;
    return {
      w: (area.clientWidth + gap) / grid.cols,
      h: (area.clientHeight + gap) / grid.rows,
    };
  }

  function start(event: PointerEvent, id: string, mode: Drag["mode"]) {
    if (!editing || event.button > 0) return;
    // Buttons in the title bar (such as Hide) keep working while editing.
    if (mode === "move" && (event.target as HTMLElement).closest("button")) return;
    event.preventDefault();
    (event.currentTarget as HTMLElement).setPointerCapture(event.pointerId);
    const { x, y, w, h } = layout[id];
    drag = { id, mode, pointer: event.pointerId, startX: event.clientX, startY: event.clientY, origin: { x, y, w, h }, rect: { x, y, w, h } };
  }

  function move(event: PointerEvent) {
    if (!drag || event.pointerId !== drag.pointer) return;
    const size = cell();
    const dx = Math.round((event.clientX - drag.startX) / size.w);
    const dy = Math.round((event.clientY - drag.startY) / size.h);
    const o = drag.origin;
    drag.rect =
      drag.mode === "move"
        ? {
            x: Math.max(0, Math.min(grid.cols - o.w, o.x + dx)),
            y: Math.max(0, Math.min(grid.rows - o.h, o.y + dy)),
            w: o.w,
            h: o.h,
          }
        : {
            x: o.x,
            y: o.y,
            w: Math.max(MIN.w, Math.min(grid.cols - o.x, o.w + dx)),
            h: Math.max(MIN.h, Math.min(grid.rows - o.y, o.h + dy)),
          };
  }

  function end(event: PointerEvent) {
    if (!drag || event.pointerId !== drag.pointer) return;
    if (ok) onchange({ ...layout, [drag.id]: { ...drag.rect } });
    drag = null;
  }

  function hide(id: string) {
    onchange({ ...layout, [id]: { ...layout[id], hidden: true } });
  }

  /** Brings a hidden window back where it was, or into the first gap that fits. */
  export function restore(id: string) {
    const p = layout[id];
    const spot = fits(layout, id, p, grid)
      ? p
      : (freeSpot(layout, id, p.w, p.h, grid) ?? freeSpot(layout, id, MIN.w, MIN.h, grid) ?? p);
    onchange({ ...layout, [id]: { x: spot.x, y: spot.y, w: spot.w, h: spot.h } });
  }

  const place = (r: Rect) => `grid-column: ${r.x + 1} / span ${r.w}; grid-row: ${r.y + 1} / span ${r.h};`;
</script>

<div
  class="workspace"
  class:editing
  bind:this={area}
  style:grid-template-columns="repeat({grid.cols}, minmax(0, 1fr))"
  style:grid-template-rows="repeat({grid.rows}, minmax(0, 1fr))"
>
  {#if editing}
    {#each { length: grid.cols * grid.rows } as _, i (i)}
      <div class="cell" style={place({ x: i % grid.cols, y: Math.floor(i / grid.cols), w: 1, h: 1 })}></div>
    {/each}
  {/if}

  {#each windows as win (win.id)}
    {@const p = layout[win.id]}
    {#if p && !p.hidden}
      <section class="window panel" class:moving={drag?.id === win.id} style={place(p)} aria-label={win.title}>
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <header
          class="bar"
          onpointerdown={(e) => start(e, win.id, "move")}
          onpointermove={move}
          onpointerup={end}
          onpointercancel={end}
        >
          <h2>{win.title}</h2>
          <span class="spacer"></span>
          {#if editing}
            <button class="ghost small" aria-label="Hide {win.title}" onclick={() => hide(win.id)}>Hide</button>
          {:else if tools}
            {@render tools(win.id)}
          {/if}
        </header>
        <div class="body">{@render body(win.id)}</div>
        {#if editing}
          <!-- svelte-ignore a11y_no_static_element_interactions -->
          <div
            class="resize"
            aria-hidden="true"
            onpointerdown={(e) => start(e, win.id, "resize")}
            onpointermove={move}
            onpointerup={end}
            onpointercancel={end}
          ></div>
        {/if}
      </section>
    {/if}
  {/each}

  {#if drag}
    <div class="ghost-rect" class:blocked={!ok} style={place(drag.rect)}></div>
  {/if}
</div>

<style>
  .workspace {
    display: grid;
    gap: var(--gap);
    height: 100%;
    min-height: 0;
  }

  .cell {
    border-radius: 8px;
    border: 1px dashed color-mix(in srgb, var(--line) 70%, transparent);
    pointer-events: none;
  }

  .window {
    display: flex;
    flex-direction: column;
    min-width: 0;
    min-height: 0;
    padding: 0;
    position: relative;
    overflow: hidden;
  }

  .editing .window {
    outline: 2px solid color-mix(in srgb, var(--accent) 45%, transparent);
  }

  .window.moving {
    opacity: 0.55;
  }

  .bar {
    display: flex;
    align-items: center;
    gap: 8px;
    min-height: 52px;
    padding: 4px 8px 4px 16px;
    border-bottom: 1px solid var(--line);
  }

  .bar h2 {
    margin: 0;
    font-size: 1rem;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .editing .bar {
    cursor: grab;
    touch-action: none;
    background: color-mix(in srgb, var(--accent) 12%, transparent);
  }

  .bar :global(button.small) {
    min-height: 40px;
    padding: 0 12px;
    font-size: 0.9rem;
  }

  .body {
    flex: 1;
    min-height: 0;
    padding: 14px;
    overflow: auto;
    container-type: size;
  }

  .resize {
    position: absolute;
    right: 0;
    bottom: 0;
    width: 48px;
    height: 48px;
    cursor: nwse-resize;
    touch-action: none;
    background: linear-gradient(135deg, transparent 50%, var(--accent) 50%);
    border-bottom-right-radius: inherit;
  }

  .ghost-rect {
    border-radius: 12px;
    border: 3px solid var(--accent);
    background: color-mix(in srgb, var(--accent) 15%, transparent);
    pointer-events: none;
    z-index: 2;
  }

  .ghost-rect.blocked {
    border-color: var(--danger);
    background: color-mix(in srgb, var(--danger) 15%, transparent);
  }
</style>
