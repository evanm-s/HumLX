<script lang="ts">
  import { onMount } from "svelte";
  import Toasts from "./components/Toasts.svelte";
  import UpdateButton from "./components/UpdateButton.svelte";
  import { api, inApp } from "./lib/api";
  import { attempt, show } from "./lib/show.svelte";
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
    return show.startLive();
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
    grid-template-columns: 148px 1fr;
  }

  .rail {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding: 20px 12px;
    background: var(--panel);
    border-right: 1px solid var(--line);
  }

  .brand {
    font-size: 1.6rem;
    font-weight: 800;
    letter-spacing: -0.02em;
    text-align: center;
    padding-bottom: 12px;
  }

  .brand span {
    color: var(--accent);
  }

  .nav {
    display: grid;
    justify-items: center;
    gap: 4px;
    padding: 14px 8px;
    min-height: 96px;
    border-color: transparent;
    background: transparent;
    font-size: 0.9rem;
  }

  .nav .icon {
    font-size: 1.8rem;
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
    min-height: 88px;
    font-weight: 700;
  }

  .blackout[aria-pressed="true"] {
    background: var(--danger);
    border-color: var(--danger);
    color: #fff;
  }

  main {
    padding: 24px;
    min-width: 0;
    min-height: 0;
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .screen {
    flex: 1;
    min-height: 0;
  }

  .preview {
    padding: 10px 16px;
    border-radius: var(--radius);
    background: color-mix(in srgb, var(--warning) 15%, var(--panel));
    color: var(--warning);
    font-size: 0.9rem;
  }
</style>
