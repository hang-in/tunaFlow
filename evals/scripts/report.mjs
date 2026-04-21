#!/usr/bin/env node
// Read the latest result snapshot (or one passed as argv[2]) and emit a
// markdown diff-style report suitable for GITHUB_STEP_SUMMARY.
//
// Usage:
//   node evals/scripts/report.mjs                 # latest snapshot
//   node evals/scripts/report.mjs <path.json>     # specific snapshot
//   node evals/scripts/report.mjs --baseline prev.json current.json
//     → side-by-side diff

import { readdirSync, readFileSync, statSync } from "node:fs";
import { dirname, join, resolve } from "node:path";
import { fileURLToPath } from "node:url";

const HERE = dirname(fileURLToPath(import.meta.url));
const RESULTS_DIR = resolve(HERE, "..", "results");

function latestSnapshot() {
  const files = readdirSync(RESULTS_DIR)
    .filter((f) => f.endsWith(".json"))
    .map((f) => ({
      path: join(RESULTS_DIR, f),
      mtime: statSync(join(RESULTS_DIR, f)).mtimeMs,
    }))
    .sort((a, b) => b.mtime - a.mtime);
  return files[0]?.path;
}

function load(path) {
  return JSON.parse(readFileSync(path, "utf8"));
}

function verdictIcon(score, pass) {
  if (!pass) return "✗";
  if (score >= 0.95) return "✓✓";
  if (score >= 0.85) return "✓";
  return "~";
}

function formatTable(results) {
  const rows = results.map((r) => {
    const score = (r.score ?? 0).toFixed(2);
    const icon = verdictIcon(r.score ?? 0, r.pass);
    const note = r.error
      ? `ERROR: ${r.error}`
      : r.verdict?.missing?.length
        ? `missing: ${r.verdict.missing.slice(0, 2).join(", ")}`
        : r.verdict?.verdict ?? "";
    return `| ${icon} | \`${r.id}\` | ${r.category} | ${score} | ${note} |`;
  });
  return [
    "| | ID | Category | Score | Notes |",
    "|---|----|----------|------:|-------|",
    ...rows,
  ].join("\n");
}

function categoryBreakdown(results) {
  const byCat = new Map();
  for (const r of results) {
    const c = r.category;
    if (!byCat.has(c)) byCat.set(c, { pass: 0, fail: 0, scores: [] });
    const b = byCat.get(c);
    if (r.pass) b.pass++;
    else b.fail++;
    b.scores.push(r.score ?? 0);
  }
  const rows = [];
  for (const [cat, b] of byCat) {
    const total = b.pass + b.fail;
    const avg = b.scores.reduce((s, x) => s + x, 0) / b.scores.length;
    const rate = ((b.pass / total) * 100).toFixed(0);
    rows.push(`| ${cat} | ${b.pass}/${total} | ${rate}% | ${avg.toFixed(2)} |`);
  }
  return [
    "| Category | Pass | Rate | Avg Score |",
    "|----------|:----:|:----:|----------:|",
    ...rows,
  ].join("\n");
}

function diffTable(prev, curr) {
  const prevById = new Map(prev.results.map((r) => [r.id, r]));
  const rows = [];
  for (const c of curr.results) {
    const p = prevById.get(c.id);
    const pScore = p ? (p.score ?? 0).toFixed(2) : "—";
    const cScore = (c.score ?? 0).toFixed(2);
    const delta = p ? (c.score - p.score).toFixed(2) : "new";
    const signal =
      p && c.score < p.score - 0.05
        ? "🔻"
        : p && c.score > p.score + 0.05
          ? "🔺"
          : "·";
    rows.push(
      `| ${signal} | \`${c.id}\` | ${c.category} | ${pScore} → ${cScore} | ${delta} |`
    );
  }
  return [
    "| | ID | Category | Prev → Curr | Δ |",
    "|---|----|----------|-------------|--:|",
    ...rows,
  ].join("\n");
}

// ─── Main ────────────────────────────────────────────────────────────────────

const args = process.argv.slice(2);
let data, baseline;
if (args[0] === "--baseline") {
  baseline = load(args[1]);
  data = load(args[2]);
} else {
  const path = args[0] ?? latestSnapshot();
  if (!path) {
    console.error(`[report] no snapshot in ${RESULTS_DIR}`);
    process.exit(2);
  }
  data = load(path);
}

const out = [];
out.push(`# Prompt Regression Eval — ${data.ran_at}`);
out.push("");
out.push(
  `**Engine**: \`${data.engine}${data.model ? `:${data.model}` : ""}\` · **Judge**: \`${data.judge_engine}:${data.judge_model}\``
);
out.push("");
out.push(
  `**Total**: ${data.total} · **Pass**: ${data.passed} · **Fail**: ${data.failed} · **Avg Score**: ${data.avg_score.toFixed(3)}`
);
out.push("");

const passRate = data.passed / data.total;
if (passRate >= 0.85) out.push("🟢 **PASS** — above green threshold (≥85%)");
else if (passRate >= 0.7)
  out.push("🟡 **WARNING** — below green (85%), above yellow (70%)");
else if (passRate >= 0.5)
  out.push("🟠 **FAILING** — below yellow (70%), above red (50%)");
else out.push("🔴 **BLOCK** — below red threshold (<50%)");
out.push("");

out.push("## Category breakdown");
out.push(categoryBreakdown(data.results));
out.push("");

if (baseline) {
  out.push("## Diff vs baseline");
  out.push(`baseline: \`${baseline.ran_at}\` · ${baseline.passed}/${baseline.total} · avg ${baseline.avg_score.toFixed(2)}`);
  out.push("");
  out.push(diffTable(baseline, data));
  out.push("");
}

out.push("## Per-item");
out.push(formatTable(data.results));

console.log(out.join("\n"));
