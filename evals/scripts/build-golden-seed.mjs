#!/usr/bin/env node
// Build seed golden dataset by querying real project data from tunaFlow DB.
//
// Unlike `extract-from-trace.mjs` (which emits candidates unfiltered), this
// script hand-picks 4 entries per category based on curated SQL queries that
// target the kinds of cases we want to use as regression baselines:
//
//   - plan-generation : Architect on secall, successful plan proposals
//   - dev-implementation : Implementer on secall, "구현 시작" + "Rework" mix
//   - review-verdict : Reviewer across projects, structured verdict output
//   - rt-verdict : last assistant message of RT branches (any role)
//   - branch-adopt : last assistant message of status='adopted' branches
//
// Each entry ships with empty `expected_behaviors` — the human curator fills
// in 3-5 must-cover behaviors per entry. That's the only manual step.
//
// Writes 5 JSONL files into `evals/golden/`.

import { execFileSync } from "node:child_process";
import { homedir } from "node:os";
import { resolve, dirname } from "node:path";
import { mkdirSync, writeFileSync, existsSync } from "node:fs";
import { fileURLToPath } from "node:url";

const HERE = dirname(fileURLToPath(import.meta.url));
const REPO = resolve(HERE, "..", "..");
const OUT = resolve(REPO, "evals/golden");
const DB = process.env.TUNAFLOW_DB ?? resolve(homedir(), ".tunaflow/db/tunaflow.db");

if (!existsSync(DB)) {
  console.error(`[golden-seed] DB not found: ${DB}`);
  process.exit(2);
}
mkdirSync(OUT, { recursive: true });

function sql(q) {
  const out = execFileSync("sqlite3", [DB, "-json", q], {
    maxBuffer: 64 * 1024 * 1024,
  });
  const text = out.toString();
  return text ? JSON.parse(text) : [];
}

// ─── Queries (curated) ──────────────────────────────────────────────────────

// 1. plan-generation : Architect responses that actually contain a
//    `<!-- tunaflow:plan-proposal -->` marker (i.e. a real plan proposal, not
//    an analysis/report that happened to come from the Architect persona).
const planGenerationRows = sql(`
  SELECT m.id, m.conversation_id, m.content AS reference_output,
         m.persona, m.engine, m.model, m.timestamp,
         c.project_key,
         (SELECT u.content FROM messages u
            WHERE u.conversation_id = m.conversation_id
              AND u.role='user'
              AND u.timestamp < m.timestamp
            ORDER BY u.timestamp DESC LIMIT 1) AS user_prompt
  FROM messages m
  JOIN conversations c ON m.conversation_id = c.id
  WHERE m.role='assistant' AND m.status='done'
    AND m.persona LIKE '%Architect%'
    AND m.content LIKE '%tunaflow:plan-proposal%'
    AND length(m.content) BETWEEN 600 AND 4500
    AND c.project_key IN ('secall','gemento','tunaflow-mobile','tunaflow')
  GROUP BY c.id
  ORDER BY length(m.content) DESC
  LIMIT 4;
`);

// 2. dev-implementation : Implementer, prefer "구현 시작" prompts (first-run) over Rework.
const devImplementationRows = sql(`
  SELECT m.id, m.conversation_id, m.content AS reference_output,
         m.persona, m.engine, m.model, m.timestamp,
         c.project_key,
         (SELECT u.content FROM messages u
            WHERE u.conversation_id = m.conversation_id
              AND u.role='user'
              AND u.timestamp < m.timestamp
            ORDER BY u.timestamp DESC LIMIT 1) AS user_prompt
  FROM messages m
  JOIN conversations c ON m.conversation_id = c.id
  WHERE m.role='assistant' AND m.status='done'
    AND (m.persona LIKE '%Implementer%' OR m.persona LIKE '%Coder%')
    AND length(m.content) BETWEEN 400 AND 2000
    AND EXISTS (
      SELECT 1 FROM messages u
      WHERE u.conversation_id = m.conversation_id
        AND u.role='user' AND u.content LIKE '%🔧 구현 시작%'
        AND u.timestamp < m.timestamp
    )
  GROUP BY c.id
  ORDER BY length(m.content) DESC
  LIMIT 4;
`);

