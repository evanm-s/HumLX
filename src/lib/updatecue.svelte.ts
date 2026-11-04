// Update, as on Onyx: merges the programmer into a cue after a prompt that
// says which cue. With no cue named, it is the current cue of the selected
// cue list when that list is running, else of the only running list.

import { notify, show } from "./show.svelte";

export interface UpdateTarget {
  list: number;
  index: number;
}

class UpdateCue {
  /** The open prompt: a cue picked on the Cue Lists page, or null for the current cue. */
  request = $state<{ cue: UpdateTarget | null } | null>(null);
  /** The cue picked on the Cue Lists page, which Update acts on while it's picked. */
  picked = $state.raw<UpdateTarget | null>(null);

  /** Opens the Update prompt, for a given or picked cue, else for the current cue. */
  open(cue: UpdateTarget | null = this.picked) {
    const { values, effects } = show.programmer;
    if (Object.keys(values).length === 0 && effects.length === 0)
      return notify("The programmer is empty", "warning");
    this.request = { cue };
  }

  close() {
    this.request = null;
  }
}

export const updateCue = new UpdateCue();

/** Running cue lists (not fading out after a release), as list and current cue. */
export function runningCues(): UpdateTarget[] {
  return show.live.playbacks
    .filter((p) => !p.releasing && show.playback.cueLists.some((l) => l.id === p.list))
    .map((p) => ({ list: p.list, index: p.current }));
}

/** The cue Update acts on with none named: see the top of this file. */
export function currentCue(): UpdateTarget | null {
  const running = runningCues();
  const selected = show.playback.layout.selected;
  return running.find((r) => r.list === selected) ?? (running.length === 1 ? running[0] : null);
}
