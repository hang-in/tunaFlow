#!/usr/bin/env node
// Prompt regression runner.
//
// For each item in `evals/golden/*.jsonl`:
//   1. Submit `prompt` to a fresh scratch conversation via `/api/v1/*`
//   2. Wait for `agent:completed` WS event
//   3. Fetch the final assistant message content
//   4. Ask the judge (Haiku) to score semantic equivalence vs reference
//   5. Accumulate results; write JSON snapshot to `evals/results/`
//
// Env:
//   TUNAFLOW_BASE        default http://127.0.0.1:19840
//   TUNAFLOW_TOKEN       required (Settings > Mobile)
//   EVAL_ENGINE          generation engine, default `claude`
//   EVAL_MODEL           generation model, default = item.model_ref
//   JUDGE_ENGINE         judge engine, default `claude`
//   JUDGE_MODEL          judge model, default `claude-haiku-4-5-20251001`
//   EVAL_GOLDEN_DIR      default `evals/golden`
//   EVAL_RESULTS_DIR     default `evals/results`
//   EVAL_FILTER          regex on item.id / item.category (optional)
//   EVAL_TIMEOUT_MS      per-scenario generation timeout, default 180000

import {
  readdirSync,
  readFileSync,
  mkdirSync,
  writeFileSync,
  existsSync,
} from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const HERE = dirname(fileURLToPath(import.meta.url));
const REPO = resolve(HERE, "..", "..");
const GOLDEN_DIR = resolve(
  REPO,
  process.env.EVAL_GOLDEN_DIR ?? "evals/golden"
);
const RESULTS_DIR = resolve(
  REPO,
  process.env.EVAL_RESULTS_DIR ?? "evals/results"
);
const JUDGE_PROMPT_PATH = resolve(
  REPO,
  "evals/judge/prompts/semantic-equivalence.md"
);

const BASE = process.env.TUNAFLOW_BASE ?? "http://127.0.0.1:19840";
const TOKEN = process.env.TUNAFLOW_TOKEN ?? "";
const EVAL_ENGINE = process.env.EVAL_ENGINE ?? "claude";
const EVAL_MODEL = process.env.EVAL_MODEL ?? "";
const JUDGE_ENGINE = process.env.JUDGE_ENGINE ?? "claude";
const JUDGE_MODEL =
  process.env.JUDGE_MODEL ?? "claude-haiku-4-5-20251001";
const FILTER = process.env.EVAL_FILTER ? new RegExp(process.env.EVAL_FILTER) : null;
const GEN_TIMEOUT_MS = parseInt(
  process.env.EVAL_TIMEOUT_MS ?? "180000",
  10
);

if (!TOKEN) {
  console.error("[eval] TUNAFLOW_TOKEN not set");
  process.exit(2);
}
if (!existsSync(JUDGE_PROMPT_PATH)) {
  console.error(`[eval] judge prompt missing: ${JUDGE_PROMPT_PATH}`);
  process.exit(2);
}
const JUDGE_SYSTEM = readFileSync(JUDGE_PROMPT_PATH, "utf8");

// ─── HTTP + WS helpers (slimmer copy of scripts/beta-e2e/lib.mjs) ────────────

async function api(path, init = {}) {
  const url = path.startsWith("http") ? path : `${BASE}${path}`;
  const res = await fetch(url, {
    ...init,
    headers: {
      "Content-Type": "application/json",
      Authorization: `Bearer ${TOKEN}`,
      ...(init.headers ?? {}),
    },
  });
  const text = await res.text();
  const body = text ? tryJson(text) : null;
  if (!res.ok) {
    const err = new Error(`${init.method ?? "GET"} ${url} → ${res.status}`);
    err.status = res.status;
    err.body = body;
    throw err;
  }
  return body;
}

function tryJson(s) {
  try {
    return JSON.parse(s);
  } catch {
    return s;
  }
}

