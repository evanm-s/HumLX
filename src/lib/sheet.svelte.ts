// What's picked on the programmer sheet for deleting: whole fixture rows and
// single values. Tap to pick, then Delete (key or button) takes them out of
// the programmer. Kept apart from the fixture selection so Delete never
// removes something that wasn't picked on the sheet.

import { api } from "./api";
import { show } from "./show.svelte";
import type { Attribute, ProgrammerField, Values } from "./types";

/** A sheet column: a built-in value, or another attribute as `o:<attribute>`. */
export type Column = "intensity" | "colour" | "pan" | "tilt" | `o:${Attribute}`;

const field = (column: Column): ProgrammerField =>
  column.startsWith("o:") ? { other: column.slice(2) as Attribute } : (column as ProgrammerField);

/** Whether `values` has a value in `column`. */
export function hasValue(values: Values | undefined, column: Column): boolean {
  if (!values) return false;
  return column.startsWith("o:")
    ? values.other?.[column.slice(2) as Attribute] != null
    : values[column as keyof Values] != null;
}

class SheetPicks {
  rows = $state<number[]>([]);
  /** `<fixture>:<column>` */
  cells = $state<string[]>([]);

  /** Picks that still point at something in the programmer. */
  get live(): { rows: number[]; cells: [number, Column][] } {
    const values = show.programmer.values;
    const rows = this.rows.filter((id) => values[id]);
    const cells = this.cells
      .map((key) => {
        const at = key.indexOf(":");
        return [Number(key.slice(0, at)), key.slice(at + 1) as Column] as [number, Column];
      })
      .filter(([id, column]) => !rows.includes(id) && hasValue(values[id], column));
    return { rows, cells };
  }

  get count(): number {
    const { rows, cells } = this.live;
    return rows.length + cells.length;
  }

  rowPicked(id: number) {
    return this.rows.includes(id);
  }

  cellPicked(id: number, column: Column) {
    return this.rows.includes(id) || this.cells.includes(`${id}:${column}`);
  }

  toggleRow(id: number) {
    if (this.rows.includes(id)) this.rows = this.rows.filter((r) => r !== id);
    else {
      this.rows = [...this.rows, id];
      // The row covers its cells now.
      this.cells = this.cells.filter((key) => !key.startsWith(`${id}:`));
    }
  }

  toggleCell(id: number, column: Column) {
    const key = `${id}:${column}`;
    if (this.rows.includes(id)) {
      // Unpicking one value from a picked row leaves the rest of the row picked.
      this.rows = this.rows.filter((r) => r !== id);
      const others = (Object.keys(show.programmer.values[id] ?? {}) as string[])
        .flatMap((k) =>
          k === "other" ? Object.keys(show.programmer.values[id].other ?? {}).map((a) => `o:${a}`) : [k],
        )
        .filter((c) => c !== column);
      this.cells = [...this.cells, ...others.map((c) => `${id}:${c}`)];
    } else if (this.cells.includes(key)) this.cells = this.cells.filter((k) => k !== key);
    else this.cells = [...this.cells, key];
  }

  /** Picks every row, or unpicks them all when all are picked already. */
  toggleAllRows(ids: number[]) {
    if (ids.length > 0 && ids.every((id) => this.rows.includes(id))) this.rows = [];
    else {
      this.rows = [...ids];
      this.cells = [];
    }
  }

  /** Picks a whole column's values, or unpicks it when it's all picked already. */
  toggleColumn(ids: number[], column: Column) {
    const keys = ids.filter((id) => hasValue(show.programmer.values[id], column)).map((id) => `${id}:${column}`);
    if (keys.every((key) => this.cells.includes(key))) this.cells = this.cells.filter((k) => !keys.includes(k));
    else this.cells = [...new Set([...this.cells, ...keys])];
  }

  clear() {
    this.rows = [];
    this.cells = [];
  }

  /** Takes the picked rows and values out of the programmer. False when nothing was picked. */
  delete(): boolean {
    const { rows, cells } = this.live;
    this.clear();
    if (rows.length === 0 && cells.length === 0) return false;
    show.program(() =>
      api.removeFromProgrammer(
        rows,
        cells.map(([id, column]) => [id, field(column)]),
      ),
    );
    return true;
  }
}

export const sheetPicks = new SheetPicks();
