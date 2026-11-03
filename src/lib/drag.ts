// Dragging that works with a mouse, a pen and a finger, on both webviews.
//
// Windows (WebView2) sends pointer events for touch and honours
// `touch-action: none`. Linux (WebKitGTK) ignores `touch-action`, so a finger
// drag turns into a scroll gesture: pointermove stops arriving (or the pointer
// is cancelled) and only taps get through. Touch events still arrive there,
// and calling preventDefault on them stops the gesture, so fingers are
// followed through touch events whenever the webview sends them, and through
// pointer events otherwise.

export interface DragPoint {
  clientX: number;
  clientY: number;
}

export interface DragHandlers {
  /** A drag begins. Return false to leave this press alone (such as on a button). */
  start: (point: DragPoint, target: EventTarget | null) => boolean | void;
  move: (point: DragPoint) => void;
  /** The drag finished; cancelled when the system took the press away. */
  end?: (cancelled: boolean) => void;
}

type Active = { kind: "pointer"; id: number; touch: boolean } | { kind: "touch"; id: number };

const findTouch = (list: TouchList, id: number) => Array.from(list).find((t) => t.identifier === id);

/** Svelte action: `use:drag={{ start, move, end }}`. */
export function drag(node: HTMLElement, handlers: DragHandlers) {
  let h = handlers;
  let active: Active | null = null;

  function finish(cancelled: boolean) {
    active = null;
    h.end?.(cancelled);
  }

  function pointerdown(event: PointerEvent) {
    if (active || event.button > 0) return;
    if (h.start(event, event.target) === false) return;
    active = { kind: "pointer", id: event.pointerId, touch: event.pointerType === "touch" };
    try {
      node.setPointerCapture(event.pointerId);
    } catch {
      // Capture can fail if the pointer is already gone; moves still arrive on the node.
    }
  }

  function pointermove(event: PointerEvent) {
    if (active?.kind === "pointer" && active.id === event.pointerId) h.move(event);
  }

  function pointerup(event: PointerEvent) {
    if (active?.kind === "pointer" && active.id === event.pointerId) finish(event.type === "pointercancel");
  }

  function touchstart(event: TouchEvent) {
    const touch = event.changedTouches[0];
    if (!touch) return;
    if (active?.kind === "pointer" && active.touch) {
      // The same finger already started through pointer events: follow it by touch from here.
      active = { kind: "touch", id: touch.identifier };
      event.preventDefault();
      return;
    }
    if (active) return;
    if (h.start(touch, event.target) === false) return;
    active = { kind: "touch", id: touch.identifier };
    // Stops the webview scrolling or zooming instead of dragging.
    event.preventDefault();
  }

  function touchmove(event: TouchEvent) {
    if (active?.kind !== "touch") return;
    const touch = findTouch(event.changedTouches, active.id);
    if (!touch) return;
    event.preventDefault();
    h.move(touch);
  }

  function touchend(event: TouchEvent) {
    if (active?.kind !== "touch" || !findTouch(event.changedTouches, active.id)) return;
    finish(event.type === "touchcancel");
  }

  const passive = { passive: false } as const;
  node.addEventListener("pointerdown", pointerdown);
  node.addEventListener("pointermove", pointermove);
  node.addEventListener("pointerup", pointerup);
  node.addEventListener("pointercancel", pointerup);
  node.addEventListener("touchstart", touchstart, passive);
  node.addEventListener("touchmove", touchmove, passive);
  node.addEventListener("touchend", touchend);
  node.addEventListener("touchcancel", touchend);

  return {
    update(next: DragHandlers) {
      h = next;
    },
    destroy() {
      node.removeEventListener("pointerdown", pointerdown);
      node.removeEventListener("pointermove", pointermove);
      node.removeEventListener("pointerup", pointerup);
      node.removeEventListener("pointercancel", pointerup);
      node.removeEventListener("touchstart", touchstart);
      node.removeEventListener("touchmove", touchmove);
      node.removeEventListener("touchend", touchend);
      node.removeEventListener("touchcancel", touchend);
    },
  };
}
