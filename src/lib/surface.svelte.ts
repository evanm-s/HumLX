// The Obsidian NX-K control surface: keypad command line, programmer
// encoders and a few direct keys. The Rust side reads the USB device and
// sends `surface` events here.

import { listen } from "@tauri-apps/api/event";
import { api, inApp } from "./api";
import { backspace, parse, typeDigit, type Token } from "./command";
import { attempt, notify, show } from "./show.svelte";
import type { Attribute, SurfaceStatus } from "./types";

type SurfaceEvent = { kind: "key"; key: string; down: boolean } | { kind: "encoder"; index: number; delta: number };

/** What each encoder controls, and how far one step moves it (coarse, fine). */
const ENCODERS: { attribute: Attribute; label: string; step: [number, number] }[] = [
  { attribute: "intensity", label: "Intensity", step: [0.01, 0.002] },
  { attribute: "pan", label: "Pan", step: [0.004, 0.0005] },
  { attribute: "tilt", label: "Tilt", step: [0.004, 0.0005] },
  { attribute: "zoom", label: "Zoom", step: [0.01, 0.002] },
];

/** Window event the Programmer and Playback screens answer by opening Record. */
export const RECORD_EVENT = "humlx:record";

const WORD_KEYS: Record<string, Token> = {
  thru: "Thru",
  plus: "+",
  minus: "-",
  at: "@",
  group: "Group",
  cue: "Cue",
};

class Surface {
  status = $state<SurfaceStatus>({ connected: false, problem: null });
  line = $state<Token[]>([]);
  /** Encoders move in small steps while on. */
  fine = $state(false);
  /** The cue list Next and Last run, as chosen on the Playback screen. */
  cueList = $state<number | null>(null);

  // Encoder turns not yet sent, and whether a send is under way, by encoder.
  private pending = [0, 0, 0, 0];
  private sending = [false, false, false, false];

  readonly encoders = ENCODERS;

  async start() {
    if (!inApp) return () => {};
    this.status = await api.surfaceStatus();
    const stops = await Promise.all([
      listen<SurfaceStatus>("surface-status", (e) => (this.status = e.payload)),
      listen<SurfaceEvent>("surface", (e) => this.handle(e.payload)),
    ]);
    return () => stops.forEach((stop) => stop());
  }

  private handle(event: SurfaceEvent) {
    if (event.kind === "encoder") this.turn(event.index, event.delta);
    else if (event.down) this.press(event.key);
  }

  press(key: string) {
    if (/^[0-9]$/.test(key)) return void (this.line = typeDigit(this.line, key));
    if (key in WORD_KEYS) return void (this.line = [...this.line, WORD_KEYS[key]]);
    switch (key) {
      case "point":
        this.line = typeDigit(this.line, ".");
        return;
      case "full":
        // Full finishes the command, as on Onyx.
        this.line = [...this.line, ...(this.line.includes("@") ? [] : ["@"]), "Full"];
        return this.enter();
      case "backspace":
        this.line = backspace(this.line);
        return;
      case "enter":
        return this.enter();
      case "clear":
        if (this.line.length > 0) this.line = [];
        else show.program(() => api.clearProgrammer(null));
        return;
      case "highlight":
        show.live.highlight = !show.live.highlight;
        attempt(() => api.setHighlight(show.live.highlight));
        return;
      case "record":
        window.dispatchEvent(new Event(RECORD_EVENT));
        return;
      case "next":
      case "last": {
        const list = this.cueList ?? show.playback.cueLists[0]?.id;
        if (list == null) return notify("There are no cue lists to run yet", "warning");
        show.play(() => (key === "next" ? api.go(list) : api.back(list)));
        return;
      }
      case "encoder1":
      case "encoder2":
      case "encoder3":
      case "encoder4":
        this.fine = !this.fine;
        return;
    }
  }

  private async enter() {
    const line = this.line;
    this.line = [];
    const command = parse(
      line,
      show.patch.fixtures.map((f) => f.id),
      show.programmer.groups.map((g) => g.id),
    );
    if (typeof command === "string") return notify(command, "warning");

    if (command.kind === "cue") {
      const list = show.playback.cueLists.find((l) => l.id === (this.cueList ?? show.playback.cueLists[0]?.id));
      if (!list) return notify("There are no cue lists to run yet", "warning");
      if (command.cue < 1 || command.cue > list.cues.length) return notify(`${list.name} has no cue ${command.cue}`, "warning");
      show.play(() => api.goTo(list.id, command.cue - 1));
      return;
    }

    if (command.kind === "groups") {
      const [first, ...rest] = command.groups;
      let ok = await show.program(() => api.selectGroup(first, "set"));
      for (const id of rest) if (ok) ok = await show.program(() => api.selectGroup(id, "add"));
      if (!ok) return;
    } else if (command.fixtures) {
      if (!(await show.program(() => api.select(command.fixtures!, "set")))) return;
    }
    if (command.level != null) {
      if (show.programmer.selection.length === 0) return notify("Select fixtures first", "warning");
      show.program(() => api.setIntensity(command.level!));
    }
  }

  private turn(index: number, steps: number) {
    const encoder = ENCODERS[index - 1];
    if (!encoder || show.programmer.selection.length === 0) return;
    this.pending[index - 1] += steps;
    this.send(index - 1);
  }

  // One request per encoder at a time, so a fast spin doesn't queue up
  // hundreds of them; turns made meanwhile go in the next one.
  private async send(i: number) {
    if (this.sending[i] || this.pending[i] === 0) return;
    const { attribute, step } = ENCODERS[i];
    const delta = this.pending[i] * step[this.fine ? 1 : 0];
    this.pending[i] = 0;
    const value = Math.min(1, Math.max(0, current(attribute) + delta));
    this.sending[i] = true;
    await show.program(() =>
      attribute === "intensity"
        ? api.setIntensity(value)
        : attribute === "pan"
          ? api.setPosition(value, null)
          : attribute === "tilt"
            ? api.setPosition(null, value)
            : api.setAttribute(attribute, value),
    );
    this.sending[i] = false;
    this.send(i);
  }
}

/** The first selected fixture's value, which the encoders move from. */
function current(attribute: Attribute): number {
  const v = show.current;
  if (attribute === "intensity") return v.intensity ?? 0;
  if (attribute === "pan") return v.pan ?? 0.5;
  if (attribute === "tilt") return v.tilt ?? 0.5;
  return v.other?.[attribute] ?? 0;
}

export const surface = new Surface();

/** The command line as shown on screen. */
export function lineText(line: Token[]): string {
  return line.join(" ");
}
