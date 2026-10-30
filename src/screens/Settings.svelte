<script lang="ts">
  import { open as openDialog, save as saveDialog } from "@tauri-apps/plugin-dialog";
  import ColourPicker from "../components/ColourPicker.svelte";
  import Dialog from "../components/Dialog.svelte";
  import NumberField from "../components/NumberField.svelte";
  import { api, inApp } from "../lib/api";
  import { isIPv4, portAddressParts, rgbCss, SWATCHES } from "../lib/format";
  import { formKeys } from "../lib/keys";
  import { attempt, notify, show } from "../lib/show.svelte";
  import { updates } from "../lib/updates.svelte";
  import type { Node, Rgb } from "../lib/types";

  let draft = $state({ ...show.output, universe: show.output.universe as number | null, rateHz: show.output.rateHz as number | null });
  let nodes = $state<Node[] | null>(null);
  let searching = $state(false);

  const parts = $derived(portAddressParts(draft.universe ?? 0));
  const valid = $derived(isIPv4(draft.target) && draft.universe != null && draft.rateHz != null);
  const dirty = $derived(
    draft.enabled !== show.output.enabled ||
      draft.target.trim() !== show.output.target ||
      draft.universe !== show.output.universe ||
      draft.rateHz !== show.output.rateHz,
  );

  async function save() {
    if (!valid) return;
    const saved = await attempt(() =>
      api.setOutput({
        enabled: draft.enabled,
        target: draft.target.trim(),
        universe: draft.universe ?? 0,
        rateHz: draft.rateHz ?? 40,
      }),
    );
    if (saved) {
      show.output = saved;
      notify("Output settings saved");
    }
  }

  async function search() {
    searching = true;
    nodes = (await attempt(() => api.discoverNodes())) ?? null;
    searching = false;
  }

  function useNode(node: Node) {
    draft.target = node.ip;
    if (node.outputs.length > 0) draft.universe = node.outputs[0];
  }

  // --- Show file -------------------------------------------------------------

  const FILTERS = [{ name: "HumLX show", extensions: ["humlx", "json"] }];
  let file = $state<string | null>(null);
  let confirmNew = $state(false);
  attempt(() => api.showFile()).then((f) => (file = f ?? null));

  const fileName = (path: string) => path.split(/[\\/]/).pop() ?? path;

  async function saveAs() {
    if (!inApp) return notify("Saving show files needs the desktop app", "warning");
    const path = await saveDialog({ filters: FILTERS, defaultPath: file ?? "Studio.humlx" });
    if (!path) return;
    const saved = await attempt(() => api.saveShowAs(path));
    if (saved) {
      file = saved;
      notify(`Saved ${fileName(saved)}`);
    }
  }

  async function openShow() {
    if (!inApp) return notify("Opening show files needs the desktop app", "warning");
    const path = await openDialog({ filters: FILTERS, multiple: false, directory: false });
    if (typeof path !== "string") return;
    const opened = await attempt(() => api.openShow(path));
    if (opened) {
      await show.load();
      notify(`Opened ${fileName(opened)}`);
    }
  }

  async function newShow() {
    const ok = await attempt(async () => (await api.newShow(), true));
    confirmNew = false;
    if (ok) {
      await show.load();
      notify("Started a new show");
    }
  }

  // --- OSC -------------------------------------------------------------------

  let osc = $state<{ enabled: boolean; port: number | null } | null>(null);
  let savedOsc = $state<{ enabled: boolean; port: number } | null>(null);
  attempt(() => api.oscSettings()).then((s) => {
    if (s) (osc = { ...s }), (savedOsc = s);
  });
  const oscDirty = $derived(!!osc && !!savedOsc && (osc.enabled !== savedOsc.enabled || osc.port !== savedOsc.port));

  async function saveOsc() {
    if (!osc || osc.port == null) return;
    const saved = await attempt(() => api.setOsc({ enabled: osc!.enabled, port: osc!.port! }));
    if (saved) {
      savedOsc = saved;
      notify(saved.enabled ? `Listening for OSC on port ${saved.port}` : "OSC off");
    }
  }

  // --- Highlight -------------------------------------------------------------

  let highlightColour = $state<Rgb | undefined>();
  attempt(() => api.highlightColour()).then((c) => (highlightColour = c ?? undefined));
  let pendingHighlight: ReturnType<typeof setTimeout> | undefined;

  // Shows the colour straight away, but only saves once dragging on the disc pauses.
  function setHighlightColour(colour: Rgb, wait = 150) {
    highlightColour = colour;
    clearTimeout(pendingHighlight);
    pendingHighlight = setTimeout(() => attempt(() => api.setHighlightColour(colour)), wait);
  }

  const sameColour = (a: Rgb | undefined, b: Rgb) =>
    !!a && Math.abs(a.r - b.r) < 0.01 && Math.abs(a.g - b.g) < 0.01 && Math.abs(a.b - b.b) < 0.01;

  const status = $derived(show.live.osc);
  const example = $derived(show.playback.cueLists[0]?.id ?? 1);
