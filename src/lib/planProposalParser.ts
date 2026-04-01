/**
 * tunaFlow marker parser.
 *
 * Detects structured markers in assistant message content:
 * - `<!-- tunaflow:plan-proposal -->` — Plan proposal for promotion
 * - `<!-- tunaflow:impl-plan -->` — Developer implementation plan report
 * - `<!-- tunaflow:impl-complete -->` — Implementation completion signal
 * - `<!-- tunaflow:review-verdict -->` — Reviewer verdict
 */

export interface ParsedPlanProposal {
  title: string;
  description: string;
  expectedOutcome: string;
  subtasks: { title: string; details?: string }[];
  constraints: string[];
  nonGoals: string[];
  /** Raw markdown inside the marker (for display) */
  raw: string;
}

export type ContentSegment =
  | { type: "markdown"; content: string }
  | { type: "plan-proposal"; proposal: ParsedPlanProposal; raw: string };

const MARKER_OPEN = "<!-- tunaflow:plan-proposal -->";
const MARKER_CLOSE = "<!-- /tunaflow:plan-proposal -->";

/**
 * Split message content into alternating markdown / plan-proposal segments.
 * If no markers are found, returns a single markdown segment.
 */
export function splitPlanProposals(content: string): ContentSegment[] {
  const segments: ContentSegment[] = [];
  let cursor = 0;

  while (cursor < content.length) {
    const openIdx = content.indexOf(MARKER_OPEN, cursor);
    if (openIdx === -1) {
      // No more markers — rest is plain markdown
      const rest = content.slice(cursor);
      if (rest.trim()) segments.push({ type: "markdown", content: rest });
      break;
    }

    // Markdown before the marker
    if (openIdx > cursor) {
      const before = content.slice(cursor, openIdx);
      if (before.trim()) segments.push({ type: "markdown", content: before });
    }

    const bodyStart = openIdx + MARKER_OPEN.length;
    const closeIdx = content.indexOf(MARKER_CLOSE, bodyStart);
    if (closeIdx === -1) {
      // Unclosed marker — treat rest as markdown
      segments.push({ type: "markdown", content: content.slice(openIdx) });
      break;
    }

    const raw = content.slice(bodyStart, closeIdx).trim();
    const proposal = parseProposalBody(raw);
    segments.push({ type: "plan-proposal", proposal, raw });
    cursor = closeIdx + MARKER_CLOSE.length;
  }

  return segments.length ? segments : [{ type: "markdown", content }];
}

/**
 * Check if content contains at least one plan-proposal marker.
 */
export function hasPlanProposal(content: string): boolean {
  return content.includes(MARKER_OPEN);
}

// ─── Internal parser ──────────────────────────────────────────────────────────

