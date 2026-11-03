import type { Channel, Rgb } from "./types";

// Mirrors nearest_wheel_slot in the engine (crates/humlx-core/src/programmer.rs),
// so the Faders view shows the slot the picked colour sends a colour wheel to.
export const NAMED: [string, Rgb][] = [
  ["red", { r: 1, g: 0, b: 0 }],
  ["orange", { r: 1, g: 0.5, b: 0 }],
  ["amber", { r: 1, g: 0.75, b: 0 }],
  ["yellow", { r: 1, g: 1, b: 0 }],
  ["lime", { r: 0.6, g: 1, b: 0 }],
  ["green", { r: 0, g: 1, b: 0 }],
  ["cyan", { r: 0, g: 1, b: 1 }],
  ["light blue", { r: 0.5, g: 0.7, b: 1 }],
  ["blue", { r: 0, g: 0, b: 1 }],
  ["uv", { r: 0.4, g: 0, b: 1 }],
  ["purple", { r: 0.6, g: 0, b: 1 }],
  ["magenta", { r: 1, g: 0, b: 1 }],
  ["pink", { r: 1, g: 0.4, b: 0.7 }],
  ["open", { r: 1, g: 1, b: 1 }],
  ["white", { r: 1, g: 1, b: 1 }],
];

/** The DMX value (0 to 255) of the wheel slot closest to `target`, if any slot is named. */
export function nearestWheelSlot(channel: Channel, target: Rgb): number | undefined {
  let best: { distance: number; value: number } | undefined;
  for (const capability of channel.capabilities ?? []) {
    const label = capability.label.toLowerCase();
    // Skip ranges like "Rainbow" or "Red to blue scroll" that are not a single slot.
    if (label.includes(" to ") || label.includes("rainbow") || label.includes("scroll")) continue;
    const named = NAMED.find(([name]) => label.includes(name));
    if (!named) continue;
    const [, c] = named;
    const distance = (c.r - target.r) ** 2 + (c.g - target.g) ** 2 + (c.b - target.b) ** 2;
    if (!best || distance < best.distance) {
      best = { distance, value: Math.floor((capability.from + capability.to) / 2) };
    }
  }
  return best?.value;
}
