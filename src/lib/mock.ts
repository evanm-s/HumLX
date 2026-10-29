// A rough in-memory stand-in for the Rust side, used only when the UI runs
// in a plain browser for previewing. It skips most validation; the real
// rules live in `crates/humlx-core`.

import type { Fixture, Live, NewFixtures, OutputSettings, PatchView, Profile } from "./types";

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
    mode.channels.forEach((c, i) => {
      let value = tests.get(f.id)?.get(i) ?? c.default ?? 0;
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
