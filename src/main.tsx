import React from "react";
import ReactDOM from "react-dom/client";
import "./index.css";
import App from "./App";
import { installCrashReporter } from "./lib/crashReporter";

installCrashReporter();

ReactDOM.createRoot(document.getElementById("root") as HTMLElement).render(
  <React.StrictMode>
    <App />
  </React.StrictMode>,
);

// HMR cleanup: kill orphan PTY sessions on hot-reload (dev mode only)
// eslint-disable-next-line @typescript-eslint/no-explicit-any
const hmr = (import.meta as any).hot;
if (hmr) {
  hmr.dispose(() => {
    import("@tauri-apps/api/core").then(({ invoke }) => {
      invoke("pty_kill_all").catch(() => {});
    }).catch(() => {});
  });
}
