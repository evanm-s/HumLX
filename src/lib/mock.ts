// A rough in-memory stand-in for the Rust side, used only when the UI runs
// in a plain browser for previewing. It skips most validation; the real
// rules live in `crates/humlx-core`.

import { recipe, SHAPES } from "../components/programmer/effects";
import type {
  Attribute,
  CueList,
  Effect,
  Family,
  PlaybackView,
  Fixture,
  Group,
  Live,
  FixtureUpdate,
  NewFixtures,
  OutputSettings,
  PatchView,
  Preset,
  Profile,
  ProgrammerView,
  ProgrammerField,
  Rgb,
  Shape,
  Values,
  OscSettings,
  Layout,
} from "./types";
import { emptyLayout } from "./types";

const files = import.meta.glob<Omit<Profile, "id">>("../../crates/humlx-core/fixtures/*.json", {
  eager: true,
  import: "default",
});

const slug = (s: string) =>
  s
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, "-")
    .replace(/^-|-$/g, "");

const profiles: Profile[] = Object.values(files).map((p) => ({
  ...p,
  id: `${slug(p.manufacturer)}/${slug(p.model)}`,
}));

let fixtures: Fixture[] = [1,2,3,4,5,6].map((id) => ({ id, name: `F${id}`, profile: profiles[0].id, mode: profiles[0].modes[0].name, address: id, footprint: 1, typeName: "x" }));
let output: OutputSettings = { enabled: true, target: "255.255.255.255", universe: 0, rateHz: 40 };
let blackout = false;
let grandMaster = 1;
let highlight = false;
let highlightColour: Rgb = { r: 1, g: 1, b: 1 };
const tests = new Map<number, Map<number, number>>();
let packets = 0;
let selection: number[] = [];
let values: Record<number, Values> = {};
let groups: Group[] = [];
let presets: Preset[] = [];
let cueLists: CueList[] = [];
let osc: OscSettings = { enabled: false, port: 8000 };
let effects: Effect[] = []; // in the programmer
let effectSize = 1;
const speedBpm: [number, number] = [120, 120];
const speedOrigin = [Date.now(), Date.now()];
const taps: number[][] = [[], []];
const beats = (): [number, number] =>
  [0, 1].map((i) => ((Date.now() - speedOrigin[i]) / 60000) * speedBpm[i]) as [number, number];
const running = new Map<number, number>(); // cue list -> current cue
const masters = new Map<number, number>();
const layout: Layout = emptyLayout();
const forget = (id: number) => {
  for (const page of layout.pages) page.buttons = page.buttons.map((b) => (b === id ? null : b));
  layout.slots = layout.slots.map((s) => (s === id ? null : s));
  if (layout.selected === id) layout.selected = null;
};

const playback = (): PlaybackView => ({
  cueLists: structuredClone(cueLists),
  effectSize,
  speedBpm: [...speedBpm] as [number, number],
  masters: cueLists.map((l) => [l.id, masters.get(l.id) ?? 1]),
  layout: structuredClone(layout),
});

/** Playbacks (tracked, no fades) with the programmer on top. */
function combined(): Record<number, Values> {
  const out: Record<number, Values> = {};
  for (const [id, current] of running) {
    const list = cueLists.find((l) => l.id === id);
    if (!list) continue;
    for (const cue of list.cues.slice(0, current + 1))
      for (const [f, v] of Object.entries(cue.values)) {
        const scaled = { ...v, intensity: v.intensity == null ? undefined : v.intensity * (masters.get(id) ?? 1) };
        out[+f] = { ...out[+f], ...Object.fromEntries(Object.entries(scaled).filter(([, x]) => x !== undefined)) };
      }
  }
  for (const [f, v] of Object.entries(values)) out[+f] = { ...out[+f], ...v };
  if (highlight)
    for (const f of fixtures)
      out[f.id] = selection.includes(f.id)
        ? { ...out[f.id], intensity: 1, colour: highlightColour, other: { ...out[f.id]?.other, zoom: 0 } }
        : { ...out[f.id], intensity: 0 };
  return out;
}

