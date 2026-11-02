import type { Attribute, Direction, EffectFamily, Pattern, Rgb, Shape } from "../../lib/types";

/** Each Programmer window that can run effects, and the family they move. */
export const EFFECT_WINDOWS: Record<string, EffectFamily> = {
  intensity: "intensity",
  colour: "colour",
  position: "position",
  beam: "beam",
};

export const FAMILY_LABELS: Record<EffectFamily, string> = {
  intensity: "intensity",
  colour: "colour",
  position: "position",
  beam: "beam",
};

interface ShapeInfo {
  label: string;
  hint: string;
  family: EffectFamily;
  /** The shape's own fade between steps, from 0 (snap) to 1. */
  transition: number;
  /** How much of each cycle the shape's steps take, if not all of it. */
  width?: number;
  /** Whether the shape's colours are picked by hand, and how many it can have. */
  colours?: { min: number; max: number };
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
  burst: {
    label: "Strobe burst",
    hint: "A quick burst of flashes, then dark",
    family: "intensity",
    transition: 0,
    width: 0.5,
    icon: "M2 18V2H4V18H6V2H8V18H10V2H12V18H26",
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
    colours: { min: 2, max: 2 },
    icon: "M3 5H12V15H3ZM16 5H25V15H16Z",
  },
  steps: {
    label: "Colour steps",
    hint: "Steps through colours you pick",
    family: "colour",
    transition: 0,
    colours: { min: 2, max: 8 },
    icon: "M2 6H8V14H2ZM11 6H17V14H11ZM20 6H26V14H20Z",
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
  fan: {
    label: "Fan",
    hint: "Spreads out from the middle and back",
    family: "position",
    transition: 1,
    icon: "M14 18L3 3M14 18L8.5 2M14 18V2M14 18L19.5 2M14 18L25 3",
  },
  zoomPulse: {
    label: "Zoom pulse",
    hint: "Zoom opens and closes",
    family: "beam",
    transition: 1,
    icon: "M2 10L12 5V15ZM26 10L16 3V17Z",
  },
  irisPulse: {
    label: "Iris pulse",
    hint: "Iris opens and closes",
    family: "beam",
    transition: 1,
    icon: "M14 2A8 8 0 1 1 13.9 2ZM14 6.5A3.5 3.5 0 1 1 13.9 6.5Z",
  },
};

export const shapesOf = (family: EffectFamily) =>
  (Object.keys(SHAPES) as Shape[]).filter((s) => SHAPES[s].family === family);

export const PATTERNS: Record<Pattern, { label: string; hint: string }> = {
  inOrder: {
    label: "In order",
    hint: "One after another, in the order picked",
  },
  mirror: { label: "Mirror", hint: "From both ends to the middle" },
  pairs: { label: "Pairs", hint: "Two at a time" },
  oddEven: { label: "Odd/Even", hint: "Odd fixtures against even ones" },
};

// --- Preview ---------------------------------------------------------------
// A copy of the engine's effect maths (crates/humlx-core/src/effect.rs), so
// the effect dialog can show an effect moving before it is made.

/** The values one step of an effect sets. */
export interface Step {
  intensity?: number;
  colour?: Rgb;
  pan?: number;
  tilt?: number;
  other?: Partial<Record<Attribute, number>>;
}

export interface PreviewEffect {
  steps: Step[];
  layering: "onTop" | "replace";
  curve: "smooth" | "linear";
  wrap: boolean;
  fan: boolean;
  transition: number;
  width: number;
  direction: Direction;
  pattern: Pattern;
  rate: number;
  size: number;
  spread: number;
}

const PATH_STEPS = 32;

/** The steps and settings a shape starts with, as the engine makes them. */
export function recipe(
  shape: Shape,
  colours: Rgb[] = [],
): Pick<PreviewEffect, "steps" | "layering" | "curve" | "wrap" | "fan"> {
  const level = (intensity: number): Step => ({ intensity });
  const path = (f: (a: number) => [number, number]) =>
    Array.from({ length: PATH_STEPS }, (_, i) => {
      const [pan, tilt] = f((2 * Math.PI * i) / PATH_STEPS);
      return { pan: 0.5 + 0.5 * pan, tilt: 0.5 + 0.5 * tilt };
    });
  const base = {
    layering: "onTop" as const,
    curve: "smooth" as const,
    wrap: true,
    fan: false,
  };
  switch (shape) {
    case "pulse":
      return { ...base, steps: [level(0), level(1)] };
    case "chase":
      return { ...base, steps: [level(1), level(0)] };
    case "rampUp":
      return {
        ...base,
        steps: [level(0), level(1)],
        curve: "linear",
        wrap: false,
      };
    case "rampDown":
      return {
        ...base,
        steps: [level(1), level(0)],
        curve: "linear",
        wrap: false,
      };
    case "flicker":
      return {
        ...base,
        steps: [1, 0.35, 0.8, 0.15, 0.95, 0.5, 0.7, 0.25].map(level),
        curve: "linear",
      };
    case "burst":
      return { ...base, steps: [1, 0, 1, 0, 1, 0].map(level) };
    case "rainbow":
      return {
        ...base,
        layering: "replace",
        curve: "linear",
        steps: [
          [1, 0, 0],
          [1, 1, 0],
          [0, 1, 0],
          [0, 1, 1],
          [0, 0, 1],
          [1, 0, 1],
        ].map(([r, g, b]) => ({ colour: { r, g, b } })),
      };
    case "swap":
      return {
        ...base,
        layering: "replace",
        steps: [{ colour: colours[0] ?? { r: 1, g: 0, b: 0 } }, { colour: colours[1] ?? { r: 0, g: 0, b: 1 } }],
      };
    case "steps":
      return {
        ...base,
        layering: "replace",
        steps: (colours.length >= 2 ? colours : STEP_COLOURS).map((colour) => ({
          colour,
        })),
      };
    case "circle":
      return {
        ...base,
        curve: "linear",
        steps: path((a) => [Math.cos(a), Math.sin(a)]),
      };
    case "figure8":
      return {
        ...base,
        curve: "linear",
        steps: path((a) => [Math.sin(a), Math.sin(2 * a)]),
      };
    case "panSweep":
      return { ...base, steps: [{ pan: 0 }, { pan: 1 }] };
    case "tiltNod":
      return { ...base, steps: [{ tilt: 0 }, { tilt: 1 }] };
    case "fan":
      return { ...base, fan: true, steps: [{ pan: 0.5 }, { pan: 1 }] };
    case "zoomPulse":
      return {
        ...base,
        steps: [{ other: { zoom: 0 } }, { other: { zoom: 1 } }],
      };
    case "irisPulse":
      return {
        ...base,
        steps: [{ other: { iris: 0 } }, { other: { iris: 1 } }],
      };
  }
}

/** The colours colour steps start with. */
export const STEP_COLOURS: Rgb[] = [
  { r: 1, g: 0, b: 0 },
  { r: 0, g: 1, b: 0 },
  { r: 0, g: 0, b: 1 },
];

const mix = (a: number | undefined, b: number | undefined, k: number) =>
  a != null && b != null ? a + (b - a) * k : (a ?? b);

/** The step values at `phase` cycles into the effect. */
export function sample(e: PreviewEffect, phase: number): Step {
  const n = e.steps.length;
  if (n < 2) return e.steps[0] ?? {};
  const wrap1 = (x: number) => ((x % 1) + 1) % 1;
  let p = wrap1(phase);
  if (e.direction === "backward") p = wrap1(1 - p);
  else if (e.direction === "bounce") p = 1 - Math.abs(2 * p - 1);
  const width = Math.min(Math.max(e.width, 0.01), 1);
  p = p < width ? p / width : 1;
  const segments = e.wrap ? n : n - 1;
  const x = p * segments;
  const i = Math.min(Math.floor(x), segments - 1);
  const f = x - i;
  const t = Math.min(Math.max(e.transition, 0), 1);
  let k = t <= 0 || f < 1 - t ? 0 : Math.min((f - (1 - t)) / t, 1);
  if (e.curve === "smooth") k = 0.5 - 0.5 * Math.cos(k * Math.PI);
  const a = e.steps[i];
  const b = e.steps[(i + 1) % n];
  const colour =
    a.colour && b.colour
      ? {
          r: mix(a.colour.r, b.colour.r, k)!,
          g: mix(a.colour.g, b.colour.g, k)!,
          b: mix(a.colour.b, b.colour.b, k)!,
        }
      : (a.colour ?? b.colour);
  const keys = new Set([...Object.keys(a.other ?? {}), ...Object.keys(b.other ?? {})]) as Set<Attribute>;
  const other = keys.size
    ? Object.fromEntries([...keys].map((key) => [key, mix(a.other?.[key], b.other?.[key], k) ?? 0]))
    : undefined;
  return {
    intensity: mix(a.intensity, b.intensity, k),
    colour,
    pan: mix(a.pan, b.pan, k),
    tilt: mix(a.tilt, b.tilt, k),
    other,
  };
}

/** Which slot fixture `index` of `count` takes in a pattern, and how many slots there are. */
function slot(pattern: Pattern, index: number, count: number): [number, number] {
  const c = Math.max(count, 1);
  switch (pattern) {
    case "inOrder":
      return [index, c];
    case "mirror":
      return [Math.min(index, c - 1 - index), Math.ceil(c / 2)];
    case "pairs":
      return [Math.floor(index / 2), Math.ceil(c / 2)];
    case "oddEven":
      return [index % 2, Math.min(c, 2)];
  }
}

/** What fixture `index` of `count` shows at `elapsed` seconds, on a full, white, centred base. */
export function previewAt(e: PreviewEffect, index: number, count: number, elapsed: number) {
  const [s, slots] = slot(e.pattern, index, count);
  const step = sample(e, elapsed * e.rate - (Math.min(Math.max(e.spread, 0), 1) * s) / slots);
  const size = Math.min(Math.max(e.size, 0), 1);
  // How far, and which way, this fixture moves in a fan.
  const fan = e.fan && count > 1 ? (2 * index) / (count - 1) - 1 : 1;
  const clamp = (v: number) => Math.min(Math.max(v, 0), 1);
  const move = (v: number | undefined, reach = 0.5) => {
    if (v == null) return 0.5;
    const s = 0.5 + (v - 0.5) * fan;
    return clamp(e.layering === "onTop" ? 0.5 + (s - 0.5) * size * reach : 0.5 + (s - 0.5) * size);
  };
  const c = step.colour;
  const beam = Object.values(step.other ?? {})[0];
  return {
    level: 1 - size * (1 - (step.intensity ?? 1)),
    colour: c
      ? {
          r: 1 + (c.r - 1) * size,
          g: 1 + (c.g - 1) * size,
          b: 1 + (c.b - 1) * size,
        }
      : { r: 1, g: 1, b: 1 },
    pan: move(step.pan),
    tilt: move(step.tilt),
    beam: move(beam, 1),
  };
}

/**
 * Where fixture `index` of `count` points over one cycle of a position
 * effect, around `base`, as the engine moves it. For drawing the path.
 */
export function pathOf(
  e: Pick<
    PreviewEffect,
    "steps" | "layering" | "curve" | "wrap" | "fan" | "transition" | "width" | "direction" | "size"
  >,
  index: number,
  count: number,
  base: { pan?: number; tilt?: number },
  points = 64,
) {
  const fan = e.fan && count > 1 ? (2 * index) / (count - 1) - 1 : 1;
  const size = Math.min(Math.max(e.size, 0), 1);
  const move = (b: number | undefined, v: number | undefined) => {
    if (v == null) return b ?? 0.5;
    const s = 0.5 + (v - 0.5) * fan;
    const out = e.layering === "onTop" ? (b ?? 0.5) + (s - 0.5) * size * 0.5 : 0.5 + (s - 0.5) * size;
    return Math.min(Math.max(out, 0), 1);
  };
  const full = { ...e, pattern: "inOrder" as const, rate: 1, spread: 0 };
  return Array.from({ length: points }, (_, i) => {
    const step = sample(full, i / points);
    return { pan: move(base.pan, step.pan), tilt: move(base.tilt, step.tilt) };
  });
}
