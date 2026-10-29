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
};
