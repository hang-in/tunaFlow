/**
 * Identity summary markdown 파서 (projectIdentityAnalysisPlan subtask-04).
 *
 * Rust 의 `identity_analyzer::IDENTITY_PROMPT_TEMPLATE` 이 생성하는 고정 5 섹션
 * markdown 을 파싱. LLM 출력이 섹션 일부를 누락할 수 있으므로 **best-effort** —
 * 섹션 없으면 빈 문자열 / 빈 배열 반환.
 *
 * Frontmatter 포맷:
 * ```
 * ---
 * project_key: proj-x
 * period_start: 123
 * period_end: 456
 * artifact_refs: [a1,a2]
 * supersedes: prev-id
 * ---
 * ```
 */

export type ParsedIdentityFrontmatter = {
  projectKey: string;
  periodStart: number;
  periodEnd: number;
  artifactRefs: string[];
  supersedes?: string;
};

export type InflectionPoint = {
  what: string;
  why: string;
  when: string;
  artifactId?: string;
};

export type DoAvoid = {
  do: string[];
  avoid: string[];
};

export type ParsedIdentity = {
  frontmatter: ParsedIdentityFrontmatter | null;
  sections: {
    projectIdentity: string;
    userWorkingPreference: string;
    agentOperatingPreference: string;
    inflectionPoints: InflectionPoint[];
    doAvoid: DoAvoid;
  };
};

const SECTION_HEADERS = [
  "### Project identity",
  "### User working preference",
  "### Agent operating preference",
  "### Recent inflection points",
  "### Do / Avoid",
];

/** `---\nk: v\n...---\n` 블록이 있으면 파싱 후 [fm, body] 반환. 없으면 [null, content]. */
export function extractFrontmatter(
  content: string,
): [ParsedIdentityFrontmatter | null, string] {
  if (!content.startsWith("---\n")) return [null, content];
  const closeIdx = content.indexOf("\n---", 4);
  if (closeIdx === -1) return [null, content];
  const yaml = content.slice(4, closeIdx);
  const body = content.slice(closeIdx + 4).replace(/^\n+/, "");

  const map: Record<string, string> = {};
  for (const line of yaml.split("\n")) {
    const m = line.match(/^([a-z_]+):\s*(.*)$/i);
    if (m) map[m[1]] = m[2].trim();
  }
  const fm: ParsedIdentityFrontmatter = {
    projectKey: map.project_key ?? "",
    periodStart: Number(map.period_start) || 0,
    periodEnd: Number(map.period_end) || 0,
    artifactRefs: parseRefs(map.artifact_refs ?? ""),
    supersedes: map.supersedes && map.supersedes.length > 0 ? map.supersedes : undefined,
  };
  return [fm, body];
}

function parseRefs(raw: string): string[] {
  const m = raw.match(/^\[(.*)\]$/);
  if (!m) return [];
  return m[1]
    .split(",")
    .map((s) => s.trim())
    .filter((s) => s.length > 0);
}

/** 주어진 headers 순서로 body 를 섹션별 텍스트로 split. 각 섹션은 해당 header 다음줄부터
 *  다음 header 직전까지. 섹션 없으면 "" 반환. */
export function splitBySections(body: string, headers: readonly string[]): string[] {
  const out: string[] = [];
  for (let i = 0; i < headers.length; i++) {
    const header = headers[i];
    const idx = body.indexOf(header);
    if (idx === -1) {
      out.push("");
      continue;
    }
    const start = idx + header.length;
    // 다음 header 중 idx 뒤에 있는 것들만 고려
    let end = body.length;
    for (const next of headers) {
      if (next === header) continue;
      const nextIdx = body.indexOf(next, start);
      if (nextIdx !== -1 && nextIdx < end) end = nextIdx;
    }
    out.push(body.slice(start, end).trim());
  }
  return out;
}

/** Inflection points 섹션 파서. LLM 포맷 가이드:
 *  ```
 *  - What changed: ...
 *  - Why: <artifact_id>
 *  - When: <date>
 *  ```
 *  3 개 항목 그룹 — 각 그룹은 빈 줄 또는 첫 "- What" 로 경계.
 *  best-effort: 1 그룹당 what/why/when 중 찾을 수 있는 만큼 채움. */
export function parseInflectionPoints(section: string): InflectionPoint[] {
  if (!section.trim()) return [];
  const lines = section.split("\n").map((l) => l.trim());
  const groups: string[][] = [];
  let current: string[] = [];
  for (const line of lines) {
    if (!line) {
      if (current.length > 0) {
        groups.push(current);
        current = [];
      }
      continue;
    }
    // "- What" 으로 시작하면 새 그룹 시작
    if (/^-\s*what/i.test(line) && current.some((l) => /what/i.test(l))) {
      groups.push(current);
      current = [line];
    } else {
      current.push(line);
    }
  }
  if (current.length > 0) groups.push(current);

  const out: InflectionPoint[] = [];
  for (const g of groups) {
    const what = findFieldLine(g, ["what changed", "what"]);
    const why = findFieldLine(g, ["why"]);
    const when = findFieldLine(g, ["when"]);
    if (what || why || when) {
      const artifactId = why.match(/\[?([a-z0-9][a-z0-9-]+)\]?/i)?.[1];
      out.push({ what, why, when, artifactId });
    }
  }
  return out;
}

function findFieldLine(group: string[], fields: string[]): string {
  for (const line of group) {
    for (const f of fields) {
      const re = new RegExp(`^-\\s*${f}\\s*:?\\s*`, "i");
      if (re.test(line)) {
        return line.replace(re, "").trim();
      }
    }
  }
  return "";
}

/** Do / Avoid 섹션 파서. "Do:" / "Avoid:" 서브 헤더 아래 bullet 수집. */
export function parseDoAvoid(section: string): DoAvoid {
  if (!section.trim()) return { do: [], avoid: [] };
  const doList: string[] = [];
  const avoidList: string[] = [];
  let mode: "do" | "avoid" | null = null;
  for (const raw of section.split("\n")) {
    const line = raw.trim();
    if (/^[*#-]*\s*do\s*[:]/i.test(line) || /^\*\*do/i.test(line)) {
      mode = "do";
      continue;
    }
    if (/^[*#-]*\s*avoid\s*[:]/i.test(line) || /^\*\*avoid/i.test(line)) {
      mode = "avoid";
      continue;
    }
    const bullet = line.match(/^[-*]\s+(.+)/);
    if (bullet && mode) {
      (mode === "do" ? doList : avoidList).push(bullet[1].trim());
    }
  }
  return { do: doList, avoid: avoidList };
}

/** Top-level parser. frontmatter + 5 섹션 best-effort. */
export function parseIdentitySummary(content: string): ParsedIdentity {
  const [frontmatter, body] = extractFrontmatter(content);
  const sections = splitBySections(body, SECTION_HEADERS);
  return {
    frontmatter,
    sections: {
      projectIdentity: sections[0] ?? "",
      userWorkingPreference: sections[1] ?? "",
      agentOperatingPreference: sections[2] ?? "",
      inflectionPoints: parseInflectionPoints(sections[3] ?? ""),
      doAvoid: parseDoAvoid(sections[4] ?? ""),
    },
  };
}
