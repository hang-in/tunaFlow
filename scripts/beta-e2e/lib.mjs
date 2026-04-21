// Shared helpers for Phase 5 beta E2E scripts.
//
// Usage from any scenario:
//   import { api, ws, waitUntil, assert, log, env } from "./lib.mjs";
//
// Requires Node 20+ (built-in fetch + WebSocket).
// Requires a running tunaFlow instance — either local (`npm run tauri dev`)
// or via tunnel. Set:
//   TUNAFLOW_BASE=http://127.0.0.1:8787      (default)
//   TUNAFLOW_TOKEN=<api token>               (required, from Settings > Mobile)

export const env = {
  base: process.env.TUNAFLOW_BASE ?? "http://127.0.0.1:8787",
  token: process.env.TUNAFLOW_TOKEN ?? "",
  verbose: process.env.VERBOSE === "1",
};

if (!env.token) {
  console.error("[beta-e2e] TUNAFLOW_TOKEN not set — Settings > Mobile > API 토큰 복사 후 export");
  process.exit(2);
}

// ─── Logging ─────────────────────────────────────────────────────────────────

const color = {
  gray: (s) => `\x1b[90m${s}\x1b[0m`,
  green: (s) => `\x1b[32m${s}\x1b[0m`,
  red: (s) => `\x1b[31m${s}\x1b[0m`,
  yellow: (s) => `\x1b[33m${s}\x1b[0m`,
  bold: (s) => `\x1b[1m${s}\x1b[0m`,
};

export const log = {
  step: (msg) => console.log(color.bold(`▶ ${msg}`)),
  ok: (msg) => console.log(color.green(`  ✓ ${msg}`)),
  fail: (msg) => console.log(color.red(`  ✗ ${msg}`)),
  info: (msg) => env.verbose && console.log(color.gray(`  · ${msg}`)),
  warn: (msg) => console.log(color.yellow(`  ! ${msg}`)),
};

// ─── HTTP ─────────────────────────────────────────────────────────────────────

export async function api(path, init = {}) {
  const url = path.startsWith("http") ? path : `${env.base}${path}`;
  const headers = {
    "Content-Type": "application/json",
    Authorization: `Bearer ${env.token}`,
    ...(init.headers ?? {}),
  };
  const res = await fetch(url, { ...init, headers });
  const text = await res.text();
  let body;
  try { body = text ? JSON.parse(text) : null; } catch { body = text; }
  if (!res.ok) {
    const err = new Error(`${init.method ?? "GET"} ${url} → ${res.status}`);
    err.status = res.status;
    err.body = body;
    throw err;
  }
  log.info(`${init.method ?? "GET"} ${path} → ${res.status}`);
  return body;
}

// ─── WebSocket subscription ──────────────────────────────────────────────────

export function ws({ since, onEvent, onOpen, onClose } = {}) {
  const wsBase = env.base.replace(/^http/, "ws");
  const qs = new URLSearchParams({ token: env.token });
  if (since != null) qs.set("since", String(since));
  const url = `${wsBase}/ws/events?${qs}`;
  log.info(`WS connect: ${url.replace(env.token, "***")}`);

  const socket = new WebSocket(url);
  const events = [];
  socket.addEventListener("open", () => { log.info("WS open"); onOpen?.(); });
  socket.addEventListener("message", (e) => {
    try {
      const evt = JSON.parse(e.data);
      events.push(evt);
      onEvent?.(evt);
    } catch {
      log.warn(`WS non-JSON message: ${e.data.slice(0, 80)}`);
    }
  });
  socket.addEventListener("close", () => { log.info("WS close"); onClose?.(); });
  socket.addEventListener("error", (e) => log.warn(`WS error: ${e.message ?? "?"}`));

  return {
    socket,
    events,
    close: () => socket.close(),
    waitFor: (predicate, timeoutMs = 30000) =>
      new Promise((resolve, reject) => {
        const existing = events.find(predicate);
        if (existing) return resolve(existing);
        const timer = setTimeout(() => {
          socket.removeEventListener("message", handler);
          reject(new Error(`WS waitFor timeout (${timeoutMs}ms)`));
        }, timeoutMs);
        const handler = (e) => {
          try {
            const evt = JSON.parse(e.data);
            if (predicate(evt)) {
              clearTimeout(timer);
              socket.removeEventListener("message", handler);
              resolve(evt);
            }
          } catch { /* ignore */ }
        };
        socket.addEventListener("message", handler);
      }),
  };
}

// ─── Utilities ───────────────────────────────────────────────────────────────

export async function waitUntil(fn, { timeoutMs = 10000, intervalMs = 200 } = {}) {
  const deadline = Date.now() + timeoutMs;
  while (Date.now() < deadline) {
    const v = await fn();
    if (v) return v;
    await new Promise((r) => setTimeout(r, intervalMs));
  }
  throw new Error(`waitUntil timed out after ${timeoutMs}ms`);
}

export function assert(cond, msg) {
  if (!cond) throw new Error(`assertion failed: ${msg}`);
}

export function runScenario(name, fn) {
  return (async () => {
    log.step(name);
    const t0 = Date.now();
    try {
      await fn();
      log.ok(`${name} (${Date.now() - t0}ms)`);
      process.exit(0);
    } catch (e) {
      log.fail(`${name}: ${e.message}`);
      if (e.body) console.error(e.body);
      if (env.verbose) console.error(e.stack);
      process.exit(1);
    }
  })();
}
