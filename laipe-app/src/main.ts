import { createApp } from "vue";
import App from "./App.vue";
import "./style.css";
import { initConsole, installConsoleHook } from "laipe-vue";

// Install the console hook BEFORE createApp so any logs from app
// startup (and any errors during App.vue's setup) are captured.
installConsoleHook();
// initConsole pulls a backend snapshot and subscribes to the
// `console:entry` Tauri event. No-op in browser-only mode.
initConsole().catch((e: unknown) => {
  // eslint-disable-next-line no-console
  console.error("[main] initConsole failed:", e);
});

createApp(App).mount("#app");
