import { createApp } from "vue";
import App from "./App.vue";
import "./style.css";
import { initConsole, installConsoleHook, localStorageConfig, setConfigStorage, whenConfigReady } from "laipe-vue";
import { router } from "./router";

// Install the console hook BEFORE createApp so any logs from app
// startup (and any errors during App.vue's setup) are captured.
installConsoleHook();
// initConsole pulls a backend snapshot and subscribes to the
// `console:entry` Tauri event. No-op in browser-only mode.
initConsole().catch((e: unknown) => {
  // eslint-disable-next-line no-console
  console.error("[main] initConsole failed:", e);
});

// Load the persisted provider/agent config (localStorage keys
// `laipe.config.v1` / `laipe.agent.v1`) BEFORE mounting so the first
// render already sees saved settings instead of defaults. Explicit
// wiring even though useConfig now self-loads at module init — this
// is the documented pattern for swapping in a custom storage later.
setConfigStorage(localStorageConfig);

const app = createApp(App);
app.use(router);
// Wait for the storage load (async adapter or not) so the first render
// already sees the saved config instead of a flash of defaults.
void whenConfigReady().then(() => app.mount("#app"));
