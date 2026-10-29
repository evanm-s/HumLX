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
