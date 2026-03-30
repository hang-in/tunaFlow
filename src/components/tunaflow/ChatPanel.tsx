import { useRef, useEffect, useState } from "react";
import { cn } from "@/lib/utils";
import { useChatStore } from "@/stores/chatStore";
import { MessageItem } from "./MessageItem";
import { RoundtableView } from "./RoundtableView";
import { NewMessageInput } from "./NewMessageInput";
import { CreateRoundtableDialog } from "./CreateRoundtableDialog";
import { SaveArtifactDialog } from "./SaveArtifactDialog";

export function ChatPanel() {
  // Selective subscriptions — only re-render when these specific fields change
  const messages = useChatStore((s) => s.messages);
  const branches = useChatStore((s) => s.branches);
  const selectedConversationId = useChatStore((s) => s.selectedConversationId);
  const conversations = useChatStore((s) => s.conversations);
  const runningThreadIds = useChatStore((s) => s.runningThreadIds);
  const error = useChatStore((s) => s.error);
  const projectLoading = useChatStore((s) => s.projectLoading);
  const activeBranchId = useChatStore((s) => s.activeBranchId);
  const createBranch = useChatStore((s) => s.createBranch);
  const createMemo = useChatStore((s) => s.createMemo);
  const openThread = useChatStore((s) => s.openThread);
  const sendFollowup = useChatStore((s) => s.sendFollowup);
  const deleteMessagePair = useChatStore((s) => s.deleteMessagePair);
  const scrollToMessageId = useChatStore((s) => s.scrollToMessageId);

  const bottomRef = useRef<HTMLDivElement>(null);
  const scrollAreaRef = useRef<HTMLDivElement>(null);
  const [view, setView] = useState<"stream" | "roundtable">("stream");
  const [rtDialogCheckpoint, setRtDialogCheckpoint] = useState<string | null>(null);
  const [highlightedMsgId, setHighlightedMsgId] = useState<string | null>(null);
  const [artifactContent, setArtifactContent] = useState<string | null>(null);

  const currentConv = conversations.find((c) => c.id === selectedConversationId);
  const isRoundtable = currentConv?.mode === "roundtable";

  // Create branch and immediately open in drawer
  const handleCreateBranch = async (checkpointId: string) => {
    if (!selectedConversationId) return;
    await createBranch(selectedConversationId, checkpointId);
    // Find the newly created branch by checkpointId (most recent)
    const { branches: freshBranches } = useChatStore.getState();
    const newBranch = freshBranches
      .filter((b) => b.checkpointId === checkpointId && b.status === "active")
      .sort((a, b) => b.createdAt - a.createdAt)[0];
    if (newBranch) {
      openThread(newBranch.id);
    }
  };

  // Auto-switch view based on conversation mode
  useEffect(() => {
    setView(isRoundtable ? "roundtable" : "stream");
  }, [isRoundtable, selectedConversationId]);

  // Scroll to specific message (memo click, etc.)
  useEffect(() => {
    if (!scrollToMessageId) return;
    const el = document.getElementById(`msg-${scrollToMessageId}`);
    if (el) {
      el.scrollIntoView({ behavior: "smooth", block: "center" });
      setHighlightedMsgId(scrollToMessageId);
      setTimeout(() => setHighlightedMsgId(null), 2000);
    }
    useChatStore.setState({ scrollToMessageId: null });
  }, [scrollToMessageId, messages]);

  // Auto-scroll on new messages
  const lastMsg = messages[messages.length - 1];
  const scrollKey = `${messages.length}:${lastMsg?.id}:${lastMsg?.status}`;
  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "auto" });
  }, [scrollKey]);

  if (!selectedConversationId) {
    return (
      <div className="flex flex-col flex-1 min-w-0 bg-background items-center justify-center">
        <p className="text-muted-foreground text-sm">Select a conversation to start</p>
      </div>
    );
  }

  return (
    <div data-testid="chat-panel" className="flex flex-col flex-1 min-w-0 overflow-hidden">

      {/* Error banner */}
      {error && (
        <div className="px-4 py-1.5 bg-destructive/8 border-b border-destructive/15 text-destructive/80 text-[11px] shrink-0">
          {error}
        </div>
      )}

      {/* Scrollable message area */}
      <div className="flex-1 overflow-y-auto">
        {view === "roundtable" && isRoundtable ? (
          <RoundtableView
            messages={messages}
            onBranch={(id) => handleCreateBranch(id)}
            onBranchRT={!activeBranchId ? (id) => setRtDialogCheckpoint(id) : undefined}
            onMemo={!activeBranchId ? (id) => createMemo(id, messages.find((m) => m.id === id)?.content ?? "") : undefined}
            onFollowup={(engine, content) => sendFollowup(engine, "message", content)}
            onSaveArtifact={(content) => setArtifactContent(content)}
          />
        ) : (
          <div className="py-3 space-y-0.5">
            {messages.length === 0 && !runningThreadIds.includes(selectedConversationId!) && (
              <div className="flex items-center justify-center h-32 text-muted-foreground text-sm">
                No messages yet
              </div>
            )}
            {messages.map((msg, idx) => {
              const prev = idx > 0 ? messages[idx - 1] : null;
              const grouped = !!prev
                && prev.role === msg.role
                && prev.engine === msg.engine
                && prev.persona === msg.persona
                && msg.status !== "streaming";
              const msgBranches = !activeBranchId
                ? branches.filter((b) => b.checkpointId === msg.id)
                : [];
              return (
                <div key={msg.id} id={`msg-${msg.id}`} className={cn(highlightedMsgId === msg.id && "ring-1 ring-primary/40 rounded-md transition-all duration-500")}>
                <MessageItem
                  message={msg}
                  grouped={grouped}
                  onBranch={!activeBranchId ? (id) => handleCreateBranch(id) : undefined}
                  onBranchRT={!activeBranchId ? (id) => setRtDialogCheckpoint(id) : undefined}
                  onMemo={!activeBranchId ? (id) => createMemo(id, msg.content) : undefined}
                  onFollowup={(engine, content) => sendFollowup(engine, "message", content)}
                  onDeletePair={(id) => deleteMessagePair(id)}
                  onSaveArtifact={(content) => setArtifactContent(content)}
                  threadBranches={msgBranches.length > 0 ? msgBranches : undefined}
                  onOpenThread={!activeBranchId ? (branchId) => openThread(branchId) : undefined}
                />
                </div>
              );
            })}
            {runningThreadIds.includes(selectedConversationId!) && messages[messages.length - 1]?.status !== "streaming" && (
              <div className="flex items-center gap-1 px-4 py-3 text-muted-foreground text-xs">
                <span className="typing-dot w-1.5 h-1.5 rounded-full bg-muted-foreground" />
                <span className="typing-dot w-1.5 h-1.5 rounded-full bg-muted-foreground" />
                <span className="typing-dot w-1.5 h-1.5 rounded-full bg-muted-foreground" />
              </div>
            )}
            <div ref={bottomRef} />
          </div>
        )}
      </div>

      {/* Input — fixed at bottom */}
      <div className="shrink-0">
        <NewMessageInput onCreateRT={() => setRtDialogCheckpoint("")} />
      </div>
      <CreateRoundtableDialog
        open={rtDialogCheckpoint !== null}
        onClose={() => setRtDialogCheckpoint(null)}
        checkpointId={rtDialogCheckpoint || null}
      />
      <SaveArtifactDialog
        open={artifactContent !== null}
        onClose={() => setArtifactContent(null)}
        initialContent={artifactContent ?? ""}
      />
      {/* Project loading modal */}
      {projectLoading && (
        <div className="fixed inset-0 z-[90] flex items-center justify-center">
          <div className="absolute inset-0 bg-black/30 backdrop-blur-[1px]" />
          <div className="relative bg-background border border-border/40 rounded-lg shadow-xl px-8 py-6 flex flex-col items-center gap-3">
            <div className="flex gap-1">
              <span className="typing-dot w-2 h-2 rounded-full bg-primary" />
              <span className="typing-dot w-2 h-2 rounded-full bg-primary" />
              <span className="typing-dot w-2 h-2 rounded-full bg-primary" />
            </div>
            <p className="text-[13px] font-medium text-foreground">{projectLoading}</p>
            <p className="text-[10px] text-muted-foreground/50">This may take a moment for large projects</p>
          </div>
        </div>
      )}
    </div>
  );
}
