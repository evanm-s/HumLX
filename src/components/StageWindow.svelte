<script lang="ts">
  // The stage plan, popped out over the rest of the screen so it is big
  // enough to aim precisely. Point: tap or drag on the plan and the selected,
  // calibrated movers aim there. Calibrate: point one mover at each floor mark
  // so HumLX can work out where it hangs. Set up: the stage's size and marks.

  import { onDestroy, onMount, untrack } from "svelte";
  import NumberField from "./NumberField.svelte";
  import PositionPad from "./PositionPad.svelte";
  import { api } from "../lib/api";
  import { drag, type DragPoint } from "../lib/drag";
  import { formKeys } from "../lib/keys";
  import { attempt, show } from "../lib/show.svelte";
  import {
    defaultMarks,
    defaultStage,
    floorAt,
    beamEnd,
    meets,
    knownOf,
    minReadings,
    onStage,
    fromMeasuredPart,
    measured as fromReference,
    measuredPart,
    referenceAt,
    REFERENCE_POINTS,
    type ReferencePoint,
    type Known,
    type Mark,
    type Plan,
    type Stage,
  } from "../lib/stage";
  import type { Fixture } from "../lib/types";

  interface Props {
    onclose: () => void;
    /** Opens on Calibrate for this fixture, as the Patch page does. */
    fixture?: number;
    /** Opens on this tab, such as Set up from the Stage page. */
    startMode?: "point" | "calibrate" | "setup";
  }

  let { onclose, fixture: startFixture, startMode }: Props = $props();

  type Mode = "point" | "calibrate" | "setup";
  const MODE_KEY = "humlx.stage.mode";
  // svelte-ignore state_referenced_locally
  let mode = $state<Mode>(startMode ?? (startFixture != null ? "calibrate" : readMode()));
  let stage = $state<Stage>(defaultStage());

  function readMode(): Mode {
    try {
      const saved = localStorage.getItem(MODE_KEY);
      if (saved === "point" || saved === "calibrate" || saved === "setup") return saved;
    } catch {
      // Storage can be unavailable; start on Point.
    }
    return "point";
  }

  function setMode(next: Mode) {
    if (mode === "setup" && next !== "setup") draft = null;
    if (next === "setup") draft = copyPlan(stage);
    if (next === "calibrate") startCalibrating();
    else stopHighlight();
    mode = next;
    try {
      localStorage.setItem(MODE_KEY, next === "setup" ? "point" : next);
    } catch {
      // Only a convenience.
    }
  }

  // Another screen changed the plan or calibration.
  $effect(() => {
    if (show.stageRevision)
      untrack(async () => {
        const loaded = await attempt(() => api.stage());
        if (loaded) stage = loaded;
      });
  });

  onMount(async () => {
    const loaded = await attempt(() => api.stage());
    if (loaded) stage = loaded;
    if (mode === "calibrate") startCalibrating();
    if (mode === "setup") draft = copyPlan(stage);
  });

  // --- Fixtures -------------------------------------------------------------

  const movers = $derived(show.patch.fixtures.filter((f) => show.channels(f).some((c) => c.attribute === "pan")));
  const solutionOf = (id: number) => stage.movers.find((m) => m.fixture === id)?.solution ?? null;
  const calibrationOf = (id: number) => stage.movers.find((m) => m.fixture === id);

  /** Pan and tilt as the fixture is getting them now, from the output. */
  function outputPosition(f: Fixture): [number, number] | null {
    const channels = show.channels(f);
    const level = (attribute: "pan" | "tilt") => {
      const coarse = channels.findIndex((c) => c.attribute === attribute && !c.fineOf);
      if (coarse < 0) return null;
      const fine = channels.findIndex((c) => c.attribute === attribute && c.fineOf === channels[coarse].name);
      const at = (i: number) => show.live.universe[f.address - 1 + i] ?? 0;
      return fine < 0 ? at(coarse) / 255 : (at(coarse) * 256 + at(fine)) / 65535;
    };
    const pan = level("pan");
    const tilt = level("tilt");
    return pan == null || tilt == null ? null : [pan, tilt];
  }

  interface Drawn {
    fixture: Fixture;
    /** The mover, in metres. */
    at: [number, number, number];
    /** Where its beam lands, or how far along to draw it when it doesn't. */
    end: [number, number, number] | null;
    lands: boolean;
    /** Where the beam passes the Height control's height, when pointing above the floor. */
    crosses: [number, number, number] | null;
    selected: boolean;
  }

  const drawn = $derived.by((): Drawn[] =>
    movers.flatMap((fixture) => {
      const solution = solutionOf(fixture.id);
      if (!solution) return [];
      const position = outputPosition(fixture);
      const end = position ? beamEnd(plan, solution, position[0], position[1]) : null;
      return [
        {
          fixture,
          at: [...solution.position] as [number, number, number],
          end: end?.at ?? null,
          lands: end?.lands ?? false,
          crosses:
            position && mode === "point" && height > 0
              ? meets(plan, solution, position[0], position[1], height)
              : null,
          selected: show.programmer.selection.includes(fixture.id),
        },
      ];
    }),
  );

  // --- The plan -------------------------------------------------------------

  /** The plan being edited on Set up, otherwise the saved one. */
  let draft = $state<Plan | null>(null);
  const plan = $derived<Plan>(draft ?? stage);

  function copyPlan(s: Plan): Plan {
    return {
      width: s.width,
      depth: s.depth,
      margin: { ...s.margin },
      riser: s.riser,
      marks: s.marks.map((m) => ({ ...m })),
      scenery: (s.scenery ?? []).map((p) => ({ ...p, points: p.points.map((pt) => [...pt] as [number, number]) })),
      reference: { ...(s.reference ?? { from: "front-middle", x: 0, y: 0 }) },
    };
  }

  const margins = [
    ["left", "Left (m)"],
    ["right", "Right (m)"],
    ["front", "In front (m)"],
    ["back", "Behind (m)"],
  ] as const;

  // The view takes in the studio margin and any mover hung outside it.
  const view = $derived.by(() => {
    let left = -plan.width / 2 - plan.margin.left;
    let right = plan.width / 2 + plan.margin.right;
    let front = -plan.margin.front;
    let back = plan.depth + plan.margin.back;
    for (const d of drawn) {
      left = Math.min(left, d.at[0] - 0.6);
      right = Math.max(right, d.at[0] + 0.6);
      front = Math.min(front, d.at[1] - 0.6);
      back = Math.max(back, d.at[1] + 0.6);
    }
    return { left, right, front, back };
  });

  // Up stage is at the top, so y is flipped on screen.
  const viewBox = $derived(`${view.left} ${-view.back} ${view.right - view.left} ${view.back - view.front}`);
  const unit = $derived(Math.max(view.right - view.left, view.back - view.front) / 100);

  // The side view looks across the stage: front of stage on the left, up stage
  // on the right, height going up.
  const side = $derived.by(() => {
    const low = -plan.riser - 0.3;
    const high = Math.max(
      2.2,
      height + 0.5,
      ...drawn.map((d) => d.at[2] + 0.6),
      ...(plan.scenery ?? []).map((p) => p.height + 0.4),
    );
    return { low, high, box: `${view.front} ${-high} ${view.back - view.front} ${high - low}` };
  });

  const gridLines = $derived.by(() => {
    const xs: number[] = [];
    const ys: number[] = [];
    for (let x = Math.ceil(view.left); x <= view.right; x++) xs.push(x);
    for (let y = Math.ceil(view.front); y <= view.back; y++) ys.push(y);
    return { xs, ys };
  });

  let svg: SVGSVGElement;

  /** A screen point in stage metres. */
  function toStage(p: DragPoint): { x: number; y: number } | null {
    const ctm = svg.getScreenCTM();
    if (!ctm) return null;
    const point = new DOMPoint(p.clientX, p.clientY).matrixTransform(ctm.inverse());
    return { x: point.x, y: -point.y };
  }

  const metres = (v: number) => `${v.toFixed(2)} m`;

  // --- Point ----------------------------------------------------------------

  let target = $state<{ x: number; y: number } | null>(null);
  let height = $state(0);
  let fine = $state(false);
  let dragFrom: { finger: { x: number; y: number }; target: { x: number; y: number } } | null = null;

  const selectedMovers = $derived(movers.filter((f) => show.programmer.selection.includes(f.id)));
  const uncalibrated = $derived(selectedMovers.filter((f) => !solutionOf(f.id)));
  const pointing = $derived(selectedMovers.length - uncalibrated.length);

  // Only one point request at a time; drags send the latest spot when the last one is done.
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

  function planDown(p: DragPoint, element: EventTarget | null) {
    if ((element as Element | null)?.closest?.("button, .side-wrap")) return false;
    const at = toStage(p);
    if (!at) return false;
    if (mode === "point") {
      if (fine && target) dragFrom = { finger: at, target: { ...target } };
      else {
        dragFrom = { finger: at, target: at };
        aimAt(at);
      }
    } else if (mode === "setup" && draft && pickingReference) {
      draft.reference = { from: "custom", x: round5(at.x), y: round5(at.y) };
    } else if (mode === "setup" && draft && pickedMark != null) {
      moveMark(pickedMark, at);
    } else if (mode === "setup" && draft && pickedPiece != null) {
      draft.scenery[pickedPiece]?.points.push([round5(at.x), round5(at.y)]);
    }
  }

  function planMove(p: DragPoint) {
    if (mode !== "point" || !dragFrom) return;
    const at = toStage(p);
    if (!at) return;
    const scale = fine ? 0.1 : 1;
    aimAt({
      x: dragFrom.target.x + (at.x - dragFrom.finger.x) * scale,
      y: dragFrom.target.y + (at.y - dragFrom.finger.y) * scale,
    });
  }

  function setHeight(next: number) {
    height = next;
    if (target) send({ ...target, height });
  }

  // --- Calibrate ------------------------------------------------------------

  let calibrating = $state<number | null>(null);
  let currentMark = $state<number | null>(null);
  let calFine = $state(true);
  let turnedHighlightOn = false;

  const calibration = $derived(calibrating == null ? undefined : calibrationOf(calibrating));
  const readingFor = (mark: number) => calibration?.readings.find((r) => r.mark === mark);
  const errorFor = (mark: number) => calibration?.solution?.errors.find((e) => e.mark === mark)?.metres;
  const worst = $derived(Math.max(0, ...(calibration?.solution?.errors.map((e) => e.metres) ?? [])));
  const calValues = $derived(calibrating == null ? {} : (show.programmer.values[calibrating] ?? {}));

  function startCalibrating() {
    if (startFixture != null && calibrating == null && movers.some((m) => m.id === startFixture))
      return chooseMover(startFixture);
    const first = show.programmer.selection.find((id) => movers.some((m) => m.id === id)) ?? movers[0]?.id;
    if (first != null) chooseMover(first);
  }

  async function chooseMover(id: number) {
    calibrating = id;
    await show.program(() => api.select([id], "set"));
    // Highlight puts just this mover on, full and tight, to see it on the marks.
    if (!show.live.highlight) {
      show.live.highlight = true;
      turnedHighlightOn = true;
      await attempt(() => api.setHighlight(true));
    }
    currentMark = stage.marks.find((m) => !readingFor(m.id))?.id ?? stage.marks[0]?.id ?? null;
    resetHang();
  }

  // Where the mover hangs, when it has been measured. Then only which way it
  // faces is worked out, which suits wide washes that are hard to centre.
  const known = $derived<Known>(calibrating == null ? [null, null, null] : knownOf(stage, calibrating));
  const measured = $derived(known.filter((k) => k != null).length);
  const needed = $derived(minReadings(known));
  /** The hang position fields; blank ones are left to the calibration. */
  let hang = $state<Known>([null, null, null]);

  function resetHang() {
    if (calibrating != null) hang = [...knownOf(stage, calibrating)];
  }

  async function saveHang(position: Known) {
    if (calibrating == null) return;
    const fixture = calibrating;
    const saved = await attempt(() => api.setFixturePosition(fixture, position));
    if (saved) {
      stage = saved;
      resetHang();
    }
  }

  /** Gives every mover this height, such as the lighting grid's, keeping the rest of what's measured. */
  async function heightForAll(z: number) {
    for (const f of movers) {
      const [x, y] = knownOf(stage, f.id);
      const saved = await attempt(() => api.setFixturePosition(f.id, [x, y, z]));
      if (saved) stage = saved;
    }
    resetHang();
  }

  /** Fills the blank fields from where the calibration worked the mover out to be. */
  function fillFromFit() {
    const solved = calibrating == null ? undefined : solutionOf(calibrating)?.position;
    if (!solved) return;
    hang = hang.map((v, i) => v ?? Math.round(solved[i] * 100) / 100) as Known;
  }

  function stopHighlight() {
    if (turnedHighlightOn && show.live.highlight) {
      show.live.highlight = false;
      attempt(() => api.setHighlight(false));
    }
    turnedHighlightOn = false;
  }

  onDestroy(stopHighlight);

  async function confirmMark() {
    if (calibrating == null || currentMark == null) return;
    const fixture = calibrating;
    const done = currentMark;
    const saved = await attempt(() => api.calibrateMark(fixture, done));
    if (!saved) return;
    stage = saved;
    // On to the next mark without a reading, if any.
    const marks = stage.marks;
    const at = marks.findIndex((m) => m.id === done);
    const next = [...marks.slice(at + 1), ...marks.slice(0, at)].find((m) => !readingFor(m.id));
    if (next) currentMark = next.id;
  }

  async function forget(mark: number | null) {
    if (calibrating == null) return;
    const fixture = calibrating;
    const saved = await attempt(() => api.forgetCalibration(fixture, mark));
    if (saved) stage = saved;
  }

  /** Once a mover is worked out, sends it to the current mark to check or touch up. */
  function aimAtMark() {
    const mark = stage.marks.find((m) => m.id === currentMark);
    if (!mark) return;
    show.program(() => api.pointAt(mark.x, mark.y, mark.z - floorAt(stage, mark.x, mark.y)));
  }

  function calibrationSummary(id: number) {
    const c = calibrationOf(id);
    if (c?.solution) return "Calibrated";
    const n = c?.readings.length ?? 0;
    return n === 0 ? "Not calibrated" : `${n} of ${minReadings(knownOf(stage, id))} marks`;
  }

  // --- Set up ---------------------------------------------------------------

  let pickedMark = $state<number | null>(null);
  /** Taps on the plan move the reference point. */
  let pickingReference = $state(false);

  const referenceName = $derived(
    plan.reference?.from === "custom"
      ? "your reference point"
      : (REFERENCE_POINTS.find(([p]) => p === (plan.reference?.from ?? "front-middle"))?.[1] ?? "").toLowerCase(),
  );

  function chooseReference(point: ReferencePoint) {
    if (!draft) return;
    // A picked spot starts where the reference is now.
    const [x, y] = referenceAt(draft);
    draft.reference = { from: point, x, y };
    pickingReference = point === "custom";
    if (pickingReference) {
      pickedMark = null;
      pickedPiece = null;
    }
  }

  function pickReference() {
    pickingReference = !pickingReference;
    pickedMark = null;
    pickedPiece = null;
  }
  /** The scenery piece that taps on the plan add points to. */
  let pickedPiece = $state<number | null>(null);

  function pickMark(id: number) {
    pickingReference = false;
    pickedMark = pickedMark === id ? null : id;
    pickedPiece = null;
  }

  function pickPiece(index: number) {
    pickingReference = false;
    pickedPiece = pickedPiece === index ? null : index;
    pickedMark = null;
  }

  function addPiece() {
    if (!draft) return;
    const name = draft.scenery.length === 0 ? "LED wall" : `Scenery ${draft.scenery.length + 1}`;
    draft.scenery.push({ name, points: [], height: 3 });
    pickPiece(draft.scenery.length - 1);
  }

  function removePiece(index: number) {
    if (!draft) return;
    draft.scenery.splice(index, 1);
    pickedPiece = null;
  }

  const round5 = (v: number) => Math.round(v * 20) / 20;

  function moveMark(id: number, at: { x: number; y: number }) {
    const mark = draft?.marks.find((m) => m.id === id);
    if (mark) {
      mark.x = round5(at.x);
      mark.y = round5(at.y);
    }
  }

  function addMark() {
    if (!draft) return;
    const id = Math.max(0, ...draft.marks.map((m) => m.id)) + 1;
    draft.marks.push({ id, name: `Mark ${id}`, x: 0, y: draft.depth / 2, z: 0 });
    pickedMark = id;
  }

  function removeMark(id: number) {
    if (!draft) return;
    draft.marks = draft.marks.filter((m) => m.id !== id);
    if (pickedMark === id) pickedMark = null;
  }

  async function savePlan() {
    if (!draft) return;
    const d = $state.snapshot(draft);
    // Marks sit on whichever floor they are over.
    const marks: Mark[] = d.marks.map((m) => ({ ...m, name: m.name.trim() || `Mark ${m.id}`, z: floorAt(d, m.x, m.y) }));
    const scenery = d.scenery.filter((p) => p.points.length >= 2).map((p) => ({ ...p, name: p.name.trim() || "Scenery" }));
    const saved = await attempt(() => api.setStagePlan({ ...d, marks, scenery }));
    if (!saved) return;
    stage = saved;
    draft = copyPlan(saved);
  }

  const hasCalibration = $derived(stage.movers.some((m) => m.readings.length > 0));

  // --- Keys -----------------------------------------------------------------

  function keydown(e: KeyboardEvent) {
    if (e.defaultPrevented) return;
    if (e.key === "Escape") {
      e.preventDefault();
      onclose();
    }
  }
