<script lang="ts">
  import NumberField from "../components/NumberField.svelte";
  import { api } from "../lib/api";
  import { isIPv4, portAddressParts } from "../lib/format";
  import { attempt, notify, show } from "../lib/show.svelte";
  import type { Node } from "../lib/types";

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
</script>

<div class="settings">
  <header class="row">
    <h1>Art-Net output</h1>
  </header>

  <section class="panel card">
    <div class="row">
      <h2>Output</h2>
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
</div>

<style>
  .settings {
    max-width: 1000px;
    display: grid;
    gap: var(--gap);
    align-content: start;
  }

  .card {
    padding: 24px;
    display: grid;
    gap: 20px;
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
    gap: 20px;
    align-items: center;
    text-align: left;
    min-height: 72px;
  }

  .use {
    color: var(--accent);
    font-weight: 600;
  }
</style>