function openWs({ onEvent } = {}) {
  const wsBase = BASE.replace(/^http/, "ws");
  const qs = new URLSearchParams({ token: TOKEN });
  const socket = new WebSocket(`${wsBase}/ws/events?${qs}`);
  const events = [];
  socket.addEventListener("message", (e) => {
    const evt = tryJson(e.data);
    if (evt && typeof evt === "object") {
      events.push(evt);
      onEvent?.(evt);
    }
  });
  return {
    socket,
    events,
    close: () => socket.close(),
    waitFor: (predicate, timeoutMs) =>
      new Promise((resolvePromise, reject) => {
        const prior = events.find(predicate);
        if (prior) return resolvePromise(prior);
        const t = setTimeout(() => {
          socket.removeEventListener("message", handler);
          reject(new Error(`WS waitFor timeout (${timeoutMs}ms)`));
        }, timeoutMs);
        const handler = (e) => {
          const evt = tryJson(e.data);
          if (evt && predicate(evt)) {
            clearTimeout(t);
            socket.removeEventListener("message", handler);
            resolvePromise(evt);
          }
        };
        socket.addEventListener("message", handler);
      }),
  };
}

async function waitSocketOpen(socket, timeoutMs = 5000) {
  if (socket.readyState === WebSocket.OPEN) return;
  await new Promise((res, rej) => {
    const t = setTimeout(() => rej(new Error("WS open timeout")), timeoutMs);
    socket.addEventListener("open", () => {
      clearTimeout(t);
      res();
    });
    socket.addEventListener("error", (e) => {
      clearTimeout(t);
      rej(new Error(`WS error: ${e.message ?? "?"}`));
    });
  });
}

// ─── Scaffolding: scratch project + conversation for a single item ───────────

async function scratchConv(label) {
  const suffix = Date.now();
  const projectKey = `eval-${suffix}`;
  await api("/api/v1/projects", {
    method: "POST",
    body: JSON.stringify({
      key: projectKey,
      name: `[eval] ${label}`,
      path: "/tmp/tunaflow-eval",
    }),
  });
  const conv = await api("/api/v1/conversations", {
    method: "POST",
    body: JSON.stringify({
      projectKey,
      label: `[eval] ${label}`,
      mode: "chat",
    }),
  });
  return { projectKey, conv };
}

async function generate({ convId, prompt, engine, model }) {
  const socket = openWs();
  await waitSocketOpen(socket.socket);
  const payload = { prompt, engine };
  if (model) payload.model = model;
  api(`/api/v1/conversations/${convId}/send`, {
    method: "POST",
    body: JSON.stringify(payload),
  }).catch(() => {
    // 202 "queued" is expected; real errors surface via WS / fetchMessages
  });
  const done = await socket.waitFor(
    (e) => e.type === "agent:completed" && e.conversationId === convId,
    GEN_TIMEOUT_MS
  );
  socket.close();

  const msgs = await api(`/api/v1/conversations/${convId}/messages`);
  // Final assistant message (highest timestamp, role=assistant, status=done)
  const assistant = [...msgs]
    .filter((m) => m.role === "assistant" && m.status === "done")
    .sort((a, b) => b.timestamp - a.timestamp)[0];
  if (!assistant) throw new Error("no assistant message after agent:completed");
  return { content: assistant.content, doneEvent: done };
}

// ─── Judge call ──────────────────────────────────────────────────────────────

function judgeUserPrompt(item, candidate) {
  return [
    `CATEGORY: ${item.category}`,
    `SOURCE_PROMPT_KIND: ${item.source_prompt_kind ?? "user"}`,
    "",
    "EXPECTED_BEHAVIORS:",
    (item.expected_behaviors && item.expected_behaviors.length
      ? item.expected_behaviors.map((b, i) => `  ${i + 1}. ${b}`).join("\n")
      : "  (none)"),
    "",
    "PROMPT:",
    "```",
    item.prompt,
    "```",
    "",
    "REFERENCE:",
    "```",
    item.reference_output,
    "```",
    "",
    "CANDIDATE:",
    "```",
    candidate,
    "```",
    "",
    "Output the strict JSON now.",
  ].join("\n");
}

async function askJudge(item, candidate) {
  const { projectKey, conv } = await scratchConv("judge");
  const fullPrompt =
    `# SYSTEM\n${JUDGE_SYSTEM}\n\n# TASK\n${judgeUserPrompt(item, candidate)}`;
  const { content } = await generate({
    convId: conv.id,
    prompt: fullPrompt,
    engine: JUDGE_ENGINE,
    model: JUDGE_MODEL,
  });
  // Best-effort JSON extraction from judge's reply.
  const match = content.match(/\{[\s\S]*\}/);
  if (!match) {
    return {
      score: 0,
      verdict: "parse_error",
      reasoning: `no JSON object in judge reply: ${content.slice(0, 200)}`,
      pass: false,
    };
  }
  try {
    return JSON.parse(match[0]);
  } catch (e) {
    return {
      score: 0,
      verdict: "parse_error",
      reasoning: `JSON.parse failed: ${e.message}`,
      pass: false,
    };
  }
}

