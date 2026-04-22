#!/usr/bin/env node
// Force-reindex project documents + cleanup stale entries.
//
// Usage:
//   TUNAFLOW_TOKEN=<token> node scripts/reindex-docs.mjs <projectKey>
//   (optional) TUNAFLOW_BASE=http://127.0.0.1:19840
//   (optional) --no-cleanup   # skip stale cleanup
//   (optional) --no-force     # use SHA change detection (incremental)
//
// docs/reference/work-safety.md, etc. 처럼 bulk 재조직 후 DB 재동기화 용.
// 기본: cleanup + force. result 는 `document:indexed` WS event 로 발행됨.

const base = process.env.TUNAFLOW_BASE ?? "http://127.0.0.1:19840";
const token = process.env.TUNAFLOW_TOKEN ?? "";
if (!token) {
  console.error("[reindex-docs] TUNAFLOW_TOKEN not set — Settings > Mobile 에서 복사 후 export");
  process.exit(2);
}

const args = process.argv.slice(2);
const projectKey = args.find((a) => !a.startsWith("--"));
const noCleanup = args.includes("--no-cleanup");
const noForce = args.includes("--no-force");

if (!projectKey) {
  console.error("[reindex-docs] usage: node scripts/reindex-docs.mjs <projectKey> [--no-cleanup] [--no-force]");
  process.exit(2);
}

const params = new URLSearchParams({
  force: String(!noForce),
  cleanup: String(!noCleanup),
});
const url = `${base}/api/v1/projects/${encodeURIComponent(projectKey)}/documents/index?${params}`;

console.log(`[reindex-docs] project=${projectKey} force=${!noForce} cleanup=${!noCleanup}`);
console.log(`[reindex-docs] POST ${url}`);

const res = await fetch(url, {
  method: "POST",
  headers: { "Content-Type": "application/json", Authorization: `Bearer ${token}` },
});
const body = await res.text();
if (!res.ok) {
  console.error(`[reindex-docs] FAILED ${res.status}: ${body}`);
  process.exit(1);
}
console.log(`[reindex-docs] ${res.status}`);
console.log(body);
console.log(
  "\n[reindex-docs] 작업은 background 에서 진행됩니다. WS event 'document:indexed' 로 결과 확인 가능.\n" +
  "  또는 잠시 뒤 다음 SQL 로 지표 확인:\n" +
  "  sqlite3 ~/.tunaflow/db/tunaflow.db \"SELECT COUNT(DISTINCT file_path) FROM conversation_chunks WHERE project_key='" +
  projectKey + "' AND source_type='document';\""
);
