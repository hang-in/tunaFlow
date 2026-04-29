/**
 * claudeTransportFlipHardeningPlan T6 — Claude 세션 재시작 frontend wrapper.
 *
 * Backend (`agents.rs::restart_sdk_session`) 가 모드별 분기 (sdk-url 은 process
 * kill + DB clear, cli 는 DB resume_token NULL) 처리. 본 wrapper 는 단순히
 * Tauri invoke 를 호출하고 에러 toast 까지 포함.
 */
import { invoke } from "@tauri-apps/api/core";
import { toast } from "sonner";

/**
 * 사용자가 명시적으로 Claude 세션 재시작을 trigger 했을 때 호출.
 * 다음 send 가 fresh session 으로 시작 → ContextPack 이 full mode 로 자동 발동
 * (T2+T3 의 자동 fallback 와 동일 효과, 단지 사용자 manual).
 */
export async function restartClaudeSession(conversationId: string): Promise<void> {
  try {
    await invoke("restart_sdk_session", { conversationId });
    toast.success("Claude 세션 재시작 완료", {
      description: "다음 send 가 fresh session 으로 시작됩니다.",
      duration: 4000,
    });
  } catch (e) {
    console.warn("[claude-session] restart failed:", e);
    toast.error("Claude 세션 재시작 실패", {
      description: String(e),
      duration: 6000,
    });
  }
}
