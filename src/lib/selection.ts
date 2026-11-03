// Tapping fixtures and groups builds a selection without a separate
// deselect step: a tap soon after the previous one adds to the selection,
// and a tap after a pause starts a new selection.

import type { SelectMode } from "./types";

/** How long after a selection tap the next tap still adds to it. */
export const SELECT_WINDOW_MS = 2000;

let lastTap = -Infinity;

/**
 * The select mode for a tap on a fixture or group: `adding` (toggle for
 * fixtures, add for groups) within the window, otherwise "set". Each tap
 * restarts the window.
 */
export function tapMode(adding: SelectMode): SelectMode {
  const now = performance.now();
  const mode = now - lastTap <= SELECT_WINDOW_MS ? adding : "set";
  lastTap = now;
  return mode;
}

/** Count a non-tap selection (e.g. All) as a tap, so taps right after it trim it. */
export function restartSelectWindow() {
  lastTap = performance.now();
}

/** Make the next tap start a new selection, e.g. after None. */
export function endSelectWindow() {
  lastTap = -Infinity;
}

/**
 * The select mode for a tap on a fixture or group whose fixtures are
 * `targets`. Within the window a tap toggles: a target that is wholly
 * selected is deselected, otherwise its missing fixtures are added. After a
 * pause it starts a new selection, except that tapping exactly what is
 * already selected deselects it rather than doing nothing.
 */
export function tapSelectMode(targets: number[], selection: number[]): SelectMode {
  const chosen = new Set(selection);
  const whole = targets.length > 0 && targets.every((id) => chosen.has(id));
  const mode = tapMode(whole ? "toggle" : "add");
  if (mode === "set" && whole && chosen.size === new Set(targets).size) return "toggle";
  return mode;
}
