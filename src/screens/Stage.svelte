<script lang="ts">
  // The Stage page: every light on the stage plan, drawn with what it is
  // putting out now: its colour and brightness, and the patch of floor it
  // lights. Movers aim from their calibration; fixed lights are placed by hand
  // with where they point. Tap a light to select it.

  import { onMount, untrack } from "svelte";
  import NumberField from "../components/NumberField.svelte";
  import StageWindow from "../components/StageWindow.svelte";
  import { api } from "../lib/api";
  import { drag, type DragPoint } from "../lib/drag";
  import { tapSelectMode } from "../lib/selection";
  import { attempt, show } from "../lib/show.svelte";
  import {
    beam,
    defaultStage,
    floorAt,
    fromMeasuredPart,
    hit,
    measuredPart,
    referenceAt,
    pool,
    towards,
    type Extra,
    type Placed,
    type Stage,
    type Vec3,
  } from "../lib/stage";
  import type { Channel, Fixture } from "../lib/types";
  import { cssColour, defaultBeam, lookOf, type Look } from "../lib/visual";

  let stage = $state<Stage>(defaultStage());

  async function load() {
    const loaded = await attempt(() => api.stage());
    if (loaded) stage = loaded;
  }

  onMount(load);

  // Another screen changed the plan or calibration.
  $effect(() => {
    if (show.stageRevision) untrack(load);
  });

  type Tab = "look" | "place";
  let tab = $state<Tab>("look");
  /** The Stage window, open for calibrating a mover or setting up the plan. */
  let windowFor = $state<{ fixture?: number; setup?: boolean } | null>(null);

  // --- The lights -----------------------------------------------------------

  interface Light {
    fixture: Fixture;
    /** 0 for the fixture itself, then its copies on the same address. */
    copy: number;
    key: string;
    label: string;
    channels: Channel[];
    mover: boolean;
    calibrated: boolean;
    placed: Placed;
    /** Where it hangs, when known. */
    at: Vec3 | null;
    /** Which way its beam goes, when known. */
    axis: Vec3 | null;
    degrees: number;
    look: Look;
    /** The floor it lights, when it is on and pointing down. */
    patch: Vec3[] | null;
    /** Where the middle of its beam comes down. */
    centre: Vec3 | null;
    selected: boolean;
  }

  const placementOf = (id: number): Placed =>
    stage.positions.find((p) => p.fixture === id) ?? { fixture: id };

  /** Where one light hangs and points: the fixture itself, or one of its copies. */
  interface Body {
    x?: number | null;
    y?: number | null;
    z?: number | null;
    aim?: [number, number] | null;
  }

  function bodyOf(placed: Placed, copy: number): Body {
    return copy === 0 ? placed : (placed.copies?.[copy - 1] ?? {});
  }

  const letter = (copy: number) => String.fromCharCode(97 + copy);

  const lights = $derived.by((): Light[] => {
    const selection = new Set(show.programmer.selection);
    return show.patch.fixtures.flatMap((fixture) => {
      const channels = show.channels(fixture);
      const mover = channels.some((c) => c.attribute === "pan");
      const placed = placementOf(fixture.id);
      const solution = mover ? (stage.movers.find((m) => m.fixture === fixture.id)?.solution ?? null) : null;
      const look = lookOf(fixture, channels, show.live.universe);
      const degrees = placed.beam ?? defaultBeam(channels);
      const copies = mover ? 0 : (placed.copies?.length ?? 0);
      return Array.from({ length: copies + 1 }, (_, copy): Light => {
        const body = bodyOf(placed, copy);
        const known = body.x != null && body.y != null && body.z != null;
        const at: Vec3 | null = solution ? [...solution.position] : known ? [body.x!, body.y!, body.z!] : null;
        let axis: Vec3 | null = null;
        if (solution && look.position) axis = beam(solution, look.position[0], look.position[1]);
        else if (!mover && at)
          axis = body.aim ? towards(at, [body.aim[0], body.aim[1], floorAt(stage, ...body.aim)]) : [0, 0, -1];
        const centre = at && axis ? hit(stage, at, axis) : null;
        const lit = look.intensity > 0.01;
        return {
          fixture,
          copy,
          key: `${fixture.id}-${copy}`,
          label: copies > 0 ? `${fixture.id}${letter(copy)}` : String(fixture.id),
          channels,
          mover,
          calibrated: !!solution,
          placed,
          at,
          axis,
          degrees,
          look,
          patch: at && axis && lit ? pool(stage, at, axis, degrees, reachOf(at, centre)) : null,
          centre,
          selected: selection.has(fixture.id),
        };
      });
    });
  });

  /**
   * How far out to draw a beam's edges: a wide beam at a low angle would
   * otherwise streak off across the room, where it gives little light.
   */
  function reachOf(at: Vec3, centre: Vec3 | null) {
    const throwTo = centre ? Math.hypot(centre[0] - at[0], centre[1] - at[1], centre[2] - at[2]) : 6;
    return Math.min(30, throwTo * 2);
  }

  /** The middle of a pool and how far it spreads, for its fade towards the edge. */
  function spread(l: Light): { cx: number; cy: number; r: number } {
    const points = l.patch!;
    const [cx, cy] = l.centre ?? [0, 1].map((i) => points.reduce((sum, p) => sum + p[i], 0) / points.length);
    const r = Math.max(0.1, ...points.map((p) => Math.hypot(p[0] - cx, p[1] - cy)));
    return { cx, cy: -cy, r };
  }

  const onPlan = $derived(lights.filter((l) => l.at));
  const offPlan = $derived(lights.filter((l) => !l.at));

  // --- The plan -------------------------------------------------------------

  const view = $derived.by(() => {
    let left = -stage.width / 2 - stage.margin.left;
    let right = stage.width / 2 + stage.margin.right;
    let front = -stage.margin.front;
    let back = stage.depth + stage.margin.back;
    for (const l of onPlan) {
      left = Math.min(left, l.at![0] - 0.6);
      right = Math.max(right, l.at![0] + 0.6);
      front = Math.min(front, l.at![1] - 0.6);
      back = Math.max(back, l.at![1] + 0.6);
    }
    return { left, right, front, back };
  });

  // Up stage is at the top, so y is flipped on screen.
  const viewBox = $derived(`${view.left} ${-view.back} ${view.right - view.left} ${view.back - view.front}`);
  const unit = $derived(Math.max(view.right - view.left, view.back - view.front) / 100);

  const side = $derived.by(() => {
    const low = -stage.riser - 0.3;
    const high = Math.max(2.2, ...onPlan.map((l) => l.at![2] + 0.6), ...stage.scenery.map((p) => p.height + 0.4));
    return { low, high, box: `${view.front} ${-high} ${view.back - view.front} ${high - low}` };
  });

  const gridLines = $derived.by(() => {
    const xs: number[] = [];
    const ys: number[] = [];
    for (let x = Math.ceil(view.left); x <= view.right; x++) xs.push(x);
    for (let y = Math.ceil(view.front); y <= view.back; y++) ys.push(y);
    return { xs, ys };
  });

  const polygon = (points: Vec3[]) => points.map((p) => `${p[0]},${-p[1]}`).join(" ");

  /** The beam seen from the side: from the light to the near and far edges of its pool. */
  function sideCone(l: Light): string {
    const ys = l.patch!.map((p) => p[1]);
    const near = l.patch![ys.indexOf(Math.min(...ys))];
    const far = l.patch![ys.indexOf(Math.max(...ys))];
    return [l.at!, near, far].map((p) => `${p[1]},${-p[2]}`).join(" ");
  }

  /** Brightness on screen: dim lights still show, and off ones are dark. */
  const glow = (l: Light) => 0.18 + 0.62 * Math.sqrt(l.look.intensity);

  let svg: SVGSVGElement;

  function toStage(p: DragPoint): { x: number; y: number } | null {
    const ctm = svg.getScreenCTM();
    if (!ctm) return null;
    const point = new DOMPoint(p.clientX, p.clientY).matrixTransform(ctm.inverse());
    return { x: point.x, y: -point.y };
  }

  /** The light drawn under a spot on the plan, if any. */
  function lightAt(at: { x: number; y: number }): Light | undefined {
    let best: { light: Light; distance: number } | undefined;
    for (const light of onPlan) {
      const distance = Math.hypot(light.at![0] - at.x, light.at![1] - at.y);
      if (distance <= unit * 2.6 && (!best || distance < best.distance)) best = { light, distance };
    }
    return best?.light;
  }

  let dragging: "point" | "position" | "aim" | null = null;

  function planDown(p: DragPoint) {
    const at = toStage(p);
    if (!at) return false;
    const light = lightAt(at);
    if (tab === "look") {
      if (pointing && !light) {
        dragging = "point";
        aimAt(at);
        return;
      }
      if (light) tapLight(light.fixture.id);
      return false;
    }
    if (light && (light.fixture.id !== picked || light.copy !== pickedCopy)) {
      pick(light.fixture.id, light.copy);
      return false;
    }
    if (picked == null) return false;
    dragging = tapSets;
    placeAt(at);
  }

  function planMove(p: DragPoint) {
    const at = toStage(p);
    if (!at || !dragging) return;
    if (dragging === "point") aimAt(at);
    else placeAt(at);
  }

  function planEnd() {
    if ((dragging === "position" || dragging === "aim") && picked != null) {
      const done = dragging;
      save(picked);
      // A fixed light placed for the first time goes on to where it points.
      if (done === "position" && !pickedLight?.mover && !bodyOf(placementOf(picked), pickedCopy).aim) tapSets = "aim";
    }
    dragging = null;
  }

  // --- Look -----------------------------------------------------------------

  let pointing = $state(false);
  let height = $state(0);
  let target = $state<{ x: number; y: number } | null>(null);

  function tapLight(id: number) {
    show.program(() => api.select([id], tapSelectMode([id], show.programmer.selection)));
  }

  const selectedMovers = $derived(lights.filter((l) => l.selected && l.mover));
  const aimable = $derived(selectedMovers.filter((l) => l.calibrated).length);

  let sending = false;
  let queued: { x: number; y: number; height: number } | null = null;

  async function send(spot: { x: number; y: number; height: number }) {
    queued = spot;
    if (sending) return;
    sending = true;
    while (queued) {
      const next = queued;
      queued = null;
      await show.program(() => api.pointAt(next.x, next.y, next.height));
    }
    sending = false;
  }

  function aimAt(spot: { x: number; y: number }) {
    target = spot;
    send({ ...spot, height });
  }

  function setHeight(next: number) {
    height = next;
    if (target) send({ ...target, height });
  }

  // --- Place ----------------------------------------------------------------

  let picked = $state<number | null>(null);
  /** Which light on the picked fixture's address: 0 for the fixture, then its copies. */
  let pickedCopy = $state(0);
  let tapSets = $state<"position" | "aim">("position");
  /** Height given to a light placed for the first time: the last one used. */
  let lastHeight = 4.5;

  const pickedLight = $derived(lights.find((l) => l.fixture.id === picked && l.copy === pickedCopy));
  const pickedBodies = $derived(picked == null ? [] : lights.filter((l) => l.fixture.id === picked));

  function pick(id: number, copy = 0) {
    picked = id;
    pickedCopy = copy;
    const light = lights.find((l) => l.fixture.id === id && l.copy === copy);
    const body = light && bodyOf(light.placed, copy);
    tapSets = !light?.mover && light?.at && !body?.aim ? "aim" : "position";
  }

  const round5 = (v: number) => Math.round(v * 20) / 20;

  /** Changes a placement on screen; `save` sends it. */
  function edit(id: number, change: Partial<Placed>) {
    const next = { ...placementOf(id), ...change, fixture: id };
    const others = stage.positions.filter((p) => p.fixture !== id);
    stage.positions = [...others, next].sort((a, b) => a.fixture - b.fixture);
  }

  /** Changes where the picked light hangs or points, whether it's the fixture or a copy. */
  function editBody(change: Body) {
    if (picked == null) return;
    if (pickedCopy === 0) return edit(picked, change);
    const copies = [...(placementOf(picked).copies ?? [])];
    const old = copies[pickedCopy - 1];
    if (!old) return;
    const next = { ...old, ...change };
    copies[pickedCopy - 1] = { x: next.x ?? old.x, y: next.y ?? old.y, z: next.z ?? old.z, aim: next.aim };
    edit(picked, { copies });
  }

  function placeAt(at: { x: number; y: number }) {
    if (picked == null) return;
    const spot: [number, number] = [round5(at.x), round5(at.y)];
    if (tapSets === "aim") editBody({ aim: spot });
    else editBody({ x: spot[0], y: spot[1], z: bodyOf(placementOf(picked), pickedCopy).z ?? lastHeight });
  }

  async function save(id: number) {
    const placed = $state.snapshot(placementOf(id));
    const body = bodyOf(placed, pickedCopy);
    if (body.z != null) lastHeight = body.z;
    const saved = await attempt(() => api.setPlacement(placed));
    if (saved) stage = saved;
    else load();
  }

  function setField(change: Partial<Placed>) {
    if (picked == null) return;
    edit(picked, change);
    save(picked);
  }

  function setBody(change: Body) {
    if (picked == null) return;
    editBody(change);
    save(picked);
  }

  function setAimPart(i: 0 | 1, v: number | null) {
    if (picked == null || pickedLight?.at == null) return;
    const aim = [...(bodyOf(placementOf(picked), pickedCopy).aim ?? [pickedLight.at[0], pickedLight.at[1]])] as [
      number,
      number,
    ];
    if (v != null) aim[i] = v;
    setBody({ aim });
  }

  /** Another light on the same address, hung a metre to the right of the last one to start with. */
  function addCopy() {
    if (picked == null) return;
    const placed = placementOf(picked);
    const last = bodyOf(placed, placed.copies?.length ?? 0);
    const copy: Extra = {
      x: round5((last.x ?? 0) + 1),
      y: last.y ?? 0,
      z: last.z ?? lastHeight,
      aim: last.aim ? [last.aim[0] + 1, last.aim[1]] : null,
    };
    const copies = [...(placed.copies ?? []), copy];
    setField({ copies });
    pick(picked, copies.length);
    tapSets = "position";
  }

  function removeFromPlan() {
    if (picked == null) return;
    if (pickedCopy > 0) {
      const copies = (placementOf(picked).copies ?? []).filter((_, i) => i !== pickedCopy - 1);
      pickedCopy = 0;
      return setField({ copies });
    }
    setField({ x: null, y: null, z: null, aim: null, beam: null, copies: [] });
  }

  function status(l: Light) {
    if (l.mover) return l.calibrated ? "Calibrated" : l.at ? "Not calibrated" : "Not placed";
    if (!l.at) return l.placed.x != null ? "Needs a height" : "Not placed";
    const more = l.placed.copies?.length ? ` · ${l.placed.copies.length + 1} lights` : "";
    return (l.placed.aim ? "Placed and aimed" : "Placed, pointing down") + more;
  }

  function closeWindow() {
    windowFor = null;
    load();
  }
