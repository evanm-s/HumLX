// Keyboard support for touch-first forms: Enter in a field presses the form's
// main (`.primary`) button, and Escape backs out.

import { tick } from "svelte";

interface Options {
  /** Called on Escape. */
  onescape?: () => void;
}

/** Svelte action for a dialog or panel. Fields marked `data-enter="ignore"` are skipped. */
export function formKeys(node: HTMLElement, options: Options = {}) {
  let opts = options;

  async function keydown(e: KeyboardEvent) {
    if (e.defaultPrevented || e.isComposing) return;
    if (e.key === "Escape" && opts.onescape) {
      e.preventDefault();
      opts.onescape();
      return;
    }
    const field = e.target as HTMLElement;
    if (e.key !== "Enter" || !(field instanceof HTMLInputElement) || field.dataset.enter === "ignore") return;
    if (field.type === "file" || field.type === "checkbox" || field.type === "radio") return;
    e.preventDefault();
    // Commit fields that only update on change (such as NumberField) before pressing.
    field.dispatchEvent(new Event("change", { bubbles: true }));
    await tick();
    node.querySelector<HTMLButtonElement>("button.primary:not(:disabled)")?.click();
  }

  node.addEventListener("keydown", keydown);
  return {
    update(next: Options = {}) {
      opts = next;
    },
    destroy() {
      node.removeEventListener("keydown", keydown);
    },
  };
}
