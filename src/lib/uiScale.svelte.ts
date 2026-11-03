// Screen scale: HumLX was laid out for a 2560x1440 screen, so on a smaller
// screen the desktop app zooms the whole page out to fit, like Ctrl+minus in
// a browser. Everything (touch maths included) keeps working in the same CSS
// pixels. The choice is remembered on this computer.

import { getCurrentWebview } from "@tauri-apps/api/webview";
import { currentMonitor } from "@tauri-apps/api/window";
import { inApp } from "./api";

const KEY = "humlx.uiScale";
/** The screen size the layout was designed at, in logical pixels. */
const DESIGN = { w: 2560, h: 1440 };

export const SCALES = [0.6, 0.67, 0.75, 0.85, 1] as const;
export type ScaleChoice = "auto" | number;

/** Auto scale for a screen of this logical size: fit the design, never above 100% or below 50%. */
export function autoScale(width: number, height: number): number {
  const fit = Math.min(1, width / DESIGN.w, height / DESIGN.h);
  return Math.max(0.5, Math.floor(fit * 100 + 0.001) / 100);
}

class UiScale {
  choice = $state<ScaleChoice>("auto");
  /** What Auto works out to on this screen. */
  auto = $state(1);
  /** The zoom in use. */
  readonly current = $derived(this.choice === "auto" ? this.auto : this.choice);

  async start() {
    if (!inApp) return;
    try {
      const saved = localStorage.getItem(KEY);
      const n = Number(saved);
      if (saved && saved !== "auto" && n >= 0.5 && n <= 1.5) this.choice = n;
    } catch {
      // Storage unavailable: use Auto.
    }
    try {
      const monitor = await currentMonitor();
      if (monitor) this.auto = autoScale(monitor.size.width / monitor.scaleFactor, monitor.size.height / monitor.scaleFactor);
    } catch {
      // Unknown screen: Auto stays at 100%.
    }
    await this.apply();
  }

  async set(choice: ScaleChoice) {
    this.choice = choice;
    try {
      localStorage.setItem(KEY, String(choice));
    } catch {
      // Not remembered, but still applied.
    }
    await this.apply();
  }

  private async apply() {
    if (!inApp) return;
    try {
      await getCurrentWebview().setZoom(this.current);
    } catch (e) {
      console.warn("Couldn't set the screen scale", e);
    }
  }
}

export const uiScale = new UiScale();
