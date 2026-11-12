// App-wide state shared by every screen.

import { api } from "./api";
import { emptyLayout, MAX_COLUMNS, RESOLUME } from "./types";
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
    grandMaster: 1,
    highlight: false,
    tested: [],
    playbacks: [],
    masters: [],
    speedBpm: [120, 120],
    beats: [0, 0],
    osc: { listening: false, error: null, received: 0, last: null },
    remotes: 0,
    values: {},
    intensity: {},
  });
  playback = $state<PlaybackView>({
    cueLists: [],
    effectSize: 1,
    speedBpm: [120, 120],
    masters: [],
    layout: emptyLayout(),
  });
  programmer = $state<ProgrammerView>({ selection: [], values: {}, effects: [], groups: [], presets: [] });
  loaded = $state(false);
  /** Bumped when a different show is loaded, so screens start afresh. */
  revision = $state(0);
  /** Bumped when another screen changed the stage plan or calibration. */
  stageRevision = $state(0);

  private changedParts = new Set<string>();
  private changeTimer: ReturnType<typeof setTimeout> | undefined;

  /**
   * Another screen (a remote, or the Studio PC) changed these parts of the
   * show. Reloads them, gathering a burst of changes such as a drag into one.
   */
  changed(parts: string[]) {
    for (const part of parts) this.changedParts.add(part);
    if (this.changeTimer) return;
    this.changeTimer = setTimeout(() => {
      this.changeTimer = undefined;
      const changed = this.changedParts;
      this.changedParts = new Set();
      this.reload(changed).catch(() => {});
    }, 120);
  }

  private async reload(parts: Set<string>) {
    if (parts.has("show")) return this.load();
    if (parts.has("patch")) {
      const overview = await api.overview();
      this.profiles = overview.profiles;
      this.patch = overview.patch;
      this.output = overview.output;
    }
    if (parts.has("programmer")) this.programmer = await api.programmer();
    if (parts.has("playback")) this.playback = await api.playback();
    if (parts.has("stage")) this.stageRevision++;
  }

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
    const channels = this.profile(fixture.profile)?.modes.find((m) => m.name === fixture.mode)?.channels ?? [];
    if (fixture.profile !== RESOLUME) return channels;
    // A Resolume fixture's columns carry the names given on the Patch page.
    return channels.map((c) =>
      c.attribute === "column"
        ? {
            ...c,
            capabilities: c.capabilities?.map((cap) =>
              cap.from >= 1 && cap.from <= MAX_COLUMNS
                ? { ...cap, label: columnName(fixture, cap.from) }
                : cap,
            ),
          }
        : c,
    );
  }

  /** Programmer values of the first selected fixture, which the controls show. */
  get current(): Values {
    const first = this.programmer.selection[0];
    return (first != null && this.programmer.values[first]) || {};
  }

  /**
   * What the first selected fixture is being given right now: cues with the
   * programmer on top. Programmer values come straight from the programmer
   * so a change shows before the next live poll.
   */
  get onStage(): Values {
    const first = this.programmer.selection[0];
    if (first == null) return {};
    const live = this.live.values?.[first] ?? {};
    const own = this.programmer.values[first] ?? {};
    return { ...live, ...own, other: { ...live.other, ...own.other } };
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

/** A Resolume fixture's name for column `n`, counting from 1. */
export function columnName(fixture: Fixture, n: number): string {
  const name = fixture.options.columns?.[n - 1]?.trim();
  return name ? `${n} ${name}` : `Column ${n}`;
}
