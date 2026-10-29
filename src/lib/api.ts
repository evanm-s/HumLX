// Typed wrappers around the Rust commands.

import { invoke as tauriInvoke } from "@tauri-apps/api/core";
import type {
  FixtureChanges,
  Live,
  NewFixtures,
  Node,
  OutputSettings,
  Overview,
  PatchView,
  Profile,
  ProgrammerView,
  Rgb,
  SelectMode,
  Family,
  Attribute,
  PlaybackView,
  EffectSettings,
  RecordTarget,
  OscSettings,
} from "./types";

/** True when running inside the desktop app rather than a plain browser. */
export const inApp = "__TAURI_INTERNALS__" in window;

type Invoke = <T>(command: string, args?: Record<string, unknown>) => Promise<T>;

// In a plain browser (`npm run dev`) there is no Rust side, so a small
// in-memory stand-in is used to preview the UI.
let invokeImpl: Invoke | undefined = inApp ? tauriInvoke : undefined;

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
  discoverNodes: () => invoke<Node[]>("discover_nodes"),

  programmer: () => invoke<ProgrammerView>("programmer"),
  select: (fixtures: number[], mode: SelectMode) => invoke<ProgrammerView>("select", { fixtures, mode }),
  selectGroup: (id: number, mode: SelectMode) => invoke<ProgrammerView>("select_group", { id, mode }),
  setIntensity: (value: number) => invoke<ProgrammerView>("set_intensity", { value }),
  setColour: (colour: Rgb) => invoke<ProgrammerView>("set_colour", { colour }),
  setPosition: (pan: number | null, tilt: number | null) => invoke<ProgrammerView>("set_position", { pan, tilt }),
  setAttribute: (attribute: Attribute, value: number) => invoke<ProgrammerView>("set_attribute", { attribute, value }),
  clearProgrammer: (family: Family | null) => invoke<ProgrammerView>("clear_programmer", { family }),
  saveGroup: (name: string, id: number | null) => invoke<ProgrammerView>("save_group", { name, id }),
  deleteGroup: (id: number) => invoke<ProgrammerView>("delete_group", { id }),
  recordPreset: (family: Family, name: string, id: number | null) =>
    invoke<ProgrammerView>("record_preset", { family, name, id }),
  applyPreset: (id: number) => invoke<ProgrammerView>("apply_preset", { id }),
  deletePreset: (id: number) => invoke<ProgrammerView>("delete_preset", { id }),

  oscSettings: () => invoke<OscSettings>("osc_settings"),
  setOsc: (settings: OscSettings) => invoke<OscSettings>("set_osc", { settings }),
  showFile: () => invoke<string | null>("show_file"),
  saveShowAs: (path: string) => invoke<string>("save_show_as", { path }),
  openShow: (path: string) => invoke<string>("open_show", { path }),
  newShow: () => invoke<void>("new_show"),

  playback: () => invoke<PlaybackView>("playback"),
  newCueList: (name: string) => invoke<PlaybackView>("new_cue_list", { name }),
  renameCueList: (id: number, name: string) => invoke<PlaybackView>("rename_cue_list", { id, name }),
  deleteCueList: (id: number) => invoke<PlaybackView>("delete_cue_list", { id }),
  /** Records into an existing cue list, or a new one when `target` has a name. */
  recordCue: (target: RecordTarget, name: string, fade: number, after: number | null) =>
    invoke<PlaybackView>("record_cue", { target, name, fade, after }),
  updateCue: (list: number, index: number) => invoke<PlaybackView>("update_cue", { list, index }),
  editCue: (list: number, index: number, name: string | null, fade: number | null) =>
    invoke<PlaybackView>("edit_cue", { list, index, name, fade }),
  deleteCue: (list: number, index: number) => invoke<PlaybackView>("delete_cue", { list, index }),
  go: (list: number) => invoke<PlaybackView>("go", { list }),
  back: (list: number) => invoke<PlaybackView>("back", { list }),
  goTo: (list: number, index: number) => invoke<PlaybackView>("go_to", { list, index }),
  releaseCueList: (list: number) => invoke<PlaybackView>("release_cue_list", { list }),
  releaseAll: () => invoke<PlaybackView>("release_all"),
  setMaster: (list: number, level: number) => invoke<PlaybackView>("set_master", { list, level }),
  saveEffect: (settings: EffectSettings, id: number | null) => invoke<PlaybackView>("save_effect", { settings, id }),
  deleteEffect: (id: number) => invoke<PlaybackView>("delete_effect", { id }),
  setEffectRunning: (id: number, on: boolean) => invoke<PlaybackView>("set_effect_running", { id, on }),
};
