import { useRef, useEffect, useState, useCallback } from "react";
import { cn } from "@/lib/utils";
import { useChatStore } from "@/stores/chatStore";
import { StatusBar } from "./StatusBar";
import { MessageItem } from "./MessageItem";
import { RoundtableView } from "./RoundtableView";
import { NewMessageInput } from "./NewMessageInput";
import { ChatObjectTabs } from "./ChatObjectTabs";
import { InlineRename } from "./InlineRename";
import { CreateRoundtableDialog } from "./CreateRoundtableDialog";
import { Users, MessageSquare } from "lucide-react";

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
  const renameConversation = useChatStore((s) => s.renameConversation);
  const deleteMessagePair = useChatStore((s) => s.deleteMessagePair);

  const bottomRef = useRef<HTMLDivElement>(null);
  const [view, setView] = useState<"stream" | "roundtable">("stream");
  const [rtDialogCheckpoint, setRtDialogCheckpoint] = useState<string | null>(null);

  const currentConv = conversations.find((c) => c.id === selectedConversationId);
  const isRoundtable = currentConv?.mode === "roundtable";

  // Auto-switch view based on conversation mode
  useEffect(() => {
    setView(isRoundtable ? "roundtable" : "stream");
  }, [isRoundtable, selectedConversationId]);

  // Auto-scroll on new messages
  // Auto-scroll — only when last message changes or new messages arrive
  const lastMsg = messages[messages.length - 1];
  const scrollKey = `${messages.length}:${lastMsg?.id}:${lastMsg?.status}`;
  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "auto" });
  }, [scrollKey]);

  if (!selectedConversationId) {
    return (
      <div className="flex flex-col flex-1 min-w-0 h-full bg-background items-center justify-center">
        <p className="text-muted-foreground text-sm">Select a conversation to start</p>
      </div>
    );
  }

  return (
    <div data-testid="chat-panel" className="flex flex-col flex-1 min-w-0 h-full bg-background">
      {/* Breadcrumb path */}
      <StatusBar />

      {/* Chat object tabs — main + open branch/RT */}
      <ChatObjectTabs />

      {/* Header */}
      <div className="flex items-center gap-3 px-4 h-10 border-b border-border/60 shrink-0">
        <div className="flex items-center gap-2 flex-1 min-w-0">
          {isRoundtable ? (
            <Users className="w-3.5 h-3.5 text-agent-gemini shrink-0" />
          ) : (
            <MessageSquare className="w-3.5 h-3.5 text-muted-foreground/60 shrink-0" />
          )}
          <h2 className="text-[13px] font-medium text-foreground truncate min-w-0">
            {selectedConversationId && currentConv ? (
              <InlineRename
                value={currentConv.customLabel ?? currentConv.label}
                onSave={(v) => renameConversation(selectedConversationId, v)}
              />
            ) : "Conversation"}
          </h2>
          <span className={cn(
            "text-[9px] font-medium px-1.5 py-0.5 rounded uppercase tracking-wider shrink-0",
            isRoundtable
              ? "text-agent-gemini/80 bg-agent-gemini/8"
              : "text-muted-foreground/60 bg-muted"
          )}>
            {isRoundtable ? "Roundtable" : "Chat"}
          </span>
        </div>

        {/* View toggle — only show for roundtable */}
        {isRoundtable && (
          <div className="flex items-center gap-0.5 bg-muted rounded-md p-0.5 shrink-0">
            <button
              onClick={() => setView("stream")}
              className={cn(
                "flex items-center gap-1 px-2 py-0.5 rounded text-[10px] font-medium transition-colors",
                view === "stream" ? "bg-background text-foreground shadow-sm" : "text-muted-foreground hover:text-foreground"
              )}
            >
              <MessageSquare className="w-2.5 h-2.5" />
              Stream
            </button>
            <button
              onClick={() => setView("roundtable")}
              className={cn(
                "flex items-center gap-1 px-2 py-0.5 rounded text-[10px] font-medium transition-colors",
                view === "roundtable" ? "bg-background text-foreground shadow-sm" : "text-muted-foreground hover:text-foreground"
              )}
            >
              <Users className="w-2.5 h-2.5" />
              Table
            </button>
          </div>
        )}
      </div>

      {/* Error banner */}
      {error && (
        <div className="px-4 py-1.5 bg-destructive/8 border-b border-destructive/15 text-destructive/80 text-[11px] shrink-0">
          {error}
        </div>
      )}

      {/* Scrollable area — messages + sticky input */}
      <div className="flex-1 overflow-y-auto">
        {view === "roundtable" && isRoundtable ? (
          <RoundtableView messages={messages} onBranch={(id) => createBranch(selectedConversationId, id)} />
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
                <MessageItem
                  key={msg.id}
                  message={msg}
                  grouped={grouped}
                  onBranch={!activeBranchId ? (id) => createBranch(selectedConversationId, id) : undefined}
                  onBranchRT={!activeBranchId ? (id) => setRtDialogCheckpoint(id) : undefined}
                  onMemo={!activeBranchId ? (id) => createMemo(id, msg.content) : undefined}
                  onFollowup={(engine, content) => sendFollowup(engine, "message", content)}
                  onDeletePair={(id) => deleteMessagePair(id)}
                  threadBranches={msgBranches.length > 0 ? msgBranches : undefined}
                  onOpenThread={!activeBranchId ? (branchId) => openThread(branchId) : undefined}
                />
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

        {/* Input — sticky at bottom, transparent background */}
        <div className="sticky bottom-0 z-20">
          <div className="pointer-events-none h-6 bg-gradient-to-t from-background to-transparent" />
          <div className="bg-background">
            <NewMessageInput />
          </div>
        </div>
      </div>
      <CreateRoundtableDialog
        open={rtDialogCheckpoint !== null}
        onClose={() => setRtDialogCheckpoint(null)}
        checkpointId={rtDialogCheckpoint}
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
