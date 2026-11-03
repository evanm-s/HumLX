// Typed wrappers around the Rust commands.

import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import type {
  CueMode,
  FixtureChanges,
  FixtureUpdate,
  Live,
  NewFixtures,
  Node,
  OutputSettings,
  Overview,
  PatchView,
  Profile,
  ProgrammerView,
  ProgrammerField,
  Rgb,
  SelectMode,
  Family,
  Attribute,
  PlaybackView,
  EffectSettings,
  RecordTarget,
  OscSettings,
  SurfaceStatus,
  Led,
} from "./types";
import type { Known, Placed, Plan, Stage } from "./stage";
import { link } from "./remote.svelte";
import type { RemoteStatus } from "./types";

/** True when running inside the desktop app rather than a plain browser. */
export const inApp = "__TAURI_INTERNALS__" in window;

/** True in a browser on another computer, driving the Studio PC that served the page. */
export const remote = !inApp && document.querySelector('meta[name="humlx-remote"]') != null;

type Invoke = <T>(command: string, args?: Record<string, unknown>) => Promise<T>;

// In a plain browser (`npm run dev`) there is no Rust side, so a small
// in-memory stand-in is used to preview the UI.
// A remote sends commands to the Studio PC over the network.
let invokeImpl: Invoke | undefined = inApp ? tauriInvoke : remote ? (c, a) => link.invoke(c, a) : undefined;

async function invoke<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  if (!invokeImpl) {
    const { mockInvoke } = await import("./mock");
    invokeImpl = mockInvoke;
  }
  try {
    return await invokeImpl<T>(command, args);
  } catch (error) {
    // Rust errors arrive as plain message strings.
    throw new Error(typeof error === "string" ? error : String(error));
  }
}

