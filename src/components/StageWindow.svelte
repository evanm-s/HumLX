<script lang="ts">
  // The stage plan, popped out over the rest of the screen so it is big
  // enough to aim precisely. Point: tap or drag on the plan and the selected,
  // calibrated movers aim there. Calibrate: point one mover at each floor mark
  // so HumLX can work out where it hangs. Set up: the stage's size and marks.

  import { onDestroy, onMount } from "svelte";
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
    landing,
    MIN_READINGS,
    onStage,
    type Mark,
    type Plan,
    type Stage,
  } from "../lib/stage";
  import type { Fixture } from "../lib/types";

  interface Props {
    onclose: () => void;
  }

  let { onclose }: Props = $props();

  type Mode = "point" | "calibrate" | "setup";
  const MODE_KEY = "humlx.stage.mode";
  let mode = $state<Mode>(readMode());
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

  onMount(async () => {
    const loaded = await attempt(() => api.stage());
    if (loaded) stage = loaded;
    if (mode === "calibrate") startCalibrating();
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
    at: [number, number];
    lands: [number, number] | null;
    selected: boolean;
  }

  const drawn = $derived.by((): Drawn[] =>
    movers.flatMap((fixture) => {
      const solution = solutionOf(fixture.id);
      if (!solution) return [];
      const position = outputPosition(fixture);
      return [
        {
          fixture,
          at: [solution.position[0], solution.position[1]] as [number, number],
          lands: position ? landing(plan, solution, position[0], position[1]) : null,
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
    return { width: s.width, depth: s.depth, margin: s.margin, riser: s.riser, marks: s.marks.map((m) => ({ ...m })) };
  }

  // The view takes in the studio margin and any mover hung outside it.
  const view = $derived.by(() => {
    let left = -plan.width / 2 - plan.margin;
    let right = plan.width / 2 + plan.margin;
    let front = -plan.margin;
    let back = plan.depth + plan.margin;
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
    if ((element as Element | null)?.closest?.("button")) return false;
    const at = toStage(p);
    if (!at) return false;
    if (mode === "point") {
      if (fine && target) dragFrom = { finger: at, target: { ...target } };
      else {
        dragFrom = { finger: at, target: at };
        aimAt(at);
      }
    } else if (mode === "setup" && draft && pickedMark != null) {
      moveMark(pickedMark, at);
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
    return n === 0 ? "Not calibrated" : `${n} of ${MIN_READINGS} marks`;
  }

  // --- Set up ---------------------------------------------------------------

  let pickedMark = $state<number | null>(null);

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
    const d = draft;
    // Marks sit on whichever floor they are over.
    const marks: Mark[] = d.marks.map((m) => ({ ...m, name: m.name.trim() || `Mark ${m.id}`, z: floorAt(d, m.x, m.y) }));
    const saved = await attempt(() => api.setStagePlan({ ...d, marks }));
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

        {#each drawn as d (d.fixture.id)}
          {#if d.lands}
            <line
              class="beam"
              class:selected={d.selected}
              x1={d.at[0]}
              y1={-d.at[1]}
              x2={d.lands[0]}
              y2={-d.lands[1]}
              stroke-width={unit * 0.35}
            />
            <circle class="spot" class:selected={d.selected} cx={d.lands[0]} cy={-d.lands[1]} r={unit * 1.3} />
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
            {metres(target.x)} {target.x < 0 ? "left" : "right"} · {metres(target.y)} up stage
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
                Needs {MIN_READINGS} marks to work this mover out; {stage.marks.length > MIN_READINGS
                  ? "more makes it more accurate and shows up a bad reading"
                  : "add a fifth mark on Set up to check it"}.
              {/if}
            </p>
            {#if calibration && calibration.readings.length > 0}
              <button class="danger" onclick={() => forget(null)}>Reset this mover</button>
            {/if}
          {/if}
        {/if}
      {:else if draft}
        <div class="sizes">
          <NumberField label="Stage width (m)" value={draft.width} onchange={(v) => draft && v != null && (draft.width = v)} fractional increment={0.5} min={0.5} max={200} />
          <NumberField label="Stage depth (m)" value={draft.depth} onchange={(v) => draft && v != null && (draft.depth = v)} fractional increment={0.5} min={0.5} max={200} />
          <NumberField label="Studio floor round it (m)" value={draft.margin} onchange={(v) => draft && v != null && (draft.margin = v)} fractional increment={0.5} max={50} />
          <NumberField label="Stage height above studio floor (m)" value={draft.riser} onchange={(v) => draft && v != null && (draft.riser = v)} fractional increment={0.05} max={10} />
        </div>
        <h3>Marks</h3>
        <p class="muted small">
          In metres from the front edge of the stage, in the middle; left and right as seen from the desk. Pick a mark
          and tap the plan to drop it there.
        </p>
        <div class="setup-marks">
          {#each draft.marks as mark (mark.id)}
            <div class="setup-mark" class:picked={pickedMark === mark.id}>
              <button
                class="pick"
                aria-pressed={pickedMark === mark.id}
                aria-label="Pick {mark.name}"
                onclick={() => (pickedMark = pickedMark === mark.id ? null : mark.id)}>{mark.id}</button
              >
              <input type="text" aria-label="Mark name" bind:value={mark.name} />
              <div class="pair">
                <NumberField label="Across" value={mark.x} onchange={(v) => v != null && (mark.x = v)} fractional increment={0.05} min={-200} max={200} />
                <NumberField label="Up stage" value={mark.y} onchange={(v) => v != null && (mark.y = v)} fractional increment={0.05} min={-200} max={200} />
              </div>
              <button class="ghost" aria-label="Remove {mark.name}" onclick={() => removeMark(mark.id)}>✕</button>
            </div>
          {/each}
        </div>
        <div class="row wrap">
          <button onclick={addMark}>Add mark</button>
          <button onclick={() => draft && (draft.marks = defaultMarks(draft.width, draft.depth))}>Corners and centre</button>
          <span class="spacer"></span>
          <button class="primary" onclick={savePlan}>Save</button>
        </div>
        {#if hasCalibration}
          <p class="muted small">
            Moving or removing marks works the calibrated movers out again from the readings left.
          </p>
        {/if}
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

  .side {
    display: flex;
    flex-direction: column;
    gap: 14px;
    min-height: 0;
  }
  .side :global(.pad) {
    width: 100%;
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
