import { defineConfig } from "vite";
import { svelte } from "@sveltejs/vite-plugin-svelte";

// Settings follow the Tauri guide: a fixed port the Rust side points at, and
// no screen clearing so Rust errors stay visible.
export default defineConfig({
  plugins: [svelte()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**", "**/crates/**", "**/target/**"] },
  },
  build: {
    target: "es2022",
  },
});
