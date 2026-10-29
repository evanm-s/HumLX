// Window layouts for workspace screens: each window sits on a grid of cells,
// and the layout is remembered on this computer.

export interface Rect {
  x: number;
  y: number;
  w: number;
  h: number;
}

export interface Placement extends Rect {
  hidden?: boolean;
}

export type Layout = Record<string, Placement>;

export interface Grid {
  cols: number;
  rows: number;
}

export function overlaps(a: Rect, b: Rect): boolean {
  return a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h;
}

export function inside(r: Rect, grid: Grid): boolean {
  return r.x >= 0 && r.y >= 0 && r.w >= 1 && r.h >= 1 && r.x + r.w <= grid.cols && r.y + r.h <= grid.rows;
}

/** Whether `rect` is free of every visible window except `id`. */
export function fits(layout: Layout, id: string, rect: Rect, grid: Grid): boolean {
  return inside(rect, grid) && Object.entries(layout).every(([other, p]) => other === id || p.hidden || !overlaps(rect, p));
}

/** The first free spot for a window of this size, scanning row by row, or null. */
export function freeSpot(layout: Layout, id: string, w: number, h: number, grid: Grid): Rect | null {
  for (let y = 0; y + h <= grid.rows; y++) {
    for (let x = 0; x + w <= grid.cols; x++) {
      if (fits(layout, id, { x, y, w, h }, grid)) return { x, y, w, h };
    }
  }
  return null;
}

/** Loads a saved layout, falling back to `defaults` for anything missing or invalid. */
export function loadLayout(key: string, defaults: Layout, grid: Grid): Layout {
  const layout = structuredClone(defaults);
  try {
    const saved = JSON.parse(localStorage.getItem(key) ?? "null") as Layout | null;
    if (saved && typeof saved === "object") {
      for (const id of Object.keys(defaults)) {
        const p = saved[id];
        if (p && inside(p, grid)) layout[id] = { x: p.x, y: p.y, w: p.w, h: p.h, hidden: !!p.hidden };
      }
    }
  } catch {
    // Storage unavailable or corrupt: use the defaults.
  }
  return layout;
}

export function saveLayout(key: string, layout: Layout) {
  try {
    localStorage.setItem(key, JSON.stringify(layout));
  } catch {
    // Not fatal: the layout just isn't remembered.
  }
}