const COLOUR_ATTRIBUTES: Attribute[] = [
  "red",
  "green",
  "blue",
  "white",
  "warm_white",
  "cool_white",
  "amber",
  "lime",
  "uv",
  "cyan",
  "magenta",
  "yellow",
  "color_wheel",
];
const FAMILY: Record<string, Family> = { intensity: "intensity", colour: "colour", pan: "position", tilt: "position" };

function only(v: Values, family: Family): Values {
  const out: Values = {};
  if (family === "intensity" && v.intensity != null) out.intensity = v.intensity;
  if (family === "colour" && v.colour) out.colour = v.colour;
  if (family === "position") {
    if (v.pan != null) out.pan = v.pan;
    if (v.tilt != null) out.tilt = v.tilt;
  }
  if (family === "beam" && v.other && Object.keys(v.other).length) out.other = { ...v.other };
  return out;
}

const programmer = (): ProgrammerView => ({
  selection: [...selection],
  values: structuredClone(values),
  effects: structuredClone(effects),
  groups: structuredClone(groups),
  presets: structuredClone(presets),
});

function update(change: (v: Values) => void) {
  for (const id of selection) change((values[id] ??= {}));
  return programmer();
}

function nextId(ids: number[]) {
  let n = 1;
  while (ids.includes(n)) n++;
  return n;
}

/** GO, releasing the lists it replaces straight away (the app fades them). */
function go(list: number) {
  const l = cueLists.find((x) => x.id === list)!;
  if (!l.cues.length) throw "cue 1 does not exist";
  const current = running.get(list);
  running.set(list, current == null || current + 1 >= l.cues.length ? 0 : current + 1);
  for (const id of [...running.keys()]) {
    const other = cueLists.find((x) => x.id === id);
    if (id !== list && other && (l.solo || (l.releaseOnNextGo && other.releaseOnNextGo))) running.delete(id);
  }
}

function addCueList(name: string) {
  const id = nextId(cueLists.map((l) => l.id));
  cueLists.push({ id, name: name.trim() || `Cue list ${id}`, cues: [], releaseOnNextGo: true, solo: false });
  return id;
}

/** Very rough stand-in for the Rust renderer, enough to preview levels. */
function programmed(f: Fixture): (number | undefined)[] {
  const v = combined()[f.id];
  const mode = modeOf(f);
  if (!v || !mode) return [];
  const has = (a: string) => mode.channels.some((c) => c.attribute === a);
  const level = v.intensity ?? 0;
  const colour = v.colour ?? (has("intensity") ? undefined : { r: 1, g: 1, b: 1 });
  const scale = has("intensity") ? 1 : level;
  const w = colour && has("white") ? Math.min(colour.r, colour.g, colour.b) : 0;
  return mode.channels.map((c) => {
    if (c.fineOf) return undefined;
    const byte = (x: number) => Math.round(Math.max(0, Math.min(1, x)) * 255);
    switch (c.attribute) {
      case "intensity":
        return v.intensity == null ? undefined : byte(level);
      case "red":
        return colour && byte((colour.r - w) * scale);
      case "green":
        return colour && byte((colour.g - w) * scale);
      case "blue":
        return colour && byte((colour.b - w) * scale);
      case "white":
        return colour && byte(w * scale);
      case "pan":
        return v.pan == null ? undefined : byte(v.pan);
      case "tilt":
        return v.tilt == null ? undefined : byte(v.tilt);
      default:
        return v.other?.[c.attribute] == null ? undefined : byte(v.other[c.attribute]!);
    }
  });
}

function modeOf(f: { profile: string; mode: string }) {
  return profiles.find((p) => p.id === f.profile)?.modes.find((m) => m.name === f.mode);
}

function occupancy(): (number | null)[] {
  const owners: (number | null)[] = Array(512).fill(null);
  for (const f of fixtures) for (let i = 0; i < f.footprint; i++) owners[f.address - 1 + i] = f.id;
  return owners;
}

function view(): PatchView {
  return { fixtures: [...fixtures].sort((a, b) => a.id - b.id), occupancy: occupancy() };
}

