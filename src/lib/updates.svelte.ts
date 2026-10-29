// Checks GitHub Releases for a newer version of HumLX and installs it on
// request. Only works in the desktop app.

import { getVersion } from "@tauri-apps/api/app";
import { relaunch } from "@tauri-apps/plugin-process";
import { check, type Update } from "@tauri-apps/plugin-updater";
import { inApp } from "./api";

const EVERY_MS = 6 * 60 * 60 * 1000;

class Updates {
  version = $state<string | null>(null);
  available = $state<Update | null>(null);
  checking = $state(false);
  /** Download progress from 0 to 1 while installing, else null. */
  progress = $state<number | null>(null);
  error = $state<string | null>(null);
  lastChecked = $state<Date | null>(null);

  start() {
    if (!inApp) return;
    getVersion().then((v) => (this.version = v));
    setTimeout(() => this.check(), 5000);
    setInterval(() => this.check(), EVERY_MS);
  }

  async check() {
    if (!inApp || this.checking || this.progress != null) return;
    this.checking = true;
    this.error = null;
    try {
      this.available = await check();
      this.lastChecked = new Date();
    } catch (error) {
      this.error = String(error);
    } finally {
      this.checking = false;
    }
  }

  /** Downloads and installs the update, then restarts the app. */
  async install() {
    const update = this.available;
    if (!update) return;
    this.error = null;
    this.progress = 0;
    let total = 0;
    let done = 0;
    try {
      await update.downloadAndInstall((event) => {
        if (event.event === "Started") total = event.data.contentLength ?? 0;
        else if (event.event === "Progress") {
          done += event.data.chunkLength;
          this.progress = total ? done / total : null;
        }
      });
      await relaunch();
    } catch (error) {
      this.error = String(error);
      this.progress = null;
    }
  }
}

export const updates = new Updates();
