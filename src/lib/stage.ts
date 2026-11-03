// The stage plan and mover calibration, as kept by the Rust side
// (crates/humlx-core/src/stage.rs), plus the bits of its maths the plan needs
// to draw where each beam lands.

import type { FixtureOptions } from "./types";

/** Metres; origin at the front edge of the stage in the middle, x to the right from the desk, y up stage. */
export interface Mark {
  id: number;
  name: string;
  x: number;
  y: number;
  /** Height above the stage floor: 0 on the stage, minus the stage height on the studio floor. */
  z: number;
}

export interface Reading {
  mark: number;
  pan: number;
  tilt: number;
}

export interface MarkError {
  mark: number;
  metres: number;
}

export interface Solution {
  position: [number, number, number];
  /** Rows of the matrix turning the mover's own directions into stage directions. */
  rotation: [number, number, number][];
  panRange: number;
  tiltRange: number;
  errors: MarkError[];
}

export interface Calibration {
  fixture: number;
  readings: Reading[];
  solution?: Solution | null;
}

export interface Stage {
  width: number;
  depth: number;
  /** Studio floor shown round the stage, in metres each side. */
  margin: number;
  /** Height of the stage floor above the studio floor. */
  riser: number;
  marks: Mark[];
  movers: Calibration[];
}

export type Plan = Omit<Stage, "movers">;

/** Readings needed before a mover can be worked out. */
export const MIN_READINGS = 4;

export function defaultMarks(width: number, depth: number): Mark[] {
  const [l, r] = [-width / 2, width / 2];
  return (
    [
      ["Front left", l, 0],
      ["Front right", r, 0],
      ["Back left", l, depth],
      ["Back right", r, depth],
      ["Centre", 0, depth / 2],
    ] as const
  ).map(([name, x, y], i) => ({ id: i + 1, name, x, y, z: 0 }));
}

export function defaultStage(): Stage {
  return { width: 8, depth: 6, margin: 2, riser: 0, marks: defaultMarks(8, 6), movers: [] };
}

export function onStage(stage: Plan, x: number, y: number) {
  return Math.abs(x) <= stage.width / 2 && y >= 0 && y <= stage.depth;
}

/** Height of the floor at a spot: the stage floor on the stage, the studio floor round it. */
export function floorAt(stage: Plan, x: number, y: number) {
  return onStage(stage, x, y) ? 0 : -stage.riser;
}

/** Programmer pan and tilt as the fixture gets them, after its patch orientation. */
export function orient(o: FixtureOptions, pan: number, tilt: number): [number, number] {
  let [p, t] = o.swapPanTilt ? [tilt, pan] : [pan, tilt];
  if (o.invertPan) p = 1 - p;
  if (o.invertTilt) t = 1 - t;
  p += o.panOffset / Math.max(1, o.panRange);
  t += o.tiltOffset / Math.max(1, o.tiltRange);
  const clamp = (v: number) => Math.max(0, Math.min(1, v));
  return [clamp(p), clamp(t)];
}

/** The other way round from `orient`. */
export function unorient(o: FixtureOptions, pan: number, tilt: number): [number, number] {
  let p = pan - o.panOffset / Math.max(1, o.panRange);
  let t = tilt - o.tiltOffset / Math.max(1, o.tiltRange);
  if (o.invertPan) p = 1 - p;
  if (o.invertTilt) t = 1 - t;
  if (o.swapPanTilt) [p, t] = [t, p];
  const clamp = (v: number) => Math.max(0, Math.min(1, v));
  return [clamp(p), clamp(t)];
}

const rad = (deg: number) => (deg * Math.PI) / 180;

/** Beam direction on stage for pan and tilt as the fixture gets them. */
export function beam(s: Solution, pan: number, tilt: number): [number, number, number] {
  const p = (pan - 0.5) * rad(s.panRange);
  const t = (tilt - 0.5) * rad(s.tiltRange);
  const local = [Math.sin(t) * Math.cos(p), Math.sin(t) * Math.sin(p), Math.cos(t)];
  return s.rotation.map((row) => row[0] * local[0] + row[1] * local[1] + row[2] * local[2]) as [number, number, number];
}

/**
 * Where the beam comes down to `height` above the floor under it (the stage
 * floor first, then the studio floor), or null when it never does.
 */
export function meets(
  stage: Plan,
  s: Solution,
  pan: number,
  tilt: number,
  height = 0,
): [number, number, number] | null {
  const d = beam(s, pan, tilt);
  const [px, py, pz] = s.position;
  if (Math.abs(d[2]) < 1e-9) return null;
  for (const floor of [0, -stage.riser]) {
    const z = floor + height;
    const t = (z - pz) / d[2];
    if (t <= 0) continue;
    const x = px + d[0] * t;
    const y = py + d[1] * t;
    if (floorAt(stage, x, y) === floor) return [x, y, z];
  }
  return null;
}

/** Where the beam meets the floor, or null when it points up and away. */
export function landing(stage: Plan, s: Solution, pan: number, tilt: number): [number, number] | null {
  const at = meets(stage, s, pan, tilt);
  return at && [at[0], at[1]];
}

/** The end of the beam to draw: where it lands, or a long way off when it never does. */
export function beamEnd(stage: Plan, s: Solution, pan: number, tilt: number): { at: [number, number, number]; lands: boolean } {
  const at = meets(stage, s, pan, tilt);
  if (at) return { at, lands: true };
  const d = beam(s, pan, tilt);
  return { at: s.position.map((p, i) => p + d[i] * 40) as [number, number, number], lands: false };
}

/** Pan and tilt (as the fixture gets them) that point at a spot; used by the browser preview. */
export function aim(s: Solution, target: [number, number, number], current: [number, number]): [number, number] {
  const to = target.map((v, i) => v - s.position[i]);
  const len = Math.hypot(...to) || 1;
  const dir = to.map((v) => v / len);
  // Transpose of the rotation brings the direction into the mover's frame.
  const local = [0, 1, 2].map((j) => s.rotation[0][j] * dir[0] + s.rotation[1][j] * dir[1] + s.rotation[2][j] * dir[2]);
  const tilt = Math.acos(Math.max(-1, Math.min(1, local[2])));
  const pan = Math.atan2(local[1], local[0]);
  const [pr, tr] = [rad(s.panRange), rad(s.tiltRange)];
  let best: [number, number, number] | null = null;
  for (const [p0, t0] of [
    [pan, tilt],
    [pan + Math.PI, -tilt],
  ])
    for (let k = -3; k <= 3; k++) {
      const p = 0.5 + (p0 + k * 2 * Math.PI) / pr;
      const t = 0.5 + t0 / tr;
      if (p < 0 || p > 1 || t < 0 || t > 1) continue;
      const travel = ((p - current[0]) * pr) ** 2 + ((t - current[1]) * tr) ** 2;
      if (!best || travel < best[0]) best = [travel, p, t];
    }
  return best ? [best[1], best[2]] : current;
}
