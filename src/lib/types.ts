// Shapes of the data the Rust side sends. Keep in step with
// `src-tauri/src/commands.rs` and `crates/humlx-core`.

export type Attribute =
  | "intensity"
  | "red"
  | "green"
  | "blue"
  | "white"
  | "warm_white"
  | "cool_white"
  | "amber"
  | "lime"
  | "uv"
  | "cyan"
  | "magenta"
  | "yellow"
  | "color_wheel"
  | "color_temperature"
  | "pan"
  | "tilt"
  | "pan_tilt_speed"
  | "shutter"
  | "gobo"
  | "gobo_rotation"
  | "prism"
  | "zoom"
  | "focus"
  | "iris"
  | "frost"
  | "effect"
  | "speed"
  | "control"
  | "generic";

export interface Capability {
  from: number;
  to: number;
  label: string;
}

export interface Channel {
  name: string;
  attribute: Attribute;
  fineOf?: string;
  default: number;
  highlight?: number;
  capabilities?: Capability[];
}

export interface Mode {
  name: string;
  channels: Channel[];
}

export interface Profile {
  id: string;
  manufacturer: string;
  model: string;
  categories?: string[];
  modes: Mode[];
}

export interface Fixture {
  id: number;
  name: string;
  profile: string;
  mode: string;
  address: number;
  footprint: number;
  typeName: string;
}

export interface PatchView {
  fixtures: Fixture[];
  /** Owner of each channel; index 0 is address 1. */
  occupancy: (number | null)[];
}

export interface OutputSettings {
  enabled: boolean;
  target: string;
  universe: number;
  rateHz: number;
}

export interface OutputStatus {
  packetsSent: number;
  lastError: string | null;
}

export interface Overview {
  profiles: Profile[];
  patch: PatchView;
  output: OutputSettings;
  warnings: string[];
}

export interface Live {
  universe: number[];
  status: OutputStatus;
  blackout: boolean;
  /** Grand master level from 0 to 1, scaling all intensity. */
  grandMaster: number;
  /** Selected fixtures picked out, everything else out. */
  highlight: boolean;
  tested: number[];
  playbacks: PlaybackStatus[];
  /** Master level by cue list number. */
  masters: [number, number][];
  /** Tempo of speed masters A and B, which OSC can change too. */
  speedBpm: [number, number];
  /** Beats counted by speed masters A and B. */
  beats: [number, number];
  osc: OscStatus;
}

export interface OscSettings {
  enabled: boolean;
  port: number;
}

export interface OscStatus {
  listening: boolean;
  error: string | null;
  received: number;
  last: string | null;
}

export interface NewFixtures {
  profile: string;
  mode: string;
  count: number;
  firstId?: number | null;
  address?: number | null;
  name?: string | null;
}

export interface FixtureChanges {
  id?: number;
  name?: string;
  mode?: string;
  address?: number;
}

export interface FixtureUpdate {
  /** The fixture's current number. */
  id: number;
  changes: FixtureChanges;
}

export interface Node {
  ip: string;
  shortName: string;
  longName: string;
  outputs: number[];
}

export type Family = "intensity" | "colour" | "position" | "beam" | "control";

export interface Rgb {
  r: number;
  g: number;
  b: number;
}

export interface Values {
  intensity?: number;
  colour?: Rgb;
  pan?: number;
  tilt?: number;
  other?: Partial<Record<Attribute, number>>;
}

export interface Group {
  id: number;
  name: string;
  fixtures: number[];
}

export interface Preset {
  id: number;
  name: string;
  family: Family;
  all?: Values;
  fixtures?: Record<string, Values>;
  /** Effects the preset puts on the selected fixtures. */
  effects?: Effect[];
}

export interface ProgrammerView {
  selection: number[];
  values: Record<string, Values>;
  /** Effects being built, which record into cues with the values. */
  effects: Effect[];
  groups: Group[];
  presets: Preset[];
}

export type SelectMode = "set" | "toggle" | "add";

/** Hold waits for GO; follow goes to the next cue by itself. */
export type CueMode = "hold" | "follow";