export const api = {
  overview: () => invoke<Overview>("overview"),
  addFixtures: (request: NewFixtures) => invoke<PatchView>("add_fixtures", { request }),
  updateFixture: (id: number, changes: FixtureChanges) =>
    invoke<PatchView>("update_fixture", { id, changes }),
  updateFixtures: (updates: FixtureUpdate[]) => invoke<PatchView>("update_fixtures", { updates }),
  removeFixtures: (ids: number[]) => invoke<PatchView>("remove_fixtures", { ids }),
  importProfile: (json: string, manufacturer: string | null) =>
    invoke<Profile[]>("import_profile", { json, manufacturer }),
  setTestLevel: (fixture: number, channel: number, value: number) =>
    invoke<void>("set_test_level", { fixture, channel, value }),
  locate: (fixture: number) => invoke<void>("locate", { fixture }),
  release: (fixture: number | null) => invoke<void>("release", { fixture }),
  live: () => invoke<Live>("live"),
  setOutput: (settings: OutputSettings) => invoke<OutputSettings>("set_output", { settings }),
  setBlackout: (on: boolean) => invoke<void>("set_blackout", { on }),
  /** Scales all intensity output, from 0 to 1. */
  setGrandMaster: (level: number) => invoke<void>("set_grand_master", { level }),
  setHighlight: (on: boolean) => invoke<void>("set_highlight", { on }),
  highlightColour: () => invoke<Rgb>("highlight_colour"),
  setHighlightColour: (colour: Rgb) => invoke<Rgb>("set_highlight_colour", { colour }),
  discoverNodes: () => invoke<Node[]>("discover_nodes"),

  programmer: () => invoke<ProgrammerView>("programmer"),
  select: (fixtures: number[], mode: SelectMode) => invoke<ProgrammerView>("select", { fixtures, mode }),
  selectGroup: (id: number, mode: SelectMode) => invoke<ProgrammerView>("select_group", { id, mode }),
  setIntensity: (value: number) => invoke<ProgrammerView>("set_intensity", { value }),
  setColour: (colour: Rgb) => invoke<ProgrammerView>("set_colour", { colour }),
  setPosition: (pan: number | null, tilt: number | null) => invoke<ProgrammerView>("set_position", { pan, tilt }),
  setAttribute: (attribute: Attribute, value: number) => invoke<ProgrammerView>("set_attribute", { attribute, value }),
  /** Clears `family` (or everything) from `fixtures`, or from every fixture when `fixtures` is null. */
  clearProgrammer: (family: Family | null, fixtures: number[] | null = null) =>
    invoke<ProgrammerView>("clear_programmer", { family, fixtures }),
  /** Takes whole fixtures (`rows`) and single values (`cells`) out of the programmer. */
  removeFromProgrammer: (rows: number[], cells: [number, ProgrammerField][]) =>
    invoke<ProgrammerView>("remove_from_programmer", { rows, cells }),
  /** Load: what running cue lists put out for `fixtures`, or every fixture when null, into the programmer. */
  loadLive: (fixtures: number[] | null) => invoke<ProgrammerView>("load_live", { fixtures }),
  saveGroup: (name: string, id: number | null) => invoke<ProgrammerView>("save_group", { name, id }),
  deleteGroup: (id: number) => invoke<ProgrammerView>("delete_group", { id }),
  recordPreset: (family: Family, name: string, id: number | null) =>
    invoke<ProgrammerView>("record_preset", { family, name, id }),
  applyPreset: (id: number) => invoke<ProgrammerView>("apply_preset", { id }),
  deletePreset: (id: number) => invoke<ProgrammerView>("delete_preset", { id }),

  stage: () => invoke<Stage>("stage"),
  setStagePlan: (plan: Plan) => invoke<Stage>("set_stage_plan", { plan }),
  /** Records that the fixture, where the programmer has it now, is on the mark. */
  calibrateMark: (fixture: number, mark: number) => invoke<Stage>("calibrate_mark", { fixture, mark }),
  /** Sets where a fixture hangs, in metres; null for any part not measured. */
  setFixturePosition: (fixture: number, position: Known) =>
    invoke<Stage>("set_fixture_position", { fixture, position }),
  setPlacement: (placed: Placed) => invoke<Stage>("set_placement", { placed }),
  /** Forgets one reading, or the fixture's whole calibration when `mark` is null. */
  forgetCalibration: (fixture: number, mark: number | null) =>
    invoke<Stage>("forget_calibration", { fixture, mark }),
  /** Points the selected, calibrated movers at a spot, `height` above the floor there. */
  pointAt: (x: number, y: number, height: number) => invoke<ProgrammerView>("point_at", { x, y, height }),

  oscSettings: () => invoke<OscSettings>("osc_settings"),
  setOsc: (settings: OscSettings) => invoke<OscSettings>("set_osc", { settings }),
  showFile: () => invoke<string | null>("show_file"),
  saveShowAs: (path: string) => invoke<string>("save_show_as", { path }),
  openShow: (path: string) => invoke<string>("open_show", { path }),
  newShow: () => invoke<void>("new_show"),
  surfaceStatus: () => invoke<SurfaceStatus>("surface_status"),
  setSurfaceLeds: (leds: Record<string, Led>) => invoke<void>("set_surface_leds", { leds }),

  playback: () => invoke<PlaybackView>("playback"),
  newCueList: (name: string) => invoke<PlaybackView>("new_cue_list", { name }),
  renameCueList: (id: number, name: string) => invoke<PlaybackView>("rename_cue_list", { id, name }),
  deleteCueList: (id: number) => invoke<PlaybackView>("delete_cue_list", { id }),
  duplicateCueList: (id: number) => invoke<PlaybackView>("duplicate_cue_list", { id }),
  selectCueList: (list: number | null) => invoke<PlaybackView>("select_cue_list", { list }),
  setSlot: (slot: number, list: number | null) => invoke<PlaybackView>("set_slot", { slot, list }),
  setGridButton: (page: number, button: number, list: number | null) =>
    invoke<PlaybackView>("set_grid_button", { page, button, list }),
  swapGridButtons: (a: [number, number], b: [number, number]) => invoke<PlaybackView>("swap_grid_buttons", { a, b }),
  addGridPage: (name: string) => invoke<PlaybackView>("add_grid_page", { name }),
  renameGridPage: (page: number, name: string) => invoke<PlaybackView>("rename_grid_page", { page, name }),
  deleteGridPage: (page: number) => invoke<PlaybackView>("delete_grid_page", { page }),
  /** Records into an existing cue list, or a new one when `target` has a name. */
  recordCue: (target: RecordTarget, name: string, fade: number, after: number | null) =>
    invoke<PlaybackView>("record_cue", { target, name, fade, after }),
  updateCue: (list: number, index: number) => invoke<PlaybackView>("update_cue", { list, index }),
  editCue: (
    list: number,
    index: number,
    name: string | null,
    fade: number | null,
    mode: CueMode | null = null,
    follow: number | null = null,
  ) => invoke<PlaybackView>("edit_cue", { list, index, name, fade, mode, follow }),
  deleteCue: (list: number, index: number) => invoke<PlaybackView>("delete_cue", { list, index }),
  go: (list: number) => invoke<PlaybackView>("go", { list }),
  back: (list: number) => invoke<PlaybackView>("back", { list }),
  goTo: (list: number, index: number) => invoke<PlaybackView>("go_to", { list, index }),
  tapCueList: (list: number) => invoke<PlaybackView>("tap_cue_list", { list }),
  setCueListOptions: (id: number, releaseOnNextGo: boolean, solo: boolean) =>
    invoke<PlaybackView>("set_cue_list_options", { id, releaseOnNextGo, solo }),
  releaseCueList: (list: number) => invoke<PlaybackView>("release_cue_list", { list }),
  releaseAll: () => invoke<PlaybackView>("release_all"),
  setMaster: (list: number, level: number) => invoke<PlaybackView>("set_master", { list, level }),
  saveEffect: (settings: EffectSettings, id: number | null) => invoke<PlaybackView>("save_effect", { settings, id }),
  deleteEffect: (id: number) => invoke<PlaybackView>("delete_effect", { id }),
  setEffectSize: (level: number) => invoke<PlaybackView>("set_effect_size", { level }),
  tapSpeed: (master: number) => invoke<PlaybackView>("tap_speed", { master }),
  setSpeedBpm: (master: number, bpm: number) => invoke<PlaybackView>("set_speed_bpm", { master, bpm }),

  remoteStatus: () => invoke<RemoteStatus>("remote_status"),
  setRemote: (on: boolean) => invoke<RemoteStatus>("set_remote", { on }),
  remotePairingCode: () => invoke<RemoteStatus>("remote_pairing_code"),
  forgetRemoteDevice: (id: number) => invoke<RemoteStatus>("forget_remote_device", { id }),
};