// 3. review-verdict : Reviewer across projects, prefer ones with structured verdict output.
const reviewVerdictRows = sql(`
  SELECT m.id, m.conversation_id, m.content AS reference_output,
         m.persona, m.engine, m.model, m.timestamp,
         c.project_key,
         (SELECT u.content FROM messages u
            WHERE u.conversation_id = m.conversation_id
              AND u.role='user'
              AND u.timestamp < m.timestamp
            ORDER BY u.timestamp DESC LIMIT 1) AS user_prompt,
         (SELECT a.content FROM messages a
            WHERE a.conversation_id = m.conversation_id
              AND a.role='assistant'
              AND a.id != m.id
              AND a.timestamp < m.timestamp
            ORDER BY a.timestamp DESC LIMIT 1) AS prior_assistant
  FROM messages m
  JOIN conversations c ON m.conversation_id = c.id
  WHERE m.role='assistant' AND m.status='done'
    AND m.persona LIKE '%Reviewer%'
    AND length(m.content) BETWEEN 800 AND 3500
  GROUP BY c.id
  ORDER BY length(m.content) DESC
  LIMIT 4;
`);

// 4. rt-verdict : last assistant message of an RT branch.
const rtVerdictRows = sql(`
  SELECT b.id AS branch_id, b.label, b.status, c.id AS conv_id, c.project_key,
         (SELECT m.id FROM messages m
            WHERE m.conversation_id=c.id AND m.role='assistant' AND m.status='done'
            ORDER BY m.timestamp DESC LIMIT 1) AS last_msg_id,
         (SELECT m.content FROM messages m
            WHERE m.conversation_id=c.id AND m.role='assistant' AND m.status='done'
            ORDER BY m.timestamp DESC LIMIT 1) AS reference_output,
         (SELECT m.persona FROM messages m
            WHERE m.conversation_id=c.id AND m.role='assistant' AND m.status='done'
            ORDER BY m.timestamp DESC LIMIT 1) AS persona,
         (SELECT m.engine FROM messages m
            WHERE m.conversation_id=c.id AND m.role='assistant' AND m.status='done'
            ORDER BY m.timestamp DESC LIMIT 1) AS engine,
         (SELECT m.model FROM messages m
            WHERE m.conversation_id=c.id AND m.role='assistant' AND m.status='done'
            ORDER BY m.timestamp DESC LIMIT 1) AS model,
         (SELECT m.timestamp FROM messages m
            WHERE m.conversation_id=c.id AND m.role='assistant' AND m.status='done'
            ORDER BY m.timestamp DESC LIMIT 1) AS timestamp
  FROM branches b
  JOIN conversations c ON b.conversation_id=c.id
  WHERE b.mode='roundtable'
    AND (SELECT length(m.content) FROM messages m
           WHERE m.conversation_id=c.id AND m.role='assistant' AND m.status='done'
           ORDER BY m.timestamp DESC LIMIT 1) > 400
  GROUP BY c.id   -- dedup: a single RT conversation can have many branches
  ORDER BY b.created_at DESC
  LIMIT 4;
`);

// 5. branch-adopt : last assistant of status='adopted' branch. adopted_message_id
//    is null across the fleet, so fall back to the most recent done assistant.
const branchAdoptRows = sql(`
  SELECT b.id AS branch_id, b.label, b.mode, c.id AS conv_id, c.project_key,
         (SELECT m.id FROM messages m
            WHERE m.conversation_id=c.id AND m.role='assistant' AND m.status='done'
            ORDER BY m.timestamp DESC LIMIT 1) AS last_msg_id,
         (SELECT m.content FROM messages m
            WHERE m.conversation_id=c.id AND m.role='assistant' AND m.status='done'
            ORDER BY m.timestamp DESC LIMIT 1) AS reference_output,
         (SELECT m.engine FROM messages m
            WHERE m.conversation_id=c.id AND m.role='assistant' AND m.status='done'
            ORDER BY m.timestamp DESC LIMIT 1) AS engine,
         (SELECT m.model FROM messages m
            WHERE m.conversation_id=c.id AND m.role='assistant' AND m.status='done'
            ORDER BY m.timestamp DESC LIMIT 1) AS model,
         (SELECT m.timestamp FROM messages m
            WHERE m.conversation_id=c.id AND m.role='assistant' AND m.status='done'
            ORDER BY m.timestamp DESC LIMIT 1) AS timestamp,
         (SELECT u.content FROM messages u
            WHERE u.conversation_id=c.id AND u.role='user'
            ORDER BY u.timestamp ASC LIMIT 1) AS first_user_prompt
  FROM branches b
  JOIN conversations c ON b.conversation_id=c.id
  WHERE b.status='adopted'
    AND (SELECT length(m.content) FROM messages m
           WHERE m.conversation_id=c.id AND m.role='assistant' AND m.status='done'
           ORDER BY m.timestamp DESC LIMIT 1) BETWEEN 300 AND 3000
  GROUP BY c.id   -- dedup: same conversation may have multiple adopted branches
  ORDER BY b.created_at DESC
  LIMIT 4;
`);

