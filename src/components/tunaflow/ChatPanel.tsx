import { useRef, useEffect, useState } from "react";
import { cn } from "@/lib/utils";
import { useChatStore } from "@/stores/chatStore";
import { StatusBar } from "./StatusBar";
import { MessageItem } from "./MessageItem";
import { RoundtableView } from "./RoundtableView";
import { NewMessageInput } from "./NewMessageInput";
import { BranchBar } from "./BranchBar";
import { ChatObjectTabs } from "./ChatObjectTabs";
import { InlineRename } from "./InlineRename";
import { Users, MessageSquare } from "lucide-react";

export function ChatPanel() {
  const {
    messages,
    branches,
    selectedConversationId,
    conversations,
    isRunning,
    runningThreadIds,
    error,
    activeBranchId,
    activeSkills,
    crossSessionIds,
    createBranch,
    createMemo,
    openThread,
    sendFollowup,
    renameConversation,
    deleteMessagePair,
  } = useChatStore();

  const bottomRef = useRef<HTMLDivElement>(null);
  const [view, setView] = useState<"stream" | "roundtable">("stream");

  const currentConv = conversations.find((c) => c.id === selectedConversationId);
  const isRoundtable = currentConv?.mode === "roundtable";

  // Auto-switch view based on conversation mode
  useEffect(() => {
    setView(isRoundtable ? "roundtable" : "stream");
  }, [isRoundtable, selectedConversationId]);

  // Auto-scroll on new messages
  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "auto" });
  }, [messages, runningThreadIds]);

  const branchObj = activeBranchId
    ? { id: activeBranchId, label: activeBranchId.slice(0, 12) + "..." }
    : undefined;

  if (!selectedConversationId) {
    return (
      <div className="flex flex-col flex-1 min-w-0 h-full bg-background items-center justify-center">
        <p className="text-muted-foreground text-sm">Select a conversation to start</p>
      </div>
    );
  }

  return (
    <div className="flex flex-col flex-1 min-w-0 h-full bg-background">
      {/* Status Bar */}
      <StatusBar
        mode={isRoundtable ? "roundtable" : "chat"}
        branch={branchObj}
        agentCount={3}
        activeSkills={activeSkills.length}
        crossSessionCount={crossSessionIds.length}
      />

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
            {isRoundtable ? "RT" : "Chat"}
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

      {/* Branch bar — relocated from right panel */}
      {(activeBranchId || branches.length > 0) && (
        <BranchBar />
      )}

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
            {messages.map((msg) => {
              const msgBranches = !activeBranchId
                ? branches.filter((b) => b.checkpointId === msg.id)
                : [];
              return (
                <MessageItem
                  key={msg.id}
                  message={msg}
                  onBranch={!activeBranchId ? (id) => createBranch(selectedConversationId, id) : undefined}
                  onBranchRT={!activeBranchId ? (id) => createBranch(selectedConversationId, id, undefined, "roundtable") : undefined}
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
    </div>
  );
}
