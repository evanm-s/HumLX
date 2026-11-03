<script lang="ts">
  import AddressMap from "../components/AddressMap.svelte";
  import Dialog from "../components/Dialog.svelte";
  import FixtureOptionsForm from "../components/FixtureOptionsForm.svelte";
  import NumberField from "../components/NumberField.svelte";
  import StageWindow from "../components/StageWindow.svelte";
  import { api } from "../lib/api";
  import { fixtureColour, range } from "../lib/format";
  import { formKeys } from "../lib/keys";
  import { attempt, notify, show } from "../lib/show.svelte";
  import { DEFAULT_OPTIONS, type Fixture, type FixtureChanges, type FixtureOptions } from "../lib/types";

  // Selected fixture numbers. One opens the edit form; more open bulk edit.
  let selectedIds = $state<number[]>([]);
  // In multi-select, taps add and remove fixtures instead of replacing.
  let multi = $state(false);
  // Where a Shift+click range starts.
  let anchor: number | null = null;
  const selectedId = $derived(selectedIds.length === 1 ? selectedIds[0] : null);
  const selected = $derived(show.fixture(selectedId));
  const selection = $derived(show.patch.fixtures.filter((f) => selectedIds.includes(f.id)));

  function clearSelection() {
    selectedIds = [];
    anchor = null;
  }

  function selectOnly(id: number | null) {
    selectedIds = id == null ? [] : [id];
    anchor = id;
  }

  function toggle(id: number) {
    selectedIds = selectedIds.includes(id) ? selectedIds.filter((x) => x !== id) : [...selectedIds, id];
    anchor = id;
  }

  function tap(id: number, event?: MouseEvent) {
    if (event?.shiftKey && anchor != null) {
      const ids = show.patch.fixtures.map((f) => f.id);
      const [a, b] = [ids.indexOf(anchor), ids.indexOf(id)].sort((x, y) => x - y);
      if (a >= 0) {
        selectedIds = [...new Set([...selectedIds, ...ids.slice(a, b + 1)])];
        return;
      }
    }
    if (multi || event?.ctrlKey || event?.metaKey) toggle(id);
    else selectOnly(selectedIds.length === 1 && selectedIds[0] === id ? null : id);
  }

  function selectAll() {
    selectedIds = show.patch.fixtures.map((f) => f.id);
    multi = true;
  }

  // Drop numbers that no longer exist, e.g. after a remove or show load.
  $effect(() => {
    const ids = show.patch.fixtures.map((f) => f.id);
    if (selectedIds.some((id) => !ids.includes(id))) selectedIds = selectedIds.filter((id) => ids.includes(id));
  });

  // Whether a fixture has pan or tilt, so the orientation controls apply.
  function moves(fixture: Fixture): boolean {
    const mode = show.profile(fixture.profile)?.modes.find((m) => m.name === fixture.mode);
    return !!mode?.channels.some((c) => c.attribute === "pan" || c.attribute === "tilt");
  }

  const sameOptions = (a: FixtureOptions, b: FixtureOptions) =>
    (Object.keys(DEFAULT_OPTIONS) as (keyof FixtureOptions)[]).every((k) => a[k] === b[k]);

  // Short labels for the list, e.g. "85%" or "pan +90°".
  function optionTags(o: FixtureOptions | undefined): string[] {
    if (!o) return [];
    const tags: string[] = [];
    if (o.intensityLimit < 100) tags.push(`max ${o.intensityLimit}%`);
    if (o.swapPanTilt) tags.push("swap");
    if (o.invertPan) tags.push("inv pan");
    if (o.invertTilt) tags.push("inv tilt");
    if (o.panOffset) tags.push(`pan ${o.panOffset > 0 ? "+" : ""}${o.panOffset}°`);
    if (o.tiltOffset) tags.push(`tilt ${o.tiltOffset > 0 ? "+" : ""}${o.tiltOffset}°`);
    return tags;
  }

  // Add form
  let profileId = $state<string | null>(null);
  let modeName = $state<string | null>(null);
  let count = $state<number | null>(1);
  let firstId = $state<number | null>(null);
  let address = $state<number | null>(null);
  let name = $state("");
  let search = $state("");

  const profile = $derived(profileId ? show.profile(profileId) : undefined);
  const mode = $derived(profile?.modes.find((m) => m.name === modeName) ?? profile?.modes[0]);
  const matches = $derived(
    show.profiles.filter((p) => `${p.manufacturer} ${p.model}`.toLowerCase().includes(search.trim().toLowerCase())),
  );
  const nextId = $derived(Math.max(0, ...show.patch.fixtures.map((f) => f.id)) + 1);
  const needed = $derived((mode?.channels.length ?? 0) * Math.max(1, count ?? 1));
  const autoAddress = $derived(findFree(needed));

  function findFree(channels: number): number | null {
    if (channels < 1) return null;
    let run = 0;
    for (let i = 0; i < 512; i++) {
      run = show.patch.occupancy[i] == null ? run + 1 : 0;
      if (run === channels) return i + 2 - channels;
    }
    return null;
  }

  function chooseProfile(id: string) {
    profileId = id;
    modeName = show.profile(id)?.modes[0]?.name ?? null;
  }

  async function add() {
    if (!profile || !mode) return;
    const patch = await attempt(() =>
      api.addFixtures({
        profile: profile.id,
        mode: mode.name,
        count: Math.max(1, count ?? 1),
        firstId,
        address,
        name: name.trim() || null,
      }),
    );
    if (patch) {
      show.patch = patch;
      notify(`Added ${count ?? 1} × ${profile.model}`);
      count = 1;
      firstId = null;
      address = null;
      name = "";
    }
  }

  // Edit form, reset whenever a different fixture is selected.
  let edit = $state({
    id: 0 as number | null,
    name: "",
    mode: "",
    address: 1 as number | null,
    options: { ...DEFAULT_OPTIONS } as FixtureOptions,
  });
  let confirmRemove = $state(false);

  $effect(() => {
    if (selected) {
      edit = {
        id: selected.id,
        name: selected.name,
        mode: selected.mode,
        address: selected.address,
        options: { ...(selected.options ?? DEFAULT_OPTIONS) },
      };
      confirmRemove = false;
    }
  });

  const editModes = $derived(selected ? (show.profile(selected.profile)?.modes ?? []) : []);
  const dirty = $derived(
    !!selected &&
      (edit.id !== selected.id ||
        edit.name.trim() !== selected.name ||
        edit.mode !== selected.mode ||
        edit.address !== selected.address ||
        !sameOptions(edit.options, selected.options ?? DEFAULT_OPTIONS)),
  );

  async function save() {
    if (!selected || edit.id == null || edit.address == null) return;
    const patch = await attempt(() =>
      api.updateFixture(selected.id, {
        id: edit.id ?? undefined,
        name: edit.name,
        mode: edit.mode,
        address: edit.address ?? undefined,
        options: edit.options,
      }),
    );
    if (patch) {
      show.patch = patch;
      selectOnly(edit.id);
    }
  }

  async function remove() {
    if (!selected) return;
    if (!confirmRemove) {
      confirmRemove = true;
      return;
    }
    const patch = await attempt(() => api.removeFixtures([selected.id]));
    if (patch) {
      show.patch = patch;
      clearSelection();
    }
  }

  // Bulk edit. Blank fields leave that property as it is.
  let bulk = $state({ firstId: null as number | null, address: null as number | null, name: "", mode: null as string | null });
  let confirmBulkRemove = $state(false);
  // Options in bulk edit start from the fixtures' own when they all match,
  // and set every selected fixture the same once touched.
  let bulkOptions = $state<FixtureOptions>({ ...DEFAULT_OPTIONS });
  let bulkOptionsTouched = $state(false);
  const selectionKey = $derived(selectedIds.join(","));

  $effect(() => {
    void selectionKey;
    bulk = { firstId: null, address: null, name: "", mode: null };
    const first = selection[0]?.options ?? DEFAULT_OPTIONS;
    const shared = selection.every((f) => sameOptions(f.options ?? DEFAULT_OPTIONS, first));
    bulkOptions = { ...(shared ? first : DEFAULT_OPTIONS) };
    bulkOptionsTouched = false;
    confirmBulkRemove = false;
  });

  // Mode can change together only when every selected fixture is the same type.
  const bulkModes = $derived(
    selection.length > 0 && selection.every((f) => f.profile === selection[0].profile)
      ? (show.profile(selection[0].profile)?.modes ?? [])
      : [],
  );
  const bulkFootprints = $derived(
    selection.map((f) =>
      bulk.mode ? (bulkModes.find((m) => m.name === bulk.mode)?.channels.length ?? f.footprint) : f.footprint,
    ),
  );
  const bulkSpan = $derived(bulkFootprints.reduce((a, b) => a + b, 0));
  const bulkDirty = $derived(
    bulk.firstId != null || bulk.address != null || bulk.name.trim() !== "" || bulk.mode != null || bulkOptionsTouched,
  );
  const bulkMoves = $derived(selection.some(moves));
  const bulkLocated = $derived(selection.length > 0 && selection.every((f) => show.live.tested.includes(f.id)));

  async function saveBulk() {
    if (!bulkDirty) return;
    const baseName = bulk.name.trim();
    let address = bulk.address;
    const updates = selection.map((f, i) => {
      const changes: FixtureChanges = {};
      if (bulk.firstId != null) changes.id = bulk.firstId + i;
      if (baseName) changes.name = `${baseName} ${i + 1}`;
      if (bulk.mode) changes.mode = bulk.mode;
      if (bulkOptionsTouched) changes.options = { ...bulkOptions };
      if (address != null) {
        changes.address = address;
        address += bulkFootprints[i];
      }
      return { id: f.id, changes };
    });
    const patch = await attempt(() => api.updateFixtures(updates));
    if (patch) {
      show.patch = patch;
      selectedIds = updates.map((u) => u.changes.id ?? u.id);
      notify(`Updated ${updates.length} fixtures`);
    }
  }

  async function removeBulk() {
    if (!confirmBulkRemove) {
      confirmBulkRemove = true;
      return;
    }
    const count = selectedIds.length;
    const patch = await attempt(() => api.removeFixtures(selectedIds));
    if (patch) {
      show.patch = patch;
      clearSelection();
      notify(`Removed ${count} fixtures`);
    }
  }

  async function toggleLocateAll() {
    const located = bulkLocated;
    for (const f of selection) {
      if (located) await attempt(() => api.release(f.id));
      else if (!show.live.tested.includes(f.id)) await attempt(() => api.locate(f.id));
    }
  }

  async function toggleLocate(fixture: Fixture) {
    if (show.live.tested.includes(fixture.id)) await attempt(() => api.release(fixture.id));
    else await attempt(() => api.locate(fixture.id));
  }

  // Import
  let importing = $state(false);

  // Calibrate opens the stage window on this mover. Locate's test levels
  // would hold it still, so they are let go first.
  let calibrating = $state<number | null>(null);
  const canPoint = (f: Fixture) => show.channels(f).some((c) => c.attribute === "pan");

  async function calibrate(fixture: Fixture) {
    if (show.live.tested.includes(fixture.id)) await attempt(() => api.release(fixture.id));
    calibrating = fixture.id;
  }
  let manufacturer = $state("");
  let fileInput = $state<HTMLInputElement>();

  async function importFile(event: Event) {
    const file = (event.target as HTMLInputElement).files?.[0];
    (event.target as HTMLInputElement).value = "";
    if (!file) return;
    const profiles = await attempt(async () => api.importProfile(await file.text(), manufacturer.trim() || null));
    if (profiles) {
      show.profiles = profiles;
      importing = false;
      notify(`Imported ${file.name}`);
    }
  }