</script>

<div class="settings">
  <header class="row">
    <h1>Settings</h1>
  </header>

  <section class="panel card">
    <div class="row">
      <h2>Show file</h2>
      <span class="spacer"></span>
      <button onclick={() => (confirmNew = true)}>New show</button>
      <button onclick={openShow}>Open…</button>
      <button class="primary" onclick={saveAs}>Save as…</button>
    </div>
    <p class="muted">
      Everything saves automatically on this computer. Save a show file to keep a copy, move it to another computer
      or keep different setups. Fixture types you imported are saved inside it.
      {#if file}<br />Last saved or opened: <strong>{file}</strong>{/if}
    </p>
  </section>

  <section class="panel card" use:formKeys>
    <div class="row">
      <h2>Art-Net output</h2>
      <span class="spacer"></span>
      <div class="segmented">
        <button aria-pressed={draft.enabled} onclick={() => (draft.enabled = true)}>On</button>
        <button aria-pressed={!draft.enabled} onclick={() => (draft.enabled = false)}>Off</button>
      </div>
    </div>

    <label class="field">
      Send to (node IP address, or 255.255.255.255 to broadcast)
      <input bind:value={draft.target} inputmode="decimal" class:invalid={!isIPv4(draft.target)} />
    </label>

    <div class="pair">
      <div>
        <NumberField label="Art-Net universe (port-address)" bind:value={draft.universe} min={0} max={32767} />
        <p class="muted hint">Net {parts.net} · Sub-net {parts.subNet} · Universe {parts.universe}</p>
      </div>
      <NumberField label="Refresh rate (packets per second)" bind:value={draft.rateHz} min={1} max={44} />
    </div>

    <div class="row">
      <span class="muted">
        {#if show.live.status.lastError}
          <span class="bad">Output problem: {show.live.status.lastError}</span>
        {:else}
          {show.live.status.packetsSent.toLocaleString()} packets sent
        {/if}
      </span>
      <span class="spacer"></span>
      <button class="primary" disabled={!dirty || !valid} onclick={save}>Save</button>
    </div>
  </section>

  <section class="panel card osc" use:formKeys>
    <div class="row">
      <h2>OSC</h2>
      <span class="spacer"></span>
      {#if osc}
        <div class="segmented">
          <button aria-pressed={osc.enabled} onclick={() => (osc!.enabled = true)}>On</button>
          <button aria-pressed={!osc.enabled} onclick={() => (osc!.enabled = false)}>Off</button>
        </div>
      {/if}
    </div>
    <p class="muted">
      Run cue lists from other software or a phone, such as QLab, Bitfocus Companion or TouchOSC. Send OSC over UDP
      to this computer's IP address.
    </p>
    {#if osc}
      <div class="pair">
        <NumberField label="Listen on port" bind:value={osc.port} min={1} max={65535} />
        <div class="osc-status">
          {#if status.error}
            <span class="bad">{status.error}</span>
          {:else if status.listening}
            <span class="good">Listening</span> · {status.received.toLocaleString()} messages received
            {#if status.last}<br /><span class="muted">Last: <code>{status.last}</code></span>{/if}
          {:else}
            <span class="muted">Not listening</span>
          {/if}
        </div>
      </div>
      <div class="row">
        <span class="spacer"></span>
        <button class="primary" disabled={!oscDirty || osc.port == null} onclick={saveOsc}>Save</button>
      </div>
    {/if}
    <table class="addresses">
      <thead><tr><th>Address</th><th>Does</th></tr></thead>
      <tbody>
        <tr><td><code>/humlx/cuelist/{example}/go</code></td><td>GO</td></tr>
        <tr><td><code>/humlx/cuelist/{example}/back</code></td><td>Back one cue</td></tr>
        <tr><td><code>/humlx/cuelist/{example}/goto/3</code></td><td>Go to cue 3 (or send the number to <code>…/goto</code>)</td></tr>
        <tr><td><code>/humlx/cuelist/{example}/release</code></td><td>Release the cue list</td></tr>
        <tr><td><code>/humlx/cuelist/{example}/master</code> 0.5</td><td>Set the master, from 0 to 1</td></tr>
        <tr><td><code>/humlx/releaseall</code></td><td>Release everything</td></tr>
        <tr><td><code>/humlx/blackout</code> 1 or 0</td><td>Blackout on or off</td></tr>
      </tbody>
    </table>
    <p class="muted hint">
      Use the cue list's number (shown on the Playback screen) or its name with <code>_</code> for spaces, such as
      <code>/humlx/cuelist/walk_in/go</code>. Buttons that send 0 when let go are ignored on release.
    </p>
  </section>

  <section class="panel card">
    <div class="row">
      <h2>Highlight colour</h2>
      <span class="spacer"></span>
      {#if highlightColour}<span class="current" style:background={rgbCss(highlightColour)}></span>{/if}
    </div>
    <p class="muted">
      The colour that <strong>Highlight</strong> turns the selected fixtures. Pick one that stands out from what's
      already on stage. Fixtures without colour mixing still go to full.
    </p>
    {#if highlightColour}
      <div class="highlight-colour">
        <ColourPicker colour={highlightColour} onchange={(c) => setHighlightColour(c)} />
        <div class="swatches">
          {#each SWATCHES as s (s.name)}
            <button
              class="swatch"
              title={s.name}
              aria-label={s.name}
              aria-pressed={sameColour(highlightColour, s.colour)}
              style:background={rgbCss(s.colour)}
              onclick={() => setHighlightColour(s.colour, 0)}
            ></button>
          {/each}
        </div>
      </div>
    {/if}
  </section>

  <section class="panel card">
    <div class="row">
      <h2>Find nodes</h2>
      <span class="spacer"></span>
      <button onclick={search} disabled={searching}>{searching ? "Searching…" : "Search network"}</button>
    </div>
    {#if nodes && nodes.length === 0}
      <p class="muted">No nodes answered. Check the node is powered and on the same network as this PC.</p>
    {:else if nodes}
      {#each nodes as node (node.ip + node.shortName)}
        <button class="node" onclick={() => useNode(node)}>
          <span><strong>{node.shortName || "Unnamed node"}</strong> <span class="muted">{node.longName}</span></span>
          <span class="muted">{node.ip} · outputs {node.outputs.join(", ") || "none"}</span>
          <span class="use">Use</span>
        </button>
      {/each}
    {:else}
      <p class="muted">Searches the network for Art-Net nodes. Tap one to send to it.</p>
    {/if}
  </section>

  <section class="panel card">
    <div class="row">
      <h2>Updates</h2>
      <span class="spacer"></span>
      <button disabled={!inApp || updates.checking} onclick={() => updates.check()}>
        {updates.checking ? "Checking…" : "Check now"}
      </button>
    </div>
    <p class="muted">
      {#if !inApp}
        Updates work in the desktop app.
      {:else if updates.error}
        <span class="bad">Couldn't check for updates: {updates.error}</span>
      {:else if updates.available}
        Version {updates.available.version} is available. Tap <strong>Update</strong> in the side bar to install it.
      {:else if updates.lastChecked}
        HumLX {updates.version} is the latest version.
      {:else}
        HumLX {updates.version ?? ""} checks for a new version when it starts and every few hours.
      {/if}
    </p>
  </section>
</div>

{#if confirmNew}
  <Dialog title="Start a new show?" onclose={() => (confirmNew = false)}>
    <p>
      This clears the patch, groups, presets, cue lists and effects. Output and OSC settings stay. Save a show file
      first if you want to keep the current show.
    </p>
    {#snippet actions()}
      <button onclick={() => (confirmNew = false)}>Cancel</button>
      <button class="primary" onclick={newShow}>Start new show</button>
    {/snippet}
  </Dialog>
{/if}

<style>
  .settings {
    display: grid;
    grid-template-columns: repeat(auto-fit, minmax(min(760px, 100%), 1fr));
    gap: var(--gap);
    align-content: start;
    overflow-y: auto;
    height: 100%;
  }

  header {
    grid-column: 1 / -1;
  }

  .osc {
    grid-row: span 3;
  }

  .osc-status {
    padding-top: 28px;
    line-height: 1.6;
  }

  .good {
    color: var(--ok);
  }

  .current {
    width: 40px;
    height: 40px;
    border-radius: 50%;
    box-shadow: 0 0 0 1px var(--line);
  }

  .highlight-colour {
    display: grid;
    grid-template-columns: minmax(0, 240px) 1fr;
    gap: var(--gap);
    align-items: start;
  }

  .swatches {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 6px;
  }

  .swatch {
    min-height: 48px;
    padding: 0;
    box-shadow: inset 0 0 0 1px rgb(255 255 255 / 0.15);
  }

  .swatch[aria-pressed="true"] {
    box-shadow:
      inset 0 0 0 3px #000,
      0 0 0 3px var(--accent);
  }

  .addresses {
    border-collapse: collapse;
    width: 100%;
  }

  .addresses th,
  .addresses td {
    text-align: left;
    padding: 8px 12px 8px 0;
    border-bottom: 1px solid var(--line);
  }

  .addresses th {
    color: var(--muted);
    font-size: 0.8rem;
  }

  code {
    font-family: ui-monospace, "Cascadia Mono", Consolas, monospace;
    color: var(--accent);
  }

  .card {
    padding: 16px;
    display: grid;
    gap: 14px;
    align-content: start;
  }

  .pair {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: var(--gap);
    align-items: start;
  }

  .hint {
    margin: 8px 0 0;
    font-size: 0.85rem;
  }

  .invalid {
    border-color: var(--danger);
  }

  .bad {
    color: var(--danger);
  }

  .node {
    display: grid;
    grid-template-columns: 1fr auto auto;
    gap: 14px;
    align-items: center;
    text-align: left;
    min-height: 52px;
  }

  .use {
    color: var(--accent);
    font-weight: 600;
  }
</style>
