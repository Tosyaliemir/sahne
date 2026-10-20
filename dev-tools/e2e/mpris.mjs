// MPRIS end-to-end test (T4): runs the real Sahne backend against the mock
// player and checks what it logs. In CI it runs on a private session bus
// and a virtual display:
//   dbus-run-session -- xvfb-run -a node dev-tools/e2e/mpris.mjs
// Locally `node dev-tools/e2e/mpris.mjs` works too (the mock has its own
// bus name). Build first: `cargo build` in src-tauri and in
// dev-tools/mock-player.
import { spawn } from "node:child_process";
import { mkdtempSync, rmSync } from "node:fs";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../..", import.meta.url));
const SAHNE = process.env.SAHNE_BIN ?? join(root, "src-tauri/target/debug/sahne");
const MOCK = process.env.MOCK_BIN ?? join(root, "dev-tools/mock-player/target/debug/mock-player");
const BUS = "org.mpris.MediaPlayer2.sahnemock";

/** A child process whose output can be awaited line by line. */
function run(name, cmd, args, env = {}) {
  const child = spawn(cmd, args, { env: { ...process.env, ...env }, stdio: ["pipe", "pipe", "pipe"] });
  const lines = [];
  const waiters = new Set();
  const onData = (chunk) => {
    for (const raw of chunk.toString().split("\n")) {
      // Strip ANSI colour codes.
      // eslint-disable-next-line no-control-regex
      const line = raw.replace(/\x1b\[[0-9;]*m/g, "").trimEnd();
      if (!line) continue;
      lines.push(line);
      if (process.env.E2E_VERBOSE) console.log(`[${name}] ${line}`);
      for (const w of waiters) if (w.test(line)) w.done(line);
    }
  };
  child.stdout.on("data", onData);
  child.stderr.on("data", onData);
  return {
    child,
    send: (text) => child.stdin.write(`${text}\n`),
    /** Resolves with the first line (from now on, or `since` index) matching `re`. */
    waitFor(re, ms = 15_000, since = lines.length) {
      const earlier = lines.slice(since).find((l) => re.test(l));
      if (earlier) return Promise.resolve(earlier);
      return new Promise((resolve, reject) => {
        const w = {
          test: (l) => re.test(l),
          done: (l) => {
            clearTimeout(timer);
            waiters.delete(w);
            resolve(l);
          },
        };
        const timer = setTimeout(() => {
          waiters.delete(w);
          reject(new Error(`${name}: no line matching ${re} within ${ms} ms`));
        }, ms);
        waiters.add(w);
      });
    },
    mark: () => lines.length,
    /** Asks the process to quit and resolves once it has exited (SIGKILL after 5 s). */
    stop() {
      if (child.exitCode !== null || child.signalCode !== null) return Promise.resolve();
      return new Promise((resolve) => {
        const timer = setTimeout(() => child.kill("SIGKILL"), 5_000);
        child.once("exit", () => {
          clearTimeout(timer);
          resolve();
        });
        child.kill("SIGTERM");
      });
    },
  };
}

const steps = [];
async function step(title, fn) {
  const started = Date.now();
  await fn();
  steps.push(title);
  console.log(`ok  ${title} (${Date.now() - started} ms)`);
}

const home = mkdtempSync(join(tmpdir(), "sahne-e2e-"));
const env = {
  RUST_LOG: "sahne_lib=debug",
  SAHNE_PLAYER: "sahnemock",
  XDG_CONFIG_HOME: join(home, "config"),
  XDG_CACHE_HOME: join(home, "cache"),
  NO_COLOR: "1",
};
let mock = run("mock", MOCK, []);
const sahne = run("sahne", SAHNE, [], env);
let failed = false;
try {
  await step("follows the mock player", async () => {
    await mock.waitFor(/ready/);
    await sahne.waitFor(new RegExp(`following ${BUS.replaceAll(".", "\\.")}`), 30_000);
    await sahne.waitFor(/track: Rick Astley – Never Gonna Give You Up/);
  });

  await step("follows a track change", async () => {
    const at = sahne.mark();
    mock.send("n");
    await sahne.waitFor(/track: Queen – Bohemian Rhapsody/, 10_000, at);
  });

  await step("follows a seek", async () => {
    const at = sahne.mark();
    mock.send("s 60");
    await sahne.waitFor(/position 6\d{4}ms .*hard=true/, 10_000, at);
  });

  await step("notices the player quitting", async () => {
    const at = sahne.mark();
    mock.send("q");
    await sahne.waitFor(
      new RegExp(`${BUS.replaceAll(".", "\\.")} quit|no track|no MPRIS player found`),
      10_000,
      at,
    );
  });

  await step("reconnects to a new player and estimates a stuck position", async () => {
    const at = sahne.mark();
    mock = run("mock", MOCK, ["--zero-position"]);
    await sahne.waitFor(/track: Rick Astley – Never Gonna Give You Up/, 20_000, at);
    const connected = Date.now();
    // The mock reports position 0 forever; Sahne switches to its own clock,
    // but only after zeros over about two seconds, not on the first burst.
    await sahne.waitFor(/source=Estimated/, 15_000, at);
    const after = Date.now() - connected;
    if (after < 1_500) throw new Error(`switched to the estimate after only ${after} ms`);
  });
} catch (e) {
  failed = true;
  console.error(`FAIL ${e.message}`);
} finally {
  // Wait for both to exit: Sahne can still be writing caches under `home`.
  await Promise.all([mock.stop(), sahne.stop()]);
  try {
    rmSync(home, { recursive: true, force: true, maxRetries: 5, retryDelay: 200 });
  } catch (e) {
    console.warn(`could not remove ${home}: ${e.message}`);
  }
}
console.log(`${steps.length} step(s) passed`);
process.exit(failed ? 1 : 0);
