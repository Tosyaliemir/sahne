import { mount } from "svelte";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import App from "./App.svelte";
import MiniApp from "./MiniApp.svelte";
import "./app.css";

import { LogLimiter } from "./lib/logLimit";

// Every warning crosses IPC into the backend log; a per-frame warning must
// not flood it. At most 20 messages per 10 s, repeats folded into a count.
const limiter = new LogLimiter((level, message) => {
  invoke("frontend_log", { level, message }).catch(() => {});
});
function report(level: "error" | "warn", message: string) {
  limiter.log(level, message);
}
window.addEventListener("error", (e) => report("error", `${e.message} at ${e.filename}:${e.lineno}`));
window.addEventListener("unhandledrejection", (e) =>
  report("error", `unhandled rejection: ${String(e.reason)}`),
);
// The CSP (tauri.conf.json) should never fire; if it does, it's a bug to see in the log.
document.addEventListener("securitypolicyviolation", (e) =>
  report("error", `CSP blocked ${e.blockedURI || "inline"} (${e.violatedDirective})`),
);
const warn = console.warn.bind(console);
console.warn = (...args: unknown[]) => {
  warn(...args);
  report("warn", args.map(String).join(" "));
};
const error = console.error.bind(console);
console.error = (...args: unknown[]) => {
  error(...args);
  report("error", args.map((a) => (a instanceof Error ? `${a.message}\n${a.stack}` : String(a))).join(" "));
};

const target = document.getElementById("app");
if (!target) throw new Error("#app missing from index.html");

// Both windows load the same page; the window label picks the UI.
const isMini = getCurrentWindow().label === "mini";
export default mount(isMini ? MiniApp : App, { target });
