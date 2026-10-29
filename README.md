# HumLX

Touch-first DMX lighting control over Art-Net for Windows, built for the
Humdinger studio: one universe, a small rig, and a UI the whole team can pick
up quickly.

## What works today

- **Programmer**: select fixtures or groups, then set intensity, colour (a
  colour disc and quick swatches), pan and tilt (a touch pad with a fine mode)
  and beam controls, all on screen at once. A programmer sheet lists every
  value that's set. The screen is made of windows that can be moved, resized
  and hidden with **Edit layout**, and the layout is remembered. Colours work across RGB, RGBW, CMY and colour-wheel
  fixtures alike.
- **Cues and cue lists**: tap **Record cue** on the Programmer or Playback
  screen, then pick a cue list or name a new one. Cues track, so each
  one only stores what changed. Give a cue a name and fade time, and update or
  delete it later. **GO** fades to the next cue, **Back** steps back.
- **Playback faders**: each cue list gets a strip with a master fader, GO, Back
  and Release. Intensity from several lists combines highest-takes-precedence;
  the programmer always sits on top.
- **Effects**: dimmer chases (sine, square or saw), rainbow colour and position
  circles on any selection, with rate, size and spread across the fixtures.
- **Groups and presets**: save a selection as a group, and record intensity,
  colour, position and beam presets that recall onto any selection.
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
- The patch, groups, presets, cue lists, effects and output settings save automatically after
  every change.

OSC cue triggering, show files and a simple mode follow in the next step.

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

Every push runs the tests and builds a Windows installer and a Linux
AppImage, which can be downloaded from the run's artifacts on GitHub.

### Trying it on Linux

Download the AppImage from a CI run, then `chmod +x` it and run it. To run
from source on Arch-based systems instead:

```sh
sudo pacman -S --needed webkit2gtk-4.1 base-devel rustup nodejs npm
rustup default stable
npm install && npm run tauri dev
```

Without a node on the network the app still runs: the address map and channel
test faders show the live levels. If the AppImage shows a blank window, run it
with `WEBKIT_DISABLE_DMABUF_RENDERER=1`.

The show is saved to `%APPDATA%\au.com.humdinger.humlx\show.json`, and
imported fixture types next to it in `fixtures\`.
