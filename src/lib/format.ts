import type { Attribute, Capability } from "./types";

/** A stable colour per fixture number, for the address map and lists. */
export function fixtureHue(id: number): number {
  return (id * 47) % 360;
}

export function fixtureColour(id: number, lightness = 62): string {
  return `hsl(${fixtureHue(id)} 70% ${lightness}%)`;
}

export function range(address: number, footprint: number): string {
  return footprint > 1 ? `${address}–${address + footprint - 1}` : `${address}`;
}

export function percent(value: number): string {
  return `${Math.round((value / 255) * 100)}%`;
}

export function capabilityAt(capabilities: Capability[] | undefined, value: number): string | undefined {
  return capabilities?.find((c) => value >= c.from && value <= c.to)?.label;
}

const ATTRIBUTE_NAMES: Partial<Record<Attribute, string>> = {
  uv: "UV",
  color_wheel: "Colour wheel",
  color_temperature: "Colour temp",
  pan_tilt_speed: "P/T speed",
  warm_white: "Warm white",
  cool_white: "Cool white",
  gobo_rotation: "Gobo rotation",
};

export function attributeName(attribute: Attribute): string {
  return ATTRIBUTE_NAMES[attribute] ?? attribute.charAt(0).toUpperCase() + attribute.slice(1).replace(/_/g, " ");
}

/** Colour used to tint a fader by what it controls. */
export function attributeColour(attribute: Attribute): string {
  switch (attribute) {
    case "red":
      return "#ff4d4d";
    case "green":
      return "#3ddc84";
    case "blue":
      return "#4d8bff";
    case "white":
    case "cool_white":
      return "#f2f4ff";
    case "warm_white":
      return "#ffe3b0";
    case "amber":
      return "#ffb02e";
    case "lime":
      return "#c6ff4d";
    case "uv":
      return "#9b5cff";
    case "cyan":
      return "#3de0f0";
    case "magenta":
      return "#ff4dd2";
    case "yellow":
      return "#ffe14d";
    case "intensity":
      return "#ffb547";
    default:
      return "#8f9bb3";
  }
}

/** Splits a 15-bit Art-Net port-address into its parts for display. */
export function portAddressParts(address: number) {
  return { net: (address >> 8) & 0x7f, subNet: (address >> 4) & 0x0f, universe: address & 0x0f };
}

export function isIPv4(text: string): boolean {
  const parts = text.trim().split(".");
  return parts.length === 4 && parts.every((p) => /^\d{1,3}$/.test(p) && Number(p) <= 255);
}
