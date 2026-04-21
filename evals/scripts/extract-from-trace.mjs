#!/usr/bin/env node
// Extract successful assistant messages from tunaFlow's messages table,
// grouped by role (Architect / Implementer / Reviewer / Synthesizer), and
// emit them as JSONL candidates for the golden dataset. Human then picks N
// per category and fills in `expected_behaviors`.
//
// Usage:
//   node evals/scripts/extract-from-trace.mjs [limit_per_category=10]
//     > evals/golden/candidates.jsonl
//
//   TUNAFLOW_DB=/alt/path.db node evals/scripts/extract-from-trace.mjs
//
// Requires `sqlite3` CLI (macOS ships with it; Linux/Debian: `apt install
// sqlite3`). No npm deps.

import { execFileSync } from "node:child_process";
import { homedir } from "node:os";
import { resolve } from "node:path";
import { existsSync } from "node:fs";

const DB =
  process.env.TUNAFLOW_DB ??
  resolve(homedir(), ".tunaflow/db/tunaflow.db");
const LIMIT = parseInt(process.argv[2] ?? "10", 10);

if (!existsSync(DB)) {
  console.error(`[extract] DB not found: ${DB}`);
  console.error(`[extract] set TUNAFLOW_DB to override`);
  process.exit(2);
}

// ─── Category → persona filter ───────────────────────────────────────────────
// persona column values look like "Architect Claude · Architect" — the
// second clause is the role; we match on LIKE '%Role%' for resilience.
const CATEGORIES = {
  "plan-generation": "AND m_a.persona LIKE '%Architect%'",
  "dev-implementation":
    "AND (m_a.persona LIKE '%Implementer%' OR m_a.persona LIKE '%Coder%')",
  "review-verdict":
    "AND m_a.persona LIKE '%Reviewer%' AND m_a.persona NOT LIKE '%Synthesizer%'",
  "rt-verdict": "AND m_a.persona LIKE '%Synthesizer%'",
};

function sql(q) {
  try {
    const out = execFileSync("sqlite3", [DB, "-json", q], {
      maxBuffer: 64 * 1024 * 1024,
    });
    const text = out.toString();
    return text ? JSON.parse(text) : [];
  } catch (e) {
    console.error(`[extract] sqlite3 failed: ${e.message}`);
    process.exit(1);
  }
}

// ─── Main ────────────────────────────────────────────────────────────────────

let emitted = 0;
for (const [cat, filter] of Object.entries(CATEGORIES)) {
  const rows = sql(`
    SELECT m_a.id AS id,
           m_a.conversation_id AS conversationId,
           m_a.content AS reference_output,
           m_a.persona AS persona,
           m_a.engine AS engine,
           m_a.model AS model,
           m_a.timestamp AS timestamp,
           (SELECT m_u.content FROM messages m_u
              WHERE m_u.conversation_id = m_a.conversation_id
                AND m_u.role = 'user'
                AND m_u.timestamp < m_a.timestamp
              ORDER BY m_u.timestamp DESC LIMIT 1) AS user_prompt
    FROM messages m_a
    WHERE m_a.role = 'assistant'
      AND m_a.status = 'done'
      AND length(m_a.content) > 200
      ${filter}
    ORDER BY m_a.timestamp DESC
    LIMIT ${LIMIT};
  `);

  for (const [idx, r] of rows.entries()) {
    let promptKind = "user";
    let prompt = r.user_prompt;
    // Reviewer / Synthesizer are often auto-invoked (no direct user msg).
    // Fall back to the prior assistant message as the "input" — imperfect
    // but lets the regression judge see what the model was reacting to.
    if (!prompt) {
      const prior = sql(`
        SELECT content FROM messages
        WHERE conversation_id = '${r.conversationId}'
          AND role = 'assistant'
          AND id != '${r.id}'
          AND timestamp < ${r.timestamp}
        ORDER BY timestamp DESC LIMIT 1;
      `);
      if (prior[0] && prior[0].content) {
        prompt = `[auto-invoked; prior assistant output]\n${prior[0].content}`;
        promptKind = "prior_assistant";
      } else {
        continue; // truly orphan → skip
      }
    }
    const entry = {
      id: `${cat}-${String(idx + 1).padStart(2, "0")}`,
      category: cat,
      prompt,
      source_prompt_kind: promptKind,
      context_hint: "",
      engine_ref: r.engine ?? "claude",
      model_ref: r.model ?? "",
      reference_output: r.reference_output,
      expected_behaviors: [],
      rubric_threshold: 0.7,
      created_at: r.timestamp,
      source_message_id: r.id,
      source_conversation_id: r.conversationId,
      source_persona: r.persona,
    };
    process.stdout.write(JSON.stringify(entry) + "\n");
    emitted++;
  }
}

// ─── Branch-adopt: separate query (reads from branches + adopted_message_id)
const adopt = sql(`
  SELECT m.id AS id,
         m.conversation_id AS conversationId,
         m.content AS reference_output,
         m.timestamp AS timestamp,
         b.label AS branch_label,
         b.id AS branch_id,
         (SELECT content FROM messages
            WHERE conversation_id = 'branch:' || b.id
              AND role = 'user'
            ORDER BY timestamp ASC LIMIT 1) AS user_prompt
  FROM branches b
  JOIN messages m ON m.id = b.adopted_message_id
  WHERE b.status = 'adopted'
    AND b.adopted_message_id IS NOT NULL
    AND length(m.content) > 100
  ORDER BY m.timestamp DESC
  LIMIT ${LIMIT};
`);

for (const [idx, r] of adopt.entries()) {
  if (!r.user_prompt) continue;
  const entry = {
    id: `branch-adopt-${String(idx + 1).padStart(2, "0")}`,
    category: "branch-adopt",
    prompt: r.user_prompt,
    context_hint: `branch label: ${r.branch_label}`,
    engine_ref: "claude",
    model_ref: "",
    reference_output: r.reference_output,
    expected_behaviors: [],
    rubric_threshold: 0.6, // summaries are judged on coverage, not exactness
    created_at: r.timestamp,
    source_message_id: r.id,
    source_conversation_id: r.conversationId,
    source_branch_id: r.branch_id,
  };
  process.stdout.write(JSON.stringify(entry) + "\n");
  emitted++;
}

console.error(`[extract] emitted ${emitted} candidates across 5 categories`);
