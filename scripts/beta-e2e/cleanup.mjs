#!/usr/bin/env node
// Soft-hide test projects created by beta-e2e / eval runners.
//
// E2E scripts create projects with keys matching `e2e-p-<timestamp>` and
// `eval-<timestamp>` and don't clean them up on their own (intentional —
// so post-mortem debugging can inspect the actual DB state). This runs
// after a successful session to keep the sidebar uncluttered.
//
// Usage:
//   node scripts/beta-e2e/cleanup.mjs           # dry-run — list matches only
//   node scripts/beta-e2e/cleanup.mjs --force   # actually hide
//   node scripts/beta-e2e/cleanup.mjs --force --pattern '^e2e-'  # custom regex
//
// Direct-DB write is safe under WAL even while the app is running, but
// the UI reflects the change only after the sidebar re-queries projects
// (switch tabs or restart the app). No API endpoint is available for
// project hide — so we go straight to SQLite.

import { execFileSync } from "node:child_process";
import { existsSync } from "node:fs";
import { homedir } from "node:os";
import { resolve } from "node:path";

const DB =
  process.env.TUNAFLOW_DB ??
  resolve(homedir(), ".tunaflow/db/tunaflow.db");

const args = process.argv.slice(2);
const force = args.includes("--force");
const patternIdx = args.indexOf("--pattern");
// Default: anything starting with `e2e-p-` or `eval-`. Two separate LIKEs
// so SQLite uses the primary key index on `key` efficiently.
const sqlFilter = (() => {
  if (patternIdx >= 0 && args[patternIdx + 1]) {
    const p = args[patternIdx + 1].replace(/'/g, "''");
    return `key GLOB '${p}*'`;
  }
  return "(key LIKE 'e2e-p-%' OR key LIKE 'eval-%')";
})();

if (!existsSync(DB)) {
  console.error(`[cleanup] DB not found: ${DB}`);
  process.exit(2);
}

function sql(q) {
  try {
    const out = execFileSync("sqlite3", [DB, "-json", q], {
      maxBuffer: 32 * 1024 * 1024,
    });
    const text = out.toString();
    return text ? JSON.parse(text) : [];
  } catch (e) {
    console.error(`[cleanup] sqlite3 failed: ${e.message}`);
    process.exit(1);
  }
}

const matches = sql(
  `SELECT key, name FROM projects WHERE ${sqlFilter} AND hidden = 0;`
);

if (matches.length === 0) {
  console.log("[cleanup] no test projects to hide");
  process.exit(0);
}

console.log(`[cleanup] ${matches.length} test project(s) match:`);
for (const p of matches) {
  console.log(`  ${p.key}  ·  ${p.name}`);
}

if (!force) {
  console.log(
    "\n[cleanup] dry-run — pass --force to actually soft-hide (sets hidden=1)"
  );
  process.exit(0);
}

const now = Date.now();
// `-batch` suppresses the interactive prompt `sqlite3` sometimes uses.
execFileSync("sqlite3", [
  DB,
  "-batch",
  `UPDATE projects SET hidden = 1, updated_at = ${now} WHERE ${sqlFilter} AND hidden = 0;`,
]);

console.log(`\n[cleanup] soft-hid ${matches.length} project(s)`);
console.log(
  "[cleanup] sidebar may need a refresh (switch tabs or restart) to reflect"
);
