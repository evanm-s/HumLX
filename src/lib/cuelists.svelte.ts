// Helpers shared by the Cue Lists and Playback screens: the selected cue list,
// where each list is placed, and Space for GO.

import { api } from "./api";
import { show } from "./show.svelte";
import type { CueList } from "./types";

/** Window event asking the app to show a screen; its detail is the screen id. */
export const SCREEN_EVENT = "humlx:screen";

export function showScreen(id: string) {
  window.dispatchEvent(new CustomEvent(SCREEN_EVENT, { detail: id }));
}

/** The selected cue list, which GO, Back, Next, Last and Record act on. */
export function selectedList(): CueList | undefined {
  const id = show.playback.layout.selected;
  return show.playback.cueLists.find((l) => l.id === id);
}

export function selectList(id: number | null) {
  if (show.playback.layout.selected === id) return;
  // Shown straight away; the engine keeps it for Next, Last and the show file.
  show.playback.layout.selected = id;
  return show.play(() => api.selectCueList(id));
}

/** Where a list sits on the Playback screen, such as "Slot 3" or "Grid 1·12". */
export function placements(id: number): string[] {
  const { pages, slots } = show.playback.layout;
  const out: string[] = [];
  slots.forEach((s, i) => s === id && out.push(`Slot ${i + 1}`));
  pages.forEach((p, page) => p.buttons.forEach((b, i) => b === id && out.push(`Grid ${page + 1}·${i + 1}`)));
  return out;
}

export const status = (id: number) => show.live.playbacks.find((p) => p.list === id);

/** A list's fader level; from the live poll too, since OSC can move it. */
export const master = (id: number) =>
  (show.live.masters.find(([l]) => l === id) ?? show.playback.masters.find(([l]) => l === id))?.[1] ?? 1;

/**
 * Space is GO and Shift+Space is Back for the selected cue list, unless typing
 * or a dialog is open.
 */
export function goKeys(e: KeyboardEvent) {
  if (e.key !== " " || e.repeat) return;
  const target = e.target as HTMLElement;
  if (target.closest("input, textarea, select, [role=dialog]") || document.querySelector("[role=dialog]")) return;
  const list = selectedList();
  if (!list) return;
  e.preventDefault();
  show.play(() => (e.shiftKey ? api.back(list.id) : api.go(list.id)));
}