</script>

<svelte:window onkeydown={keydown} />

<div class="backdrop" role="presentation">
  <div class="window panel" role="dialog" aria-modal="true" aria-label="Stage" tabindex="-1" use:formKeys>
    <header class="row">
      <h2>Stage</h2>
      <div class="segmented" role="tablist" aria-label="Stage mode">
        <button role="tab" aria-selected={mode === "point"} onclick={() => setMode("point")}>Point</button>
        <button role="tab" aria-selected={mode === "calibrate"} onclick={() => setMode("calibrate")}>Calibrate</button>
        <button role="tab" aria-selected={mode === "setup"} onclick={() => setMode("setup")}>Set up</button>
      </div>
      <span class="spacer"></span>
      <button class="ghost" aria-label="Close" onclick={onclose}>✕</button>
    </header>

    <div class="plan-area" use:drag={{ start: planDown, move: planMove, end: () => (dragFrom = null) }}>
      <svg
        bind:this={svg}
        class="plan"
        class:aiming={mode === "point"}
        {viewBox}
        preserveAspectRatio="xMidYMid meet"
        role="application"
        aria-label="Stage plan"
      >
        <rect
          class="studio"
          x={view.left}
          y={-view.back}
          width={view.right - view.left}
          height={view.back - view.front}
        />
        {#each gridLines.xs as x (x)}
          <line class="grid" x1={x} x2={x} y1={-view.back} y2={-view.front} stroke-width={unit * 0.15} />
        {/each}
        {#each gridLines.ys as y (y)}
          <line class="grid" x1={view.left} x2={view.right} y1={-y} y2={-y} stroke-width={unit * 0.15} />
        {/each}
        <rect
          class="stage"
          x={-plan.width / 2}
          y={-plan.depth}
          width={plan.width}
          height={plan.depth}
          stroke-width={unit * 0.4}
        />
        <text class="caption" x="0" y={unit * 3.2} font-size={unit * 2.2} text-anchor="middle">Front of stage</text>
        <text class="caption" x="0" y={-plan.depth - unit * 1.4} font-size={unit * 2.2} text-anchor="middle">Up stage</text>

        {#each plan.scenery ?? [] as piece, i (i)}
          {@const mid = piece.points[Math.floor((piece.points.length - 1) / 2)]}
          <g class="scenery" class:picked={mode === "setup" && pickedPiece === i}>
            <polyline
              points={piece.points.map(([x, y]) => `${x},${-y}`).join(" ")}
              stroke-width={unit * 0.9}
            />
            {#each piece.points as [x, y], j (j)}
              <circle cx={x} cy={-y} r={unit * (mode === "setup" ? 0.8 : 0.45)} />
            {/each}
            {#if mid}
              <text x={mid[0]} y={-mid[1] - unit * 1.6} font-size={unit * 1.9} text-anchor="middle">{piece.name}</text>
            {/if}
          </g>
        {/each}

        {#each plan.marks as mark (mark.id)}
          {@const current = mode === "calibrate" && mark.id === currentMark}
          {@const read = mode === "calibrate" && readingFor(mark.id)}
          {@const picked = mode === "setup" && mark.id === pickedMark}
          <g class="mark" class:current class:read class:picked transform="translate({mark.x} {-mark.y})">
            {#if current}<circle class="ring" r={unit * 3} stroke-width={unit * 0.5} />{/if}
            <line x1={-unit * 1.4} x2={unit * 1.4} stroke-width={unit * 0.45} />
            <line y1={-unit * 1.4} y2={unit * 1.4} stroke-width={unit * 0.45} />
            <text x={unit * 1.8} y={-unit * 1.2} font-size={unit * 1.9}>{mark.name}</text>
          </g>
        {/each}

        {#if mode === "setup"}
          {@const [rx, ry] = referenceAt(plan)}
          <g class="reference" transform="translate({rx} {-ry})">
            <line x2={unit * 5} stroke-width={unit * 0.4} />
            <line y2={-unit * 5} stroke-width={unit * 0.4} />
            <circle r={unit * 0.9} />
            <text x={unit * 1.2} y={unit * 2.6} font-size={unit * 1.8}>0</text>
          </g>
        {/if}

        {#each drawn as d (d.fixture.id)}
          {#if d.end}
            {@const via = d.crosses ?? d.end}
            <line
              class="beam"
              class:selected={d.selected}
              x1={d.at[0]}
              y1={-d.at[1]}
              x2={via[0]}
              y2={-via[1]}
              stroke-width={unit * 0.35}
            />
            {#if d.crosses || !d.lands}
              <!-- On past the height it is aimed at, to the floor (or off into the distance). -->
              <line
                class="beam onward"
                class:selected={d.selected}
                x1={via[0]}
                y1={-via[1]}
                x2={d.end[0]}
                y2={-d.end[1]}
                stroke-width={unit * 0.3}
                stroke-dasharray="{unit * 1.2} {unit * 0.9}"
              />
            {/if}
            {#if d.lands}
              <circle class="spot" class:selected={d.selected} class:past={!!d.crosses} cx={d.end[0]} cy={-d.end[1]} r={unit * 1.3} />
            {/if}
            {#if d.crosses}
              <circle class="head" class:selected={d.selected} cx={d.crosses[0]} cy={-d.crosses[1]} r={unit * 1.5} stroke-width={unit * 0.45} />
            {/if}
          {/if}
          <g class="mover" class:selected={d.selected} transform="translate({d.at[0]} {-d.at[1]})">
            <circle r={unit * 1.9} stroke-width={unit * 0.4} />
            <text y={unit * 0.7} font-size={unit * 1.9} text-anchor="middle">{d.fixture.id}</text>
          </g>
        {/each}

        {#if mode === "point" && target}
          <g class="target" transform="translate({target.x} {-target.y})">
            <circle r={unit * 2.6} stroke-width={unit * 0.4} />
            <line x1={-unit * 4} x2={unit * 4} stroke-width={unit * 0.25} />
            <line y1={-unit * 4} y2={unit * 4} stroke-width={unit * 0.25} />
          </g>
        {/if}
      </svg>
      <div class="side-wrap">
      <svg class="side-view" viewBox={side.box} preserveAspectRatio="xMidYMid meet" aria-label="Side view">
        <rect class="studio" x={view.front} y={-side.high} width={view.back - view.front} height={side.high - side.low} />
        <line class="floor" x1={view.front} x2={view.back} y1={plan.riser} y2={plan.riser} />
        <rect class="stage" x="0" y="0" width={plan.depth} height={Math.max(plan.riser, 0.04)} />
        {#each plan.scenery ?? [] as piece, i (i)}
          {#if piece.points.length > 1}
            {@const ys = piece.points.map((p) => p[1])}
            {@const floor = floorAt(plan, piece.points[0][0], piece.points[0][1])}
            <rect
              class="scenery-side"
              x={Math.min(...ys)}
              y={-floor - piece.height}
              width={Math.max(Math.max(...ys) - Math.min(...ys), 0.08)}
              height={piece.height}
            />
          {/if}
        {/each}
        {#if mode === "point" && height > 0}
          <line class="height-line" x1={view.front} x2={view.back} y1={-height} y2={-height} />
        {/if}
        {#if mode === "point" && target}
          {@const floor = floorAt(plan, target.x, target.y)}
          <!-- Someone standing on the spot, for scale. -->
          <line class="person" x1={target.y} x2={target.y} y1={-floor} y2={-floor - 1.45} />
          <circle class="person" cx={target.y} cy={-floor - 1.6} r="0.15" />
        {/if}
        {#each drawn as d (d.fixture.id)}
          {#if d.end}
            {@const via = d.crosses ?? d.end}
            <line class="beam" class:selected={d.selected} x1={d.at[1]} y1={-d.at[2]} x2={via[1]} y2={-via[2]} />
            {#if d.crosses || !d.lands}
              <line class="beam onward" class:selected={d.selected} x1={via[1]} y1={-via[2]} x2={d.end[1]} y2={-d.end[2]} />
            {/if}
            {#if d.crosses}
              <circle class="head" class:selected={d.selected} cx={d.crosses[1]} cy={-d.crosses[2]} r="0.12" />
            {/if}
          {/if}
          <circle class="mover-dot" class:selected={d.selected} cx={d.at[1]} cy={-d.at[2]} r="0.18" />
        {/each}
      </svg>
      <p class="side-caption">Side view · front of stage on the left</p>
      </div>
    </div>

    <aside class="side scroll">
      {#if mode === "point"}
        <p class="lead">
          {#if pointing > 0}
            Tap or drag on the plan to point {pointing === 1 ? "the selected mover" : `the ${pointing} selected movers`}.
          {:else if selectedMovers.length > 0}
            The selected movers aren't calibrated yet.
          {:else}
            Select movers to point them.
          {/if}
        </p>
        {#if uncalibrated.length > 0}
          <p class="muted">Not calibrated, so left alone: {uncalibrated.map((f) => f.id).join(", ")}</p>
        {/if}
        {#if target}
          <p class="readout">
            {metres(fromReference(plan, target.x, target.y)[0])} across · {metres(fromReference(plan, target.x, target.y)[1])} up
            stage, from {referenceName}
            {onStage(plan, target.x, target.y) ? "" : " (studio floor)"}
          </p>
        {/if}
        <label class="field">
          Height above the floor: {height.toFixed(1)} m
          <input
            type="range"
            min="0"
            max="2.5"
            step="0.1"
            value={height}
            oninput={(e) => setHeight(Number((e.target as HTMLInputElement).value))}
          />
        </label>
        <div class="row wrap">
          <button aria-pressed={height === 0} onclick={() => setHeight(0)}>Floor</button>
          <button aria-pressed={height === 1.6} onclick={() => setHeight(1.6)}>Head height</button>
          <button aria-pressed={fine} class:on={fine} onclick={() => (fine = !fine)}>Fine</button>
        </div>
        <p class="muted small">
          Fine moves the spot a tenth as far as your finger, from where it is, so you can nudge it into place.
        </p>
      {:else if mode === "calibrate"}
        {#if movers.length === 0}
          <p class="muted">There are no movers in the patch.</p>
        {:else}
          <div class="movers">
            {#each movers as f (f.id)}
              <button aria-pressed={calibrating === f.id} onclick={() => chooseMover(f.id)}>
                <strong>{f.id}</strong>
                {f.name}
                <span class="small" class:ok={!!solutionOf(f.id)}>{calibrationSummary(f.id)}</span>
              </button>
            {/each}
          </div>
          {#if calibrating != null}
            <p class="lead">
              Put the beam's centre on <strong>{stage.marks.find((m) => m.id === currentMark)?.name ?? "a mark"}</strong>,
              then Confirm.
            </p>
            <PositionPad
              pan={calValues.pan}
              tilt={calValues.tilt}
              fine={calFine}
              onchange={(pan, tilt) => show.program(() => api.setPosition(pan, tilt))}
            />
            <div class="row wrap">
              <button aria-pressed={calFine} class:on={calFine} onclick={() => (calFine = !calFine)}>Fine</button>
              {#if calibration?.solution}
                <button onclick={aimAtMark}>Aim at mark</button>
              {/if}
              <span class="spacer"></span>
              <button class="primary" disabled={currentMark == null} onclick={confirmMark}>Confirm</button>
            </div>
            <div class="marks">
              {#each stage.marks as mark (mark.id)}
                {@const error = errorFor(mark.id)}
                <div class="mark-row" class:current={mark.id === currentMark}>
                  <button class="name" onclick={() => (currentMark = mark.id)}>{mark.name}</button>
                  <span class="small" class:warn={error != null && error > 0.15}>
                    {#if error != null}
                      {Math.round(error * 100)} cm
                    {:else if readingFor(mark.id)}
                      Recorded
                    {:else}
                      —
                    {/if}
                  </span>
                  {#if readingFor(mark.id)}
                    <button class="ghost small" onclick={() => forget(mark.id)}>Forget</button>
                  {/if}
                </div>
              {/each}
            </div>
            <p class="muted">
              {#if calibration?.solution}
                Worked out: hangs {calibration.solution.position[2].toFixed(1)} m above the stage floor, and the
                worst mark is {Math.round(worst * 100)} cm off.
                {#if worst > 0.15}Redo the marks in orange.{/if}
              {:else}
                Needs {needed} marks to work this mover out; more makes it more accurate and shows up a bad
                reading.
              {/if}
            </p>
            <section class="hang">
              <h3>Hang position</h3>
              <p class="muted small">
                Anything measured here is held while the rest is worked out from the marks, which helps a lot
                with wide beams. Leave a field blank if you don't know it.
                {#if measured === 3}With all three, {needed} marks are enough.{:else if measured > 0}With some of it,
                  {needed} marks are enough.{/if}
              </p>
              <div class="hang-fields">
                <NumberField label="Across (m)" value={measuredPart(plan, hang[0], 0)} onchange={(v) => (hang[0] = fromMeasuredPart(plan, v, 0))} fractional increment={0.05} min={-500} max={500} />
                <NumberField label="Up stage (m)" value={measuredPart(plan, hang[1], 1)} onchange={(v) => (hang[1] = fromMeasuredPart(plan, v, 1))} fractional increment={0.05} min={-500} max={500} />
                <NumberField label="Height above stage deck (m)" value={hang[2]} onchange={(v) => (hang[2] = v)} fractional increment={0.05} min={-50} max={50} />
              </div>
              {#if plan.riser > 0}
                <p class="muted small">
                  Measured from the top of the stage, which is {plan.riser} m above the studio floor.
                  {#if hang[2] != null}That's {(hang[2] + plan.riser).toFixed(2)} m above the studio floor.{/if}
                </p>
              {/if}
              <div class="row wrap">
                <button onclick={() => saveHang([...hang])}>Save</button>
                {#if hang[2] != null && movers.length > 1}
                  {@const z = hang[2]}
                  <button onclick={() => heightForAll(z)}>Use height for all movers</button>
                {/if}
                {#if calibration?.solution && hang.some((v) => v == null)}
                  <button class="ghost" onclick={fillFromFit}>Fill in from calibration</button>
                {/if}
                {#if measured > 0}
                  <button class="ghost" onclick={() => saveHang([null, null, null])}>Clear</button>
                {/if}
              </div>
            </section>
            {#if calibration && calibration.readings.length > 0}
              <button class="danger" onclick={() => forget(null)}>Reset this mover</button>
            {/if}
          {/if}
        {/if}
      {:else if draft}
        <div class="sizes">
          <NumberField label="Stage width (m)" value={draft.width} onchange={(v) => draft && v != null && (draft.width = v)} fractional increment={0.5} min={0.5} max={200} />
          <NumberField label="Stage depth (m)" value={draft.depth} onchange={(v) => draft && v != null && (draft.depth = v)} fractional increment={0.5} min={0.5} max={200} />
          <NumberField label="Stage height above studio floor (m)" value={draft.riser} onchange={(v) => draft && v != null && (draft.riser = v)} fractional increment={0.05} max={10} />
        </div>
        <h3>Studio floor round the stage</h3>
        <p class="muted small">How much floor to show on each side, in metres; left and right as seen from the desk.</p>
        <div class="sizes">
          {#each margins as [side, label] (side)}
            <NumberField {label} value={draft.margin[side]} onchange={(v) => draft && v != null && (draft.margin[side] = v)} fractional increment={0.5} max={50} />
          {/each}
        </div>
        <h3>Measured from</h3>
        <p class="muted small">
          Every position is typed in and shown from this point, in metres: across to the right as seen from the desk
          (left is minus) and up stage away from the desk. Changing it moves nothing, only the numbers.
        </p>
        <div class="row wrap">
          {#each REFERENCE_POINTS as [point, label] (point)}
            <button aria-pressed={draft.reference.from === point} onclick={() => chooseReference(point)}>{label}</button>
          {/each}
        </div>
        {#if draft.reference.from === "custom"}
          <p class="muted small">
            {pickingReference ? "Tap the plan where you measure from." : ""}
            <button class="ghost" aria-pressed={pickingReference} onclick={() => pickReference()}>Move it</button>
          </p>
        {/if}
        <h3>Marks</h3>
        <p class="muted small">Pick a mark and tap the plan to drop it there.</p>
        <div class="setup-marks">
          {#each draft.marks as mark (mark.id)}
            <div class="setup-mark" class:picked={pickedMark === mark.id}>
              <button
                class="pick"
                aria-pressed={pickedMark === mark.id}
                aria-label="Pick {mark.name}"
                onclick={() => pickMark(mark.id)}>{mark.id}</button
              >
              <input type="text" aria-label="Mark name" bind:value={mark.name} />
              <div class="pair">
                <NumberField label="Across" value={measuredPart(plan, mark.x, 0)} onchange={(v) => v != null && (mark.x = fromMeasuredPart(plan, v, 0)!)} fractional increment={0.05} min={-200} max={200} />
                <NumberField label="Up stage" value={measuredPart(plan, mark.y, 1)} onchange={(v) => v != null && (mark.y = fromMeasuredPart(plan, v, 1)!)} fractional increment={0.05} min={-200} max={200} />
              </div>
              <button class="ghost" aria-label="Remove {mark.name}" onclick={() => removeMark(mark.id)}>✕</button>
            </div>
          {/each}
        </div>
        <div class="row wrap">
          <button onclick={addMark}>Add mark</button>
          <button onclick={() => draft && (draft.marks = defaultMarks(draft.width, draft.depth))}>Corners and centre</button>
        </div>
        {#if hasCalibration}
          <p class="muted small">
            Moving or removing marks works the calibrated movers out again from the readings left.
          </p>
        {/if}
        <h3>Scenery</h3>
        <p class="muted small">
          Drawn on the plan for reference, such as the LED wall. Pick a piece and tap the plan to add its corners in
          order, or type them.
        </p>
        {#each draft.scenery as piece, i (i)}
          <div class="piece" class:picked={pickedPiece === i}>
            <div class="piece-head">
              <button
                class="pick"
                aria-pressed={pickedPiece === i}
                aria-label="Pick {piece.name} to add points"
                onclick={() => pickPiece(i)}>+</button
              >
              <input type="text" aria-label="Scenery name" bind:value={piece.name} />
              <button class="ghost" aria-label="Remove {piece.name}" onclick={() => removePiece(i)}>✕</button>
            </div>
            <NumberField label="Height (m)" value={piece.height} onchange={(v) => v != null && (piece.height = v)} fractional increment={0.1} max={50} />
            {#each piece.points as point, j (j)}
              <div class="point">
                <NumberField label="Point {j + 1} across" value={measuredPart(plan, point[0], 0)} onchange={(v) => v != null && (point[0] = fromMeasuredPart(plan, v, 0)!)} fractional increment={0.05} min={-500} max={500} />
                <NumberField label="Up stage" value={measuredPart(plan, point[1], 1)} onchange={(v) => v != null && (point[1] = fromMeasuredPart(plan, v, 1)!)} fractional increment={0.05} min={-500} max={500} />
                <button class="ghost" aria-label="Remove point {j + 1}" onclick={() => piece.points.splice(j, 1)}>✕</button>
              </div>
            {/each}
            {#if piece.points.length < 2}
              <p class="muted small">Needs at least two points; tap the plan to add them.</p>
            {/if}
          </div>
        {/each}
        <div class="row wrap">
          <button onclick={addPiece}>Add scenery</button>
          <span class="spacer"></span>
          <button class="primary" onclick={savePlan}>Save</button>
        </div>
      {/if}
    </aside>
  </div>
</div>

<style>
  .backdrop {
    position: fixed;
    inset: 0;
    background: rgb(0 0 0 / 0.6);
    z-index: 10;
    display: grid;
    padding: 24px;
  }

  .window {
    outline: none;
    display: grid;
    grid-template-columns: 1fr 520px;
    grid-template-rows: auto minmax(0, 1fr);
    gap: 16px;
    padding: 20px;
    border-top: 6px solid var(--f-position);
    min-height: 0;
  }

  header {
    grid-column: 1 / -1;
    gap: 16px;
  }

  .plan-area {
    min-height: 0;
    min-width: 0;
    display: grid;
  }

  .plan-area {
    touch-action: none;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .plan {
    flex: 1;
    min-height: 0;
  }

  .side-wrap {
    flex: 0 0 26%;
    min-height: 0;
    position: relative;
    display: grid;
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
    fill: #1a1f28;
    stroke: #4a5363;
    stroke-width: 2;
  }
  .side-view .beam {
    stroke-width: 3;
  }
  .side-view .onward {
    stroke-width: 2;
    stroke-dasharray: 8 6;
  }
  .height-line {
    stroke: var(--warning);
    stroke-width: 1;
    stroke-dasharray: 4 6;
  }
  .person {
    stroke: var(--text);
    stroke-width: 3;
    fill: none;
  }
  .mover-dot {
    fill: var(--panel-raised);
    stroke: var(--muted);
    stroke-width: 2;
  }
  .mover-dot.selected {
    stroke: var(--f-position);
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

  .mark line {
    stroke: var(--muted);
  }
  .mark text {
    fill: var(--muted);
    font-family: var(--mono);
  }
  .mark.read line {
    stroke: var(--ok);
  }
  .mark.current line,
  .mark.picked line {
    stroke: var(--warning);
  }
  .mark.current text,
  .mark.picked text {
    fill: var(--warning);
  }
  .mark .ring {
    fill: none;
    stroke: var(--warning);
  }

  .beam {
    stroke: color-mix(in srgb, var(--f-position) 30%, transparent);
  }
  .beam.selected {
    stroke: color-mix(in srgb, var(--f-position) 75%, transparent);
  }
  .spot {
    fill: color-mix(in srgb, var(--f-position) 35%, transparent);
  }
  .spot.selected {
    fill: var(--f-position);
  }
  /* The floor spot behind someone the beam is aimed at. */
  .spot.past {
    fill: color-mix(in srgb, var(--f-position) 25%, transparent);
  }
  .head {
    fill: none;
    stroke: color-mix(in srgb, var(--warning) 50%, transparent);
  }
  .head.selected {
    stroke: var(--warning);
  }
  .onward {
    fill: none;
  }
  .mover circle {
    fill: var(--panel-raised);
    stroke: var(--muted);
  }
  .mover.selected circle {
    stroke: var(--f-position);
  }
  .mover text {
    fill: var(--text);
    font-family: var(--mono);
  }

  .target circle {
    fill: none;
    stroke: var(--text);
  }
  .target line {
    stroke: var(--text);
  }

  .reference line {
    stroke: var(--warning);
  }
  .reference circle,
  .reference text {
    fill: var(--warning);
    font-family: var(--mono);
  }

  .side {
    display: flex;
    flex-direction: column;
    gap: 14px;
    min-height: 0;
  }
  /* Nothing in the side panel gets squashed: it scrolls instead. */
  .side > :global(*) {
    flex-shrink: 0;
  }
  /* The pan and tilt pad stays square and big enough to use, but leaves room
     for Confirm and the marks on a short screen. */
  .side :global(.pad) {
    width: min(100%, max(240px, 40vh));
    align-self: center;
  }

  .lead {
    margin: 0;
    font-size: 1.05rem;
  }
  .readout {
    margin: 0;
    font-family: var(--mono);
    color: var(--f-position);
  }
  .small {
    font-size: 0.85rem;
  }
  .wrap {
    flex-wrap: wrap;
    gap: 8px;
  }
  .on,
  button[aria-pressed="true"] {
    background: var(--accent);
    color: var(--accent-text);
  }

  .movers {
    display: grid;
    grid-template-columns: repeat(2, 1fr);
    gap: 6px;
  }
  .movers button {
    display: grid;
    justify-items: start;
    text-align: left;
    gap: 2px;
  }
  .ok {
    color: var(--ok);
  }
  button[aria-pressed="true"] .ok {
    color: inherit;
  }

  .marks {
    display: grid;
    gap: 4px;
  }
  .mark-row {
    display: grid;
    grid-template-columns: 1fr auto auto;
    align-items: center;
    gap: 8px;
  }
  .mark-row .name {
    text-align: left;
  }
  .mark-row.current .name {
    outline: 2px solid var(--warning);
    outline-offset: -2px;
  }
  .warn {
    color: var(--f-playback);
  }

  .scenery polyline {
    fill: none;
    stroke: color-mix(in srgb, #c56cff 70%, transparent);
    stroke-linecap: round;
    stroke-linejoin: round;
  }
  .scenery circle {
    fill: #c56cff;
  }
  .scenery text {
    fill: #c56cff;
    font-family: var(--mono);
  }
  .scenery.picked polyline {
    stroke: var(--warning);
  }
  .scenery-side {
    fill: color-mix(in srgb, #c56cff 35%, transparent);
    stroke: #c56cff;
    stroke-width: 1;
  }
  .piece {
    display: grid;
    gap: 8px;
    padding: 8px;
    border: 1px solid var(--line);
    border-radius: var(--radius);
  }
  .piece.picked {
    border-color: var(--warning);
  }
  .piece-head {
    display: grid;
    grid-template-columns: var(--touch) 1fr var(--touch);
    gap: 6px;
  }
  .piece-head .pick {
    padding: 0;
  }
  .point {
    display: grid;
    grid-template-columns: 1fr 1fr var(--touch);
    gap: 6px;
    align-items: end;
  }

  .hang {
    display: grid;
    gap: 8px;
    border-top: 1px solid var(--line);
    padding-top: 10px;
  }
  .hang p {
    margin: 0;
  }
  .hang-fields {
    display: grid;
    grid-template-columns: repeat(3, 1fr);
    gap: 8px;
  }

  .sizes {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 10px;
  }
  h3 {
    margin: 4px 0 0;
  }
  .setup-marks {
    display: grid;
    gap: 10px;
  }
  .setup-mark {
    display: grid;
    grid-template-columns: var(--touch) 1fr var(--touch);
    gap: 6px;
    align-items: end;
    padding: 8px;
    border: 1px solid var(--line);
    border-radius: var(--radius);
  }
  .setup-mark.picked {
    border-color: var(--warning);
  }
  .setup-mark .pair {
    grid-column: 1 / -1;
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
  }
  .setup-mark input[type="text"] {
    grid-column: 2;
  }
  .setup-mark .pick {
    padding: 0;
  }
  .setup-mark > .ghost {
    grid-column: 3;
    grid-row: 1;
  }
</style>
