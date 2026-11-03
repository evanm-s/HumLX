<script lang="ts">
  import { onMount } from "svelte";
  import Fader from "./components/Fader.svelte";
  import Toasts from "./components/Toasts.svelte";
  import UpdateButton from "./components/UpdateButton.svelte";
  import { api, inApp } from "./lib/api";
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

  onMount(() => {
    attempt(async () => {
      await show.load();
      // A new show has nothing to program yet, so start on the patch.
      if (show.patch.fixtures.length === 0) screen = "patch";
    });
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
      stopLive();
      stopSurface.then((stop) => stop());
    };
  });

  // Esc is the keyboard's Clear key, unless typing or a dialog is open
  // (dialogs close on Esc themselves).
  function keys(e: KeyboardEvent) {
    if (e.key !== "Escape" || e.defaultPrevented || e.repeat || e.ctrlKey || e.metaKey || e.altKey) return;
    if (document.querySelector("[role=dialog]")) return;
    if ((e.target as HTMLElement).closest?.("input, textarea, select")) return;
    e.preventDefault();
    surface.press("clear");
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

<div class="shell">
  <nav class="rail" aria-label="Screens">
    <div class="brand">HumLX<span class="gels" aria-hidden="true"></span></div>
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
    <div class="grand-master" title="Grand master: scales all intensity output">
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
    {#if !inApp}<div class="preview">Preview mode in a browser. Run the desktop app to control lights.</div>{/if}
    {#if surface.status.connected || surface.status.problem}
      <div class="surface" class:bad={surface.status.problem}>
        <span class="tag">NX-K</span>
        {#if surface.status.problem}
          <span>{surface.status.problem}</span>
        {:else}
          <span class="line" class:empty={surface.line.length === 0}
            >{lineText(surface.line) || "Type on the keypad, e.g. 1 Thru 4 @ 5 Enter"}</span
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

  .preview {
    padding: 8px 14px;
    border-radius: var(--radius);
    background: color-mix(in srgb, var(--warning) 15%, var(--panel));
    color: var(--warning);
    font-size: 0.9rem;
  }
</style>