export interface Cue {
  name: string;
  fade: number;
  mode: CueMode;
  /** For a follow cue, seconds from GO until the next cue starts. */
  follow: number;
  values: Record<string, Values>;
  /** Effects this cue starts; they run on into later cues until replaced. */
  effects?: Effect[];
}

export interface CueList {
  id: number;
  name: string;
  cues: Cue[];
}

/** Where a recorded cue goes: an existing cue list, or a new one with this name. */
export type RecordTarget =
  | { list: number }
  | { new: string }
  /** A new one-cue list (a look) on a grid button, named or "Look N". */
  | { button: { page: number; button: number; name: string } };

/** The Programmer families an effect can move. */
export type EffectFamily = "intensity" | "colour" | "position" | "beam";
export type Shape =
  | "pulse"
  | "chase"
  | "rampUp"
  | "rampDown"
  | "flicker"
  | "burst"
  | "rainbow"
  | "swap"
  | "steps"
  | "circle"
  | "figure8"
  | "panSweep"
  | "tiltNod"
  | "fan"
  | "zoomPulse"
  | "irisPulse";
/** How fixtures are spaced out in an effect, in the order they were picked. */
export type Pattern = "inOrder" | "mirror" | "pairs" | "oddEven";
export type Direction = "forward" | "backward" | "bounce";
/** Where an effect takes its speed from: its own rate, or speed master A or B. */
export type Speed = "own" | "a" | "b";

export interface EffectSettings {
  name: string;
  shape: Shape;
  /** Cycles per second. */
  rate: number;
  size: number;
  spread: number;
  pattern: Pattern;
  direction: Direction;
  /** How much of each cycle the steps take, 0 to 1. */
  width: number;
  speed: Speed;
  /** Cycles per beat when following a speed master. */
  multiplier: number;
  /** How much of each step fades to the next, 0 (snap) to 1; the shape's own if left out. */
  transition?: number;
  /** The two colours of a swap, or the colours that colour steps step through. */
  colours?: Rgb[];
  /** Steps set by hand in the step editor; the shape's own if left out. */
  steps?: Values[];
  /** Whether the effect works from the look underneath; the shape's own if left out. */
  layering?: "onTop" | "replace";
}

export interface Effect extends EffectSettings {
  id: number;
  fixtures: number[];
  /** The values the fixtures run through. */
  steps: Values[];
  layering: "onTop" | "replace";
  curve: "smooth" | "linear";
  /** Whether the last step fades back to the first. */
  wrap: boolean;
  /** Whether each fixture moves a different way along the fixture order. */
  fan: boolean;
  transition: number;
}

export interface PlaybackView {
  cueLists: CueList[];
  /** The master that scales every effect, 0 to 1. */
  effectSize: number;
  /** Tempo of speed masters A and B. */
  speedBpm: [number, number];
  masters: [number, number][];
  layout: Layout;
}

/** Grid size and number of fader slots on the Playback screen. */
export const GRID_COLUMNS = 8;
export const GRID_ROWS = 6;
export const SLOTS = 8;

export interface GridPage {
  name: string;
  /** The cue list on each button, row by row, or null. */
  buttons: (number | null)[];
}

/** Where cue lists sit on the Playback screen, and which is selected. */
export interface Layout {
  pages: GridPage[];
  /** The cue list in each fader slot, or null. */
  slots: (number | null)[];
  /** The list that GO, Back, the NX-K's Next and Last and Record act on. */
  selected: number | null;
}

export function emptyLayout(): Layout {
  return {
    pages: [{ name: "", buttons: Array(GRID_COLUMNS * GRID_ROWS).fill(null) }],
    slots: Array(SLOTS).fill(null),
    selected: null,
  };
}

export interface PlaybackStatus {
  list: number;
  current: number;
  fading: boolean;
  /** Seconds until the next cue follows, when the current cue is a follow cue. */
  followIn: number | null;
}

/** Whether the NX-K control surface is plugged in and usable. */
export interface SurfaceStatus {
  connected: boolean;
  /** Why it can't be used, when it is plugged in but not working. */
  problem: string | null;
}

/** How an NX-K key LED is lit. */
export type Led = "off" | "on" | "blink";