// ─── Main ────────────────────────────────────────────────────────────────────

function loadGolden() {
  if (!existsSync(GOLDEN_DIR)) {
    console.error(`[eval] golden dir missing: ${GOLDEN_DIR}`);
    process.exit(2);
  }
  const files = readdirSync(GOLDEN_DIR).filter((f) => f.endsWith(".jsonl"));
  const items = [];
  for (const f of files) {
    const lines = readFileSync(join(GOLDEN_DIR, f), "utf8")
      .split("\n")
      .filter(Boolean);
    for (const line of lines) {
      try {
        const o = JSON.parse(line);
        if (FILTER && !FILTER.test(o.id) && !FILTER.test(o.category)) continue;
        items.push({ ...o, _file: f });
      } catch (e) {
        console.error(`[eval] skip malformed line in ${f}: ${e.message}`);
      }
    }
  }
  return items;
}

(async () => {
  const items = loadGolden();
  if (items.length === 0) {
    console.error(`[eval] no golden items (filter=${FILTER ?? "none"})`);
    process.exit(2);
  }
  console.log(
    `[eval] running ${items.length} items · engine=${EVAL_ENGINE} · judge=${JUDGE_ENGINE}:${JUDGE_MODEL}`
  );

  const results = [];
  for (const [i, item] of items.entries()) {
    const label = `[${i + 1}/${items.length}] ${item.id}`;
    const t0 = Date.now();
    try {
      const scaffold = await scratchConv(item.id);
      const { content: candidate } = await generate({
        convId: scaffold.conv.id,
        prompt: item.prompt,
        engine: EVAL_ENGINE,
        model: EVAL_MODEL || item.model_ref,
      });
      const verdict = await askJudge(item, candidate);
      const threshold = item.rubric_threshold ?? 0.7;
      const pass = (verdict.score ?? 0) >= threshold;
      results.push({
        id: item.id,
        category: item.category,
        score: verdict.score ?? 0,
        threshold,
        pass,
        verdict,
        candidate,
        reference: item.reference_output,
        duration_ms: Date.now() - t0,
      });
      console.log(
        `  ${pass ? "✓" : "✗"} ${label} score=${(verdict.score ?? 0).toFixed(2)} (${Date.now() - t0}ms)`
      );
    } catch (e) {
      results.push({
        id: item.id,
        category: item.category,
        score: 0,
        threshold: item.rubric_threshold ?? 0.7,
        pass: false,
        error: e.message,
        duration_ms: Date.now() - t0,
      });
      console.log(`  ✗ ${label} ERROR: ${e.message}`);
    }
  }

  mkdirSync(RESULTS_DIR, { recursive: true });
  const stamp = new Date().toISOString().replace(/[:.]/g, "-");
  const out = join(
    RESULTS_DIR,
    `${stamp}-${EVAL_ENGINE}-${EVAL_MODEL || "default"}.json`
  );
  const summary = {
    ran_at: new Date().toISOString(),
    engine: EVAL_ENGINE,
    model: EVAL_MODEL || null,
    judge_engine: JUDGE_ENGINE,
    judge_model: JUDGE_MODEL,
    total: results.length,
    passed: results.filter((r) => r.pass).length,
    failed: results.filter((r) => !r.pass).length,
    avg_score:
      results.reduce((s, r) => s + (r.score ?? 0), 0) / results.length,
    results,
  };
  writeFileSync(out, JSON.stringify(summary, null, 2));
  console.log(
    `\n[eval] ${summary.passed}/${summary.total} passed · avg=${summary.avg_score.toFixed(2)} · wrote ${out}`
  );

  // Cleanup scratch `eval-*` projects created during this run.
  // Opt-in via --cleanup or EVAL_CLEANUP=1 — default off so post-mortem
  // inspection of a specific failing run is possible.
  if (process.argv.includes("--cleanup") || process.env.EVAL_CLEANUP === "1") {
    console.log("\n[eval] cleaning up scratch projects");
    const { spawnSync } = await import("node:child_process");
    spawnSync(
      "node",
      [
        resolve(REPO, "scripts/beta-e2e/cleanup.mjs"),
        "--force",
        "--pattern",
        "eval-",
      ],
      { stdio: "inherit" }
    );
  }
  process.exit(summary.failed > 0 ? 1 : 0);
})();
