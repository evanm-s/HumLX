<script lang="ts">
  import { onMount } from "svelte";
  import Fader from "./components/Fader.svelte";
  import Toasts from "./components/Toasts.svelte";
  import UpdateButton from "./components/UpdateButton.svelte";
  import UpdateCue from "./components/UpdateCue.svelte";
  import { updateCue } from "./lib/updatecue.svelte";
  import { api, inApp, remote } from "./lib/api";
  import { listen } from "@tauri-apps/api/event";
  import { link } from "./lib/remote.svelte";
  import PairRemote from "./components/PairRemote.svelte";
  import { attempt, show } from "./lib/show.svelte";
  import { lineText, surface } from "./lib/surface.svelte";
  import { updates } from "./lib/updates.svelte";
  import { SCREEN_EVENT } from "./lib/cuelists.svelte";
  import ChannelTest from "./screens/ChannelTest.svelte";
  import CueLists from "./screens/CueLists.svelte";
  import Patch from "./screens/Patch.svelte";
  import Playback from "./screens/Playback.svelte";
  import Programmer from "./screens/Programmer.svelte";
  import Settings from "./screens/Settings.svelte";
  import Stage from "./screens/Stage.svelte";

  const screens = [
    { id: "programmer", label: "Programmer", icon: "◉" },
    { id: "playback", label: "Playback", icon: "▶" },
    { id: "cuelists", label: "Cue Lists", icon: "▤" },
    { id: "stage", label: "Stage", icon: "◫" },
    { id: "patch", label: "Patch", icon: "▦" },
    { id: "test", label: "Channel test", icon: "☰" },
    { id: "output", label: "Settings", icon: "⚙" },
  ] as const;

  const grandMaster = $derived(Math.round(show.live.grandMaster * 100));

  function setGrandMaster(percent: number) {
    show.live.grandMaster = percent / 100;
    attempt(() => api.setGrandMaster(percent / 100));
  }

  type ScreenId = (typeof screens)[number]["id"];
  let screen = $state<ScreenId>("programmer");

  async function start() {
    await show.load();
    // A new show has nothing to program yet, so start on the patch.
    if (show.patch.fixtures.length === 0) screen = "patch";
  }

  // A remote loads the show once linked, and catches up on anything it
  // missed whenever the link comes back.
  $effect(() => {
    if (!remote || link.state !== "open") return;
    if (link.opened === 1) attempt(start);
    else show.changed(["patch", "programmer", "playback", "stage"]);
  });

  onMount(() => {
    let stopChanges = () => {};
    if (remote) {
      link.onchange = (parts) => show.changed(parts);
      link.start();
    } else {
      attempt(start);
      if (inApp) listen<string[]>("remote-change", (e) => show.changed(e.payload)).then((stop) => (stopChanges = stop));
    }
    updates.start();
    const stopSurface = surface.start();
    const stopLive = show.startLive();
    // Screens can ask for another screen, such as Playback's "Edit in Cue Lists".
    const go = (e: Event) => {
      const id = (e as CustomEvent<string>).detail;
      if (screens.some((s) => s.id === id)) screen = id as ScreenId;
    };
    window.addEventListener(SCREEN_EVENT, go);
    return () => {
      window.removeEventListener(SCREEN_EVENT, go);
      stopChanges();
      stopLive();
      stopSurface.then((stop) => stop());
    };
  });

  // Esc is the keyboard's Clear key and U is Update, unless typing or a
  // dialog is open (dialogs close on Esc themselves).
  function keys(e: KeyboardEvent) {
    const key = e.key === "Escape" ? "clear" : e.key.toLowerCase() === "u" ? "update" : null;
    if (!key || e.defaultPrevented || e.repeat || e.ctrlKey || e.metaKey || e.altKey) return;
    if (document.querySelector("[role=dialog]")) return;
    if ((e.target as HTMLElement).closest?.("input, textarea, select")) return;
    e.preventDefault();
    surface.press(key);
  }

  const status = $derived(
    !show.output.enabled
      ? { kind: "off", text: "Output off" }
      : show.live.status.lastError
        ? { kind: "bad", text: "Output problem" }
        : {
            kind: "ok",
            text: `${show.output.target === "255.255.255.255" ? "Broadcast" : show.output.target} · U${show.output.universe}`,
          },
  );
