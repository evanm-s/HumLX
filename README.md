# HumLX

Touch-first DMX lighting control over Art-Net for Windows, built for the
Humdinger studio: one universe, a small rig, and a UI the whole team can pick
up quickly.

## What works today

- **Patch** fixtures by type and mode, several at once, with automatic
  numbering and addressing, overlap checks and a live 512-channel address map.
- **Fixture types**: generic dimmer, RGB and RGBW pars, and moving head spot
  and wash profiles are built in. Real fixtures can be imported from the
  [Open Fixture Library](https://open-fixture-library.org) (download a fixture
  as "Open Fixture Library JSON").
- **Channel test**: a fader per channel to check every fixture against the rig,
  with **Locate** to bring a fixture up at full in white, centred.
- **Art-Net output** of one universe at up to 44 packets per second, to a node's
  IP or broadcast, with node discovery and a **Blackout** button.
- The patch and output settings save automatically after every change.

Groups, the programmer, presets, cues, playback, effects and OSC follow in the
next steps of the plan.

## Project layout

| Path                  | What's there                                                            |
| --------------------- | ----------------------------------------------------------------------- |
| `crates/humlx-core`   | The engine in plain Rust: Art-Net, fixture profiles, patch, show files. |
| `src-tauri`           | The desktop app shell: starts the engine and exposes commands to the UI. |
| `src`                 | The touch UI in Svelte and TypeScript.                                   |

The engine has no UI code so it can be tested on its own (`cargo test`).

## Developing

You need [Rust](https://rustup.rs), [Node.js](https://nodejs.org) 22 and, on
Windows, the WebView2 runtime (already part of Windows 11).

```sh
npm install
npm run tauri dev      # run the app with hot reload
npm run dev            # UI only, in a browser, with a pretend backend
cargo test --workspace # engine tests
npm run check          # type-check the UI
npm run tauri build    # Windows installer in target/release/bundle
```

Every push to a pull request runs the tests and builds a Windows installer,
which can be downloaded from the run's artifacts on GitHub.

The show is saved to `%APPDATA%\au.com.humdinger.humlx\show.json`, and
imported fixture types next to it in `fixtures\`.
