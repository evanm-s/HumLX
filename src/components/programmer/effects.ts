import type { EffectFamily, Pattern, Shape } from "../../lib/types";

/** Each Programmer window that can run effects, and the family they move. */
export const EFFECT_WINDOWS: Record<string, EffectFamily> = {
  intensity: "intensity",
  colour: "colour",
  position: "position",
};

export const FAMILY_LABELS: Record<EffectFamily, string> = {
  intensity: "intensity",
  colour: "colour",
  position: "position",
};

interface ShapeInfo {
  label: string;
  hint: string;
  family: EffectFamily;
  /** The shape's own fade between steps, from 0 (snap) to 1. */
  transition: number;
  /** An SVG path in a 28 by 20 box that pictures the shape. */
  icon: string;
}

export const SHAPES: Record<Shape, ShapeInfo> = {
  pulse: {
    label: "Pulse",
    hint: "Rises and falls smoothly",
    family: "intensity",
    transition: 1,
    icon: "M2 18C6 18 7 2 10 2S14 18 18 18 22 2 26 2",
  },
  chase: {
    label: "Chase",
    hint: "Snaps on and off",
    family: "intensity",
    transition: 0,
    icon: "M2 2H8V18H14V2H20V18H26",
  },
  rampUp: {
    label: "Ramp up",
    hint: "Fades up, then snaps down",
    family: "intensity",
    transition: 1,
    icon: "M2 18L14 2V18L26 2",
  },
  rampDown: {
    label: "Ramp down",
    hint: "Snaps up, then fades down",
    family: "intensity",
    transition: 1,
    icon: "M2 2L14 18V2L26 18",
  },
  flicker: {
    label: "Flicker",
    hint: "Jumps around like a flame",
    family: "intensity",
    transition: 0.4,
    icon: "M2 4L5 14 8 6 11 17 14 3 17 11 20 8 23 15 26 5",
  },
  rainbow: {
    label: "Rainbow",
    hint: "Cycles round the colour wheel",
    family: "colour",
    transition: 1,
    icon: "M14 2A8 8 0 1 1 6 10M6 10L3 7M6 10L9 7",
  },
  swap: {
    label: "Two colours",
    hint: "Swaps between two colours",
    family: "colour",
    transition: 0,
    icon: "M3 5H12V15H3ZM16 5H25V15H16Z",
  },
  circle: {
    label: "Circle",
    hint: "Traces a circle around the position",
    family: "position",
    transition: 1,
    icon: "M14 2A8 8 0 1 1 13.9 2Z",
  },
  figure8: {
    label: "Figure 8",
    hint: "Traces a figure of eight",
    family: "position",
    transition: 1,
    icon: "M14 10C17 4 24 3 25 10S17 16 14 10 4 4 3 10 11 16 14 10Z",
  },
  panSweep: {
    label: "Pan sweep",
    hint: "Sweeps side to side",
    family: "position",
    transition: 1,
    icon: "M3 10H25M3 10L7 6M3 10L7 14M25 10L21 6M25 10L21 14",
  },
  tiltNod: {
    label: "Tilt nod",
    hint: "Nods up and down",
    family: "position",
    transition: 1,
    icon: "M14 2V18M14 2L10 6M14 2L18 6M14 18L10 14M14 18L18 14",
  },
};

export const shapesOf = (family: EffectFamily) =>
  (Object.keys(SHAPES) as Shape[]).filter((s) => SHAPES[s].family === family);

export const PATTERNS: Record<Pattern, { label: string; hint: string }> = {
  inOrder: { label: "In order", hint: "One after another, in the order picked" },
  mirror: { label: "Mirror", hint: "From both ends to the middle" },
  pairs: { label: "Pairs", hint: "Two at a time" },
  oddEven: { label: "Odd/Even", hint: "Odd fixtures against even ones" },
};
