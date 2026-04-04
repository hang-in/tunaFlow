import { useRef, useEffect, useState, useMemo } from "react";
import { invoke } from "@tauri-apps/api/core";
import { X, Check, GitBranch, Users, Trash2, ChevronsLeft, ChevronsRight, ChevronRight, AlertTriangle } from "lucide-react";
import { ask } from "@tauri-apps/plugin-dialog";
import type { Message, Plan } from "@/types";
import { AgentAvatar } from "./AgentAvatar";
import { cn, normalizeEngine, AGENT_DOT_COLORS, AGENT_DISPLAY_NAMES, formatTimestamp } from "@/lib/utils";
import { useChatStore } from "@/stores/chatStore";
import { MessageItem } from "./MessageItem";
import { NewMessageInput } from "./NewMessageInput";
import { InlineRename } from "./InlineRename";
import { RoundtableView } from "./RoundtableView";
import { CreateRoundtableDialog } from "./CreateRoundtableDialog";
import { requestPlanRevision } from "@/lib/workflowOrchestration";
import * as planApi from "@/lib/api/plans";

export function BranchThreadPanel() {
  const {
    threadBranchId,
    threadBranchConvId,
    threadMessages,
    threadBranchLabel,
    threadParentMessage,
    selectedConversationId,
    runningThreadIds,
    closeThread,
    adoptBranch,
    sendThreadMessage,
    renameBranch,
    deleteBranch,
    createBranch,
    createMemo,
    openThread,
    loadBranches,
    branches,
    conversations,
  } = useChatStore();

  const bottomRef = useRef<HTMLDivElement>(null);
  const [rtDialogCheckpoint, setRtDialogCheckpoint] = useState<string | null>(null);
  const [linkedPlan, setLinkedPlan] = useState<Plan | null>(null);

  // Detect if this branch is linked to a plan (implementation branch)
  useEffect(() => {
    if (!threadBranchId) { setLinkedPlan(null); return; }
    planApi.findPlanByBranch(threadBranchId).then(setLinkedPlan).catch(() => setLinkedPlan(null));
  }, [threadBranchId]);

  const isImplBranch = linkedPlan?.implementationBranchId === threadBranchId;

  // Scroll to bottom only when new messages are added (not on content updates)
  const prevCountRef = useRef(threadMessages.length);
  useEffect(() => {
    if (threadMessages.length > prevCountRef.current) {
      bottomRef.current?.scrollIntoView({ behavior: "auto" });
    }
    prevCountRef.current = threadMessages.length;
  }, [threadMessages.length]);

  if (!threadBranchId) return null;

  const threadBranch = branches.find((b) => b.id === threadBranchId);
  // Detect subtask discussion branch (not impl/review, label starts with known prefix)
  const isSubtaskDiscussion = threadBranch && !isImplBranch &&
    linkedPlan?.reviewBranchId !== threadBranchId &&
    (threadBranch.label.startsWith("Subtask") || threadBranch.label.startsWith("검토") || threadBranch.label.startsWith("작업지시"));
  const isRT = threadBranch?.mode === "roundtable";
  const isReadOnly = threadBranch?.status === "adopted" || threadBranch?.status === "archived";

  // Build full navigation chain: [Main, ...ancestors, current, ...descendants]
  const { fullChain, currentIdx, windowStart, visibleChain, hasLeftOverflow, hasRightOverflow } = useMemo(() => {
    const chain: { id: string | null; label: string; isRT?: boolean }[] = [];
    // Walk up: ancestors
    let cur = threadBranch;
    while (cur?.parentBranchId) {
      const parent = branches.find((b) => b.id === cur!.parentBranchId);
      if (!parent) break;
      chain.unshift({ id: parent.id, label: parent.customLabel ?? parent.label, isRT: parent.mode === "roundtable" });
      cur = parent;
    }
    // Root conversation
    const conv = selectedConversationId ? conversations.find((c) => c.id === selectedConversationId) : null;
    chain.unshift({ id: null, label: conv?.customLabel ?? conv?.label ?? "Main" });
    // Current
    chain.push({ id: threadBranchId, label: threadBranchLabel ?? threadBranchId, isRT });
    const idx = chain.length - 1;
    // Walk down: descendants (follow most recent child at each level)
    let descendantId: string | null = threadBranchId;
    while (descendantId) {
      const children = branches
        .filter((b) => b.parentBranchId === descendantId)
        .sort((a, b) => b.createdAt - a.createdAt);
      if (children.length === 0) break;
      const child = children[0];
      chain.push({ id: child.id, label: child.customLabel ?? child.label, isRT: child.mode === "roundtable" });
      descendantId = child.id;
    }
    // Visible window: 2 before + current + 2 after
    const wStart = Math.max(0, idx - 2);
    const wEnd = Math.min(chain.length - 1, idx + 2);
    return {
      fullChain: chain,
      currentIdx: idx,
      windowStart: wStart,
      visibleChain: chain.slice(wStart, wEnd + 1),
      hasLeftOverflow: wStart > 0,
      hasRightOverflow: wEnd < chain.length - 1,
    };
  }, [threadBranchId, threadBranchLabel, isRT, branches, selectedConversationId, conversations]);

  const handleAdopt = async () => {
    if (!selectedConversationId) return;
    await adoptBranch(threadBranchId, selectedConversationId);
    // Navigate to parent branch or close
    if (threadBranch?.parentBranchId) {
      openThread(threadBranch.parentBranchId);
    } else {
      closeThread();
    }
  };

  // Create sub-branch and immediately switch drawer to it
  const handleCreateSubBranch = async (checkpointId: string) => {
    if (!threadBranchConvId || !threadBranchId) return;
    await createBranch(threadBranchConvId, checkpointId, undefined, undefined, threadBranchId);
    const { branches: freshBranches } = useChatStore.getState();
    const newBranch = freshBranches
      .filter((b) => b.checkpointId === checkpointId && b.parentBranchId === threadBranchId && b.status === "active")
      .sort((a, b) => b.createdAt - a.createdAt)[0];
    if (newBranch) {
      openThread(newBranch.id);
    }
  };

  const handleBack = () => {
    if (currentIdx > 0) {
      const prev = fullChain[currentIdx - 1];
      prev.id ? openThread(prev.id) : closeThread();
    } else {
      closeThread();
    }
  };

  // Parent message meta
  const parentEngine = threadParentMessage?.engine;
  const parentKnown = normalizeEngine(parentEngine ?? "");
  const parentDotColor = parentKnown ? AGENT_DOT_COLORS[parentKnown] : "bg-muted-foreground/40";
  const parentName = threadParentMessage
    ? threadParentMessage.role === "user"
      ? "You"
      : threadParentMessage.persona ?? (parentKnown ? AGENT_DISPLAY_NAMES[parentKnown] : "Assistant")
    : null;

  return (
    <div className="flex flex-col w-full h-full bg-background">
      {/* Header — navigator + actions */}
      <div className="flex items-center gap-1 px-3 h-10 shrink-0">
        {/* Badge */}
        <span className={cn("text-[8px] font-medium px-1.5 py-0.5 rounded uppercase tracking-wider shrink-0",
          isRT ? "text-agent-gemini/60 bg-agent-gemini/8" : "text-primary/50 bg-primary/6"
        )}>
          {isRT ? "RT" : "Branch"}
        </span>
        <GitLinkBadge
          branchId={threadBranchId}
          gitBranch={threadBranch?.gitBranch ?? null}
          isReadOnly={isReadOnly}
        />
        {isReadOnly && (
          <span className={cn("text-[8px] font-medium px-1 py-0.5 rounded uppercase tracking-wider shrink-0",
            threadBranch?.status === "adopted" ? "text-status-approved/60 bg-status-approved/8" : "text-muted-foreground/40 bg-muted"
          )}>
            {threadBranch?.status}
          </span>
        )}

        {/* Navigator — centered */}
        <div className="flex-1 flex items-center justify-center gap-0.5 min-w-0">
          {/* << jump to start */}
          {hasLeftOverflow && (
            <button
              onClick={() => { const first = fullChain[0]; first.id ? openThread(first.id) : closeThread(); }}
              className="p-0.5 rounded text-muted-foreground/30 hover:text-foreground hover:bg-accent/50 transition-colors shrink-0"
              title={fullChain[0].label}
            >
              <ChevronsLeft className="w-3.5 h-3.5" />
            </button>
          )}

          {/* Visible chain */}
          {visibleChain.map((item, i) => {
            const globalIdx = windowStart + i;
            const isCurrent = globalIdx === currentIdx;
            return (
              <span key={globalIdx} className="flex items-center gap-0.5 shrink-0">
                {i > 0 && <ChevronRight className="w-3 h-3 text-muted-foreground/20 shrink-0" />}
                <button
                  onClick={() => {
                    if (isCurrent) return;
                    item.id ? openThread(item.id) : closeThread();
                  }}
                  className={cn(
                    "text-[11px] truncate max-w-[80px] rounded px-1 py-0.5 transition-colors",
                    isCurrent
                      ? "text-foreground font-medium bg-accent"
                      : "text-muted-foreground/50 hover:text-foreground hover:bg-accent/50"
                  )}
                  title={item.label}
                >
                  {item.label}
                </button>
              </span>
            );
          })}

          {/* >> jump to end */}
          {hasRightOverflow && (
            <button
              onClick={() => { const last = fullChain[fullChain.length - 1]; last.id && openThread(last.id); }}
              className="p-0.5 rounded text-muted-foreground/30 hover:text-foreground hover:bg-accent/50 transition-colors shrink-0"
              title={fullChain[fullChain.length - 1].label}
            >
              <ChevronsRight className="w-3.5 h-3.5" />
            </button>
          )}
        </div>

        {/* Actions */}
        <div className="flex items-center gap-0.5 shrink-0">
          {/* Plan revision request — shown only for implementation branches */}
          {isImplBranch && !isReadOnly && linkedPlan && (
            <PlanRevisionActions
              plan={linkedPlan}
              threadMessages={threadMessages}
              threadBranchConvId={threadBranchConvId}
            />
          )}
          {/* Subtask discussion: [완료] button — archives branch + minor++ */}
          {isSubtaskDiscussion && !isReadOnly && (
            <button
              onClick={async () => {
                if (!threadBranchId) return;
                await invoke("archive_branch", { id: threadBranchId });
                const convId = selectedConversationId ?? "";
                if (convId) await loadBranches(convId);
                closeThread();
              }}
              className="flex items-center gap-0.5 px-1.5 py-0.5 rounded text-[9px] font-medium text-status-approved/70 hover:bg-status-approved/8 transition-colors"
              title="대화 완료 → Branch 아카이브"
            >
              <Check className="w-2.5 h-2.5" />완료
            </button>
          )}
          {!isReadOnly && threadBranch?.checkpointId && (
            <button onClick={handleAdopt} title="Adopt" className="flex items-center gap-0.5 px-1.5 py-0.5 rounded text-[9px] font-medium text-primary/70 hover:bg-primary/8 transition-colors">
              <Check className="w-2.5 h-2.5" /> Adopt
            </button>
          )}
          {!isReadOnly && (
            <button onClick={async () => {
              // Check for adopted/archived descendants
              const descendants: string[] = [];
              const queue = [threadBranchId];
              while (queue.length) {
                const id = queue.shift()!;
                const children = branches.filter((b) => b.parentBranchId === id);
                for (const c of children) { descendants.push(c.id); queue.push(c.id); }
              }
              const hasAdopted = branches.some((b) => descendants.includes(b.id) && (b.status === "adopted" || b.status === "archived"));
              const message = hasAdopted
                ? `"${threadBranchLabel}" 브랜치에 채택된 결과가 포함되어 있습니다.\n하위 브랜치와 이력이 모두 삭제됩니다. 계속하시겠습니까?`
                : `"${threadBranchLabel}" 브랜치를 삭제하시겠습니까?`;
              const yes = await ask(message, { title: "브랜치 삭제", kind: "warning" });
              if (yes) {
                closeThread();
                deleteBranch(threadBranchId);
              }
            }} title="Delete" className="p-1 rounded text-muted-foreground/50 hover:text-destructive hover:bg-destructive/10 transition-colors">
              <Trash2 className="w-3 h-3" />
            </button>
          )}
          <button onClick={closeThread} title="Close" className="p-1 rounded text-muted-foreground/50 hover:text-foreground hover:bg-accent transition-colors">
            <X className="w-3 h-3" />
          </button>
        </div>
      </div>

      {/* Parent anchor */}
      {threadParentMessage && (
        <div className="flex gap-2.5 px-3.5 py-2 bg-accent/10 shrink-0">
          <div className="shrink-0 mt-0.5">
            <AgentAvatar engine={threadParentMessage.engine} isUser={threadParentMessage.role === "user"} size="md" />
          </div>
          <div className="flex-1 min-w-0">
            <div className="flex items-center gap-1.5 mb-0.5">
              <span className="inline-flex items-center gap-1">
                {!threadParentMessage.role || threadParentMessage.role !== "user" && (
                  <span className={cn("w-1.5 h-1.5 rounded-full", parentDotColor)} />
                )}
                <span className="text-[10px] font-medium text-foreground/70">{parentName}</span>
              </span>
              <span className="text-[8px] text-muted-foreground/40 font-mono">
                {formatTimestamp(threadParentMessage.timestamp)}
              </span>
            </div>
            <p className="text-[11px] text-muted-foreground/50 leading-snug line-clamp-2">
              {threadParentMessage.content.slice(0, 200)}
            </p>
          </div>
        </div>
      )}

      {/* Thread messages */}
      <div className="flex-1 overflow-y-auto">
        {threadMessages.length === 0 ? (
          <div className="flex flex-col items-center justify-center h-32 text-muted-foreground/40 text-[12px] gap-1.5">
            {isRT ? <Users className="w-4 h-4" /> : <GitBranch className="w-4 h-4" />}
            <p>{isRT ? "No roundtable messages yet" : "No replies yet"}</p>
          </div>
        ) : isRT ? (
          <>
            <RoundtableView
              messages={threadMessages}
              conversationId={threadBranchConvId ?? undefined}
              onBranch={!isReadOnly ? (id) => handleCreateSubBranch(id) : undefined}
              onBranchRT={!isReadOnly ? (id) => setRtDialogCheckpoint(id) : undefined}
              onMemo={!isReadOnly ? (id) => createMemo(id, threadMessages.find((m) => m.id === id)?.content ?? "") : undefined}
              onFollowup={!isReadOnly ? (engine, content) => sendThreadMessage(content, engine as any) : undefined}
              onDelete={!isReadOnly ? async (id) => {
                await invoke("delete_message_pair", { messageId: id });
                if (threadBranchConvId) {
                  const msgs = await invoke<Message[]>("list_messages", { conversationId: threadBranchConvId });
                  useChatStore.setState({ threadMessages: msgs });
                }
              } : undefined}
            />
            <div ref={bottomRef} />
          </>
        ) : (
          <div className="py-2 space-y-0.5">
            {threadMessages.map((msg, idx) => {
              const prev = idx > 0 ? threadMessages[idx - 1] : null;
              const grouped = !!prev
                && prev.role === msg.role
                && prev.engine === msg.engine
                && prev.persona === msg.persona
                && msg.status !== "streaming";
              const msgBranches = branches.filter((b) => b.checkpointId === msg.id);
              return (
                <MessageItem
                  key={msg.id}
                  message={msg}
                  grouped={grouped}
                  onBranch={!isReadOnly ? (id) => handleCreateSubBranch(id) : undefined}
                  onBranchRT={!isReadOnly ? (id) => setRtDialogCheckpoint(id) : undefined}
                  onMemo={!isReadOnly ? (id) => createMemo(id, msg.content) : undefined}
                  onFollowup={!isReadOnly ? (engine, content) => sendThreadMessage(content, engine as any) : undefined}
                  threadBranches={msgBranches.length > 0 ? msgBranches : undefined}
                  onOpenThread={(branchId) => openThread(branchId)}
                />
              );
            })}
            {runningThreadIds.length > 0 && threadMessages[threadMessages.length - 1]?.status !== "streaming" && (
              <div className="flex items-center gap-1 px-4 py-2 text-muted-foreground text-xs">
                <span className="typing-dot w-1.5 h-1.5 rounded-full bg-muted-foreground" />
                <span className="typing-dot w-1.5 h-1.5 rounded-full bg-muted-foreground" />
                <span className="typing-dot w-1.5 h-1.5 rounded-full bg-muted-foreground" />
              </div>
            )}
            <div ref={bottomRef} />
          </div>
        )}
      </div>

      {/* Input — hidden for read-only branches */}
      {!isReadOnly && <NewMessageInput threadMode />}

      {/* RT creation dialog for branching from thread messages */}
      <CreateRoundtableDialog
        open={rtDialogCheckpoint !== null}
        onClose={() => setRtDialogCheckpoint(null)}
        checkpointId={rtDialogCheckpoint}
      />
    </div>
  );
}

