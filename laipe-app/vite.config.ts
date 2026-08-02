import { defineConfig } from "vite";
import vue from "@vitejs/plugin-vue";

// Tauri dev server runs on a fixed port (see src-tauri/tauri.conf.json
// `build.devUrl`). 5175 keeps it separate from vanilla-web (:5173) and
// chat-app (:5174).
const host = process.env.TAURI_DEV_HOST;

export default defineConfig({
  plugins: [vue()],
  // Tauri expects a fixed port. If not set, fail so the user notices.
  clearScreen: false,
  server: {
    port: 5175,
    strictPort: true,
    host: host || false,
    hmr: host
      ? { protocol: "ws", host, port: 5175 }
      : undefined,
    watch: {
      // Don't watch the Rust side from the Vite dev server.
      ignored: ["**/src-tauri/**"],
    },
  },
  build: {
    target: "es2022",
    // Tauri uses Chromium on Windows/Mac/Linux (and WebKit on iOS). Modern
    // targets are safe.
    minify: "esbuild",
    sourcemap: false,
  },
});