</script>

<div class="patch">
  <header class="row">
    <h1>Patch</h1>
    <span class="muted">{show.patch.fixtures.length} fixtures · {show.patch.occupancy.filter((o) => o != null).length} of 512 channels used</span>
    <span class="spacer"></span>
    <button
      aria-pressed={multi}
      class:on={multi}
      title="Tap fixtures to add or remove them. Ctrl+click and Shift+click also work."
      onclick={() => (multi = !multi)}>Multi-select</button
    >
    <button onclick={selectAll} disabled={show.patch.fixtures.length === 0}>All</button>
    <button onclick={() => (importing = true)}>Import fixture type</button>
    <button class="primary" onclick={clearSelection} disabled={selectedIds.length === 0}>Add fixtures</button>
  </header>

  <section class="list panel scroll" aria-label="Patched fixtures">
    {#if show.patch.fixtures.length === 0}
      <div class="empty muted">
        <p>No fixtures patched yet.</p>
        <p>Choose a fixture type on the right, set how many and where they start, then tap <strong>Add</strong>.</p>
      </div>
    {:else}
      <div class="head row muted">
        <span class="num">#</span><span class="name">Name</span><span class="type">Type</span><span class="addr"
          >Address</span
        >
      </div>
      {#each show.patch.fixtures as fixture (fixture.id)}
        <button
          class="fixture"
          class:selected={selectedIds.includes(fixture.id)}
          aria-pressed={selectedIds.includes(fixture.id)}
          onclick={(e) => tap(fixture.id, e)}
        >
          <span class="num" style:--colour={fixtureColour(fixture.id)}>{fixture.id}</span>
          <span class="name"
            >{fixture.name}{#each optionTags(fixture.options) as tag (tag)}<span class="tag">{tag}</span>{/each}</span
          >
          <span class="type muted">{fixture.typeName} · {fixture.mode}</span>
          <span class="addr">{range(fixture.address, fixture.footprint)}</span>
        </button>
      {/each}
    {/if}
  </section>

  <aside class="side panel scroll" use:formKeys={{ onescape: selectedIds.length ? clearSelection : undefined }}>
    {#if selection.length > 1}
      <div class="row">
        <h2>{selection.length} fixtures</h2>
        <span class="spacer"></span>
        <button aria-pressed={bulkLocated} class:locating={bulkLocated} onclick={toggleLocateAll}>
          {bulkLocated ? "Release all" : "Locate all"}
        </button>
      </div>
      <p class="muted">
        {[...new Set(selection.map((f) => f.typeName))].join(", ")}. Changes apply in fixture number order; leave a
        field blank to keep it.
      </p>
      <div class="pair">
        <NumberField
          label="Renumber from"
          bind:value={bulk.firstId}
          min={1}
          placeholder={`Keep (${selection[0].id}…)`}
        />
        <NumberField label="Re-address from" bind:value={bulk.address} min={1} max={512} placeholder="Keep" />
      </div>
      <label class="field">
        Name
        <input bind:value={bulk.name} placeholder="Keep names" />
      </label>
      {#if bulkModes.length > 1}
        <div class="field">
          Mode
          <div class="segmented">
            {#each bulkModes as m (m.name)}
              <button
                aria-pressed={bulk.mode === m.name}
                onclick={() => (bulk.mode = bulk.mode === m.name ? null : m.name)}
              >
                {m.name} <span class="muted">· {m.channels.length}ch</span>
              </button>
            {/each}
          </div>
        </div>
      {/if}
      <FixtureOptionsForm bind:options={bulkOptions} moves={bulkMoves} onchange={() => (bulkOptionsTouched = true)} />
      {#if bulkDirty}
        <p class="muted summary">
          {#if bulk.firstId != null}Numbers {bulk.firstId}–{bulk.firstId + selection.length - 1}.{/if}
          {#if bulk.address != null}Back to back at {range(bulk.address, bulkSpan)}.{/if}
          {#if bulk.name.trim()}Named {bulk.name.trim()} 1 to {selection.length}.{/if}
          {#if bulk.mode}All set to {bulk.mode}.{/if}
          {#if bulkOptionsTouched}Limits and orientation set the same on all {selection.length}.{/if}
        </p>
      {/if}
      <div class="row actions">
        <button class="danger" onclick={removeBulk}>
          {confirmBulkRemove ? `Tap again to remove ${selection.length}` : `Remove ${selection.length}`}
        </button>
        <span class="spacer"></span>
        <button onclick={clearSelection}>Done</button>
        <button class="primary" disabled={!bulkDirty} onclick={saveBulk}>Save changes</button>
      </div>
    {:else if selected}
      <div class="row">
        <h2>Fixture {selected.id}</h2>
        <span class="spacer"></span>
        {#if canPoint(selected)}
          <button onclick={() => calibrate(selected)}>Calibrate</button>
        {/if}
        <button
          aria-pressed={show.live.tested.includes(selected.id)}
          class:locating={show.live.tested.includes(selected.id)}
          onclick={() => toggleLocate(selected)}
        >
          {show.live.tested.includes(selected.id) ? "Release" : "Locate"}
        </button>
      </div>
      <p class="muted">{selected.typeName}</p>
      <div class="pair">
        <NumberField label="Fixture number" bind:value={edit.id} min={1} />
        <NumberField label="DMX address" bind:value={edit.address} min={1} max={512} />
      </div>
      <label class="field">Name <input bind:value={edit.name} /></label>
      {#if editModes.length > 1}
        <div class="field">
          Mode
          <div class="segmented">
            {#each editModes as m (m.name)}
              <button aria-pressed={edit.mode === m.name} onclick={() => (edit.mode = m.name)}>
                {m.name} <span class="muted">· {m.channels.length}ch</span>
              </button>
            {/each}
          </div>
        </div>
      {/if}
      <FixtureOptionsForm bind:options={edit.options} moves={moves(selected)} />
      <div class="row actions">
        <button class="danger" onclick={remove}>{confirmRemove ? "Tap again to remove" : "Remove"}</button>
        <span class="spacer"></span>
        <button onclick={clearSelection}>Done</button>
        <button class="primary" disabled={!dirty} onclick={save}>Save changes</button>
      </div>
    {:else}
      <h2>Add fixtures</h2>
      <div class="field">
        Fixture type
        {#if show.profiles.length > 8}
          <input placeholder="Search types" bind:value={search} data-enter="ignore" />
        {/if}
        <div class="types">
          {#each matches as p (p.id)}
            <button class="type-option" aria-pressed={p.id === profileId} onclick={() => chooseProfile(p.id)}>
              <span class="muted">{p.manufacturer}</span>
              <span>{p.model}</span>
            </button>
          {/each}
        </div>
      </div>

      {#if profile && mode}
        {#if profile.modes.length > 1}
          <div class="field">
            Mode
            <div class="segmented">
              {#each profile.modes as m (m.name)}
                <button aria-pressed={m.name === mode.name} onclick={() => (modeName = m.name)}>
                  {m.name} <span class="muted">· {m.channels.length}ch</span>
                </button>
              {/each}
            </div>
          </div>
        {/if}
        <div class="pair">
          <NumberField label="How many" bind:value={count} min={1} max={64} />
          <NumberField label="First fixture number" bind:value={firstId} min={1} placeholder={String(nextId)} />
        </div>
        <div class="pair">
          <NumberField
            label="Start address"
            bind:value={address}
            min={1}
            max={512}
            placeholder={autoAddress ? `${autoAddress} (next free)` : "No space"}
          />
          <label class="field">Name <input bind:value={name} placeholder={profile.model} /></label>
        </div>
        <p class="muted summary">
          {mode.channels.length} channels each. Uses addresses
          {range(address ?? autoAddress ?? 0, needed)}.
        </p>
        <button class="primary wide" onclick={add}>Add {count ?? 1} × {profile.model}</button>
      {:else}
        <p class="muted">Pick a type to continue. Missing one? Import it from the Open Fixture Library.</p>
      {/if}
    {/if}
  </aside>

  <section class="map panel">
    <AddressMap
      occupancy={show.patch.occupancy}
      selected={selectedIds}
      levels={show.live.universe}
      onselect={(owner) => owner != null && (multi ? toggle(owner) : selectOnly(owner))}
    />
  </section>
</div>

{#if calibrating != null}
  <StageWindow fixture={calibrating} onclose={() => (calibrating = null)} />
{/if}

{#if importing}
  <Dialog title="Import fixture type" onclose={() => (importing = false)}>
    <p>
      Pick a fixture file. These work:
    </p>
    <ul>
      <li>
        <strong>QLC+</strong> fixture definitions (<em>.qxf</em>), from QLC+'s library or ones you made in its
        fixture editor.
      </li>
      <li>
        <strong>Open Fixture Library</strong>: find the fixture on open-fixture-library.org, choose
        <em>Download</em>, then <em>Open Fixture Library JSON</em>.
      </li>
      <li>HumLX fixture files.</li>
    </ul>
    <label class="field">
      Manufacturer (needed for Open Fixture Library files, optional otherwise)
      <input bind:value={manufacturer} placeholder="e.g. Chauvet DJ" />
    </label>
    <input bind:this={fileInput} type="file" accept=".json,.qxf,application/json" hidden onchange={importFile} />
    {#snippet actions()}
      <button onclick={() => (importing = false)}>Cancel</button>
      <button class="primary" onclick={() => fileInput?.click()}>Choose file…</button>
    {/snippet}
  </Dialog>
{/if}

<style>
  .patch {
    height: 100%;
    display: grid;
    grid-template-columns: 1fr minmax(400px, 520px);
    grid-template-rows: auto 1fr auto;
    gap: var(--gap);
  }

  header {
    grid-column: 1 / -1;
  }

  .list {
    padding: 8px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .empty {
    padding: 40px;
    text-align: center;
  }

  .head,
  .fixture {
    display: grid;
    grid-template-columns: 56px 1.2fr 1.6fr 100px;
    gap: 12px;
    align-items: center;
    padding: 0 12px;
    text-align: left;
  }

  .head {
    min-height: 32px;
    font-size: 0.85rem;
  }

  .fixture {
    min-height: 44px;
    background: transparent;
  }

  .fixture.selected {
    background: var(--accent);
    color: var(--accent-text);
  }

  .fixture.selected :global(.muted) {
    color: #4a5260;
  }

  .num {
    font-family: var(--mono);
    font-weight: 600;
    font-variant-numeric: tabular-nums;
  }

  .fixture .num {
    display: inline-grid;
    place-items: center;
    width: 44px;
    height: 32px;
    border-radius: var(--radius);
    background: color-mix(in srgb, var(--colour) 25%, transparent);
    color: var(--colour);
  }

  .name,
  .type {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .tag {
    margin-left: 6px;
    padding: 1px 5px;
    border: 1px solid var(--line);
    border-radius: var(--radius);
    font-size: 0.75rem;
    color: var(--muted);
  }

  .fixture.selected .tag {
    color: #4a5260;
    border-color: #4a5260;
  }

  .addr {
    font-family: var(--mono);
    font-variant-numeric: tabular-nums;
    text-align: right;
  }

  .side {
    padding: 16px;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  .side p {
    margin: 0;
  }

  .pair {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: var(--gap);
  }

  .types {
    display: grid;
    grid-template-columns: 1fr 1fr;
    gap: 8px;
  }

  .type-option {
    display: grid;
    text-align: left;
    padding: 6px 12px;
    min-height: 52px;
    align-content: center;
  }

  .type-option span:first-child {
    font-size: 0.8rem;
  }

  .type-option[aria-pressed="true"] span {
    color: var(--accent-text);
  }

  .summary {
    font-size: 0.9rem;
  }

  .wide {
    width: 100%;
  }

  .actions {
    margin-top: auto;
  }

  .on {
    background: var(--accent);
    color: var(--accent-text);
  }

  .locating {
    border-color: var(--f-intensity);
    color: var(--f-intensity);
  }

  .map {
    grid-column: 1 / -1;
    padding: 12px;
  }
</style>