</script>

<svelte:window onkeydown={keys} />

{#if remote && link.state === "unpaired"}
  <PairRemote />
{:else}
<div class="shell">
  <nav class="rail" aria-label="Screens">
    <div class="brand">
      HumLX
      {#if remote}
        <span class="remote-tag" title="Controlling the Studio PC remotely">Remote</span>
      {:else if show.live.remotes > 0}
        <button class="remote-tag" onclick={() => (screen = "output")} title="Devices controlling this computer">
          {show.live.remotes} remote{show.live.remotes === 1 ? "" : "s"}
        </button>
      {:else}
        <span class="gels" aria-hidden="true"></span>
      {/if}
    </div>
    {#each screens as s (s.id)}
      <button class="nav" aria-current={screen === s.id ? "page" : undefined} onclick={() => (screen = s.id)}>
        <span class="icon" aria-hidden="true">{s.icon}</span>
        <span>{s.label}</span>
      </button>
    {/each}
    <span class="spacer"></span>
    <UpdateButton />
    <button class="status {status.kind}" onclick={() => (screen = "output")} title={show.live.status.lastError ?? ""}>
      <span class="led"></span>{status.text}
    </button>
    <div class="grand-master" title="Grand master: scales all intensity">
      <Fader
        max={100}
        value={grandMaster}
        label="Grand master"
        colour="var(--f-intensity)"
        active={grandMaster < 100}
        format={(v) => `${v}%`}
        onchange={setGrandMaster}
      />
    </div>
    <button
      class="blackout"
      aria-pressed={show.live.blackout}
      onclick={() => attempt(() => api.setBlackout(!show.live.blackout))}
    >
      {show.live.blackout ? "Blackout on" : "Blackout"}
    </button>
  </nav>

  <main>
    {#if remote && link.state === "lost"}
      <div class="preview">Connection to the Studio PC lost. Reconnecting… Lights keep running.</div>
    {:else if !inApp && !remote}
      <div class="preview">Browser preview. Use the desktop app to control lights.</div>
    {/if}
    {#if surface.status.connected || surface.status.problem}
      <div class="surface" class:bad={surface.status.problem}>
        <span class="tag">NX-K</span>
        {#if surface.status.problem}
          <span>{surface.status.problem}</span>
        {:else}
          <span class="line" class:empty={surface.line.length === 0}
            >{lineText(surface.line) || "e.g. 1 Thru 4 @ 5 Enter"}</span
          >
          {#if surface.fine}<span class="fine">Fine</span>{/if}
          <span class="muted">Encoders: {surface.encoders.map((e) => e.label).join(" · ")}</span>
        {/if}
      </div>
    {/if}
    <div class="screen">
      {#if show.loaded}
        {#key show.revision}
          {#if screen === "programmer"}<Programmer />{:else if screen === "playback"}<Playback />{:else if screen === "cuelists"}<CueLists />{:else if screen === "stage"}<Stage />{:else if screen === "patch"}<Patch />{:else if screen === "test"}<ChannelTest
            />{:else}<Settings />{/if}
        {/key}
      {/if}
    </div>
  </main>
</div>
{#if updateCue.request}<UpdateCue cue={updateCue.request.cue} />{/if}
{/if}

<Toasts />

<style>
  .shell {
    height: 100%;
    display: grid;
    grid-template-columns: 112px 1fr;
  }

  .rail {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 14px 8px;
    background: var(--panel);
    border-right: 1px solid var(--line);
    min-height: 0;
    overflow-y: auto;
  }

  .brand {
    display: grid;
    justify-items: center;
    gap: 6px;
    font-size: 1.6rem;
    font-weight: 700;
    letter-spacing: -0.01em;
    padding-bottom: 10px;
  }

  /* The four attribute colours, as a key to the rest of the app. */
  .gels {
    width: 68px;
    height: 8px;
    background: linear-gradient(
      90deg,
      var(--f-intensity) 0 25%,
      var(--f-colour) 25% 50%,
      var(--f-position) 50% 75%,
      var(--f-beam) 75%
    );
  }

  .nav {
    display: grid;
    justify-items: center;
    gap: 4px;
    padding: 8px 6px;
    min-height: 68px;
    background: transparent;
    font-size: 0.9rem;
  }

  .nav .icon {
    font-size: 1.5rem;
    line-height: 1;
  }

  .nav[aria-current="page"] {
    background: var(--accent);
    color: var(--accent-text);
  }

  .status {
    display: grid;
    justify-items: center;
    gap: 6px;
    padding: 12px 8px;
    font-size: 0.75rem;
    line-height: 1.2;
    overflow-wrap: anywhere;
  }

  .led {
    width: 14px;
    height: 14px;
    background: var(--muted);
  }

  .ok .led {
    background: var(--ok);
  }

  .bad .led {
    background: var(--danger);
  }

  .status {
    font-family: var(--mono);
  }

  /* Takes the rail's spare height, up to a comfortable throw, so Blackout
     stays on screen on a laptop. */
  .grand-master {
    flex: 1 1 0;
    min-height: 170px;
    max-height: 300px;
    display: flex;
    justify-content: center;
  }

  .grand-master :global(.fader) {
    gap: 4px;
  }

  .grand-master :global(.track) {
    min-height: 90px;
  }

  .grand-master :global(.detail) {
    display: none;
  }

  /* Down from full is worth noticing: the rail's outline turns amber. */
  .grand-master :global(.fader.active .track) {
    box-shadow: inset 0 0 0 2px var(--f-intensity);
  }

  .blackout {
    min-height: 64px;
    font-weight: 700;
  }

  .blackout:not([aria-pressed="true"]) {
    box-shadow: inset 0 0 0 2px var(--danger);
    background: transparent;
    color: var(--danger);
  }

  .blackout[aria-pressed="true"] {
    background: var(--danger);
    border-color: var(--danger);
    color: var(--accent-text);
  }

  main {
    padding: 14px;
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .screen {
    flex: 1;
    min-height: 0;
  }

  .surface {
    display: flex;
    align-items: center;
    gap: 14px;
    padding: 6px 12px;
    border-radius: var(--radius);
    background: var(--panel);
    font-size: 0.9rem;
  }

  .surface.bad {
    color: var(--warning);
  }

  .surface .tag {
    font-weight: 700;
    color: var(--accent);
  }

  .surface .line {
    flex: 1;
    font-family: var(--mono);
    font-size: 1.1rem;
  }

  .surface .line.empty {
    color: var(--muted);
    font-family: inherit;
    font-size: 0.9rem;
  }

  .surface .fine {
    padding: 2px 8px;
    border-radius: var(--radius);
    background: var(--accent);
    color: var(--bg, #000);
    font-weight: 700;
  }

  .surface .muted {
    color: var(--muted);
  }

  /* Takes the place of the gels under the name, so the rail keeps its height. */
  .remote-tag {
    min-height: 0;
    padding: 1px 8px;
    border: none;
    border-radius: var(--radius);
    background: color-mix(in srgb, var(--f-position) 22%, var(--panel));
    color: var(--f-position);
    font-size: 0.7rem;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
  }
  .preview {
    padding: 8px 14px;
    border-radius: var(--radius);
    background: color-mix(in srgb, var(--warning) 15%, var(--panel));
    color: var(--warning);
    font-size: 0.9rem;
  }
</style>
