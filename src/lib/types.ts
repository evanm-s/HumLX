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
export type RecordTarget = { list: number } | { new: string };

/** The Programmer families an effect can move. */
export type EffectFamily = "intensity" | "colour" | "position";
export type Shape =
  | "pulse"
  | "chase"
  | "rampUp"
  | "rampDown"
  | "flicker"
  | "rainbow"
  | "swap"
  | "circle"
  | "figure8"
  | "panSweep"
  | "tiltNod";
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
  /** The two colours of a swap. */
  colours?: Rgb[];
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
  transition: number;
}

export interface PlaybackView {
  cueLists: CueList[];
  /** The master that scales every effect, 0 to 1. */
  effectSize: number;
  /** Tempo of speed masters A and B. */
  speedBpm: [number, number];
  masters: [number, number][];
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
