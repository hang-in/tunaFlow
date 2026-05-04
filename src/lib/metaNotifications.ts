/**
 * Meta notifications — 워크플로우/에이전트 이벤트를 메타 에이전트 알림으로
 * dispatch 하는 공통 헬퍼.
 *
 * **설계 원칙** (docs/plans/metaAgentPlan.md):
 * - "제안하되 결정하지 않는다" — 메타는 사용자 승인 게이트만 제공
 * - 읽기 전용 권한. 실제 실행은 기존 워크플로우 경로 (architect_redesign_requested 등)
 *
 * **이벤트 구조**:
 * - PR-1 (현재): runtime 만, localStorage 캐시로 페이지 간 유지
 * - PR-2: DB persist (notifications 테이블)
 * - PR-3: Tier 2 메타 LLM 브리핑 (Haiku/Gemini Flash)
 */

/**
 * **kind 정리 (2026-05-04, reviewerVerdictDirectArchitectPlan)**:
 * - review_passed / review_failed / doom_loop_warning / doom_loop_escalated 4 kind 는
 *   Architect 직행 라우팅으로 dispatch 가 사라짐 → MetaNotificationKind 에서 제거
 * - tier2_brief 신규 추가 — Tier 2 분석 (Haiku/Flash) 결과 dispatch 전용
 * - DB 의 deprecated kind row 는 free-text 라 그대로 남음. UI 는 unknown kind 폴백 처리
 */
export type MetaNotificationKind =
  | "tier2_brief"             // Tier 2 (Haiku/Flash) 분석 결과 brief
  | "architect_redesign_requested"  // 사용자가 재설계 버튼 명시 클릭
  | "plan_completed"          // Plan 전체 사이클 완료
  | "plan_promoted"           // 새 Plan 등록됨
  | "tool_request_failed"     // 도구 호출 실패
  | "insight_detected"        // 새 Insight finding 감지
  | "generic";                // 기타 폴백 (legacy review-cycle row 도 여기로 폴백)

export interface MetaNotificationRoute {
  /** 이동할 탭 (AppShell 의 `tunaflow:switch-tab` 이벤트와 매칭) */
  tab?: "workflow" | "insight" | "plans" | "artifacts" | "chat";
  /** 워크플로우 탭 내부 stage (HarnessSummary) */
  stage?: "all" | "plan-check" | "dev" | "review" | "done";
  /** 이동 후 포커스할 plan (PlanCard) */
  planId?: string;
  /** 이동 후 포커스할 branch (리뷰/구현 shadow conv) */
  branchId?: string;
  /** 이동 후 스크롤할 message id */
  messageId?: string;
}

export interface MetaNotification {
  id: string;
  kind: MetaNotificationKind;
  title: string;
  summary?: string;
  /** 단일 프로젝트 키 — 다른 프로젝트 알림과 분리 */
  projectKey?: string;
  /** epoch ms */
  createdAt: number;
  /** 읽음/dismiss 상태 — PR-2 에서 DB 로 이동 */
  read?: boolean;
  dismissed?: boolean;
  /** 클릭 시 이동할 경로 — dispatch 시 선택적 */
  route?: MetaNotificationRoute;
}

/** 메타 알림 이벤트 — MetaFloatingChat 이 수신해 inbox 에 누적.
 *
 *  **PR 정책 A**: DB persist (meta_notifications 테이블) + **메타 conversation 에 role='system'
 *  메시지로 mirror**. 사용자가 메타 채팅 열 때 최근 이벤트가 이미 대화에 나타나 있게 함
 *  — "빈 채팅에 배지만 있어서 맥락 모름" 문제 해소. metaAgentPlan.md "제안하되 결정하지
 *  않음" 원칙은 유지: 이 메시지는 참고용 컨텍스트일 뿐 메타 LLM 은 여전히 사용자 승인
 *  전엔 plan/subtask 를 건드리지 않음.
 */
export async function dispatchMetaNotification(input: Omit<MetaNotification, "id" | "createdAt" | "read" | "dismissed">): Promise<void> {
  const { invoke } = await import("@tauri-apps/api/core");
  let row: { id: string; projectKey: string | null; kind: string; title: string; summary: string | null; routeJson: string | null; createdAt: number } | null = null;
  try {
    row = await invoke("create_meta_notification", {
      input: {
        projectKey: input.projectKey ?? null,
        kind: input.kind,
        title: input.title,
        summary: input.summary ?? null,
        routeJson: input.route ? JSON.stringify(input.route) : null,
      },
    });
  } catch (e) {
    console.warn("[meta-notif] DB persist failed, emitting runtime-only:", e);
  }

  const notif: MetaNotification = row
    ? {
        id: row.id,
        kind: row.kind as MetaNotificationKind,
        title: row.title,
        summary: row.summary ?? undefined,
        projectKey: row.projectKey ?? undefined,
        createdAt: row.createdAt,
        route: input.route,
        read: false,
        dismissed: false,
      }
    : {
        id: crypto.randomUUID(),
        createdAt: Date.now(),
        read: false,
        dismissed: false,
        ...input,
      };

  // A — meta conversation 에도 system 메시지로 mirror (메타 채팅 열면 바로 보이게).
  // projectKey 있을 때만 (프로젝트 단위 meta conv). 실패는 조용히 무시.
  if (input.projectKey) {
    try {
      const { getOrCreateMetaConversation } = await import("./metaConversation");
      const metaConvId = await getOrCreateMetaConversation(input.projectKey);
      const body = [
        `### ${input.title}`,
        input.summary ?? "",
        input.route
          ? `\n_이동: ${input.route.tab ?? ""}${input.route.stage ? "/" + input.route.stage : ""}${input.route.planId ? " (plan " + input.route.planId.slice(0, 8) + ")" : ""}_`
          : "",
      ].filter(Boolean).join("\n\n");
      await invoke("persist_system_msg", { conversationId: metaConvId, content: body }).catch(() => {});
    } catch (e) { console.debug("[meta-mirror]", e); }
  }

  window.dispatchEvent(new CustomEvent("tunaflow:meta-task", { detail: notif }));
}