// ─── Plan Revision Actions ─────────────────────────────────────────────────

function PlanRevisionActions({ plan, threadMessages, threadBranchConvId }: {
  plan: Plan; threadMessages: Message[]; threadBranchConvId: string | null;
}) {
  const [mode, setMode] = useState<"idle" | "select" | "busy">("idle");
  const [engine, setEngine] = useState("claude");

  if (mode === "busy") return <span className="text-[9px] text-amber-600/50">전송 중...</span>;

  if (mode === "select") return (
    <div className="flex items-center gap-1">
      <select value={engine} onChange={(e) => setEngine(e.target.value)} className="text-[9px] bg-input border border-border rounded px-1 py-0.5 outline-none">
        {["claude", "codex", "gemini", "opencode", "ollama"].map((e) => <option key={e} value={e}>{e}</option>)}
      </select>
      <button
        onClick={async () => {
          setMode("busy");
          try {
            const msgs = threadMessages.length > 0 ? threadMessages : await invoke<Message[]>("list_messages", { conversationId: threadBranchConvId! });
            const { sendWithEngine } = useChatStore.getState();
            await requestPlanRevision(plan, msgs, engine, async (eng, prompt, sys) => {
              await sendWithEngine(eng, prompt, undefined, sys);
            });
          } catch (e) { console.error("[revision]", e); }
          setMode("idle");
        }}
        className="text-[9px] px-1.5 py-0.5 rounded bg-amber-500/10 text-amber-600 hover:bg-amber-500/20 transition-colors"
      >전송</button>
      <button onClick={() => setMode("idle")} className="text-[9px] text-muted-foreground hover:text-foreground">취소</button>
    </div>
  );

  return (
    <button
      onClick={() => setMode("select")}
      title="계획 수정 요청 — Architect에게 전달"
      className="flex items-center gap-0.5 px-1.5 py-0.5 rounded text-[9px] font-medium text-amber-600/60 hover:text-amber-600 hover:bg-amber-500/10 transition-colors"
    >
      <AlertTriangle className="w-2.5 h-2.5" />계획 수정
    </button>
  );
}

