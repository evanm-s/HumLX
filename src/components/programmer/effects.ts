import type { EffectKind } from "../../lib/types";

/** Each Programmer window that can run an effect, and the kind it makes. */
export const EFFECT_WINDOWS: Record<string, EffectKind> = {
  intensity: "dimmer",
  colour: "rainbow",
  position: "circle",
};

export const EFFECT_KINDS: Record<EffectKind, { label: string; hint: string }> = {
  dimmer: { label: "Dimmer", hint: "Intensity rises and falls" },
  rainbow: { label: "Rainbow", hint: "Colour cycles round the wheel" },
  circle: { label: "Circle", hint: "Moving heads trace a circle" },
};
