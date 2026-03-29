import { useRef, useEffect, useState } from "react";
import { X, Check, GitBranch, Users, Trash2 } from "lucide-react";
import { ask } from "@tauri-apps/plugin-dialog";
import { AgentAvatar } from "./AgentAvatar";
import { cn, normalizeEngine, AGENT_DOT_COLORS, AGENT_DISPLAY_NAMES, formatTimestamp } from "@/lib/utils";
import { useChatStore } from "@/stores/chatStore";
import { MessageItem } from "./MessageItem";
import { NewMessageInput } from "./NewMessageInput";
import { InlineRename } from "./InlineRename";
import { RoundtableView } from "./RoundtableView";
import { CreateRoundtableDialog } from "./CreateRoundtableDialog";

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
    branches,
    conversations,
  } = useChatStore();

  const bottomRef = useRef<HTMLDivElement>(null);
  const [rtDialogCheckpoint, setRtDialogCheckpoint] = useState<string | null>(null);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [threadMessages]);

  if (!threadBranchId) return null;

  const threadBranch = branches.find((b) => b.id === threadBranchId);
  const isRT = threadBranch?.mode === "roundtable";
  const isReadOnly = threadBranch?.status === "adopted" || threadBranch?.status === "archived";

  // Build full parent chain for breadcrumb
  const parentChain: { id: string | null; label: string }[] = [];
  {
    // Walk up parentBranchId chain
    let current = threadBranch;
    while (current?.parentBranchId) {
      const parent = branches.find((b) => b.id === current!.parentBranchId);
      if (!parent) break;
      parentChain.unshift({ id: parent.id, label: parent.customLabel ?? parent.label });
      current = parent;
    }
    // Add "Main" as root
    const conv = selectedConversationId ? conversations.find((c) => c.id === selectedConversationId) : null;
    parentChain.unshift({ id: null, label: conv?.customLabel ?? conv?.label ?? "Main" });
  }
  const hasParent = parentChain.length > 0;

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
    // Navigate to immediate parent (last item in parentChain)
    const immediateParent = parentChain[parentChain.length - 1];
    if (immediateParent?.id) {
      openThread(immediateParent.id);
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
      {/* Header with full breadcrumb */}
      <div className="flex items-center gap-1 px-3 h-10 shrink-0">
        {/* Full breadcrumb path */}
        <div className="flex items-center gap-0.5 min-w-0 overflow-hidden shrink">
          {parentChain.map((crumb, i) => (
            <span key={i} className="flex items-center gap-0.5 shrink-0">
              {i > 0 && <span className="text-[10px] text-muted-foreground/30">/</span>}
              <button
                onClick={() => crumb.id ? openThread(crumb.id) : closeThread()}
                className="text-[11px] text-muted-foreground/50 hover:text-foreground truncate max-w-[60px] transition-colors"
                title={crumb.label}
              >
                {crumb.label}
              </button>
            </span>
          ))}
          <span className="text-[10px] text-muted-foreground/30 shrink-0">/</span>
        </div>

        {/* Current branch */}
        <div className="flex items-center gap-1.5 flex-1 min-w-0">
          {isRT
            ? <Users className="w-3.5 h-3.5 text-agent-gemini/60 shrink-0" />
            : <GitBranch className="w-3 h-3 text-primary/60 shrink-0" />}
          <h2 className="text-[12px] font-medium text-foreground truncate min-w-0">
            <InlineRename value={threadBranchLabel ?? ""} onSave={(v) => renameBranch(threadBranchId, v)} inputClassName="text-[11px] w-full" />
          </h2>
          <span className={cn("text-[8px] font-medium px-1 py-0.5 rounded uppercase tracking-wider shrink-0",
            isRT ? "text-agent-gemini/60 bg-agent-gemini/8" : "text-primary/50 bg-primary/6"
          )}>
            {isRT ? "RT" : "Branch"}
          </span>
          {isReadOnly && (
            <span className={cn("text-[8px] font-medium px-1 py-0.5 rounded uppercase tracking-wider shrink-0",
              threadBranch?.status === "adopted" ? "text-status-approved/60 bg-status-approved/8" : "text-muted-foreground/40 bg-muted"
            )}>
              {threadBranch?.status}
            </span>
          )}
        </div>

        {/* Actions */}
        <div className="flex items-center gap-0.5 shrink-0">
          {!isReadOnly && threadBranch?.checkpointId && (
            <button onClick={handleAdopt} title="Adopt" className="flex items-center gap-0.5 px-1.5 py-0.5 rounded text-[9px] font-medium text-primary/70 hover:bg-primary/8 transition-colors">
              <Check className="w-2.5 h-2.5" /> Adopt
            </button>
          )}
          {!isReadOnly && (
            <button onClick={async () => {
              const yes = await ask(`"${threadBranchLabel}" 브랜치를 삭제하시겠습니까?`, { title: "브랜치 삭제", kind: "warning" });
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
