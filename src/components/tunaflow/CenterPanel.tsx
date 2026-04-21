import { useState, useRef, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { cn } from "@/lib/utils";
import { useChatStore } from "@/stores/chatStore";
import { ChevronLeft, ChevronRight, Loader2, Search, StickyNote } from "lucide-react";

import { lazy, Suspense } from "react";
import { ChatPanel } from "./ChatPanel";
import { PlansPanel } from "./context-panel/PlansPanel";
import { HarnessSummary, type WorkflowStageId } from "./context-panel/HarnessSummary";
import { ReviewPanel } from "./context-panel/ReviewPanel";
import { InsightPanel } from "./context-panel/InsightPanel";
import { ArtifactsPanel } from "./context-panel/ArtifactsPanel";
import { NotificationBell } from "./NotificationBell";

const TerminalPanel = lazy(() => import("./TerminalPanel").then((m) => ({ default: m.TerminalPanel })));

type CenterTab = "chat" | "workflow" | "insight" | "notes";

const TABS: { id: CenterTab; label: string }[] = [
  { id: "chat", label: "Chat" },
  { id: "workflow", label: "Workflow" },
  { id: "insight", label: "Insight" },
  { id: "notes", label: "Notes" },
];

/** Map PlanPhase → WorkflowStageId for auto-switching the HarnessSummary sub-tab.
 *  drafting + subtask_review 는 모두 "Plan Check" 단계로 통합 (s37). */
const PHASE_TO_STAGE: Record<string, WorkflowStageId> = {
  drafting: "plan-check", subtask_review: "plan-check",
  approval: "dev",
  implementation: "dev", rework: "dev",
  review: "review", done: "done",
};

export function CenterPanel() {
  const [activeTab, setActiveTab] = useState<CenterTab>("chat");
  const [activeStage, setActiveStage] = useState<WorkflowStageId>("plan-check");
  const [planRefreshKey, setPlanRefreshKey] = useState(0);
  const [terminalOpen, setTerminalOpen] = useState(false);
  const [terminalHeight, setTerminalHeight] = useState(320);
  const terminalDragRef = useRef<{ startY: number; startH: number } | null>(null);
  const [reviewExpanded, setReviewExpanded] = useState(false);

  // Listen for terminal toggle from RuntimeStatusBar
  useEffect(() => {
    const handler = (e: Event) => setTerminalOpen((e as CustomEvent).detail as boolean);
    window.addEventListener("tunaflow:terminal-toggle", handler);
    return () => window.removeEventListener("tunaflow:terminal-toggle", handler);
  }, []);

  // Terminal drag-resize: min 160px, max 50% of window height
  const handleTerminalDragStart = useCallback((e: React.MouseEvent) => {
    e.preventDefault();
    terminalDragRef.current = { startY: e.clientY, startH: terminalHeight };
    const onMove = (ev: MouseEvent) => {
      if (!terminalDragRef.current) return;
      const delta = terminalDragRef.current.startY - ev.clientY;
      const next = Math.max(160, Math.min(window.innerHeight * 0.5, terminalDragRef.current.startH + delta));
      setTerminalHeight(next);
    };
    const onUp = () => {
      terminalDragRef.current = null;
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
    };
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
  }, [terminalHeight]);

  const selectedProjectKey = useChatStore((s) => s.selectedProjectKey);
  const artifacts = useChatStore((s) => s.artifacts);
  const selectedConversationId = useChatStore((s) => s.selectedConversationId);
  const conversations = useChatStore((s) => s.conversations);
  const activeBranchId = useChatStore((s) => s.activeBranchId);
  const parentConversationId = useChatStore((s) => s.parentConversationId);
  const drawerPinned = useChatStore((s) => s.drawerPinned);
  const memos = useChatStore((s) => s.memos);
  const deleteMemo = useChatStore((s) => s.deleteMemo);
  const selectConversation = useChatStore((s) => s.selectConversation);

  const canonicalConvId = activeBranchId && parentConversationId
    ? parentConversationId
    : selectedConversationId;

  const currentConv = conversations.find((c) => c.id === selectedConversationId);
  const isScratchpad = currentConv?.type === "scratchpad";

  // Scratchpad: force Chat tab
  const effectiveTab = isScratchpad ? "chat" : activeTab;

  // Reset to Chat tab on project switch
  useEffect(() => { setActiveTab("chat"); }, [selectedProjectKey]);

  // Listen for tab/stage switch events
  useEffect(() => {
    const tabHandler = (e: Event) => {
      const tab = (e as CustomEvent).detail as CenterTab;
      if (TABS.some((t) => t.id === tab)) setActiveTab(tab);
    };
    const stageHandler = (e: Event) => {
      const stage = (e as CustomEvent).detail as WorkflowStageId;
      if (stage) setActiveStage(stage);
    };
    window.addEventListener("tunaflow:switch-tab", tabHandler);
    window.addEventListener("tunaflow:switch-stage", stageHandler);
    return () => {
      window.removeEventListener("tunaflow:switch-tab", tabHandler);
      window.removeEventListener("tunaflow:switch-stage", stageHandler);
    };
  }, []);

  const reviewArtifacts = artifacts.filter((a) => a.type === "review-findings");

  const [planCount, setPlanCount] = useState(0);
  const [insightCount, setInsightCount] = useState(0);

  // Fetch plan count badge
  useEffect(() => {
    const convId = canonicalConvId;
    if (!convId) { setPlanCount(0); return; }
    invoke<{ status: string }[]>("list_plans_by_conversation", { conversationId: convId })
      .then((plans) => setPlanCount(plans.filter((p) => p.status !== "done" && p.status !== "abandoned").length))
      .catch(() => setPlanCount(0));
  }, [canonicalConvId, planRefreshKey]);

  // Fetch open finding count for Insight tab badge.
  // Previously surfaced "completed session count" which only grew (every
  // analysis added +1, processing findings never reduced it). Now surfaces
  // "how many findings still need user action" = open findings only.
  useEffect(() => {
    if (!selectedProjectKey) { setInsightCount(0); return; }
    invoke<number>("count_open_insight_findings", { projectKey: selectedProjectKey })
      .then(setInsightCount)
      .catch(() => setInsightCount(0));
  }, [selectedProjectKey]);

  const notesCount = memos.length + artifacts.length;

  return (
    <div
      role="main"
      aria-label="메인 대화 영역"
      className="flex flex-col flex-1 min-w-0 h-full"
    >
      {/* ── Toolbar ── */}
      <div className="flex items-center px-3 pt-2 pb-1 shrink-0">
        {/* Tabs — scratchpad shows back button instead */}
        {isScratchpad ? (
          <button
            onClick={() => {
              const chatConvs = conversations
                .filter((c) => c.type === "main" || c.type === "discussion")
                .sort((a, b) => b.updatedAt - a.updatedAt);
              if (chatConvs.length > 0) selectConversation(chatConvs[0].id);
            }}
            className="flex items-center gap-1 text-[12px] text-muted-foreground/60 hover:text-foreground px-2 py-1 rounded-md hover:bg-accent/50 transition-colors shrink-0"
            title="최근 채팅 대화로 이동"
          >
            <ChevronLeft className="w-3.5 h-3.5" />
            메인 채팅
          </button>
        ) : (
          <div className="flex items-center gap-1 shrink-0">
            {TABS.map((tab) => (
              <button
                key={tab.id}
                onClick={() => setActiveTab(tab.id)}
                className={cn(
                  "flex items-center gap-1.5 px-2.5 py-1 rounded-md text-[12px] font-medium transition-colors",
                  activeTab === tab.id
                    ? "bg-background text-foreground shadow-sm"
                    : "text-muted-foreground/50 hover:text-foreground/80 hover:bg-background/50"
                )}
              >
                {tab.label}
                {tab.id === "workflow" && planCount > 0 && (
                  <span className="text-[9px] bg-primary/20 text-primary font-semibold px-1 rounded">{planCount}</span>
                )}
                {tab.id === "insight" && insightCount > 0 && (
                  <span className="text-[9px] bg-amber-500/20 text-amber-500 font-semibold px-1 rounded">{insightCount}</span>
                )}
                {tab.id === "notes" && notesCount > 0 && (
                  <span className="text-[9px] bg-foreground/10 text-foreground/60 font-semibold px-1 rounded">{notesCount}</span>
                )}
              </button>
            ))}
          </div>
        )}

        <div className="flex-1 min-w-0" />

        {/* Search + Notification bell */}
        <div className="flex items-center gap-1 shrink-0">
          {drawerPinned ? (
            <button
              onClick={() => window.dispatchEvent(new CustomEvent("tunaflow:open-command-palette"))}
              title="Search (Cmd+K)"
              className="p-1.5 rounded text-muted-foreground/40 hover:text-foreground hover:bg-accent/50 transition-colors"
            >
              <Search className="w-3.5 h-3.5" />
            </button>
          ) : (
            <SearchBox
              projectKey={useChatStore.getState().selectedProjectKey}
              onSelectResult={(convId) => {
                setActiveTab("chat");
                if (convId.startsWith("branch:")) {
                  useChatStore.getState().openThread(convId.replace("branch:", ""));
                } else {
                  selectConversation(convId);
                }
              }}
            />
          )}
          <NotificationBell />
        </div>
      </div>

      {/* ── Content zone ── */}
      <div className="flex-1 min-h-0 rounded-xl border-[0.5px] border-border bg-background overflow-hidden flex flex-col mx-2">

        {/* Chat — stays mounted to preserve Virtuoso scroll */}
        <div className="flex-1 min-h-0 flex flex-col" style={{ display: effectiveTab === "chat" ? "flex" : "none" }}>
          <div className="min-h-0 overflow-hidden relative" style={{ flex: "2 1 0%" }}>
            <div className="absolute inset-0 flex flex-col">
              <ChatPanel />
            </div>
          </div>
          {terminalOpen && (
            <div className="shrink-0 border-t border-border/30 bg-[#0d0f17] flex flex-col" style={{ height: terminalHeight }}>
              {/* Drag handle */}
              <div
                onMouseDown={handleTerminalDragStart}
                className="shrink-0 h-1 cursor-row-resize hover:bg-primary/30 transition-colors"
                title="드래그로 크기 조정"
              />
              <div className="flex-1 min-h-0">
                <Suspense fallback={<div className="p-2 text-xs text-muted-foreground">Loading terminal...</div>}>
                  <TerminalPanel />
                </Suspense>
              </div>
            </div>
          )}
        </div>

        {/* Workflow */}
        {effectiveTab === "workflow" && (
          <div className="flex-1 overflow-y-auto p-5">
            <div className="max-w-4xl mx-auto space-y-6">
              {canonicalConvId && (
                <HarnessSummary
                  conversationId={canonicalConvId}
                  activeStage={activeStage}
                  onStageClick={setActiveStage}
                  refreshKey={planRefreshKey}
                />
              )}
              <PlansPanel
                activeStage={activeStage}
                onPhaseChanged={(_id, phase) => {
                  const target = PHASE_TO_STAGE[phase];
                  if (target) setActiveStage(target);
                  setPlanRefreshKey((k) => k + 1);
                }}
                onStatusChanged={() => {
                  setPlanRefreshKey((k) => k + 1);
                  setActiveStage("plan-check");
                }}
                onSwitchToChat={() => setActiveTab("chat")}
              />
              {(activeStage === "review" || activeStage === "done") && reviewArtifacts.length > 0 && (
                <div className="border-t border-border/20 pt-3">
                  <button
                    onClick={() => setReviewExpanded((v) => !v)}
                    className="flex items-center gap-1.5 text-[10px] font-semibold uppercase tracking-wider text-muted-foreground/50 hover:text-foreground/70 transition-colors mb-2"
                  >
                    <ChevronRight className={cn("w-3 h-3 transition-transform", reviewExpanded && "rotate-90")} />
                    Review Results
                    <span className="text-[9px] font-normal text-muted-foreground/30">({reviewArtifacts.length})</span>
                  </button>
                  {reviewExpanded && <ReviewPanel />}
                </div>
              )}
            </div>
          </div>
        )}

        {/* Insight */}
        {effectiveTab === "insight" && (
          <div className="flex-1 min-h-0 flex flex-col overflow-hidden">
            <InsightPanel />
          </div>
        )}

        {/* Notes — Memos + Artifacts */}
        {effectiveTab === "notes" && (
          <NotesPanel
            memos={memos}
            deleteMemo={deleteMemo}
            selectedConversationId={selectedConversationId}
            selectConversation={selectConversation}
            onNavigateToChat={() => setActiveTab("chat")}
          />
        )}

      </div>
    </div>
  );
}

// ─── Notes Panel ─────────────────────────────────────────────────────────────

interface Memo {
  id: string;
  content: string;
  messageId: string;
  conversationId: string | null;
  createdAt: number;
}

function NotesPanel({
  memos, deleteMemo, selectedConversationId, selectConversation, onNavigateToChat,
}: {
  memos: Memo[];
  deleteMemo: (id: string) => void;
  selectedConversationId: string | null;
  selectConversation: (id: string) => Promise<void>;
  onNavigateToChat: () => void;
}) {
  return (
    <div className="flex-1 overflow-y-auto">
      {/* Memos section */}
      <div className="px-5 pt-5 pb-3">
        <div className="flex items-center gap-2 mb-3">
          <StickyNote className="w-3.5 h-3.5 text-muted-foreground/40" />
          <span className="text-[11px] font-semibold uppercase tracking-wider text-muted-foreground/50">
            Memos
          </span>
          {memos.length > 0 && (
            <span className="text-[9px] text-muted-foreground/30">({memos.length})</span>
          )}
        </div>
        {memos.length === 0 ? (
          <p className="text-[11px] text-muted-foreground/30 italic">
            메시지 우클릭 → Save as memo로 추가할 수 있습니다
          </p>
        ) : (
          <div className="space-y-1.5">
            {memos.map((m) => (
              <div
                key={m.id}
                className="group flex items-start gap-2.5 px-3 py-2 rounded-lg hover:bg-accent/50 cursor-pointer transition-colors"
                onClick={() => {
                  if (m.conversationId && m.conversationId !== selectedConversationId) {
                    selectConversation(m.conversationId);
                  }
                  setTimeout(() => {
                    useChatStore.setState({ scrollToMessageId: m.messageId });
                  }, 100);
                  onNavigateToChat();
                }}
              >
                <StickyNote className="w-3 h-3 text-muted-foreground/25 shrink-0 mt-0.5" />
                <div className="flex-1 min-w-0">
                  <p className="text-[12px] text-foreground/80 leading-snug line-clamp-2">{m.content}</p>
                  <p className="text-[10px] text-muted-foreground/35 mt-0.5 font-mono">
                    {new Date(m.createdAt).toLocaleDateString()}
                  </p>
                </div>
                <button
                  onClick={(e) => { e.stopPropagation(); deleteMemo(m.id); }}
                  className="shrink-0 p-0.5 rounded opacity-0 group-hover:opacity-100 text-muted-foreground/30 hover:text-destructive transition-all"
                  title="Delete"
                >
                  <span className="text-[9px]">✕</span>
                </button>
              </div>
            ))}
          </div>
        )}
      </div>

      {/* Divider */}
      <div className="mx-5 border-t border-border/20" />

      {/* Artifacts section */}
      <div className="px-5 pt-3 pb-5">
        <ArtifactsPanel />
      </div>
    </div>
  );
}

// ─── Search Box ─────────────────────────────────────────────────────────────

interface SearchResult {
  messageId: string;
  conversationId: string;
  conversationLabel: string;
  role: string;
  contentSnippet: string;
  timestamp: number;
  engine: string | null;
  persona: string | null;
}

function SearchBox({ projectKey, onSelectResult }: { projectKey: string | null; onSelectResult: (convId: string) => void }) {
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<SearchResult[]>([]);
  const [open, setOpen] = useState(false);
  const [loading, setLoading] = useState(false);
  const ref = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLInputElement>(null);
  const debounceRef = useRef<ReturnType<typeof setTimeout>>();

  useEffect(() => {
    if (!open) return;
    const handler = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) setOpen(false);
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [open]);

  const doSearch = useCallback((q: string) => {
    if (!projectKey || q.length < 2) { setResults([]); setOpen(false); return; }
    setLoading(true);
    invoke<SearchResult[]>("search_messages", { query: q, projectKey, limit: 15 })
      .then((r) => { setResults(r); setOpen(r.length > 0); })
      .catch(() => setResults([]))
      .finally(() => setLoading(false));
  }, [projectKey]);

  const handleChange = (value: string) => {
    setQuery(value);
    if (debounceRef.current) clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(() => doSearch(value), 300);
  };

  return (
    <div className="relative" style={{ width: "clamp(160px, 18vw, 320px)" }} ref={ref}>
      <div className="flex items-center gap-2 bg-background/50 hover:bg-background/70 border border-border/30 rounded-md px-2.5 py-1.5 transition-colors focus-within:border-ring/40">
        <Search className="w-3.5 h-3.5 text-muted-foreground/40 shrink-0" />
        <input
          ref={inputRef}
          value={query}
          onChange={(e) => handleChange(e.target.value)}
          onFocus={() => { if (results.length > 0) setOpen(true); }}
          placeholder="Search…"
          className="flex-1 bg-transparent text-[12px] font-medium outline-none text-foreground placeholder:text-muted-foreground/40"
        />
        {loading && <Loader2 className="w-3 h-3 animate-spin text-muted-foreground/30 shrink-0" />}
      </div>

      {open && results.length > 0 && (
        <div className="absolute right-0 top-full mt-1 w-[360px] max-h-[400px] bg-popover border border-border/40 rounded-lg shadow-xl overflow-hidden z-50">
          <div className="px-3 py-1.5 text-tf-sm text-muted-foreground/50 border-b border-border/30">
            {results.length} results
          </div>
          <div className="overflow-y-auto max-h-[350px]">
            {results.map((r) => (
              <button
                key={r.messageId}
                onClick={() => { onSelectResult(r.conversationId); setOpen(false); setQuery(""); }}
                className="w-full text-left px-3 py-2 hover:bg-accent/50 transition-colors"
              >
                <div className="flex items-center gap-2 text-tf-sm">
                  <span className="text-foreground/70 font-medium truncate flex-1">{r.conversationLabel}</span>
                  <span className="text-[9px] text-muted-foreground/40">{r.role}</span>
                  {r.persona && <span className="text-[9px] text-muted-foreground/30">{r.persona}</span>}
                </div>
                <p
                  className="text-tf-sm text-muted-foreground/60 leading-snug line-clamp-2 mt-0.5"
                  dangerouslySetInnerHTML={{ __html: r.contentSnippet.replace(/\*\*/g, (_, i) => i % 2 === 0 ? '<mark class="bg-primary/20 text-foreground rounded px-0.5">' : '</mark>') }}
                />
                <span className="text-[9px] text-muted-foreground/30 font-mono">
                  {new Date(r.timestamp).toLocaleDateString()}
                </span>
              </button>
            ))}
          </div>
        </div>
      )}
    </div>
  );
}