function parseProposalBody(raw: string): ParsedPlanProposal {
  const result: ParsedPlanProposal = {
    title: "",
    description: "",
    expectedOutcome: "",
    subtasks: [],
    constraints: [],
    nonGoals: [],
    raw,
  };

  // Extract title from "## Plan Proposal: {title}" or "# {title}"
  const titleMatch = raw.match(/^##?\s*(?:Plan Proposal:\s*)?(.+)$/m);
  if (titleMatch) result.title = titleMatch[1].trim();

  // Split into sections by ### headers
  const sections = splitSections(raw);

  for (const [header, body] of sections) {
    const h = header.toLowerCase();
    if (h.includes("description")) {
      result.description = body.trim();
    } else if (h.includes("expected outcome") || h.includes("outcome")) {
      result.expectedOutcome = body.trim();
    } else if (h.includes("subtask") || h.includes("tasks")) {
      result.subtasks = parseNumberedList(body);
    } else if (h.includes("constraint")) {
      result.constraints = parseBulletList(body);
    } else if (h.includes("non-goal") || h.includes("nongoal")) {
      result.nonGoals = parseBulletList(body);
    }
  }

  return result;
}

function splitSections(md: string): [string, string][] {
  const parts: [string, string][] = [];
  const lines = md.split("\n");
  let currentHeader = "";
  let currentBody: string[] = [];

  for (const line of lines) {
    const headerMatch = line.match(/^###\s+(.+)$/);
    if (headerMatch) {
      if (currentHeader) {
        parts.push([currentHeader, currentBody.join("\n")]);
      }
      currentHeader = headerMatch[1];
      currentBody = [];
    } else {
      currentBody.push(line);
    }
  }
  if (currentHeader) {
    parts.push([currentHeader, currentBody.join("\n")]);
  }
  return parts;
}

function parseNumberedList(text: string): { title: string; details?: string }[] {
  const items: { title: string; details?: string }[] = [];
  const re = /^\d+\.\s+(.+?)(?:\s*[—\-–]\s+(.+))?$/gm;
  let m: RegExpExecArray | null;
  while ((m = re.exec(text)) !== null) {
    // Strip markdown bold markers from title
    const title = m[1].trim().replace(/\*\*/g, "");
    const details = m[2]?.trim();
    items.push({ title, details });
  }
  return items;
}

function parseBulletList(text: string): string[] {
  const items: string[] = [];
  const re = /^[-*]\s+(.+)$/gm;
  let m: RegExpExecArray | null;
  while ((m = re.exec(text)) !== null) {
    items.push(m[1].trim());
  }
  return items;
}

// ─── Subtask done marker ─────────────────────────────────────────────────────

const SUBTASK_DONE_RE = /<!-- tunaflow:subtask-done:(\d+) -->/g;

/** Extract completed subtask numbers from message content */
export function extractCompletedSubtasks(content: string): number[] {
  const results: number[] = [];
  let m: RegExpExecArray | null;
  const re = new RegExp(SUBTASK_DONE_RE.source, "g");
  while ((m = re.exec(content)) !== null) {
    results.push(parseInt(m[1], 10));
  }
  return results;
}

/** Scan multiple messages for all completed subtask numbers */
export function scanCompletedSubtasks(messages: { role: string; content: string }[]): Set<number> {
  const done = new Set<number>();
  for (const msg of messages) {
    if (msg.role !== "assistant") continue;
    for (const n of extractCompletedSubtasks(msg.content)) done.add(n);
  }
  return done;
}

// ─── Implementation plan marker ─────────────────────────────────────────────

export interface ParsedImplPlan {
  files: { path: string; action: string }[];
  dependencies: string[];
  risks: string[];
  raw: string;
}

const IMPL_PLAN_OPEN = "<!-- tunaflow:impl-plan -->";
const IMPL_PLAN_CLOSE = "<!-- /tunaflow:impl-plan -->";

export function hasImplPlan(content: string): boolean {
  return content.includes(IMPL_PLAN_OPEN);
}

export function extractImplPlan(content: string): ParsedImplPlan | null {
  const openIdx = content.indexOf(IMPL_PLAN_OPEN);
  if (openIdx === -1) return null;
  const bodyStart = openIdx + IMPL_PLAN_OPEN.length;
  const closeIdx = content.indexOf(IMPL_PLAN_CLOSE, bodyStart);
  if (closeIdx === -1) return null;
  const raw = content.slice(bodyStart, closeIdx).trim();

  const files: { path: string; action: string }[] = [];
  const dependencies: string[] = [];
  const risks: string[] = [];

  const sections = splitSections(raw);
  for (const [header, body] of sections) {
    const h = header.toLowerCase();
    if (h.includes("file")) {
      // Parse "• src/api/rest.ts → remove" style
      const re = /^[-•*]\s+`?([^\s`→]+)`?\s*→?\s*(.*)?$/gm;
      let m: RegExpExecArray | null;
      while ((m = re.exec(body)) !== null) {
        files.push({ path: m[1].trim(), action: (m[2] || "modify").trim() });
      }
    } else if (h.includes("dependenc")) {
      dependencies.push(...parseBulletList(body));
    } else if (h.includes("risk") || h.includes("warning") || h.includes("caution")) {
      risks.push(...parseBulletList(body));
    }
  }

  return { files, dependencies, risks, raw };
}

// ─── Implementation complete marker ─────────────────────────────────────────

const IMPL_COMPLETE_MARKER = "<!-- tunaflow:impl-complete -->";

export function hasImplComplete(content: string): boolean {
  return content.includes(IMPL_COMPLETE_MARKER);
}

// ─── Review verdict marker ──────────────────────────────────────────────────

export type ReviewVerdict = "pass" | "fail" | "conditional";

export interface ParsedReviewVerdict {
  verdict: ReviewVerdict;
  findings: string[];
  recommendations: string[];
  raw: string;
}

const REVIEW_OPEN = "<!-- tunaflow:review-verdict -->";
const REVIEW_CLOSE = "<!-- /tunaflow:review-verdict -->";

export function hasReviewVerdict(content: string): boolean {
  return content.includes(REVIEW_OPEN);
}

export function extractReviewVerdict(content: string): ParsedReviewVerdict | null {
  const openIdx = content.indexOf(REVIEW_OPEN);
  if (openIdx === -1) return null;
  const bodyStart = openIdx + REVIEW_OPEN.length;
  const closeIdx = content.indexOf(REVIEW_CLOSE, bodyStart);
  if (closeIdx === -1) return null;
  const raw = content.slice(bodyStart, closeIdx).trim();

  let verdict: ReviewVerdict = "conditional";
  const verdictMatch = raw.match(/^verdict:\s*(pass|fail|conditional)/im);
  if (verdictMatch) verdict = verdictMatch[1] as ReviewVerdict;

  const findings: string[] = [];
  const recommendations: string[] = [];

  let section: "none" | "findings" | "recommendations" = "none";
  for (const line of raw.split("\n")) {
    if (/^findings:/i.test(line)) { section = "findings"; continue; }
    if (/^recommendations:/i.test(line)) { section = "recommendations"; continue; }
    const bulletMatch = line.match(/^[-*]\s+(.+)/);
    if (bulletMatch) {
      if (section === "findings") findings.push(bulletMatch[1].trim());
      else if (section === "recommendations") recommendations.push(bulletMatch[1].trim());
    }
  }

  return { verdict, findings, recommendations, raw };
}
