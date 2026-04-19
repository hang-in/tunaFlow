/**
 * 역할(Role) → AgentProfile 매핑 — Review RT, Plan 승인, Synthesis 등
 * 작업 트리거 전에 "어떤 프로필이 이 역할을 맡을지" 를 명시적으로 정의한다.
 *
 * 왜 필요한가:
 * - 이전엔 Review RT 가 `["claude", "gemini"]` 같은 하드코딩 default 로 돌아갔고,
 *   Codex fallback 은 `gpt-5-codex` 로 잠겨 있어 ChatGPT 구독 계정에선 400 에러.
 * - 역할별 프로필을 사용자가 명시적으로 할당하면 엔진/모델이 설정 그대로 흐른다.
 *
 * 저장: `getSetting("roleAssignments")` (single object, project-scope 아님 — 전역)
 * 마이그레이션: 값이 없으면 기존 agentProfiles 에서 label/personaId 매칭으로 자동 추론.
 */
import { getSetting, setSetting } from "@/lib/appStore";
import type { AgentProfile } from "@/types";

export type RoleKey = "architect" | "developer" | "reviewers" | "synthesizer";

export interface RoleAssignments {
  /** 단일 프로필 ID */
  architect?: string;
  /** 단일 프로필 ID */
  developer?: string;
  /** 프로필 ID 리스트 — Deep review 는 ≥2 필요 */
  reviewers: string[];
  /** 단일 프로필 ID — RT 최종 합성 */
  synthesizer?: string;
}

export const EMPTY_ASSIGNMENTS: RoleAssignments = { reviewers: [] };

export async function loadRoleAssignments(): Promise<RoleAssignments> {
  const saved = await getSetting<RoleAssignments | null>("roleAssignments", null);
  return saved ?? EMPTY_ASSIGNMENTS;
}

export async function saveRoleAssignments(assignments: RoleAssignments): Promise<void> {
  await setSetting("roleAssignments", assignments);
}

/** label/personaId 기반 자동 추론 — roleAssignments 가 비어있을 때만 제안용으로 사용. */
export function inferRoleAssignments(profiles: AgentProfile[]): RoleAssignments {
  const byPersona = (pid: string) => profiles.filter((p) => p.personaId === pid);
  const byLabel = (kw: string) => profiles.filter((p) => p.label.toLowerCase().includes(kw));

  const architect = byPersona("persona_architect")[0] ?? byLabel("architect")[0];
  const developer = byPersona("persona_implementer")[0] ?? byLabel("develop")[0] ?? byLabel("coder")[0];
  const reviewers = byPersona("persona_reviewer").length > 0
    ? byPersona("persona_reviewer").map((p) => p.id)
    : byLabel("review").map((p) => p.id);
  // Synthesizer 는 명시 persona 가 없으면 architect 재사용 (RT 최종 합성은 architect 성격과 가까움).
  const synthesizer = byLabel("synth")[0] ?? architect;

  return {
    architect: architect?.id,
    developer: developer?.id,
    reviewers,
    synthesizer: synthesizer?.id,
  };
}

// ─── Coverage status ─────────────────────────────────────────────────────────

export type RoleStatus = "ready" | "missing" | "model-unset";

export interface RoleCoverage {
  role: RoleKey;
  status: RoleStatus;
  profileIds: string[];
  /** 사용자에게 보여줄 짧은 설명 */
  hint: string;
}

/** 각 역할의 충족도 평가. UI 뱃지 + 진입 게이트 공통. */
export function evaluateCoverage(
  assignments: RoleAssignments,
  profiles: AgentProfile[],
): RoleCoverage[] {
  const byId = new Map(profiles.map((p) => [p.id, p]));

  const check = (role: RoleKey, ids: string[], minCount: number, label: string): RoleCoverage => {
    const valid = ids.filter((id) => byId.has(id));
    if (valid.length < minCount) {
      return { role, status: "missing", profileIds: valid, hint: `${label} 프로필이 설정되지 않았습니다 (최소 ${minCount}개 필요)` };
    }
    const missingModel = valid.some((id) => !byId.get(id)?.model);
    if (missingModel) {
      return { role, status: "model-unset", profileIds: valid, hint: `${label} 프로필에 모델이 지정되지 않아 엔진별 fallback 이 적용됩니다` };
    }
    return { role, status: "ready", profileIds: valid, hint: `${label} 준비 완료` };
  };

  return [
    check("architect", assignments.architect ? [assignments.architect] : [], 1, "Architect"),
    check("developer", assignments.developer ? [assignments.developer] : [], 1, "Developer"),
    check("reviewers", assignments.reviewers, 2, "Reviewer (≥2)"),
    check("synthesizer", assignments.synthesizer ? [assignments.synthesizer] : [], 1, "Synthesizer"),
  ];
}

/** 역할별 프로필을 실제 Participant/ReviewerChoice 형태로 반환. */
export function resolveRoleProfiles(
  role: RoleKey,
  assignments: RoleAssignments,
  profiles: AgentProfile[],
): AgentProfile[] {
  const byId = new Map(profiles.map((p) => [p.id, p]));
  const ids =
    role === "reviewers" ? assignments.reviewers
    : role === "architect" ? (assignments.architect ? [assignments.architect] : [])
    : role === "developer" ? (assignments.developer ? [assignments.developer] : [])
    : role === "synthesizer" ? (assignments.synthesizer ? [assignments.synthesizer] : [])
    : [];
  return ids.map((id) => byId.get(id)).filter((p): p is AgentProfile => !!p);
}

/** 진입 게이트 — 부족하면 toast + Settings 열기 이벤트 발화.
 *  Returns true if ready to proceed.
 */
export async function assertRoleReady(
  role: RoleKey,
  profiles: AgentProfile[],
): Promise<{ ok: boolean; coverage: RoleCoverage }> {
  const assignments = await loadRoleAssignments();
  const cov = evaluateCoverage(assignments, profiles).find((c) => c.role === role)!;
  if (cov.status === "missing") {
    const { toast } = await import("sonner");
    toast.error(cov.hint, {
      action: {
        label: "Settings 열기",
        onClick: () => window.dispatchEvent(new CustomEvent("tunaflow:open-settings", { detail: { section: "agents" } })),
      },
      duration: 8000,
    });
    return { ok: false, coverage: cov };
  }
  if (cov.status === "model-unset") {
    const { toast } = await import("sonner");
    toast.warning(cov.hint, { duration: 4000 });
  }
  return { ok: true, coverage: cov };
}
