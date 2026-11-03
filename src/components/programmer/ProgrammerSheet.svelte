<script lang="ts">
  // Every value in the programmer, a row per fixture and a column per
  // attribute, like a spreadsheet. Tap a fixture number to pick its row, or a
  // value to pick just that value, then Delete takes them out of the
  // programmer. Tapping a column heading picks the whole column.

  import { attributeName, fixtureColour, rgbCss } from "../../lib/format";
  import { hasValue, sheetPicks, type Column } from "../../lib/sheet.svelte";
  import { show } from "../../lib/show.svelte";
  import type { Attribute, Rgb, Values } from "../../lib/types";

  const rows = $derived(
    Object.entries(show.programmer.values)
      .map(([id, values]) => ({ id: Number(id), values, fixture: show.fixture(Number(id)) }))
      .sort((a, b) => a.id - b.id),
  );
  const ids = $derived(rows.map((r) => r.id));

  const other = $derived.by(() => {
    const found = new Set<Attribute>();
    for (const r of rows) for (const a of Object.keys(r.values.other ?? {})) found.add(a as Attribute);
    return [...found].sort();
  });

  const columns = $derived.by(() => {
    const has = (column: Column) => rows.some((r) => hasValue(r.values, column));
    const all: { column: Column; label: string; band: string }[] = [
      { column: "intensity", label: "Intensity %", band: "f-i" },
      { column: "colour", label: "Colour", band: "f-c" },
      { column: "pan", label: "Pan %", band: "f-p" },
      { column: "tilt", label: "Tilt %", band: "f-p" },
      ...other.map((a) => ({ column: `o:${a}` as Column, label: attributeName(a), band: "f-b" })),
    ];
    return all.filter((c) => has(c.column));
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

  function text(values: Values, column: Column): string {
    if (column === "intensity" || column === "pan" || column === "tilt") return pct(values[column]);
    if (column.startsWith("o:")) return dmx(values.other?.[column.slice(2) as Attribute]);
    return "";
  }

  function tapCell(id: number, column: Column, values: Values) {
    if (hasValue(values, column)) sheetPicks.toggleCell(id, column);
  }
</script>

{#if show.programmer.effects.length > 0}
  <p class="effects">
    Effects:
    {#each show.programmer.effects as e, i (e.id)}{i > 0 ? ", " : ""}<strong>{e.name}</strong> on {e.fixtures.length}{/each}
  </p>
{/if}
{#if rows.length === 0}
  <p class="empty muted">
    {show.programmer.effects.length ? "No values" : "Nothing in the programmer"}. Values you set on the selected fixtures show up here.
  </p>
{:else}
  <table>
    <thead>
      <tr>
        <th class="fixture" onclick={() => sheetPicks.toggleAllRows(ids)}>Fixture</th>
        {#each columns as c (c.column)}
          <th class={c.band} onclick={() => sheetPicks.toggleColumn(ids, c.column)}>{c.label}</th>
        {/each}
      </tr>
    </thead>
    <tbody>
      {#each rows as r (r.id)}
        <tr class:selected={selected.has(r.id)} class:picked={sheetPicks.rowPicked(r.id)}>
          <td class="fixture" onclick={() => sheetPicks.toggleRow(r.id)}>
            <span class="num" style:color={fixtureColour(r.id)}>{r.id}</span>
            {r.fixture?.name ?? "Unpatched"}
          </td>
          {#each columns as c (c.column)}
            <td
              class:picked={hasValue(r.values, c.column) && sheetPicks.cellPicked(r.id, c.column)}
              onclick={() => tapCell(r.id, c.column, r.values)}
            >
              {#if c.column === "colour"}
                {#if r.values.colour}
                  <span class="colour"
                    ><span class="chip" style:background={rgbCss(r.values.colour)}></span>{hex(r.values.colour)}</span
                  >
                {/if}
              {:else}
                {text(r.values, c.column)}
              {/if}
            </td>
          {/each}
        </tr>
      {/each}
    </tbody>
  </table>
{/if}

<style>
  .empty {
    margin: 0;
  }

  .effects {
    margin: 0 0 8px;
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
    letter-spacing: 0.06em;
    text-transform: uppercase;
  }

  /* Column heads carry their attribute colour, as on the window bands. */
  th.f-i {
    color: var(--f-intensity);
    box-shadow: inset 0 -3px 0 var(--f-intensity);
  }
  th.f-c {
    color: var(--f-colour);
    box-shadow: inset 0 -3px 0 var(--f-colour);
  }
  th.f-p {
    color: var(--f-position);
    box-shadow: inset 0 -3px 0 var(--f-position);
  }
  th.f-b {
    color: var(--f-beam);
    box-shadow: inset 0 -3px 0 var(--f-beam);
  }

  .fixture {
    text-align: left;
  }

  th,
  tbody td {
    cursor: pointer;
    user-select: none;
  }

  tbody tr.selected {
    background: color-mix(in srgb, var(--accent) 10%, transparent);
  }

  /* Picked for deleting. */
  td.picked {
    background: color-mix(in srgb, var(--danger) 22%, transparent);
    box-shadow: inset 0 0 0 2px var(--danger);
  }

  tr.picked td.fixture {
    background: color-mix(in srgb, var(--danger) 22%, transparent);
    box-shadow: inset 0 0 0 2px var(--danger);
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
