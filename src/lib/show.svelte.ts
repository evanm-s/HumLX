// App-wide state shared by every screen.

import { api } from "./api";
import type {
  Fixture,
  Live,
  OutputSettings,
  PatchView,
  PlaybackView,
  Profile,
  ProgrammerView,
  Values,
} from "./types";

class ShowState {
  profiles = $state<Profile[]>([]);
  patch = $state<PatchView>({ fixtures: [], occupancy: Array(512).fill(null) });
  output = $state<OutputSettings>({ enabled: true, target: "255.255.255.255", universe: 0, rateHz: 40 });
  live = $state<Live>({
    universe: Array(512).fill(0),
    status: { packetsSent: 0, lastError: null },
    blackout: false,
    tested: [],
    playbacks: [],
    runningEffects: [],
    masters: [],
    osc: { listening: false, error: null, received: 0, last: null },
  });
  playback = $state<PlaybackView>({ cueLists: [], effects: [], masters: [] });
  programmer = $state<ProgrammerView>({ selection: [], values: {}, groups: [], presets: [] });
  loaded = $state(false);
  /** Bumped when a different show is loaded, so screens start afresh. */
  revision = $state(0);

  async load() {
    const overview = await api.overview();
    this.profiles = overview.profiles;
    this.patch = overview.patch;
    this.output = overview.output;
    this.programmer = await api.programmer();
    this.playback = await api.playback();
    this.loaded = true;
    this.revision++;
    for (const warning of overview.warnings) notify(warning, "warning");
  }

  profile(id: string): Profile | undefined {
    return this.profiles.find((p) => p.id === id);
  }

  channels(fixture: Fixture) {
    return this.profile(fixture.profile)?.modes.find((m) => m.name === fixture.mode)?.channels ?? [];
  }

  /** Programmer values of the first selected fixture, which the controls show. */
  get current(): Values {
    const first = this.programmer.selection[0];
    return (first != null && this.programmer.values[first]) || {};
  }

  /** Runs a programmer command and keeps its result. */
  async program(action: () => Promise<ProgrammerView>) {
    const view = await attempt(action);
    if (view) this.programmer = view;
    return view;
  }

  /** Runs a playback command, keeping its result and the programmer, which recording clears. */
  async play(action: () => Promise<PlaybackView>) {
    const view = await attempt(action);
    if (view) {
      this.playback = view;
      this.programmer = await api.programmer();
    }
    return view;
  }

  fixture(id: number | null): Fixture | undefined {
    return id == null ? undefined : this.patch.fixtures.find((f) => f.id === id);
  }

  /** Polls output levels and health for the live displays. */
  startLive(intervalMs = 100) {
    let stopped = false;
    const tick = async () => {
      if (stopped) return;
      try {
        this.live = await api.live();
      } catch {
        // Shown through the output status; keep polling.
      }
      setTimeout(tick, intervalMs);
    };
    tick();
    return () => (stopped = true);
  }
}

export const show = new ShowState();

export interface Toast {
  id: number;
  message: string;
  kind: "error" | "warning" | "info";
}

export const toasts = $state<Toast[]>([]);
let nextToast = 1;

export function notify(message: string, kind: Toast["kind"] = "info") {
  const id = nextToast++;
  toasts.push({ id, message, kind });
  setTimeout(() => dismiss(id), kind === "error" ? 8000 : 5000);
}

export function dismiss(id: number) {
  const index = toasts.findIndex((t) => t.id === id);
  if (index >= 0) toasts.splice(index, 1);
}

/** Runs an action and shows its error, if any, as a toast. */
export async function attempt<T>(action: () => Promise<T>): Promise<T | undefined> {
  try {
    return await action();
  } catch (error) {
    notify(error instanceof Error ? error.message : String(error), "error");
    return undefined;
  }
}
