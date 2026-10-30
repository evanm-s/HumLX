<script lang="ts">
  // Raw channel control per fixture, for checking the patch against the rig.

  import AddressMap from "../components/AddressMap.svelte";
  import Fader from "../components/Fader.svelte";
  import { api } from "../lib/api";
  import { attributeColour, attributeName, capabilityAt, fixtureColour, range } from "../lib/format";
  import { attempt, show } from "../lib/show.svelte";

  let selectedId = $state<number | null>(null);

  // Keep a valid selection as the patch changes.
  $effect(() => {
    if (!show.fixture(selectedId)) selectedId = show.patch.fixtures[0]?.id ?? null;
  });

  const fixture = $derived(show.fixture(selectedId));
  const channels = $derived(fixture ? show.channels(fixture) : []);
  const levels = $derived(
    fixture ? channels.map((_, i) => show.live.universe[fixture.address - 1 + i] ?? 0) : [],
  );
  const tested = $derived(fixture ? show.live.tested.includes(fixture.id) : false);

  function setLevel(channel: number, value: number) {
    if (fixture) attempt(() => api.setTestLevel(fixture.id, channel, value));
  }
</script>

<div class="test">
  <header class="row">
    <h1>Channel test</h1>
    <span class="muted">Drag a fader to set a channel directly. Released channels go back to their resting level.</span>
    <span class="spacer"></span>
    <button disabled={show.live.tested.length === 0} onclick={() => attempt(() => api.release(null))}>
      Release all
    </button>
  </header>

  <nav class="fixtures panel scroll" aria-label="Fixtures">
    {#each show.patch.fixtures as f (f.id)}
      <button class="tile" aria-pressed={f.id === selectedId} onclick={() => (selectedId = f.id)}>
        <span class="num" style:color={fixtureColour(f.id)}>{f.id}</span>
        <span class="name">{f.name}</span>
        {#if show.live.tested.includes(f.id)}<span class="dot" aria-label="Has test levels"></span>{/if}
      </button>
    {:else}
      <p class="muted pad">Patch some fixtures first.</p>
    {/each}
  </nav>

  <section class="channels panel">
    {#if fixture}
      <div class="row">
        <h2>{fixture.id} · {fixture.name}</h2>
        <span class="muted">{fixture.typeName} · {fixture.mode} · {range(fixture.address, fixture.footprint)}</span>
        <span class="spacer"></span>
        <button class="primary" onclick={() => attempt(() => api.locate(fixture.id))}>Locate</button>
        <button disabled={!tested} onclick={() => attempt(() => api.release(fixture.id))}>Release</button>
      </div>
      <div class="faders scroll-x">
        {#each channels as channel, i (i)}
          <Fader
            value={levels[i]}
            label={channel.name}
            sublabel={`${fixture.address + i} · ${attributeName(channel.attribute)}${channel.fineOf ? " fine" : ""}`}
            detail={capabilityAt(channel.capabilities, levels[i])}
            colour={attributeColour(channel.attribute)}
            active={tested}
            onchange={(value) => setLevel(i, value)}
          />
        {/each}
      </div>
    {/if}
  </section>

  <section class="monitor panel">
    <AddressMap
      occupancy={show.patch.occupancy}
      selected={selectedId}
      levels={show.live.universe}
      onselect={(owner) => owner != null && (selectedId = owner)}
    />
  </section>
</div>

<style>
  .test {
    height: 100%;
    display: grid;
    grid-template-columns: 340px 1fr;
    grid-template-rows: auto 1fr auto;
    gap: var(--gap);
  }

  header {
    grid-column: 1 / -1;
  }

  .fixtures {
    padding: 8px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .pad {
    padding: 10px;
  }

  .tile {
    display: grid;
    grid-template-columns: 2.5ch 1fr auto;
    align-items: center;
    gap: 8px;
    text-align: left;
    min-height: 44px;
    flex-shrink: 0;
  }

  .tile[aria-pressed="true"] {
    border-color: var(--accent);
    background: color-mix(in srgb, var(--accent) 12%, var(--panel-raised));
  }

  .num {
    font-weight: 700;
    font-variant-numeric: tabular-nums;
  }

  .name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .dot {
    width: 12px;
    height: 12px;
    border-radius: 50%;
    background: var(--accent);
  }

  .channels {
    padding: 14px 16px;
    display: grid;
    grid-template-rows: auto 1fr;
    gap: 14px;
    min-height: 0;
  }

  .faders {
    display: flex;
    gap: 12px;
    min-height: 0;
    overflow-x: auto;
    padding-bottom: 8px;
  }

  .monitor {
    grid-column: 1 / -1;
    padding: 12px;
  }
</style>
