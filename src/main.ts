import { mount } from "svelte";
import { invoke } from "@tauri-apps/api/core";
import { getCurrentWindow } from "@tauri-apps/api/window";
import App from "./App.svelte";
import MiniApp from "./MiniApp.svelte";
import "./app.css";

function report(level: "error" | "warn", message: string) {
  invoke("frontend_log", { level, message }).catch(() => {});
}
window.addEventListener("error", (e) => report("error", `${e.message} at ${e.filename}:${e.lineno}`));
window.addEventListener("unhandledrejection", (e) => report("error", `unhandled rejection: ${String(e.reason)}`));
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
