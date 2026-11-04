<script lang="ts">
  // Shown in a remote browser until it's paired with the Studio PC.
  import { link } from "../lib/remote.svelte";

  let code = $state("");
  let name = $state(link.savedName);
  let error = $state<string | null>(null);
  let busy = $state(false);

  const ready = $derived(/^\d{6}$/.test(code.trim()) && !busy);

  async function pair() {
    if (!ready) return;
    busy = true;
    error = await link.pair(code, name);
    busy = false;
  }
</script>

<div class="pair">
  <form class="panel card" onsubmit={(e) => (e.preventDefault(), pair())}>
    <div class="brand">HumLX<span class="gels" aria-hidden="true"></span></div>
    <h1>Control the Studio PC from here</h1>
    <p class="muted">
      On the Studio PC, tap <strong>Settings</strong> › <strong>Remote control</strong> ›
      <strong>Pair a device</strong>, then type the code here.
    </p>
    <label class="field">
      Code
      <!-- svelte-ignore a11y_autofocus -->
      <input
        class="code"
        bind:value={code}
        inputmode="numeric"
        autocomplete="one-time-code"
        maxlength="6"
        placeholder="000000"
        autofocus
      />
    </label>
    <label class="field">
      Device name
      <input bind:value={name} maxlength="40" placeholder="Evan's laptop" />
    </label>
    {#if error}<p class="bad">{error}</p>{/if}
    <button class="primary" type="submit" disabled={!ready}>{busy ? "Pairing…" : "Pair"}</button>
  </form>
</div>

<style>
  .pair {
    display: grid;
    place-items: center;
    min-height: 100vh;
    padding: 16px;
  }
  form {
    display: flex;
    flex-direction: column;
    gap: 14px;
    width: min(420px, 100%);
    padding: 24px;
  }
  .brand {
    font-weight: 700;
    font-size: 18px;
  }
  h1 {
    margin: 0;
    font-size: 20px;
  }
  p {
    margin: 0;
  }
  .field {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }
  .code {
    font-size: 28px;
    letter-spacing: 0.3em;
    text-align: center;
  }
  .bad {
    color: var(--danger);
  }
</style>
