import { useRef, useEffect, useState, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { Virtuoso, VirtuosoHandle } from "react-virtuoso";
import { cn } from "@/lib/utils";
import type { Message, Branch } from "@/types";
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

  const virtuosoRef = useRef<VirtuosoHandle>(null);
  const virtuosoScrollerRef = useRef<HTMLElement | null>(null);
  // Ref to avoid recreating itemContent callback on every messages change
  const messagesRef = useRef(messages);
  messagesRef.current = messages;
  const branchesRef = useRef(branches);
  branchesRef.current = branches;
  // Ref to always call the latest handleCreateBranch (avoids stale closure in memoized renderMessage)
  const handleCreateBranchRef = useRef<(checkpointId: string) => void>(() => {});
  const [view, setView] = useState<"stream" | "roundtable">("stream");
  const [rtDialogCheckpoint, setRtDialogCheckpoint] = useState<string | null>(null);
  const [highlightedMsgId, setHighlightedMsgId] = useState<string | null>(null);
  const [artifactContent, setArtifactContent] = useState<string | null>(null);
  // Scroll-to-bottom button — use ref to track state without causing Virtuoso re-render loops
  const isAtBottomRef = useRef(true);
  const [showScrollBtn, setShowScrollBtn] = useState(false);

  const handleAtBottomStateChange = useCallback((atBottom: boolean) => {
    if (isAtBottomRef.current !== atBottom) {
      isAtBottomRef.current = atBottom;
      setShowScrollBtn(!atBottom);
    }
  }, []);

  const scrollToBottom = useCallback(() => {
    const scroller = virtuosoScrollerRef.current;
    if (scroller) {
      scroller.scrollTo({ top: scroller.scrollHeight, behavior: "smooth" });
    } else {
      // fallback
      virtuosoRef.current?.scrollToIndex({ index: "LAST", behavior: "smooth" });
    }
  }, []);

  const currentConv = conversations.find((c) => c.id === selectedConversationId);
  const isRoundtable = currentConv?.mode === "roundtable";

  // Create branch and immediately open in drawer
  const handleCreateBranch = async (checkpointId: string) => {
    if (!selectedConversationId) { console.warn("[branch] no selectedConversationId"); return; }
    try {
      await createBranch(selectedConversationId, checkpointId);
      const { branches: freshBranches } = useChatStore.getState();
      const newBranch = freshBranches
        .filter((b) => b.checkpointId === checkpointId && b.status === "active")
        .sort((a, b) => b.createdAt - a.createdAt)[0];
      if (newBranch) {
        openThread(newBranch.id);
      } else {
        console.warn("[branch] no matching branch found for checkpoint:", checkpointId);
      }
    } catch (err) {
      console.error("[branch] createBranch failed:", err);
    }
  };
  // Keep ref up-to-date on every render so memoized renderMessage always calls the latest version
  handleCreateBranchRef.current = handleCreateBranch;

  // Auto-switch view based on conversation mode
  useEffect(() => {
    setView(isRoundtable ? "roundtable" : "stream");
  }, [isRoundtable, selectedConversationId]);

  // Auto-recover from missed agent:completed events
  // If Idle (not running) but last message is still "streaming", reload from DB.
  //
  // Extra guards (s38 메시지 사라짐 재현 대응):
  //   1. RuntimeStatusBar 의 orphan-recovery 가 false-positive 로 runningThreadIds
  //      에서 id 를 털어버리면 여기서 `isIdle = true` 로 오인해 in-flight 스트림을
  //      DB 의 미완 상태로 덮어썼었다. 10s 로 grace 를 늘리고, 타이머가 실제로
  //      발동하는 시점에 다시 store 를 확인해 여전히 idle 인지 재검증.
  //   2. DB 에서 읽어온 최신 last msg 도 streaming 이면 아직 persist 전이라
  //      overwrite 하지 않고 그냥 돌아간다 (다음 주기 orphan-recovery 가 처리).
  useEffect(() => {
    if (!selectedConversationId) return;
    const isIdle = !runningThreadIds.includes(selectedConversationId);
    const lastMsg = messages[messages.length - 1];
    if (isIdle && lastMsg?.status === "streaming") {
      const timer = setTimeout(async () => {
        const store = useChatStore.getState();
        if (store.selectedConversationId !== selectedConversationId) return;
        if (store.runningThreadIds.includes(selectedConversationId)) return;
        const current = store.messages[store.messages.length - 1];
        if (current?.status !== "streaming") return;
        try {
          const fresh = await invoke<Message[]>("list_messages", { conversationId: selectedConversationId });
          const freshLast = fresh[fresh.length - 1];
          // DB 에도 streaming 이면 아직 persist 전. 덮어쓰면 in-flight chunk 손실.
          if (freshLast?.status === "streaming") return;
          // 핵심 가드: store 가 fresh 보다 길면 optimistic user + thinking
          // placeholder 가 아직 DB 에 insert 되지 않은 상태. orphan-recovery 의
          // false-positive 가 runningThreadIds 를 털어버렸지만 실제로는 write
          // lock 경합 중인 상황. 덮어쓰면 현재 turn 의 user/assistant 가 통째로
          // 사라진다. 2026-04-21 debug-messages 191→189 재현으로 확인.
          if (fresh.length < store.messages.length) return;
          useChatStore.setState({ messages: fresh });
        } catch {}
      }, 10000); // 10s grace — orphan-recovery 와 경쟁 회피
      return () => clearTimeout(timer);
    }
  }, [runningThreadIds, messages, selectedConversationId]);

  // Scroll to specific message (memo click, etc.)
  useEffect(() => {
    if (!scrollToMessageId) return;
    const idx = messages.findIndex((m) => m.id === scrollToMessageId);
    if (idx >= 0) {
      virtuosoRef.current?.scrollToIndex({ index: idx, align: "center", behavior: "smooth" });
      setHighlightedMsgId(scrollToMessageId);
      setTimeout(() => setHighlightedMsgId(null), 2000);
    }
    useChatStore.setState({ scrollToMessageId: null });
  }, [scrollToMessageId, messages]);

  // Scroll to bottom when conversation changes (switch or DB reload after completion)
  const prevConvRef = useRef(selectedConversationId);
  const prevCountRef = useRef(messages.length);
  useEffect(() => {
    const convChanged = prevConvRef.current !== selectedConversationId;
    const bulkLoad = !convChanged && messages.length > 0 && prevCountRef.current === 0;
    prevConvRef.current = selectedConversationId;
    prevCountRef.current = messages.length;
    if ((convChanged || bulkLoad) && messages.length > 0) {
      // Double rAF: first frame lets Virtuoso register new totalCount,
      // second frame runs after Virtuoso has laid out the new items.
      requestAnimationFrame(() => {
        requestAnimationFrame(() => {
          virtuosoRef.current?.scrollToIndex({ index: "LAST", behavior: "auto" });
        });
      });
    }
  }, [selectedConversationId, messages.length]);

  // Follow output: auto-scroll to bottom only when already at bottom
  // Returns "auto" instead of "smooth" to avoid animation-triggered re-renders
  const followOutput = useCallback(
    (isAtBottom: boolean) => (isAtBottom ? "auto" : false) as "auto" | false,
    []
  );

  // Render a single message item
  const renderMessage = useCallback(
    (index: number, _data: unknown, { msgs, brs }: { msgs: Message[]; brs: Branch[] }) => {
      const msg = msgs[index];
      if (!msg) return null;
      const prev = index > 0 ? msgs[index - 1] : null;
      const grouped =
        !!prev &&
        prev.role === msg.role &&
        prev.engine === msg.engine &&
        prev.persona === msg.persona &&
        msg.status !== "streaming";
      const msgBranches = !activeBranchId
        ? brs.filter((b) => b.checkpointId === msg.id)
        : [];
      return (
        <div
          id={`msg-${msg.id}`}
          className={cn(
            "max-w-4xl mx-auto",
            highlightedMsgId === msg.id &&
              "ring-1 ring-primary/40 rounded-md transition-all duration-500"
          )}
        >
          <MessageItem
            message={msg}
            grouped={grouped}
            onBranch={!activeBranchId ? (id) => handleCreateBranchRef.current(id) : undefined}
            onBranchRT={
              !activeBranchId ? (id) => setRtDialogCheckpoint(id) : undefined
            }
            onMemo={
              !activeBranchId ? (id) => createMemo(id, msg.content) : undefined
            }
            onFollowup={(engine, content) => sendFollowup(engine, "message", content)}
            onDeletePair={(id) => deleteMessagePair(id)}
            onSaveArtifact={(content) => setArtifactContent(content)}
            threadBranches={msgBranches.length > 0 ? msgBranches : undefined}
            onOpenThread={
              !activeBranchId ? (branchId) => openThread(branchId) : undefined
            }
          />
        </div>
      );
    },
    // Stable deps — messages/branches accessed via refs, not direct deps
    [activeBranchId, highlightedMsgId, createMemo, sendFollowup, deleteMessagePair, openThread]
  );

  if (!selectedConversationId) {
    return (
      <div className="flex flex-col flex-1 min-w-0 bg-background items-center justify-center">
        <p className="text-muted-foreground text-sm">Select a conversation to start</p>
      </div>
    );
  }

  const isRunning = runningThreadIds.includes(selectedConversationId);
  // Guard: don't show typing indicator when messages are empty (e.g. during conversation switch
  // or after delete re-render) — prevents dots appearing at wrong position in empty list.
  const showTyping =
    isRunning && messages.length > 0 && messages[messages.length - 1]?.status !== "streaming";

  return (
    <div
      data-testid="chat-panel"
      className="flex flex-col flex-1 min-w-0 overflow-hidden"
    >
      {/* Error banner */}
      {error && (
        <div className="px-4 py-1.5 bg-destructive/8 border-b border-destructive/15 text-destructive/80 text-tf-sm shrink-0">
          {error}
        </div>
      )}

      {/* Scrollable message area */}
      <div className="flex-1 overflow-hidden relative">
        {view === "roundtable" && isRoundtable ? (
          <div className="h-full overflow-y-auto">
            <RoundtableView
              messages={messages}
              onBranch={(id) => handleCreateBranch(id)}
              onBranchRT={
                !activeBranchId
                  ? (id) => setRtDialogCheckpoint(id)
                  : undefined
              }
              onMemo={
                !activeBranchId
                  ? (id) =>
                      createMemo(
                        id,
                        messages.find((m) => m.id === id)?.content ?? ""
                      )
                  : undefined
              }
              onFollowup={(engine, content) =>
                sendFollowup(engine, "message", content)
              }
              onSaveArtifact={(content) => setArtifactContent(content)}
              onDelete={async (id) => {
                await invoke("delete_message_pair", { messageId: id });
                const msgs = await invoke<Message[]>("list_messages", {
                  conversationId: selectedConversationId,
                });
                useChatStore.setState({ messages: msgs });
              }}
            />
          </div>
        ) : (
          <Virtuoso
            ref={virtuosoRef}
            scrollerRef={(el) => { virtuosoScrollerRef.current = el as HTMLElement | null; }}
            totalCount={messages.length}
            context={{ msgs: messages, brs: branches }}
            itemContent={renderMessage}
            followOutput={followOutput}
            atBottomThreshold={120}
            atBottomStateChange={handleAtBottomStateChange}
            initialTopMostItemIndex={Math.max(0, messages.length - 1)}
            className="h-full"
            style={{ height: "100%" }}
            components={{
              Header: () =>
                messages.length === 0 && !isRunning ? (
                  <div className="flex items-center justify-center h-32 text-muted-foreground text-sm">
                    No messages yet
                  </div>
                ) : (
                  <div className="pt-3" />
                ),
              Footer: () => (
                <>
                  {showTyping && (
                    <div className="flex items-center gap-1 px-4 py-3 text-muted-foreground text-xs">
                      <span className="typing-dot w-1 h-1 rounded-full bg-muted-foreground" />
                      <span className="typing-dot w-1 h-1 rounded-full bg-muted-foreground" />
                      <span className="typing-dot w-1 h-1 rounded-full bg-muted-foreground" />
                    </div>
                  )}
                  <div className="pb-1" />
                </>
              ),
            }}
          />
        )}

      </div>

      {/* Input + scroll-to-bottom button above it */}
      <div className="shrink-0 relative">
        {showScrollBtn && (
          <div className="absolute -top-10 inset-x-0 flex justify-center pointer-events-none">
            <div className="max-w-4xl w-full flex justify-end pr-4 pointer-events-none">
              <button
                onClick={scrollToBottom}
                className="pointer-events-auto w-8 h-8 rounded-full bg-background/90 hover:bg-accent border border-border text-muted-foreground hover:text-foreground flex items-center justify-center shadow-lg backdrop-blur-sm transition-all"
                aria-label="최신 메시지로"
              >
                <svg className="w-4 h-4" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2.5" strokeLinecap="round" strokeLinejoin="round">
                  <polyline points="6 9 12 15 18 9" />
                </svg>
              </button>
            </div>
          </div>
        )}
        <div className="max-w-4xl mx-auto w-full">
          <NewMessageInput onCreateRT={() => setRtDialogCheckpoint("")} />
        </div>
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
              <span className="typing-dot w-1.5 h-1.5 rounded-full bg-primary" />
              <span className="typing-dot w-1.5 h-1.5 rounded-full bg-primary" />
              <span className="typing-dot w-1.5 h-1.5 rounded-full bg-primary" />
            </div>
            <p className="text-tf-caption font-medium text-foreground">
              {projectLoading}
            </p>
            <p className="text-tf-xs text-muted-foreground/50">
              This may take a moment for large projects
            </p>
          </div>
        </div>
      )}
    </div>
  );
}
