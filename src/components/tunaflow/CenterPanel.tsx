import { useState, useRef, useEffect, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { cn } from "@/lib/utils";
import { useChatStore } from "@/stores/chatStore";
import { MessageSquare, ClipboardList, FileSearch, TestTube, FileText, GitBranch, Users, Loader2, Search, StickyNote } from "lucide-react";

import { ChatPanel } from "./ChatPanel";
import { PlansPanel } from "./context-panel/PlansPanel";
import { HarnessSummary, type WorkflowStageId } from "./context-panel/HarnessSummary";
import { ReviewPanel } from "./context-panel/ReviewPanel";
import { TestPanel } from "./context-panel/TestPanel";
import { ArtifactsPanel } from "./context-panel/ArtifactsPanel";
import { InlineRename } from "./InlineRename";

type CenterTab = "chat" | "plan" | "artifacts" | "review" | "test";

const TABS: { id: CenterTab; label: string; icon: React.ReactNode }[] = [
  { id: "chat", label: "Chat", icon: <MessageSquare className="w-3.5 h-3.5" /> },
  { id: "plan", label: "Plan", icon: <ClipboardList className="w-3.5 h-3.5" /> },
  { id: "artifacts", label: "Artifacts", icon: <FileText className="w-3.5 h-3.5" /> },
  { id: "review", label: "Review", icon: <FileSearch className="w-3.5 h-3.5" /> },
  { id: "test", label: "Test", icon: <TestTube className="w-3.5 h-3.5" /> },
];

/** Map PlanPhase → WorkflowStageId for auto-switching */
const PHASE_TO_STAGE: Record<string, WorkflowStageId> = {
  drafting: "plan", subtask_review: "subtask",
  approval: "approved",
  implementation: "dev", rework: "dev",
  review: "review", done: "decision",
};

export function CenterPanel() {
  const [activeTab, setActiveTab] = useState<CenterTab>("chat");
  const [activeStage, setActiveStage] = useState<WorkflowStageId>("plan");
  const artifacts = useChatStore((s) => s.artifacts);
  const selectedConversationId = useChatStore((s) => s.selectedConversationId);
  const conversations = useChatStore((s) => s.conversations);
  const branches = useChatStore((s) => s.branches);
  const activeBranchId = useChatStore((s) => s.activeBranchId);
  const parentConversationId = useChatStore((s) => s.parentConversationId);
  const threadBranchId = useChatStore((s) => s.threadBranchId);
  const threadBranchLabel = useChatStore((s) => s.threadBranchLabel);
  const runningThreadIds = useChatStore((s) => s.runningThreadIds);
  const messageQueue = useChatStore((s) => s.messageQueue);
  const renameConversation = useChatStore((s) => s.renameConversation);

  const canonicalConvId = activeBranchId && parentConversationId
    ? parentConversationId
    : selectedConversationId;

  const currentConv = conversations.find((c) => c.id === selectedConversationId);
  const isRoundtable = currentConv?.mode === "roundtable";
  const isScratchpad = currentConv?.type === "scratchpad";

  // Scratchpad: force Chat tab, no other tabs needed
  const effectiveTab = isScratchpad ? "chat" : activeTab;

  const memos = useChatStore((s) => s.memos);
  const deleteMemo = useChatStore((s) => s.deleteMemo);
  const selectConversation = useChatStore((s) => s.selectConversation);

  const reviewCount = artifacts.filter((a) => a.type === "review-findings" || a.type === "architect-decision").length;
  const testCount = artifacts.filter((a) => a.type === "test-report").length;

  // Memo popover
  const [memoOpen, setMemoOpen] = useState(false);
  const memoRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!memoOpen) return;
    const handler = (e: MouseEvent) => {
      if (memoRef.current && !memoRef.current.contains(e.target as Node)) setMemoOpen(false);
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [memoOpen]);

  return (
    <div className="flex flex-col flex-1 min-w-0 h-full">
      {/* ── Toolbar: tabs | path (center) | search ── */}
      <div className="flex items-center px-3 pt-2 pb-1 shrink-0">
        {/* Left: tab pills — hidden for scratchpad */}
        {!isScratchpad && <div className="flex items-center gap-1 shrink-0">
          {TABS.map((tab) => (
            <button
              key={tab.id}
              onClick={() => setActiveTab(tab.id)}
              className={cn(
                "flex items-center gap-1.5 px-2.5 py-1 rounded-md text-[13px] font-medium transition-colors",
                activeTab === tab.id
                  ? "bg-background text-foreground shadow-sm"
                  : "text-muted-foreground/50 hover:text-foreground/80 hover:bg-background/50"
              )}
            >
              {tab.icon}
              {tab.label}
              {tab.id === "artifacts" && artifacts.length > 0 && (
                <span className="text-[8px] bg-primary/10 text-primary/70 px-1 rounded">
                  {artifacts.length}
                </span>
              )}
              {tab.id === "review" && reviewCount > 0 && (
                <span className="text-[8px] bg-status-draft/10 text-status-draft/70 px-1 rounded">
                  {reviewCount}
                </span>
              )}
              {tab.id === "test" && testCount > 0 && (
                <span className="text-[8px] bg-agent-codex/10 text-agent-codex/70 px-1 rounded">
                  {testCount}
                </span>
              )}
            </button>
          ))}
        </div>}

        {/* Center: path (centered between tabs and search) */}
        <div className="flex-1 flex items-center justify-center gap-1.5 min-w-0 px-3">
          {effectiveTab === "chat" && selectedConversationId && (
            <>
              <span className={cn(
                "text-[8px] font-semibold px-1.5 py-0.5 rounded uppercase tracking-wider shrink-0",
                isRoundtable ? "text-agent-gemini/80 bg-agent-gemini/10" : "text-muted-foreground/40 bg-background/60"
              )}>
                {isRoundtable ? "RT" : "Chat"}
              </span>
              <span className="text-[13px] text-foreground/70 font-medium truncate max-w-[180px]">
                {currentConv ? (
                  <InlineRename
                    value={currentConv.customLabel ?? currentConv.label}
                    onSave={(v) => renameConversation(selectedConversationId, v)}
                  />
                ) : "Conversation"}
              </span>

              {(threadBranchId || activeBranchId) && (() => {
                const branchId = activeBranchId || threadBranchId;
                const branch = branchId ? branches.find((b) => b.id === branchId) : null;
                const label = branch?.customLabel ?? branch?.label ?? threadBranchLabel;
                const isBranchRT = branch?.mode === "roundtable";
                if (!label) return null;
                return (
                  <span className="flex items-center gap-1 text-muted-foreground/40 shrink-0">
                    <span className="text-[10px]">—</span>
                    {isBranchRT
                      ? <Users className="w-3 h-3 text-agent-gemini/40" />
                      : <GitBranch className="w-3 h-3" />}
                    <span className="text-[10px] truncate max-w-[120px]">{label}</span>
                  </span>
                );
              })()}

              {runningThreadIds.includes(selectedConversationId) && (
                <Loader2 className="w-3 h-3 animate-spin text-primary/60 shrink-0" />
              )}
            </>
          )}
        </div>

        {/* Right: memo icon + search */}
        <div className="flex items-center gap-1.5 shrink-0">
          {/* Memo icon + popover */}
          <div className="relative" ref={memoRef}>
            <button
              onClick={() => setMemoOpen((v) => !v)}
              className={cn(
                "flex items-center justify-center w-8 h-8 rounded-md transition-colors",
                memoOpen ? "bg-background text-foreground" : "text-muted-foreground/40 hover:text-foreground/70 hover:bg-background/50"
              )}
              title={`Memos (${memos.length})`}
            >
              <StickyNote className="w-4 h-4" />
              {memos.length > 0 && (
                <span className="absolute -top-0.5 -right-0.5 text-[7px] bg-primary/80 text-primary-foreground w-3.5 h-3.5 rounded-full flex items-center justify-center font-medium">
                  {memos.length}
                </span>
              )}
            </button>

            {memoOpen && (
              <div className="absolute right-0 top-full mt-1 w-[320px] max-h-[400px] bg-popover border border-border/40 rounded-lg shadow-xl overflow-hidden z-50">
                <div className="px-3 py-2 text-[12px] font-medium text-muted-foreground border-b border-border/30">
                  Memos ({memos.length})
                </div>
                <div className="overflow-y-auto max-h-[350px]">
                  {memos.length === 0 ? (
                    <div className="px-3 py-6 text-center text-[12px] text-muted-foreground/40">
                      No memos yet
                    </div>
                  ) : memos.map((m) => (
                    <div
                      key={m.id}
                      className="group flex items-start gap-2 px-3 py-2 hover:bg-accent/50 cursor-pointer transition-colors"
                      onClick={() => {
                        // Navigate to conversation and scroll to the message
                        if (m.conversationId && m.conversationId !== selectedConversationId) {
                          selectConversation(m.conversationId);
                        }
                        // Set scroll target after conversation loads
                        setTimeout(() => {
                          useChatStore.setState({ scrollToMessageId: m.messageId });
                        }, 100);
                        setMemoOpen(false);
                        setActiveTab("chat");
                      }}
                    >
                      <StickyNote className="w-3 h-3 text-muted-foreground/30 shrink-0 mt-0.5" />
                      <div className="flex-1 min-w-0">
                        <p className="text-[12px] text-foreground/80 leading-snug line-clamp-2">{m.content}</p>
                        <p className="text-[10px] text-muted-foreground/40 mt-0.5 font-mono">
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
              </div>
            )}
          </div>

          {/* Search */}
          <SearchBox
            projectKey={useChatStore.getState().selectedProjectKey}
            onSelectResult={(convId) => {
              setActiveTab("chat");
              if (convId.startsWith("branch:")) {
                const branchId = convId.replace("branch:", "");
                useChatStore.getState().openThread(branchId);
              } else {
                selectConversation(convId);
              }
            }}
          />
        </div>
      </div>

      {/* ── Content zone — bordered, elevated ── */}
      <div className="flex-1 min-h-0 rounded-xl border-[0.5px] border-border bg-background overflow-hidden flex flex-col mx-2 mb-2">
        {effectiveTab === "chat" && <ChatPanel />}

        {effectiveTab === "artifacts" && (
          <div className="flex-1 overflow-y-auto p-5">
            <div className="max-w-4xl mx-auto">
              <ArtifactsPanel />
            </div>
          </div>
        )}

        {effectiveTab === "plan" && (
          <div className="flex-1 overflow-y-auto p-5">
            <div className="max-w-4xl mx-auto">
              {canonicalConvId && (
                <HarnessSummary
                  conversationId={canonicalConvId}
                  activeStage={activeStage}
                  onStageClick={setActiveStage}
                />
              )}
              <PlansPanel
                activeStage={activeStage}
                onPhaseChanged={(_id, phase) => {
                  const target = PHASE_TO_STAGE[phase];
                  if (target) setActiveStage(target);
                }}
                onSwitchToChat={() => setActiveTab("chat")}
              />
            </div>
          </div>
        )}

        {effectiveTab === "review" && (
          <div className="flex-1 overflow-y-auto p-5">
            <div className="max-w-4xl mx-auto">
              <h3 className="text-[10px] font-semibold text-muted-foreground/50 uppercase tracking-widest mb-3">Review</h3>
              <ReviewPanel />
            </div>
          </div>
        )}

        {effectiveTab === "test" && (
          <div className="flex-1 overflow-y-auto p-5">
            <div className="max-w-4xl mx-auto">
              <h3 className="text-[10px] font-semibold text-muted-foreground/50 uppercase tracking-widest mb-3">Test</h3>
              <TestPanel />
            </div>
          </div>
        )}

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

  // Close on outside click
  useEffect(() => {
    if (!open) return;
    const handler = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) { setOpen(false); }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [open]);

  const doSearch = useCallback((q: string) => {
    if (!projectKey || q.length < 2) { setResults([]); setOpen(false); return; }
    setLoading(true);
    invoke<SearchResult[]>("search_messages", { query: q, projectKey, limit: 15 })
      .then((r) => { setResults(r); setOpen(r.length > 0); })
      .catch(() => { setResults([]); })
      .finally(() => setLoading(false));
  }, [projectKey]);

  const handleChange = (value: string) => {
    setQuery(value);
    if (debounceRef.current) clearTimeout(debounceRef.current);
    debounceRef.current = setTimeout(() => doSearch(value), 300);
  };

  return (
    <div className="relative w-[200px]" ref={ref}>
      <div className="flex items-center gap-2 bg-background/50 hover:bg-background/70 border border-border/30 rounded-md px-2.5 py-1.5 transition-colors focus-within:border-ring/40">
        <Search className="w-3.5 h-3.5 text-muted-foreground/40 shrink-0" />
        <input
          ref={inputRef}
          value={query}
          onChange={(e) => handleChange(e.target.value)}
          onFocus={() => { if (results.length > 0) setOpen(true); }}
          placeholder="Search…"
          className="flex-1 bg-transparent text-[13px] font-medium outline-none text-foreground placeholder:text-muted-foreground/40"
        />
        {loading && <Loader2 className="w-3 h-3 animate-spin text-muted-foreground/30 shrink-0" />}
      </div>

      {open && results.length > 0 && (
        <div className="absolute right-0 top-full mt-1 w-[360px] max-h-[400px] bg-popover border border-border/40 rounded-lg shadow-xl overflow-hidden z-50">
          <div className="px-3 py-1.5 text-[11px] text-muted-foreground/50 border-b border-border/30">
            {results.length} results
          </div>
          <div className="overflow-y-auto max-h-[350px]">
            {results.map((r) => (
              <button
                key={r.messageId}
                onClick={() => { onSelectResult(r.conversationId); setOpen(false); setQuery(""); }}
                className="w-full text-left px-3 py-2 hover:bg-accent/50 transition-colors"
              >
                <div className="flex items-center gap-2 text-[11px]">
                  <span className="text-foreground/70 font-medium truncate flex-1">{r.conversationLabel}</span>
                  <span className="text-[9px] text-muted-foreground/40">{r.role}</span>
                  {r.persona && <span className="text-[9px] text-muted-foreground/30">{r.persona}</span>}
                </div>
                <p className="text-[11px] text-muted-foreground/60 leading-snug line-clamp-2 mt-0.5"
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