// ─── Git Link Badge (editable) ──────────────────────────────────────────────

function GitLinkBadge({ branchId, gitBranch, isReadOnly }: { branchId: string; gitBranch: string | null; isReadOnly: boolean }) {
  const [editing, setEditing] = useState(false);
  const [value, setValue] = useState(gitBranch ?? "");
  const [actionMsg, setActionMsg] = useState<string | null>(null);
  const linkGitBranch = useChatStore((s) => s.linkGitBranch);

  const handleGitAction = async (action: "create" | "checkout") => {
    const { invoke } = await import("@tauri-apps/api/core");
    try {
      const result = await invoke<string>(action === "create" ? "create_git_branch" : "checkout_git_branch", { branchId });
      setActionMsg(result);
      setTimeout(() => setActionMsg(null), 3000);
    } catch (e) {
      setActionMsg(String(e));
      setTimeout(() => setActionMsg(null), 4000);
    }
  };

  if (editing && !isReadOnly) {
    return (
      <input
        autoFocus
        value={value}
        onChange={(e) => setValue(e.target.value)}
        onBlur={() => { linkGitBranch(branchId, value.trim() || null); setEditing(false); }}
        onKeyDown={(e) => { if (e.key === "Enter") { linkGitBranch(branchId, value.trim() || null); setEditing(false); } if (e.key === "Escape") setEditing(false); }}
        placeholder="git branch"
        className="text-[8px] font-mono text-muted-foreground/50 bg-muted/50 px-1.5 py-0.5 rounded outline-none border border-ring/30 w-[100px] shrink-0"
      />
    );
  }

  if (actionMsg) {
    return <span className="text-[8px] text-primary/60 shrink-0 truncate max-w-[150px]">{actionMsg}</span>;
  }

  if (gitBranch) {
    return (
      <span className="flex items-center gap-0.5 shrink-0">
        <button
          onClick={() => !isReadOnly && setEditing(true)}
          className={cn("text-[8px] font-mono text-muted-foreground/30 px-1 py-0.5 rounded bg-muted/50 truncate max-w-[80px]",
            !isReadOnly && "hover:text-muted-foreground/60 hover:bg-muted cursor-pointer")}
          title={isReadOnly ? gitBranch : `Click to edit: ${gitBranch}`}
        >
          {gitBranch}
        </button>
        {!isReadOnly && (
          <>
            <button onClick={() => handleGitAction("create")} title="Create git branch"
              className="text-[7px] text-muted-foreground/20 hover:text-primary/60 transition-colors">+</button>
            <button onClick={() => handleGitAction("checkout")} title="Checkout git branch"
              className="text-[7px] text-muted-foreground/20 hover:text-primary/60 transition-colors">↗</button>
          </>
        )}
      </span>
    );
  }

  if (!isReadOnly) {
    return (
      <button
        onClick={() => setEditing(true)}
        className="text-[8px] text-muted-foreground/20 hover:text-muted-foreground/40 px-1 py-0.5 rounded hover:bg-muted/50 shrink-0 transition-colors"
        title="Link git branch"
      >
        + git
      </button>
    );
  }

  return null;
}
