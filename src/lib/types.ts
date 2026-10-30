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
  runningEffects: number[];
  /** Master level by cue list number. */
  masters: [number, number][];
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
}

export interface ProgrammerView {
  selection: number[];
  values: Record<string, Values>;
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
}

export interface CueList {
  id: number;
  name: string;
  cues: Cue[];
}

/** Where a recorded cue goes: an existing cue list, or a new one with this name. */
export type RecordTarget = { list: number } | { new: string };

export type EffectKind = "dimmer" | "rainbow" | "circle";
export type Wave = "sine" | "square" | "saw";

export interface EffectSettings {
  name: string;
  kind: EffectKind;
  wave: Wave;
  /** Cycles per second. */
  rate: number;
  size: number;
  spread: number;
}

export interface Effect extends EffectSettings {
  id: number;
  fixtures: number[];
}

export interface PlaybackView {
  cueLists: CueList[];
  effects: Effect[];
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