function findFree(channels: number): number {
  const owners = occupancy();
  let run = 0;
  for (let i = 0; i < 512; i++) {
    run = owners[i] === null ? run + 1 : 0;
    if (run === channels) return i + 2 - channels;
  }
  throw "There is no free space left in the universe";
}

function universe(): number[] {
  const data = Array(512).fill(0);
  for (const f of fixtures) {
    const mode = modeOf(f);
    if (!mode) continue;
    const hasDimmer = mode.channels.some((c) => c.attribute === "intensity");
    const prog = programmed(f);
    mode.channels.forEach((c, i) => {
      let value = tests.get(f.id)?.get(i) ?? prog[i] ?? c.default ?? 0;
      const mix = ["red", "green", "blue", "white", "amber", "uv"].includes(c.attribute);
      if (hasDimmer ? c.attribute === "intensity" : mix) value = blackout ? 0 : Math.round(value * grandMaster);
      data[f.address - 1 + i] = value;
    });
  }
  return data;
}

type Args = Record<string, any>;

const commands: Record<string, (args: Args) => unknown> = {
  overview: () => ({ profiles, patch: view(), output, warnings: ["Preview mode: no lights are being controlled."] }),
  add_fixtures: ({ request }: Args & { request?: NewFixtures }) => {
    if (!request) throw "Missing request";
    const profile = profiles.find((p) => p.id === request.profile)!;
    const footprint = modeOf(request)!.channels.length;
    let id = request.firstId ?? Math.max(0, ...fixtures.map((f) => f.id)) + 1;
    let address = request.address ?? findFree(footprint * request.count);
    const name = request.name?.trim() || profile.model;
    for (let i = 0; i < request.count; i++) {
      fixtures.push({
        id: id + i,
        name: request.count > 1 ? `${name} ${i + 1}` : name,
        profile: profile.id,
        mode: request.mode,
        address: address + i * footprint,
        footprint,
        typeName: `${profile.manufacturer} ${profile.model}`,
      });
    }
    return view();
  },
  update_fixture: ({ id, changes }) => {
    const f = fixtures.find((x) => x.id === id)!;
    Object.assign(f, Object.fromEntries(Object.entries(changes).filter(([, v]) => v !== undefined)));
    f.footprint = modeOf(f)?.channels.length ?? 0;
    return view();
  },
  update_fixtures: ({ updates }: Args & { updates?: FixtureUpdate[] }) => {
    const targets = (updates ?? []).map((u) => [fixtures.find((x) => x.id === u.id)!, u.changes] as const);
    for (const [f, changes] of targets) {
      Object.assign(f, Object.fromEntries(Object.entries(changes).filter(([, v]) => v !== undefined)));
      f.footprint = modeOf(f)?.channels.length ?? 0;
    }
    fixtures.sort((a, b) => a.id - b.id);
    return view();
  },
  remove_fixtures: ({ ids }) => {
    fixtures = fixtures.filter((f) => !ids.includes(f.id));
    return view();
  },
  import_profile: () => {
    throw "Importing needs the desktop app";
  },
  set_test_level: ({ fixture, channel, value }) => {
    if (!tests.has(fixture)) tests.set(fixture, new Map());
    tests.get(fixture)!.set(channel, value);
  },
  locate: ({ fixture }) => {
    const f = fixtures.find((x) => x.id === fixture)!;
    const levels = new Map<number, number>();
    modeOf(f)?.channels.forEach((c, i) => {
      const full = ["intensity", "red", "green", "blue", "white"].includes(c.attribute) && !c.fineOf;
      levels.set(i, c.highlight ?? (full ? 255 : (c.attribute === "pan" || c.attribute === "tilt") && !c.fineOf ? 128 : c.default ?? 0));
    });
    tests.set(fixture, levels);
  },
  release: ({ fixture }) => (fixture == null ? tests.clear() : tests.delete(fixture)),
  live: (): Live => ({
    universe: universe(),
    status: { packetsSent: (packets += 4), lastError: null },
    blackout,
    grandMaster,
    highlight,
    tested: [...tests.keys()].sort((a, b) => a - b),
    playbacks: [...running].map(([list, current]) => ({ list, current, fading: false, followIn: null, releasing: false })),
    masters: cueLists.map((l) => [l.id, masters.get(l.id) ?? 1]),
    speedBpm: [...speedBpm] as [number, number],
    beats: beats(),
    osc: { listening: osc.enabled, error: null, received: 0, last: null },
  }),
  osc_settings: () => osc,
  surface_status: () => ({ connected: false, problem: null }),
  set_surface_leds: () => undefined,
  set_osc: ({ settings }) => (osc = settings),
  show_file: () => null,
  save_show_as: () => {
    throw "Saving show files needs the desktop app";
  },
  open_show: () => {
    throw "Opening show files needs the desktop app";
  },
  new_show: () => {
    fixtures = [];
    groups = [];
    presets = [];
    cueLists = [];
    effects = [];
    values = {};
    selection = [];
    running.clear();
    highlight = false;
  },
  set_output: ({ settings }) => (output = settings),
  set_blackout: ({ on }) => (blackout = on),
  set_grand_master: ({ level }) => (grandMaster = Math.max(0, Math.min(1, level))),
  set_highlight: ({ on }) => (highlight = on),
  highlight_colour: () => highlightColour,
  set_highlight_colour: ({ colour }) => (highlightColour = colour),
  programmer,
  select: ({ fixtures, mode }) => {
    if (mode === "set") selection = [...fixtures];
    else if (mode === "add") selection = [...selection, ...fixtures.filter((f: number) => !selection.includes(f))];
    else
      for (const f of fixtures)
        selection = selection.includes(f) ? selection.filter((s) => s !== f) : [...selection, f];
    return programmer();
  },
  select_group: ({ id, mode }) => commands.select({ fixtures: groups.find((g) => g.id === id)?.fixtures ?? [], mode }),
  set_intensity: ({ value }) => update((v) => (v.intensity = value)),
  set_colour: ({ colour }) =>
    update((v) => {
      v.colour = colour;
      for (const a of COLOUR_ATTRIBUTES) delete v.other?.[a];
    }),
  set_position: ({ pan, tilt }) =>
    update((v) => {
      if (pan != null) v.pan = pan;
      if (tilt != null) v.tilt = tilt;
    }),
  set_attribute: ({ attribute, value }) => update((v) => ((v.other ??= {})[attribute as Attribute] = value)),
  clear_programmer: ({ family, fixtures: only }) => {
    const hit = (id: number) => only == null || only.includes(id);
    const ofFamily = (e: Effect) => family == null || SHAPES[e.shape].family === family;
    effects = effects
      .map((e) => (ofFamily(e) ? { ...e, fixtures: e.fixtures.filter((f) => !hit(f)) } : e))
      .filter((e) => e.fixtures.length > 0);
    for (const [id, v] of Object.entries(values)) {
      if (!hit(Number(id))) continue;
      if (family == null) {
        delete values[Number(id)];
        continue;
      }
      for (const key of Object.keys(FAMILY)) if (FAMILY[key] === family) delete v[key as keyof Values];
      if (family === "beam") delete v.other;
      if (family === "colour") for (const a of [...COLOUR_ATTRIBUTES, "color_temperature" as const]) delete v.other?.[a];
    }
    return programmer();
  },
  remove_from_programmer: ({ rows, cells }) => {
    for (const [id, field] of cells as [number, ProgrammerField][]) {
      const v = values[id];
      if (!v) continue;
      if (typeof field === "string") delete v[field];
      else delete v.other?.[field.other];
      if (v.other && Object.keys(v.other).length === 0) delete v.other;
      if (Object.keys(v).length === 0) delete values[id];
    }
    const gone = rows as number[];
    effects = effects
      .map((e) => ({ ...e, fixtures: e.fixtures.filter((f) => !gone.includes(f)) }))
      .filter((e) => e.fixtures.length > 0);
    for (const id of gone) delete values[id];
    return programmer();
  },
  save_group: ({ name, id }) => {
    const gid = id ?? nextId(groups.map((g) => g.id));
    groups = [...groups.filter((g) => g.id !== gid), { id: gid, name, fixtures: [...selection] }].sort((a, b) => a.id - b.id);
    return programmer();
  },
  delete_group: ({ id }) => ((groups = groups.filter((g) => g.id !== id)), programmer()),
  record_preset: ({ family, name, id }) => {
    const ids = selection.length ? selection : Object.keys(values).map(Number);
    const recorded: Record<string, Values> = Object.fromEntries(
      ids.map((f) => [f, only(values[f] ?? {}, family)]).filter(([, v]) => Object.keys(v).length),
    );
    const list = Object.values(recorded);
    const fx = structuredClone(effects.filter((e) => SHAPES[e.shape].family === family));
    if (!list.length && !fx.length) throw "There is nothing in the programmer to record";
    const shared = list.every((v) => JSON.stringify(v) === JSON.stringify(list[0]));
    const pid = id ?? nextId(presets.map((p) => p.id));
    const preset: Preset = shared
      ? { id: pid, name, family, all: list[0] ?? {}, effects: fx }
      : { id: pid, name, family, fixtures: recorded, effects: fx };
    presets = [...presets.filter((p) => p.id !== pid), preset].sort((a, b) => a.id - b.id);
    return programmer();
  },
  apply_preset: ({ id }) => {
    const preset = presets.find((p) => p.id === id)!;
    for (const f of selection) {
      const v = preset.fixtures?.[f] ?? preset.all;
      if (v) values[f] = { ...values[f], ...structuredClone(v), other: { ...values[f]?.other, ...v.other } };
    }
    for (const e of preset.effects ?? []) {
      if (!selection.length) break;
      const family = SHAPES[e.shape].family;
      effects = effects.filter((x) => SHAPES[x.shape].family !== family || !x.fixtures.some((f) => selection.includes(f)));
      effects.push({ ...structuredClone(e), id: nextId(effects.map((x) => x.id)), fixtures: [...selection] });
    }
    return programmer();
  },
  delete_preset: ({ id }) => ((presets = presets.filter((p) => p.id !== id)), programmer()),
  playback,
  new_cue_list: ({ name }) => ((layout.selected = addCueList(name)), playback()),
  duplicate_cue_list: ({ id }) => {
    const copy = structuredClone(cueLists.find((l) => l.id === id)!);
    copy.id = nextId(cueLists.map((l) => l.id));
    copy.name += " copy";
    cueLists.push(copy);
    layout.selected = copy.id;
    return playback();
  },
  select_cue_list: ({ list }) => ((layout.selected = list), playback()),
  set_slot: ({ slot, list }) => ((layout.slots[slot] = list), playback()),
  set_grid_button: ({ page, button, list }) => ((layout.pages[page].buttons[button] = list), playback()),
  swap_grid_buttons: ({ a, b }) => {
    const x = layout.pages[a[0]].buttons[a[1]];
    layout.pages[a[0]].buttons[a[1]] = layout.pages[b[0]].buttons[b[1]];
    layout.pages[b[0]].buttons[b[1]] = x;
    return playback();
  },
  add_grid_page: ({ name }) => (layout.pages.push({ name, buttons: Array(48).fill(null) }), playback()),
  rename_grid_page: ({ page, name }) => ((layout.pages[page].name = name), playback()),
  delete_grid_page: ({ page }) => (layout.pages.length > 1 && layout.pages.splice(page, 1), playback()),
  rename_cue_list: ({ id, name }) => ((cueLists.find((l) => l.id === id)!.name = name), playback()),
  delete_cue_list: ({ id }) => (
    (cueLists = cueLists.filter((l) => l.id !== id)), forget(id), running.delete(id), playback()
  ),
  record_cue: ({ target, name, fade, after }) => {
    if (!Object.keys(values).length && !effects.length) throw "There is nothing in the programmer to record";
    const list = "list" in target ? target.list : addCueList("new" in target ? target.new : target.button.name || "Look");
    if ("button" in target) layout.pages[target.button.page].buttons[target.button.button] = list;
    else if (!("list" in target)) {
      const free = layout.slots.indexOf(null);
      if (free >= 0) layout.slots[free] = list;
    }
    layout.selected = list;
    const l = cueLists.find((x) => x.id === list)!;
    const index = after == null ? l.cues.length : Math.min(after + 1, l.cues.length);
    l.cues.splice(index, 0, { name: name.trim() || `Cue ${index + 1}`, fade, mode: "hold", follow: 0, values: structuredClone(values), effects: structuredClone(effects) });
    values = {};
    effects = [];
    running.set(list, index);
    return playback();
  },
  update_cue: ({ list, index }) => {
    const cue = cueLists.find((x) => x.id === list)!.cues[index];
    for (const [f, v] of Object.entries(values)) cue.values[f] = { ...cue.values[f], ...v };
    cue.effects = [...(cue.effects ?? []), ...structuredClone(effects)];
    values = {};
    effects = [];
    return playback();
  },
  edit_cue: ({ list, index, name, fade, mode, follow }) => {
    const cue = cueLists.find((x) => x.id === list)!.cues[index];
    if (name != null) cue.name = name;
    if (fade != null) cue.fade = fade;
    if (mode != null) cue.mode = mode;
    if (follow != null) cue.follow = follow;
    return playback();
  },
  delete_cue: ({ list, index }) => (cueLists.find((x) => x.id === list)!.cues.splice(index, 1), playback()),
  go: ({ list }) => (go(list), playback()),
  tap_cue_list: ({ list }) => {
    const l = cueLists.find((x) => x.id === list)!;
    layout.selected = list;
    if (l.cues.length === 1 && running.has(list)) running.delete(list);
    else go(list);
    return playback();
  },
  set_cue_list_options: ({ id, releaseOnNextGo, solo }) => {
    Object.assign(cueLists.find((l) => l.id === id)!, { releaseOnNextGo, solo });
    return playback();
  },
  back: ({ list }) => (running.has(list) && running.set(list, Math.max(0, running.get(list)! - 1)), playback()),
  go_to: ({ list, index }) => (running.set(list, index), playback()),
  release_cue_list: ({ list }) => (running.delete(list), playback()),
  release_all: () => (running.clear(), playback()),
  set_master: ({ list, level }) => (masters.set(list, level), playback()),
  save_effect: ({ settings, id }) => {
    const shaped = recipe(settings.shape, settings.colours);
    const made = {
      ...settings,
      ...shaped,
      steps: settings.steps ?? shaped.steps,
      layering: settings.layering ?? shaped.layering,
      transition: settings.transition ?? SHAPES[settings.shape as Shape].transition,
    };
    if (id != null) Object.assign(effects.find((e) => e.id === id)!, made);
    else {
      if (!selection.length) throw "select the fixtures for the effect first";
      effects.push({ ...made, id: nextId(effects.map((e) => e.id)), fixtures: [...selection] });
    }
    return playback();
  },
  delete_effect: ({ id }) => ((effects = effects.filter((e) => e.id !== id)), playback()),
  set_effect_size: ({ level }) => ((effectSize = level), playback()),
  tap_speed: ({ master }) => {
    const now = Date.now();
    const t = taps[master];
    if (t.length && now - t[t.length - 1] > 2000) t.length = 0;
    t.push(now);
    if (t.length > 5) t.shift();
    if (t.length > 1) speedBpm[master] = Math.min(300, Math.max(10, 60000 / ((t[t.length - 1] - t[0]) / (t.length - 1))));
    speedOrigin[master] = now;
    return playback();
  },
  set_speed_bpm: ({ master, bpm }) => {
    const b = beats()[master];
    speedBpm[master] = Math.min(300, Math.max(10, bpm));
    speedOrigin[master] = Date.now() - (b * 60000) / speedBpm[master];
    return playback();
  },
  discover_nodes: async () => {
    await new Promise((r) => setTimeout(r, 800));
    return [{ ip: "192.168.1.50", shortName: "Preview node", longName: "Pretend Art-Net node", outputs: [0, 1] }];
  },
};

export async function mockInvoke<T>(command: string, args: Args = {}): Promise<T> {
  const handler = commands[command];
  if (!handler) throw `Unknown command ${command}`;
  return (await handler(args)) as T;
}
