// Runs every scenario sequentially and prints a pass/fail table at the end.
// Each scenario is spawned as a separate Node process so exit codes are
// isolated and a single failure doesn't poison the remaining runs.
//
// Flags:
//   --cleanup    after the run, soft-hide every scratch project created
//                by the scenarios (forwards to cleanup.mjs --force).
//   Or set E2E_CLEANUP=1 to enable without editing the command line.

import { spawnSync } from "node:child_process";
import { readdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const scenarios = readdirSync(here)
  .filter((f) => /^\d{2}-.*\.mjs$/.test(f))
  .sort();
const shouldCleanup =
  process.argv.includes("--cleanup") || process.env.E2E_CLEANUP === "1";

const results = [];
for (const [idx, file] of scenarios.entries()) {
  console.log(`\n═══════ ${file} ═══════`);
  const res = spawnSync("node", [join(here, file)], { stdio: "inherit" });
  results.push({ file, ok: res.status === 0, code: res.status });
  // Brief cooldown between scenarios so backgrounded agent runs from the
  // previous scenario can drain before the next HTTP call hits the server.
  if (idx < scenarios.length - 1) {
    await new Promise((r) => setTimeout(r, 1500));
  }
}

console.log("\n═══════ Summary ═══════");
for (const r of results) {
  console.log(`${r.ok ? "✓" : "✗"} ${r.file}${r.ok ? "" : ` (exit=${r.code})`}`);
}

const failed = results.filter((r) => !r.ok).length;

if (shouldCleanup) {
  console.log("\n═══════ Cleanup ═══════");
  spawnSync("node", [join(here, "cleanup.mjs"), "--force"], {
    stdio: "inherit",
  });
}

if (failed > 0) {
  console.log(`\n${failed}/${results.length} failed`);
  process.exit(1);
}
console.log(`\n${results.length}/${results.length} passed`);
