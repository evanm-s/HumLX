<script lang="ts">
  import { onMount } from "svelte";
  import Toasts from "./components/Toasts.svelte";
  import UpdateButton from "./components/UpdateButton.svelte";
  import { api, inApp } from "./lib/api";
  import { attempt, show } from "./lib/show.svelte";
  import { lineText, surface } from "./lib/surface.svelte";
  import { updates } from "./lib/updates.svelte";
  import ChannelTest from "./screens/ChannelTest.svelte";
  import Patch from "./screens/Patch.svelte";
  import Playback from "./screens/Playback.svelte";
  import Programmer from "./screens/Programmer.svelte";
  import Settings from "./screens/Settings.svelte";

  const screens = [
    { id: "programmer", label: "Programmer", icon: "◉" },
    { id: "playback", label: "Playback", icon: "▶" },
    { id: "patch", label: "Patch", icon: "▦" },
    { id: "test", label: "Channel test", icon: "☰" },
    { id: "output", label: "Settings", icon: "⚙" },
  ] as const;

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
    return () => {
      stopLive();
      stopSurface.then((stop) => stop());
    };
  });

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

<div class="shell">
  <nav class="rail" aria-label="Screens">
    <div class="brand">Hum<span>LX</span></div>
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
          {#if screen === "programmer"}<Programmer />{:else if screen === "playback"}<Playback />{:else if screen === "patch"}<Patch />{:else if screen === "test"}<ChannelTest
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
  }

  .brand {
    font-size: 1.6rem;
    font-weight: 800;
    letter-spacing: -0.02em;
    text-align: center;
    padding-bottom: 8px;
  }

  .brand span {
    color: var(--accent);
  }

  .nav {
    display: grid;
    justify-items: center;
    gap: 4px;
    padding: 8px 6px;
    min-height: 68px;
    border-color: transparent;
    background: transparent;
    font-size: 0.9rem;
  }

  .nav .icon {
    font-size: 1.5rem;
    line-height: 1;
  }

  .nav[aria-current="page"] {
    background: var(--panel-raised);
    border-color: var(--line);
    color: var(--accent);
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
    border-radius: 50%;
    background: var(--muted);
  }

  .ok .led {
    background: var(--ok);
    box-shadow: 0 0 10px var(--ok);
  }

  .bad .led {
    background: var(--danger);
    box-shadow: 0 0 10px var(--danger);
  }

  .blackout {
    min-height: 64px;
    font-weight: 700;
  }

  .blackout[aria-pressed="true"] {
    background: var(--danger);
    border-color: var(--danger);
    color: #fff;
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
    border: 1px solid var(--line);
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
    font-family: ui-monospace, monospace;
    font-size: 1.1rem;
  }

  .surface .line.empty {
    color: var(--muted);
    font-family: inherit;
    font-size: 0.9rem;
  }

  .surface .fine {
    padding: 2px 8px;
    border-radius: 999px;
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
