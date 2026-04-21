// Runs every scenario sequentially and prints a pass/fail table at the end.
// Each scenario is spawned as a separate Node process so exit codes are
// isolated and a single failure doesn't poison the remaining runs.

import { spawnSync } from "node:child_process";
import { readdirSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const here = dirname(fileURLToPath(import.meta.url));
const scenarios = readdirSync(here)
  .filter((f) => /^\d{2}-.*\.mjs$/.test(f))
  .sort();

const results = [];
for (const file of scenarios) {
  console.log(`\n═══════ ${file} ═══════`);
  const res = spawnSync("node", [join(here, file)], { stdio: "inherit" });
  results.push({ file, ok: res.status === 0, code: res.status });
}

console.log("\n═══════ Summary ═══════");
for (const r of results) {
  console.log(`${r.ok ? "✓" : "✗"} ${r.file}${r.ok ? "" : ` (exit=${r.code})`}`);
}

const failed = results.filter((r) => !r.ok).length;
if (failed > 0) {
  console.log(`\n${failed}/${results.length} failed`);
  process.exit(1);
}
console.log(`\n${results.length}/${results.length} passed`);
