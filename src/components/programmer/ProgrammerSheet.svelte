<script lang="ts">
  // Every value in the programmer, a row per fixture and a column per
  // attribute, like a spreadsheet. Tap a row to select that fixture.

  import { api } from "../../lib/api";
  import { attributeName, fixtureColour, rgbCss } from "../../lib/format";
  import { tapMode } from "../../lib/selection";
  import { show } from "../../lib/show.svelte";
  import type { Attribute, Rgb, Values } from "../../lib/types";

  const rows = $derived(
    Object.entries(show.programmer.values)
      .map(([id, values]) => ({ id: Number(id), values, fixture: show.fixture(Number(id)) }))
      .sort((a, b) => a.id - b.id),
  );

  const has = (key: keyof Values) => rows.some((r) => r.values[key] != null);
  const other = $derived.by(() => {
    const found = new Set<Attribute>();
    for (const r of rows) for (const a of Object.keys(r.values.other ?? {})) found.add(a as Attribute);
    return [...found].sort();
  });
  const selected = $derived(new Set(show.programmer.selection));

  const pct = (v: number | undefined) => (v == null ? "" : `${Math.round(v * 100)}`);
  const dmx = (v: number | undefined) => (v == null ? "" : `${Math.round(v * 255)}`);
  const hex = (c: Rgb) =>
    "#" +
    [c.r, c.g, c.b]
      .map((v) =>
        Math.round(v * 255)
          .toString(16)
          .padStart(2, "0"),
      )
      .join("")
      .toUpperCase();

  function tap(id: number, event: MouseEvent) {
    const mode = event.shiftKey || event.ctrlKey || event.metaKey ? "toggle" : tapMode("toggle");
    show.program(() => api.select([id], mode));
  }
</script>

{#if rows.length === 0}
  <p class="empty muted">Nothing in the programmer. Values you set on the selected fixtures show up here.</p>
{:else}
  <table>
    <thead>
      <tr>
        <th class="fixture">Fixture</th>
        {#if has("intensity")}<th>Intensity %</th>{/if}
        {#if has("colour")}<th>Colour</th>{/if}
        {#if has("pan")}<th>Pan %</th>{/if}
        {#if has("tilt")}<th>Tilt %</th>{/if}
        {#each other as a (a)}<th>{attributeName(a)}</th>{/each}
      </tr>
    </thead>
    <tbody>
      {#each rows as r (r.id)}
        <tr class:selected={selected.has(r.id)} onclick={(e) => tap(r.id, e)}>
          <td class="fixture">
            <span class="num" style:color={fixtureColour(r.id)}>{r.id}</span>
            {r.fixture?.name ?? "Unpatched"}
          </td>
          {#if has("intensity")}<td>{pct(r.values.intensity)}</td>{/if}
          {#if has("colour")}
            <td>
              {#if r.values.colour}
                <span class="colour"
                  ><span class="chip" style:background={rgbCss(r.values.colour)}></span>{hex(r.values.colour)}</span
                >
              {/if}
            </td>
          {/if}
          {#if has("pan")}<td>{pct(r.values.pan)}</td>{/if}
          {#if has("tilt")}<td>{pct(r.values.tilt)}</td>{/if}
          {#each other as a (a)}<td>{dmx(r.values.other?.[a])}</td>{/each}
        </tr>
      {/each}
    </tbody>
  </table>
{/if}

<style>
  .empty {
    margin: 0;
  }

  table {
    width: 100%;
    border-collapse: collapse;
    font-variant-numeric: tabular-nums;
  }

  th,
  td {
    padding: 0 12px;
    height: 48px;
    text-align: right;
    white-space: nowrap;
    border-bottom: 1px solid var(--line);
  }

  th {
    position: sticky;
    top: 0;
    background: var(--panel);
    font-size: 0.8rem;
    font-weight: 600;
    color: var(--muted);
    height: 36px;
  }

  .fixture {
    text-align: left;
  }

  tbody tr {
    cursor: pointer;
  }

  tbody tr.selected {
    background: color-mix(in srgb, var(--accent) 16%, transparent);
  }

  td {
    color: var(--accent);
  }

  td.fixture {
    color: var(--text);
  }

  .num {
    display: inline-block;
    min-width: 32px;
    font-weight: 700;
  }

  .colour {
    display: inline-flex;
    align-items: center;
    gap: 8px;
  }

  .chip {
    width: 20px;
    height: 20px;
    border-radius: 5px;
    box-shadow: inset 0 0 0 1px rgb(255 255 255 / 0.15);
  }
</style>
