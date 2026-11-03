// The Obsidian NX-K control surface: keypad command line, programmer
// encoders and a few direct keys. The Rust side reads the USB device and
// sends `surface` events here.

import { listen } from "@tauri-apps/api/event";
import { api, inApp } from "./api";
import { backspace, parse, typeDigit, type Token } from "./command";
import { endSelectWindow } from "./selection";
import { sheetPicks } from "./sheet.svelte";
import { attempt, notify, show } from "./show.svelte";
import type { Attribute, Led, ProgrammerView, SurfaceStatus } from "./types";

type SurfaceEvent = { kind: "key"; key: string; down: boolean } | { kind: "encoder"; index: number; delta: number };

/** What each encoder controls, and how far one step moves it (coarse, fine). */
const ENCODERS: { attribute: Attribute; label: string; step: [number, number] }[] = [
  { attribute: "intensity", label: "Intensity", step: [0.01, 0.002] },
  { attribute: "pan", label: "Pan", step: [0.004, 0.0005] },
  { attribute: "tilt", label: "Tilt", step: [0.004, 0.0005] },
  { attribute: "zoom", label: "Zoom", step: [0.01, 0.002] },
];

/**
 * Window event the Programmer and Playback screens answer by opening Record.
 * Its detail is `{ cue: true }` when a cue should be recorded straight away
 * rather than arming record mode for presets.
 */
export const RECORD_EVENT = "humlx:record";

/** Record or Delete pressed: the next preset tap stores or deletes. */
export type PresetMode = "record" | "delete";

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
  /** The cue list Next, Last and Cue n Enter run: the selected one, else the first. */
  get cueList(): number | null {
    return show.playback.layout.selected ?? show.playback.cueLists[0]?.id ?? null;
  }
  /** Set by Record or Delete until a preset is stored or deleted, or Clear. */
  mode = $state<PresetMode | null>(null);
  /** In delete mode, the preset tapped and waiting for Enter. */
  pendingDelete = $state<number | null>(null);
  /** The programmer as a first Clear left it, until anything else happens. */
  private clearedOnce = $state.raw<ProgrammerView | null>(null);

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
    const stopLeds = $effect.root(() => {
      $effect(() => {
        const leds = this.leds();
        if (this.status.connected) api.setSurfaceLeds(leds).catch(() => {});
      });
    });
    return () => {
      stops.forEach((stop) => stop());
      stopLeds();
    };
  }

  /** Key LEDs showing what's going on, as Onyx lights them. */
  private leds(): Record<string, Led> {
    const leds: Record<string, Led> = {};
    if (show.live.highlight) leds.highlight = "blink";
    if (this.mode === "record") leds.record = "on";
    if (this.mode === "delete") leds.delete = "on";
    // Clear blinks when the next press clears the programmer, and is lit
    // when there's anything for it to clear.
    const { selection, values, effects } = show.programmer;
    if (this.clearClears) leds.clear = "blink";
    else if (this.line.length || this.mode || selection.length || Object.keys(values).length || effects.length)
      leds.clear = "on";
    // The tap keys are always live.
    leds.fade = "on";
    leds.delay = "on";
    if (this.fine) for (let i = 1; i <= 4; i++) leds[`encoder${i}`] = "on";
    return leds;
  }

  private handle(event: SurfaceEvent) {
    if (event.kind === "encoder") this.turn(event.index, event.delta);
    else if (event.down) this.press(event.key);
  }

  /** Whether the next Clear clears the programmer: Clear was the last thing done. */
  get clearClears(): boolean {
    return this.clearedOnce != null && this.clearedOnce === show.programmer;
  }

  /** Any other action makes the next Clear a first press again. */
  disarmClear() {
    this.clearedOnce = null;
  }

  /**
   * Clear, as on Onyx, so Clear Clear always clears the programmer. The first
   * press empties the command line, cancels Record or Delete, unpicks the
   * programmer sheet and deselects,
   * leaving the values on stage. A second press straight after clears the
   * programmer. Fixture numbers and Enter after the first press clear just
   * those fixtures instead.
   */
  async clear() {
    if (this.clearClears) {
      this.clearedOnce = null;
      this.line = [];
      await show.program(() => api.clearProgrammer(null));
      return;
    }
    this.line = [];
    this.setMode(null);
    sheetPicks.clear();
    endSelectWindow();
    if (show.programmer.selection.length > 0 && !(await show.program(() => api.select([], "set")))) return;
    this.clearedOnce = show.programmer;
  }

  press(key: string) {
    // With a dialog open, Enter presses its main button and Clear cancels it.
    const dialog = document.querySelector<HTMLElement>("[role=dialog]");
    if (dialog && (key === "enter" || key === "clear")) {
      if (key === "enter") dialog.querySelector<HTMLButtonElement>("button.primary:not(:disabled)")?.click();
      else window.dispatchEvent(new KeyboardEvent("keydown", { key: "Escape" }));
      return;
    }
    if (key === "clear") return void this.clear();
    const afterClear = this.clearClears;
    this.clearedOnce = null;
    // Numbers straight after Clear pick fixtures to clear: Clear 1 + 3 Enter.
    if (afterClear && (/^[0-9]$/.test(key) || key === "point")) this.line = ["Clear"];
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
        if (this.mode && this.line.length === 0) return this.confirm();
        return this.enter();
      // Fade and Delay tap speed masters A and B.
      case "fade":
      case "delay":
        show.play(() => api.tapSpeed(key === "fade" ? 0 : 1));
        return;
      case "highlight":
        show.live.highlight = !show.live.highlight;
        attempt(() => api.setHighlight(show.live.highlight));
        return;
      case "record":
        // Record twice records a cue, as does Record then Enter.
        window.dispatchEvent(new CustomEvent(RECORD_EVENT, { detail: { cue: this.mode === "record" } }));
        return;
      case "delete":
        // With values picked on the programmer sheet, Delete takes them out;
        // otherwise it arms deleting a preset.
        if (sheetPicks.delete()) return void this.setMode(null);
        this.setMode(this.mode === "delete" ? null : "delete");
        return;
      case "next":
      case "last": {
        const list = this.cueList;
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

  setMode(mode: PresetMode | null) {
    this.mode = mode;
    this.pendingDelete = null;
  }

  /** Enter in record or delete mode: record a cue, or delete the tapped preset. */
  confirm() {
    const { mode, pendingDelete } = this;
    this.setMode(null);
    if (mode === "record") {
      window.dispatchEvent(new CustomEvent(RECORD_EVENT, { detail: { cue: true } }));
    } else if (pendingDelete != null) {
      show.program(() => api.deletePreset(pendingDelete));
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

    if (command.kind === "clear") {
      show.program(() => api.clearProgrammer(null, command.fixtures));
      return;
    }

    if (command.kind === "cue") {
      const list = show.playback.cueLists.find((l) => l.id === this.cueList);
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