</script>

<div class="stage-page">
  <div class="plan-area" use:drag={{ start: planDown, move: planMove, end: planEnd }}>
    <svg
      bind:this={svg}
      class="plan"
      class:aiming={(tab === "look" && pointing) || (tab === "place" && picked != null)}
      {viewBox}
      preserveAspectRatio="xMidYMid meet"
      role="application"
      aria-label="Stage plan"
    >
      <defs>
        <filter id="soft" x="-20%" y="-20%" width="140%" height="140%">
          <feGaussianBlur stdDeviation={unit * 0.4} />
        </filter>
      </defs>
      <rect class="studio" x={view.left} y={-view.back} width={view.right - view.left} height={view.back - view.front} />
      {#each gridLines.xs as x (x)}
        <line class="grid" x1={x} x2={x} y1={-view.back} y2={-view.front} stroke-width={unit * 0.15} />
      {/each}
      {#each gridLines.ys as y (y)}
        <line class="grid" x1={view.left} x2={view.right} y1={-y} y2={-y} stroke-width={unit * 0.15} />
      {/each}
      <rect class="stage" x={-stage.width / 2} y={-stage.depth} width={stage.width} height={stage.depth} stroke-width={unit * 0.4} />
      <text class="caption" x="0" y={unit * 3.2} font-size={unit * 2.2} text-anchor="middle">Front of stage</text>

      <g class="pools" filter="url(#soft)">
        {#each onPlan as l (l.key)}
          {#if l.patch}
            {@const g = spread(l)}
            {@const colour = cssColour(l.look.colour)}
            {@const strength = 0.2 + 0.65 * l.look.intensity}
            <radialGradient id="pool-{l.key}" gradientUnits="userSpaceOnUse" cx={g.cx} cy={g.cy} r={g.r}>
              <stop offset="0" stop-color={colour} stop-opacity={strength} />
              <stop offset="0.6" stop-color={colour} stop-opacity={strength * 0.7} />
              <stop offset="1" stop-color={colour} stop-opacity={strength * 0.15} />
            </radialGradient>
            <polygon points={polygon(l.patch)} fill="url(#pool-{l.key})" />
          {/if}
        {/each}
      </g>

      {#each stage.scenery as piece, i (i)}
        <polyline class="scenery" points={piece.points.map(([x, y]) => `${x},${-y}`).join(" ")} stroke-width={unit * 0.9} />
      {/each}

      {#each onPlan as l (l.key)}
        {#if l.centre && (l.patch || l.selected)}
          <line
            class="axis"
            class:selected={l.selected}
            x1={l.at![0]}
            y1={-l.at![1]}
            x2={l.centre[0]}
            y2={-l.centre[1]}
            stroke={l.patch ? cssColour(l.look.colour) : undefined}
            stroke-width={unit * 0.25}
          />
        {/if}
      {/each}

      {#if tab === "place"}
        {@const [rx, ry] = referenceAt(stage)}
        <g class="reference" transform="translate({rx} {-ry})">
          <line x2={unit * 5} stroke-width={unit * 0.4} />
          <line y2={-unit * 5} stroke-width={unit * 0.4} />
          <circle r={unit * 0.9} />
          <text x={unit * 1.2} y={unit * 2.6} font-size={unit * 1.8}>0</text>
        </g>
      {/if}

      {#if tab === "place" && pickedLight?.at && !pickedLight.mover}
        {@const aim = bodyOf(pickedLight.placed, pickedCopy).aim ?? [pickedLight.at[0], pickedLight.at[1]]}
        <g class="aim-mark" transform="translate({aim[0]} {-aim[1]})">
          <circle r={unit * 2} stroke-width={unit * 0.35} />
          <line x1={-unit * 3} x2={unit * 3} stroke-width={unit * 0.2} />
          <line y1={-unit * 3} y2={unit * 3} stroke-width={unit * 0.2} />
        </g>
      {/if}

      {#each onPlan as l (l.key)}
        {@const fill = cssColour(l.look.colour, glow(l))}
        <g
          class="light"
          class:selected={l.selected}
          class:picked={tab === "place" && picked === l.fixture.id && pickedCopy === l.copy}
          transform="translate({l.at![0]} {-l.at![1]})"
        >
          {#if l.mover}
            <circle r={unit * 2} {fill} stroke-width={unit * 0.4} />
          {:else}
            <rect x={-unit * 1.8} y={-unit * 1.8} width={unit * 3.6} height={unit * 3.6} rx={unit * 0.5} {fill} stroke-width={unit * 0.4} />
          {/if}
          <text y={unit * 0.7} font-size={unit * 1.8} text-anchor="middle" class:dark={l.look.intensity > 0.45}>{l.label}</text>
        </g>
      {/each}

      {#if tab === "look" && pointing && target}
        <g class="target" transform="translate({target.x} {-target.y})">
          <circle r={unit * 2.6} stroke-width={unit * 0.4} />
        </g>
      {/if}
    </svg>

    <div class="side-wrap">
      <svg class="side-view" viewBox={side.box} preserveAspectRatio="xMidYMid meet" aria-label="Side view">
        <rect class="studio" x={view.front} y={-side.high} width={view.back - view.front} height={side.high - side.low} />
        <line class="floor" x1={view.front} x2={view.back} y1={stage.riser} y2={stage.riser} />
        <rect class="stage" x="0" y="0" width={stage.depth} height={Math.max(stage.riser, 0.04)} />
        {#each stage.scenery as piece, i (i)}
          {#if piece.points.length > 1}
            {@const ys = piece.points.map((p) => p[1])}
            {@const floor = floorAt(stage, piece.points[0][0], piece.points[0][1])}
            <rect class="scenery-side" x={Math.min(...ys)} y={-floor - piece.height} width={Math.max(Math.max(...ys) - Math.min(...ys), 0.08)} height={piece.height} />
          {/if}
        {/each}
        {#each onPlan as l (l.key)}
          {#if l.patch}
            <polygon class="cone" points={sideCone(l)} fill={cssColour(l.look.colour)} fill-opacity={0.08 + 0.3 * l.look.intensity} />
          {/if}
        {/each}
        {#each onPlan as l (l.key)}
          <circle class="dot" class:selected={l.selected} cx={l.at![1]} cy={-l.at![2]} r="0.16" fill={cssColour(l.look.colour, glow(l))} />
        {/each}
      </svg>
      <p class="side-caption">Side view · front of stage on the left</p>
    </div>
  </div>

  <aside class="panel side scroll">
    <div class="segmented" role="tablist" aria-label="Stage page mode">
      <button role="tab" aria-selected={tab === "look"} onclick={() => (tab = "look")}>Look</button>
      <button role="tab" aria-selected={tab === "place"} onclick={() => (tab = "place")}>Place lights</button>
    </div>

    {#if tab === "look"}
      <p class="lead">Tap a light to select it. Each one shows its colour and the floor it lights, live.</p>
      <div class="row wrap">
        <button aria-pressed={pointing} onclick={() => (pointing = !pointing)}>Point movers</button>
        {#if pointing}
          <button aria-pressed={height === 0} onclick={() => setHeight(0)}>Floor</button>
          <button aria-pressed={height === 1.6} onclick={() => setHeight(1.6)}>Head height</button>
        {/if}
      </div>
      {#if pointing}
        <p class="muted small">
          {#if aimable > 0}
            Tap or drag on the floor to point {aimable === 1 ? "the selected mover" : `the ${aimable} selected movers`} there.
          {:else if selectedMovers.length > 0}
            The selected movers aren't calibrated yet.
          {:else}
            Select movers, then tap the floor to point them.
          {/if}
        </p>
      {/if}
      {#if offPlan.length > 0}
        <p class="muted">
          Not on the plan yet: {offPlan.map((l) => l.fixture.id).join(", ")}.
          <button class="ghost link" onclick={() => (tab = "place")}>Place them</button>
        </p>
      {/if}
      <button class="ghost" onclick={() => (windowFor = { setup: true })}>Stage size, marks and scenery</button>
    {:else}
      <p class="muted small">
        Pick a light, then tap the plan where it hangs. For a fixed light, tap again where it points. Movers point
        from their calibration. Positions are in metres from the orange 0, which you can move in
        <button class="ghost link" onclick={() => (windowFor = { setup: true })}>Stage set up</button>.
      </p>
      <div class="lights">
        {#each lights.filter((l) => l.copy === 0) as l (l.fixture.id)}
          <button aria-pressed={picked === l.fixture.id} onclick={() => pick(l.fixture.id)}>
            <span class="chip" style:background={cssColour(l.look.colour, glow(l))}></span>
            <span class="name"><strong>{l.fixture.id}</strong> {l.fixture.name}</span>
            <span class="small" class:ok={!!l.at && (!l.mover || l.calibrated)}>{status(l)}</span>
          </button>
        {/each}
      </div>

      {#if pickedLight}
        {@const p = pickedLight.placed}
        {@const b = bodyOf(p, pickedCopy)}
        <section class="placing">
          <h3>{pickedLight.label} {pickedLight.fixture.name}</h3>
          {#if !pickedLight.mover && pickedLight.at}
            <div class="row wrap copies" role="group" aria-label="Lights on this address">
              <span class="muted small">
                {pickedBodies.length > 1
                  ? `${pickedBodies.length} lights share this address, all showing its output`
                  : "Another light on the same address, such as a pair of pars?"}
              </span>
              {#if pickedBodies.length > 1}
                {#each pickedBodies as l (l.key)}
                  <button aria-pressed={pickedCopy === l.copy} onclick={() => pick(l.fixture.id, l.copy)}>{l.label}</button>
                {/each}
              {/if}
              <button onclick={addCopy}>Add another on this address</button>
            </div>
          {/if}
          {#if !pickedLight.mover}
            <div class="row wrap tap-sets" role="group" aria-label="Tapping the plan sets">
              <span class="muted small">Tapping the plan sets</span>
              <button aria-pressed={tapSets === "position"} onclick={() => (tapSets = "position")}>Where it hangs</button>
              <button aria-pressed={tapSets === "aim"} disabled={!pickedLight.at} onclick={() => (tapSets = "aim")}>Where it points</button>
            </div>
          {/if}
          <div class="fields">
            <NumberField label="Across (m)" value={measuredPart(stage, b.x, 0)} onchange={(v) => setBody({ x: fromMeasuredPart(stage, v, 0) })} fractional increment={0.05} min={-500} max={500} />
            <NumberField label="Up stage (m)" value={measuredPart(stage, b.y, 1)} onchange={(v) => setBody({ y: fromMeasuredPart(stage, v, 1) })} fractional increment={0.05} min={-500} max={500} />
            <NumberField label="Height above stage deck (m)" value={b.z ?? null} onchange={(v) => setBody({ z: v })} fractional increment={0.05} min={-50} max={50} />
          </div>
          {#if stage.riser > 0}
            <p class="muted small">
              The stage is {stage.riser} m above the studio floor{#if b.z != null}, so this hangs
                {(b.z + stage.riser).toFixed(2)} m above the studio floor{/if}.
            </p>
          {/if}
          {#if pickedLight.mover}
            <p class="muted small">
              {#if pickedLight.calibrated}
                Drawn where its calibration puts it. Anything measured here is held by the calibration too.
              {:else}
                Calibrate it to see where it points; anything measured here helps the calibration.
              {/if}
            </p>
            <button onclick={() => (windowFor = { fixture: picked! })}>Calibrate</button>
          {:else}
            <div class="fields two">
              <NumberField label="Points at, across (m)" value={measuredPart(stage, b.aim?.[0], 0)} onchange={(v) => setAimPart(0, fromMeasuredPart(stage, v, 0))} fractional increment={0.1} min={-500} max={500} />
              <NumberField label="Points at, up stage (m)" value={measuredPart(stage, b.aim?.[1], 1)} onchange={(v) => setAimPart(1, fromMeasuredPart(stage, v, 1))} fractional increment={0.1} min={-500} max={500} />
            </div>
            <div class="row wrap">
              <button disabled={!b.aim} onclick={() => setBody({ aim: null })}>Straight down</button>
            </div>
          {/if}
          <NumberField
            label={pickedBodies.length > 1 ? "Beam width, all of them (°)" : "Beam width (°)"}
            value={p.beam ?? null}
            placeholder={String(defaultBeam(pickedLight.channels))}
            onchange={(v) => setField({ beam: v })}
            increment={5}
            min={1}
            max={120}
          />
          {#if pickedCopy > 0}
            <button class="danger" onclick={removeFromPlan}>Remove {pickedLight.label}</button>
          {:else if p.x != null || p.y != null || p.z != null || p.aim || p.beam != null}
            <button class="danger" onclick={removeFromPlan}>
              {pickedBodies.length > 1 ? "Take them all off the plan" : "Take off the plan"}
            </button>
          {/if}
        </section>
      {/if}
    {/if}
  </aside>
</div>

{#if windowFor}
  <StageWindow fixture={windowFor.fixture} startMode={windowFor.setup ? "setup" : undefined} onclose={closeWindow} />
{/if}

<style>
  .stage-page {
    display: grid;
    grid-template-columns: minmax(0, 1fr) 420px;
    gap: 12px;
    height: 100%;
    min-height: 0;
  }

  .plan-area {
    display: grid;
    grid-template-rows: minmax(0, 1fr) clamp(140px, 24%, 260px);
    gap: 10px;
    min-height: 0;
    touch-action: none;
  }
  .plan {
    width: 100%;
    height: 100%;
    touch-action: none;
    user-select: none;
    -webkit-user-select: none;
  }
  .plan.aiming {
    cursor: crosshair;
  }
  .studio {
    fill: #0a0c0f;
  }
  .grid {
    stroke: #232833;
  }
  .stage {
    fill: #1a1f28;
    stroke: #4a5363;
  }
  .caption {
    fill: var(--muted);
    font-family: var(--mono);
  }
  .pools {
    mix-blend-mode: screen;
    pointer-events: none;
  }
  .pools polygon {
    mix-blend-mode: screen;
  }
  .scenery {
    fill: none;
    stroke: color-mix(in srgb, #c56cff 70%, transparent);
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .axis {
    stroke: color-mix(in srgb, var(--text) 25%, transparent);
    stroke-opacity: 0.7;
  }
  .light circle,
  .light rect {
    stroke: #4a5363;
  }
  .light.selected circle,
  .light.selected rect {
    stroke: var(--text);
  }
  .light.picked circle,
  .light.picked rect {
    stroke: var(--warning);
  }
  .light text {
    fill: var(--text);
    font-family: var(--mono);
    pointer-events: none;
  }
  .light text.dark {
    fill: #0a0c0f;
  }
  .reference line {
    stroke: var(--warning);
  }
  .reference circle,
  .reference text {
    fill: var(--warning);
    font-family: var(--mono);
  }
  .aim-mark circle {
    fill: none;
    stroke: var(--warning);
  }
  .aim-mark line {
    stroke: var(--warning);
  }
  .target circle {
    fill: none;
    stroke: var(--text);
  }

  .side-wrap {
    position: relative;
    min-height: 0;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    overflow: hidden;
  }
  .side-view {
    width: 100%;
    height: 100%;
  }
  .side-view * {
    vector-effect: non-scaling-stroke;
  }
  .side-view .floor {
    stroke: #4a5363;
    stroke-width: 2;
  }
  .side-view .stage {
    stroke-width: 2;
  }
  .scenery-side {
    fill: color-mix(in srgb, #c56cff 35%, transparent);
    stroke: #c56cff;
    stroke-width: 1;
  }
  .cone {
    mix-blend-mode: screen;
  }
  .dot {
    stroke: #4a5363;
    stroke-width: 2;
  }
  .dot.selected {
    stroke: var(--text);
  }
  .side-caption {
    position: absolute;
    top: 6px;
    left: 10px;
    margin: 0;
    pointer-events: none;
    font-size: 0.8rem;
    color: var(--muted);
  }

  .side {
    display: flex;
    flex-direction: column;
    gap: 14px;
    min-height: 0;
    min-width: 0;
    padding: 14px;
    box-sizing: border-box;
  }
  .lead {
    margin: 0;
    font-size: 1.05rem;
  }
  .small {
    font-size: 0.85rem;
  }
  .wrap {
    flex-wrap: wrap;
    gap: 8px;
  }
  button[aria-pressed="true"] {
    background: var(--accent);
    color: var(--accent-text);
  }
  .link {
    padding: 0 4px;
    min-height: 0;
    text-decoration: underline;
  }
  .lights {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 6px;
  }
  .lights button {
    display: grid;
    grid-template-columns: 14px 1fr;
    column-gap: 8px;
    row-gap: 2px;
    justify-items: start;
    align-items: center;
    text-align: left;
  }
  .lights .chip {
    width: 14px;
    height: 14px;
    border-radius: 3px;
    border: 1px solid var(--line);
  }
  .lights .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    max-width: 100%;
  }
  .lights .small {
    grid-column: 2;
  }
  .ok {
    color: var(--ok);
  }
  button[aria-pressed="true"] .ok {
    color: inherit;
  }
  .placing {
    display: grid;
    gap: 10px;
    border-top: 1px solid var(--line);
    padding-top: 10px;
  }
  .placing h3,
  .placing p {
    margin: 0;
  }
  .fields {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
  }
  .placing > *,
  .fields > :global(*) {
    min-width: 0;
  }
  .tap-sets span,
  .copies span {
    flex-basis: 100%;
  }
</style>
