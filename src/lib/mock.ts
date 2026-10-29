// A rough in-memory stand-in for the Rust side, used only when the UI runs
// in a plain browser for previewing. It skips most validation; the real
// rules live in `crates/humlx-core`.

import type {
  Attribute,
  Family,
  Fixture,
  Group,
  Live,
  NewFixtures,
  OutputSettings,
  PatchView,
  Preset,
  Profile,
  ProgrammerView,
  Values,
} from "./types";

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

let fixtures: Fixture[] = [];
let output: OutputSettings = { enabled: true, target: "255.255.255.255", universe: 0, rateHz: 40 };
let blackout = false;
const tests = new Map<number, Map<number, number>>();
let packets = 0;
let selection: number[] = [];
let values: Record<number, Values> = {};
let groups: Group[] = [];
let presets: Preset[] = [];

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

/** Very rough stand-in for the Rust renderer, enough to preview levels. */
function programmed(f: Fixture): (number | undefined)[] {
  const v = values[f.id];
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
      if (blackout && (hasDimmer ? c.attribute === "intensity" : mix)) value = 0;
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
    tested: [...tests.keys()].sort((a, b) => a - b),
  }),
  set_output: ({ settings }) => (output = settings),
  set_blackout: ({ on }) => (blackout = on),
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
  set_colour: ({ colour }) => update((v) => (v.colour = colour)),
  set_position: ({ pan, tilt }) =>
    update((v) => {
      if (pan != null) v.pan = pan;
      if (tilt != null) v.tilt = tilt;
    }),
  set_attribute: ({ attribute, value }) => update((v) => ((v.other ??= {})[attribute as Attribute] = value)),
  clear_programmer: ({ family }) => {
    if (family == null) values = {};
    else
      for (const v of Object.values(values)) {
        for (const key of Object.keys(FAMILY)) if (FAMILY[key] === family) delete v[key as keyof Values];
        if (family === "beam") delete v.other;
      }
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
    if (!list.length) throw "There is nothing in the programmer to record";
    const shared = list.every((v) => JSON.stringify(v) === JSON.stringify(list[0]));
    const pid = id ?? nextId(presets.map((p) => p.id));
    const preset: Preset = shared ? { id: pid, name, family, all: list[0] } : { id: pid, name, family, fixtures: recorded };
    presets = [...presets.filter((p) => p.id !== pid), preset].sort((a, b) => a.id - b.id);
    return programmer();
  },
  apply_preset: ({ id }) => {
    const preset = presets.find((p) => p.id === id)!;
    for (const f of selection) {
      const v = preset.fixtures?.[f] ?? preset.all;
      if (v) values[f] = { ...values[f], ...structuredClone(v), other: { ...values[f]?.other, ...v.other } };
    }
    return programmer();
  },
  delete_preset: ({ id }) => ((presets = presets.filter((p) => p.id !== id)), programmer()),
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
