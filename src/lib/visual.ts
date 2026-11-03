// What each light is putting out, read back from the DMX universe, for the
// Stage page: how bright it is, what colour, and for movers where it points.

import { capabilityAt } from "./format";
import type { Attribute, Channel, Fixture, Rgb } from "./types";
import { NAMED } from "./wheel";

export interface Look {
  /** 0 to 1, after the dimmer, colour mix and shutter. */
  intensity: number;
  /** The colour at full brightness, each part 0 to 1. */
  colour: Rgb;
  /** Pan and tilt as the fixture gets them, when it has them. */
  position: [number, number] | null;
}

/** What each emitter adds to the mix. */
const EMITTERS: Partial<Record<Attribute, [number, number, number]>> = {
  red: [1, 0, 0],
  green: [0, 1, 0],
  blue: [0, 0, 1],
  white: [1, 1, 1],
  warm_white: [1, 0.78, 0.5],
  cool_white: [0.85, 0.92, 1],
  amber: [1, 0.6, 0],
  lime: [0.7, 1, 0],
  uv: [0.35, 0, 0.8],
};

const WARM: [number, number, number] = [1, 0.75, 0.45];
const COOL: [number, number, number] = [0.85, 0.92, 1];
/** A plain dimmer channel, which is usually a tungsten lamp. */
const TUNGSTEN: Rgb = { r: 1, g: 0.8, b: 0.55 };

/** A channel's level from 0 to 1, with its fine channel when it has one. */
function reader(channels: Channel[], fixture: Fixture, universe: number[]) {
  const at = (i: number) => universe[fixture.address - 1 + i] ?? 0;
  return (attribute: Attribute): number | null => {
    const coarse = channels.findIndex((c) => c.attribute === attribute && !c.fineOf);
    if (coarse < 0) return null;
    const fine = channels.findIndex((c) => c.attribute === attribute && c.fineOf === channels[coarse].name);
    return fine < 0 ? at(coarse) / 255 : (at(coarse) * 256 + at(fine)) / 65535;
  };
}

export function lookOf(fixture: Fixture, channels: Channel[], universe: number[]): Look {
  const level = reader(channels, fixture, universe);
  const raw = (attribute: Attribute) => {
    const i = channels.findIndex((c) => c.attribute === attribute && !c.fineOf);
    return i < 0 ? null : { channel: channels[i], value: universe[fixture.address - 1 + i] ?? 0 };
  };

  // Additive emitters, then subtractive CMY, then a colour wheel.
  let mix = [0, 0, 0];
  let emitters = false;
  for (const [attribute, rgb] of Object.entries(EMITTERS) as [Attribute, [number, number, number]][]) {
    const v = level(attribute);
    if (v == null) continue;
    emitters = true;
    mix = mix.map((m, i) => m + rgb[i] * v);
  }
  const balance = level("color_temperature");
  if (balance != null && !emitters) {
    mix = WARM.map((w, i) => w + (COOL[i] - w) * balance);
  }
  let colour: Rgb;
  let mixLevel = 1;
  if (emitters) {
    mixLevel = Math.max(...mix);
    colour = mixLevel > 0 ? { r: mix[0] / mixLevel, g: mix[1] / mixLevel, b: mix[2] / mixLevel } : { r: 1, g: 1, b: 1 };
    mixLevel = Math.min(1, mixLevel);
  } else if (balance != null) {
    const top = Math.max(...mix);
    colour = { r: mix[0] / top, g: mix[1] / top, b: mix[2] / top };
  } else {
    colour = channels.some((c) => c.attribute === "color_wheel" || c.attribute === "cyan") ? { r: 1, g: 1, b: 1 } : TUNGSTEN;
  }
  const [c, m, y] = (["cyan", "magenta", "yellow"] as const).map((a) => level(a) ?? 0);
  if (c || m || y) colour = { r: colour.r * (1 - c), g: colour.g * (1 - m), b: colour.b * (1 - y) };
  const wheel = raw("color_wheel");
  if (wheel) {
    const label = capabilityAt(wheel.channel.capabilities, wheel.value)?.toLowerCase() ?? "";
    const named = NAMED.find(([name]) => label.includes(name));
    if (named) colour = { r: colour.r * named[1].r, g: colour.g * named[1].g, b: colour.b * named[1].b };
  }

  const dimmer = level("intensity");
  let intensity = dimmer == null ? (emitters ? mixLevel : 0) : dimmer * (emitters ? mixLevel : 1);
  const shutter = raw("shutter");
  if (shutter) {
    const label = capabilityAt(shutter.channel.capabilities, shutter.value)?.toLowerCase() ?? "";
    if (label.includes("closed") || label.includes("blackout")) intensity = 0;
  }

  const pan = level("pan");
  const tilt = level("tilt");
  return { intensity, colour, position: pan == null || tilt == null ? null : [pan, tilt] };
}

/** A beam width to draw when none is set: tight for movers, wide for panels. */
export function defaultBeam(channels: Channel[]): number {
  if (channels.some((c) => c.attribute === "pan")) return 15;
  if (channels.some((c) => c.attribute === "color_temperature")) return 60;
  return 30;
}

export function cssColour(c: Rgb, brightness = 1): string {
  const v = (x: number) => Math.round(Math.max(0, Math.min(1, x * brightness)) * 255);
  return `rgb(${v(c.r)} ${v(c.g)} ${v(c.b)})`;
}
