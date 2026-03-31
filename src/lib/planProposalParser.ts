/**
 * Plan proposal marker parser.
 *
 * Detects `<!-- tunaflow:plan-proposal -->` ... `<!-- /tunaflow:plan-proposal -->`
 * blocks in assistant message content and extracts structured plan data.
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
    items.push({ title: m[1].trim(), details: m[2]?.trim() });
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
