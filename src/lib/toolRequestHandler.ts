/**
 * Tool request handler — processes `<!-- tunaflow:tool-request:TYPE:QUERY -->` markers.
 *
 * When an agent outputs a tool-request marker, this handler:
 * 1. Calls the appropriate backend (context-hub, rawq, code-review-graph)
 * 2. Formats results
 * 3. Returns a follow-up prompt for the next turn
 */

import { invoke } from "@tauri-apps/api/core";
import type { ToolRequest } from "@/lib/planProposalParser";

interface HubSearchResult {
  id: string;
  title: string;
  snippet: string;
}

interface HubDocument {
  id: string;
  title: string;
  content: string;
}

/** Execute tool requests and return formatted results as a follow-up prompt. */
export async function executeToolRequests(requests: ToolRequest[]): Promise<string | null> {
  const results: string[] = [];

  for (const req of requests.slice(0, 3)) {
    try {
      if (req.type === "docs") {
        const hits = await invoke<HubSearchResult[]>(
          "context_hub_search", { query: req.query, sourceFilter: null, limit: 3 }
        );
        if (hits.length > 0) {
          const doc = await invoke<HubDocument>(
            "context_hub_get", { documentId: hits[0].id }
          ).catch(() => null);
          if (doc) {
            results.push(`## 📚 ${doc.title}\n\n${doc.content.slice(0, 4000)}`);
          } else {
            results.push(`## 📚 ${hits[0].title}\n\n${hits[0].snippet}`);
          }
        } else {
          results.push(`> "${req.query}" 관련 문서를 찾지 못했습니다.`);
        }
      } else if (req.type === "rawq") {
        const { useChatStore } = await import("@/stores/chatStore");
        const pk = useChatStore.getState().selectedProjectKey;
        if (pk) {
          const project = await invoke<{ path?: string }>("get_project", { key: pk });
          if (project.path) {
            const searchResult = await invoke<string>("search_rawq", {
              projectPath: project.path, query: req.query, limit: 5,
            }).catch(() => "");
            if (searchResult) {
              results.push(`## 🔍 코드 검색: "${req.query}"\n\n${searchResult.slice(0, 3000)}`);
            }
          }
        }
      } else if (req.type === "graph") {
        const { useChatStore } = await import("@/stores/chatStore");
        const pk = useChatStore.getState().selectedProjectKey;
        if (pk) {
          const project = await invoke<{ path?: string }>("get_project", { key: pk });
          if (project.path) {
            const parts = req.query.split(/\s+/, 2);
            const pattern = parts[0] || "callers_of";
            const target = parts[1] || req.query;
            const graphResult = await invoke<string>("crg_query", {
              projectPath: project.path, pattern, target,
            }).catch(() => "");
            if (graphResult) {
              results.push(`## 🔗 Graph: ${pattern}("${target}")\n\n${graphResult.slice(0, 3000)}`);
            }
          }
        }
      } else if (req.type === "memory") {
        // Tier 2 Pull: semantic search over sliding-window chunks of current conv.
        // Replaces prior substring match on conversation_memory (topic/summary) —
        // that missed topically-related hits with different wording.
        const { useChatStore } = await import("@/stores/chatStore");
        const convId = useChatStore.getState().selectedConversationId;
        if (convId) {
          type Hit = { chunkId: string; text: string; score: number; timestamp: number | null };
          const hits = await invoke<Hit[]>("search_memory_semantic", {
            conversationId: convId, query: req.query, limit: 3,
          }).catch(() => [] as Hit[]);
          if (hits.length > 0) {
            const lines = hits.map((h) => {
              const when = h.timestamp
                ? new Date(h.timestamp).toLocaleString("ko-KR", { month: "2-digit", day: "2-digit", hour: "2-digit", minute: "2-digit" })
                : "";
              const header = when ? `### ${when} (유사도 ${h.score.toFixed(2)})` : `### 유사도 ${h.score.toFixed(2)}`;
              return `${header}\n${h.text.slice(0, 800)}`;
            });
            results.push(`## 🧠 대화 기억: "${req.query}"\n\n${lines.join("\n\n")}`);
          } else {
            results.push(`> "${req.query}" 관련 대화 기억을 찾지 못했습니다. (아직 인덱싱 안 됐거나 의미적으로 매칭되지 않음)`);
          }
        }
      } else if (req.type === "recent_turns") {
        // 단기 공백 전용: 현재 conv 의 최근 N turn 전문 반환. session_freshness=true 일 때
        // ContextPack 에서 recent_context 가 drop 되므로 에이전트가 자기 직전 답변을
        // 확인하려면 이 도구를 명시 호출해야 한다. N 파싱 실패 시 기본 3.
        const { useChatStore } = await import("@/stores/chatStore");
        const convId = useChatStore.getState().selectedConversationId;
        if (convId) {
          const parsedN = parseInt(req.query.trim(), 10);
          const n = Number.isFinite(parsedN) && parsedN > 0 ? Math.min(parsedN, 10) : 3;
          type Turn = { role: string; persona: string | null; engine: string | null; content: string; timestamp: number };
          const turns = await invoke<Turn[]>("list_recent_turns", { conversationId: convId, n }).catch(() => []);
          if (turns.length > 0) {
            const lines = turns.map((t) => {
              const label = t.role === "assistant"
                ? `[assistant${t.persona ? `:${t.persona}` : ""}${t.engine ? ` (${t.engine})` : ""}]`
                : `[user]`;
              return `${label}\n${t.content}`;
            });
            results.push(`## 🕒 현재 대화 최근 ${turns.length} turn (전문)\n\n${lines.join("\n\n---\n\n")}`);
          } else {
            results.push(`> 현재 대화에서 최근 turn 을 찾지 못했습니다. (아직 주고받은 메시지 없음)`);
          }
        }
      } else if (req.type === "sessions") {
        // Tier 2 Pull: cross-session search
        const { useChatStore } = await import("@/stores/chatStore");
        const convId = useChatStore.getState().selectedConversationId;
        const pk = useChatStore.getState().selectedProjectKey;
        if (convId && pk) {
          const links = await invoke<{ linkedConvId: string; score: number; method: string }[]>(
            "get_session_links", { conversationId: convId }
          ).catch(() => []);
          if (links.length > 0) {
            const lines = links.slice(0, 5).map((l) => `- ${l.linkedConvId} (score: ${l.score.toFixed(2)}, ${l.method})`);
            results.push(`## 🔗 관련 세션 (${links.length}개)\n\n${lines.join("\n")}`);
          } else {
            results.push(`> 관련 세션을 찾지 못했습니다.`);
          }
        }
      } else if (req.type === "skills") {
        // Tier 2 Pull: search skills by keyword
        const { useChatStore } = await import("@/stores/chatStore");
        const allSkills = useChatStore.getState().skills ?? [];
        const matched = allSkills.filter((s) =>
          s.name?.toLowerCase().includes(req.query.toLowerCase()) ||
          s.description?.toLowerCase().includes(req.query.toLowerCase())
        ).slice(0, 3);
        if (matched.length > 0) {
          const lines = matched.map((s) => `### ${s.name}\n${(s.content ?? s.description ?? "").slice(0, 1000)}`);
          results.push(`## 📖 스킬: "${req.query}"\n\n${lines.join("\n\n")}`);
        } else {
          results.push(`> "${req.query}" 관련 스킬을 찾지 못했습니다.`);
        }
      } else if (req.type === "artifacts") {
        // Tier 2 Pull: fetch artifact by ID or search by title
        const { useChatStore } = await import("@/stores/chatStore");
        const artifacts = useChatStore.getState().artifacts ?? [];
        const matched = artifacts.filter((a) =>
          a.id === req.query || a.title?.toLowerCase().includes(req.query.toLowerCase())
        ).slice(0, 3);
        if (matched.length > 0) {
          const lines = matched.map((a) => `### ${a.title} (${a.type}, ${a.status})\n${(a.content ?? "").slice(0, 1500)}`);
          results.push(`## 📦 아티팩트: "${req.query}"\n\n${lines.join("\n\n")}`);
        } else {
          results.push(`> "${req.query}" 관련 아티팩트를 찾지 못했습니다.`);
        }
      } else if (req.type === "lessons") {
        // Tier 2 Pull: failure lessons by pattern
        const pk = (await import("@/stores/chatStore")).useChatStore.getState().selectedProjectKey;
        if (pk) {
          const lessons = await invoke<{ pattern: string; finding: string; resolution: string | null }[]>(
            "search_similar_failures", { projectKey: pk, query: req.query, filePaths: [], limit: 3 }
          ).catch(() => []);
          if (lessons.length > 0) {
            const lines = lessons.map((l) => `- **${l.pattern}**: ${l.finding}${l.resolution ? ` → ${l.resolution}` : ""}`);
            results.push(`## ⚠️ 과거 실패 패턴: "${req.query}"\n\n${lines.join("\n")}`);
          } else {
            results.push(`> "${req.query}" 관련 실패 패턴이 없습니다.`);
          }
        }
      } else if (req.type === "insight-update") {
        // Format: FINDING_ID|STATUS|NOTE
        // STATUS: resolved|skipped|discarded|in_progress
        const parts = req.query.split("|");
        const findingId = parts[0]?.trim();
        const status = parts[1]?.trim();
        const note = parts[2]?.trim() ?? "";
        const validStatuses = ["resolved", "skipped", "discarded", "in_progress"];
        if (findingId && status && validStatuses.includes(status)) {
          await invoke("update_insight_finding_status", { id: findingId, status, resolution: note || null })
            .catch((e) => console.warn("[insight-update] failed:", e));
          results.push(`> ✅ Insight finding \`${findingId}\` → **${status}**${note ? ` (${note})` : ""}`);
          // Notify Meta — insight status updated.
          const { dispatchMetaNotification } = await import("@/lib/metaNotifications");
          const { useChatStore: chatStore2 } = await import("@/stores/chatStore");
          const insightProjectKey = chatStore2.getState().selectedProjectKey ?? undefined;
          dispatchMetaNotification({
            kind: "insight_detected",
            title: `💡 Insight ${status === "resolved" ? "해소" : status === "skipped" ? "보류" : status === "discarded" ? "폐기" : "진행"}`,
            summary: `Finding \`${findingId.slice(0, 8)}\`${note ? ` — ${note.slice(0, 80)}` : ""}`,
            projectKey: insightProjectKey,
            route: { tab: "insight" },
          });
        } else {
          results.push(`> ⚠️ insight-update 형식 오류: \`FINDING_ID|STATUS|NOTE\` 형식이어야 합니다. (STATUS: resolved|skipped|discarded|in_progress)`);
        }
      } else if (req.type === "insight") {
        // Tier 2 Pull: query insight findings from the latest session
        // Query formats:
        //   "open"              — all open/pending findings
        //   "category:CATNAME"  — findings filtered by category
        //   "severity:high"     — findings filtered by severity
        //   other text          — free text search in title/description
        const pk = (await import("@/stores/chatStore")).useChatStore.getState().selectedProjectKey;
        if (pk) {
          const sessions = await invoke<{ id: string; status: string; created_at: number }[]>(
            "list_insight_sessions", { projectKey: pk }
          ).catch(() => []);
          const latestSession = sessions.sort((a, b) => b.created_at - a.created_at)[0];
          if (latestSession) {
            const allFindings = await invoke<{
              id: string; category: string; severity: string;
              title: string; description: string; status: string; fix_difficulty: string;
            }[]>("list_insight_findings", { sessionId: latestSession.id, category: null }).catch(() => []);

            let filtered = allFindings;
            const q = req.query.trim().toLowerCase();
            if (q === "open") {
              filtered = allFindings.filter((f) => f.status === "open" || f.status === "pending");
            } else if (q.startsWith("category:")) {
              const cat = q.slice("category:".length).trim();
              filtered = allFindings.filter((f) => f.category.toLowerCase() === cat);
            } else if (q.startsWith("severity:")) {
              const sev = q.slice("severity:".length).trim();
              filtered = allFindings.filter((f) => f.severity.toLowerCase() === sev);
            } else {
              filtered = allFindings.filter(
                (f) => f.title.toLowerCase().includes(q) || f.description.toLowerCase().includes(q)
              );
            }

            if (filtered.length > 0) {
              const lines = filtered.slice(0, 10).map((f) =>
                `- **[${f.severity.toUpperCase()}]** \`${f.id.slice(0, 8)}\` ${f.title} (${f.category}, ${f.fix_difficulty}, ${f.status})\n  ${f.description.slice(0, 200)}`
              );
              results.push([
                `## 🔍 Insight Findings: "${req.query}" (${filtered.length}건)`,
                "",
                ...lines,
                "",
                "> 상태 변경: `<!-- tunaflow:tool-request:insight-update:FINDING_ID|STATUS|NOTE -->`",
              ].join("\n"));
            } else {
              results.push(`> "${req.query}" 조건에 맞는 insight finding이 없습니다.`);
            }
          } else {
            results.push(`> 이 프로젝트에 insight 세션이 없습니다. InsightPanel에서 분석을 먼저 실행하세요.`);
          }
        }
      } else if (req.type === "plans") {
        const { useChatStore } = await import("@/stores/chatStore");
        const state = useChatStore.getState();
        const projectKey = state.selectedProjectKey;
        // 프로젝트 스코프 조회 — 브랜치(shadow conv) 안에서 질의해도 메인 대화에
        // 소속된 완료 플랜을 놓치지 않도록. conversation 단위 필터는 fallback.
        const plans = projectKey
          ? await invoke<{ id: string; title: string; status: string; phase: string; conversationId: string }[]>(
              "list_plans_by_project", { projectKey }
            ).catch(() => [])
          : state.selectedConversationId
            ? await invoke<{ id: string; title: string; status: string; phase: string; conversationId: string }[]>(
                "list_plans_by_conversation", { conversationId: state.selectedConversationId }
              ).catch(() => [])
            : [];
        // `phase == 'done'` 도 완료로 간주 — status/phase 중 한쪽만 done 으로
        // 들어간 레거시 플랜도 커버 (review_verdict 경로에서 둘 다 세팅하지만
        // 과거 레코드에 한쪽이 누락된 경우 가능).
        const donePlans = plans.filter((p) => p.status === "done" || p.phase === "done");
        if (donePlans.length > 0) {
          const lines = donePlans.map((p) => `- ✅ "${p.title}" (완료)`);
          results.push([
            `## 📋 완료된 플랜 (${donePlans.length}개)`,
            "",
            ...lines,
            "",
            "> 후속 작업은 새 plan-proposal 마커로 제안하세요. 완료된 플랜에 subtask를 추가하지 마세요.",
          ].join("\n"));
        } else {
          results.push("> 완료된 플랜이 없습니다. 새 plan-proposal을 자유롭게 제안하세요.");
        }
      }
    } catch (e) {
      console.warn(`[tool-request] ${req.type}:${req.query} failed:`, e);
    }
  }

  if (results.length === 0) return null;

  return [
    `### 🛠️ 도구 호출 결과`,
    "",
    ...results,
    "",
    "> 위 정보를 참고하여 작업을 계속하세요.",
  ].join("\n");
}