// ─── Writer ─────────────────────────────────────────────────────────────────

function emit(fileName, rows, transform) {
  const path = resolve(OUT, fileName);
  const lines = rows.map(transform).filter(Boolean).map((e) => JSON.stringify(e));
  writeFileSync(path, lines.join("\n") + (lines.length ? "\n" : ""));
  console.log(`[golden-seed] wrote ${lines.length} rows → ${fileName}`);
}

emit("plan-generation-secall.jsonl", planGenerationRows, (r, i) => ({
  id: `plan-generation-${String(i + 1).padStart(2, "0")}`,
  category: "plan-generation",
  prompt: r.user_prompt,
  context_hint: `project: ${r.project_key}`,
  engine_ref: r.engine ?? "claude",
  model_ref: r.model ?? "",
  reference_output: r.reference_output,
  expected_behaviors: [],
  rubric_threshold: 0.7,
  created_at: r.timestamp,
  source_message_id: r.id,
  source_conversation_id: r.conversation_id,
  source_persona: r.persona,
  source_project: r.project_key,
}));

emit("dev-implementation-secall.jsonl", devImplementationRows, (r, i) => ({
  id: `dev-implementation-${String(i + 1).padStart(2, "0")}`,
  category: "dev-implementation",
  prompt: r.user_prompt,
  context_hint: `project: ${r.project_key} · kind: 구현 시작`,
  engine_ref: r.engine ?? "claude",
  model_ref: r.model ?? "",
  reference_output: r.reference_output,
  expected_behaviors: [],
  rubric_threshold: 0.7,
  created_at: r.timestamp,
  source_message_id: r.id,
  source_conversation_id: r.conversation_id,
  source_persona: r.persona,
  source_project: r.project_key,
}));

emit("review-verdict-mixed.jsonl", reviewVerdictRows, (r, i) => {
  const prompt = r.user_prompt
    ?? (r.prior_assistant ? `[auto-invoked; prior assistant output]\n${r.prior_assistant}` : null);
  if (!prompt) return null;
  return {
    id: `review-verdict-${String(i + 1).padStart(2, "0")}`,
    category: "review-verdict",
    prompt,
    context_hint: `project: ${r.project_key}`,
    engine_ref: r.engine ?? "codex",
    model_ref: r.model ?? "",
    reference_output: r.reference_output,
    expected_behaviors: [],
    rubric_threshold: 0.7,
    created_at: r.timestamp,
    source_message_id: r.id,
    source_conversation_id: r.conversation_id,
    source_persona: r.persona,
    source_project: r.project_key,
  };
});

emit("rt-verdict-mixed.jsonl", rtVerdictRows, (r, i) => ({
  id: `rt-verdict-${String(i + 1).padStart(2, "0")}`,
  category: "rt-verdict",
  prompt: `Summarize the roundtable below into a synthesized verdict.\n\n[RT branch label]: ${r.label}\n[RT branch status]: ${r.status}\n\nThe roundtable transcript is referenced by conversation_id: ${r.conv_id}. Read the transcript and produce the final synthesized verdict (consensus / contested / dissent, plus the final verdict line).`,
  context_hint: `project: ${r.project_key} · branch status: ${r.status}`,
  engine_ref: r.engine ?? "claude",
  model_ref: r.model ?? "",
  reference_output: r.reference_output,
  expected_behaviors: [],
  rubric_threshold: 0.65,
  created_at: r.timestamp,
  source_message_id: r.last_msg_id,
  source_conversation_id: r.conv_id,
  source_persona: r.persona,
  source_project: r.project_key,
  source_branch_id: r.branch_id,
  source_branch_label: r.label,
}));

emit("branch-adopt-mixed.jsonl", branchAdoptRows, (r, i) => ({
  id: `branch-adopt-${String(i + 1).padStart(2, "0")}`,
  category: "branch-adopt",
  prompt: r.first_user_prompt
    ? `Summarize this branch for adoption back into the parent conversation. The branch was created to explore:\n\n${r.first_user_prompt.slice(0, 500)}`
    : `Summarize this branch for adoption back into the parent conversation.`,
  context_hint: `project: ${r.project_key} · branch label: ${r.label} · mode: ${r.mode}`,
  engine_ref: r.engine ?? "claude",
  model_ref: r.model ?? "",
  reference_output: r.reference_output,
  expected_behaviors: [],
  rubric_threshold: 0.6,
  created_at: r.timestamp,
  source_message_id: r.last_msg_id,
  source_conversation_id: r.conv_id,
  source_project: r.project_key,
  source_branch_id: r.branch_id,
  source_branch_label: r.label,
}));

console.log("[golden-seed] done. Hand-fill `expected_behaviors` next.");
