import { useState, useEffect, useRef, useCallback } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { Bot, X, Pin, PinOff, Send, Loader2, Inbox, Trash2, ChevronRight } from "lucide-react";
import { cn } from "@/lib/utils";
import { useChatStore } from "@/stores/chatStore";
import { getOrCreateMetaConversation } from "@/lib/metaConversation";
import type { Message } from "@/types";
import type { MetaNotification } from "@/lib/metaNotifications";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { vizMarkersAll } from "@/lib/vizMarkers";

const NOTIF_LS_KEY = "meta-notifications-v1";
const NOTIF_MAX = 50;

function loadNotifs(): MetaNotification[] {
  try {
    const raw = localStorage.getItem(NOTIF_LS_KEY);
    if (raw) return JSON.parse(raw);
  } catch { /* ignore */ }
  return [];
}
function saveNotifs(list: MetaNotification[]) {
  try { localStorage.setItem(NOTIF_LS_KEY, JSON.stringify(list.slice(0, NOTIF_MAX))); } catch { /* ignore */ }
}

interface MetaFloatingChatProps {
  projectKey: string;
}

const BUTTON_SIZE = 36;
const POPUP_W = 360;
const POPUP_H = 520;
const DEFAULT_POS = { x: 16, y: 56 };

function loadPos(): { x: number; y: number } {
  try {
    const raw = localStorage.getItem("meta-float-pos");
    if (raw) return JSON.parse(raw);
  } catch { /* ignore */ }
  return DEFAULT_POS;
}

export function MetaFloatingChat({ projectKey }: MetaFloatingChatProps) {
  const [open, setOpen] = useState(false);
  const [pinned, setPinned] = useState(false);
  const [metaConvId, setMetaConvId] = useState<string | null>(null);
  const [messages, setMessages] = useState<Message[]>([]);
  const [input, setInput] = useState("");
  const [running, setRunning] = useState(false);
  const [streamingId, setStreamingId] = useState<string | null>(null);
  const [notifs, setNotifs] = useState<MetaNotification[]>(() => loadNotifs());
  // Single popup with two tabs — prior split (button opens chat / tiny badge opens
  // inbox dropdown) caused '배지 1이지만 클릭해도 내용 없음' because the 16×16 badge
  // was below the 44pt touch target and users hit the main button, which opens
  // chat (empty meta conv) and auto-closes inbox. Now both live in the same popup.
  const [activeTab, setActiveTab] = useState<"inbox" | "chat">("chat");
  const [pos, setPos] = useState<{ x: number; y: number }>(loadPos);
  const posRef = useRef(pos);
  posRef.current = pos;
  const wrapperRef = useRef<HTMLDivElement>(null);
  const messagesEndRef = useRef<HTMLDivElement>(null);
  const inputRef = useRef<HTMLTextAreaElement>(null);
  const runningThreadIds = useChatStore((s) => s.runningThreadIds);

  // Mount 시 DB 에서 최신 알림 로드 (v38 meta_notifications 테이블).
  useEffect(() => {
    let alive = true;
    (async () => {
      try {
        const rows = await invoke<Array<{
          id: string; projectKey: string | null; kind: string; title: string;
          summary: string | null; routeJson: string | null; createdAt: number;
          readAt: number | null; dismissedAt: number | null;
        }>>("list_meta_notifications", { projectKey, limit: NOTIF_MAX });
        if (!alive) return;
        const parsed: MetaNotification[] = rows.map((r) => ({
          id: r.id,
          kind: r.kind as MetaNotification["kind"],
          title: r.title,
          summary: r.summary ?? undefined,
          projectKey: r.projectKey ?? undefined,
          createdAt: r.createdAt,
          read: !!r.readAt,
          dismissed: !!r.dismissedAt,
          route: r.routeJson ? (() => { try { return JSON.parse(r.routeJson!); } catch { return undefined; } })() : undefined,
        }));
        setNotifs(parsed);
        saveNotifs(parsed);
      } catch (e) {
        console.warn("[meta-notif] DB load failed, using localStorage only:", e);
        setNotifs(loadNotifs());
      }
    })();
    return () => { alive = false; };
  }, [projectKey]);

  // Listen for meta task assignments — payload 기반 inbox 누적 + localStorage 보존.
  useEffect(() => {
    const handler = (e: Event) => {
      const ce = e as CustomEvent<MetaNotification | undefined>;
      const notif: MetaNotification = ce.detail ?? {
        id: crypto.randomUUID(),
        kind: "generic",
        title: "새 알림",
        createdAt: Date.now(),
        read: false,
        dismissed: false,
      };
      setNotifs((prev) => {
        const next = [notif, ...prev].slice(0, NOTIF_MAX);
        saveNotifs(next);
        return next;
      });
    };
    window.addEventListener("tunaflow:meta-task", handler);
    return () => window.removeEventListener("tunaflow:meta-task", handler);
  }, []);

  const unreadCount = notifs.filter((n) => !n.read && !n.dismissed).length;

  const markAllRead = useCallback(() => {
    setNotifs((prev) => {
      const next = prev.map((n) => ({ ...n, read: true }));
      saveNotifs(next);
      return next;
    });
    invoke("mark_all_meta_notifications_read", { projectKey }).catch((e) =>
      console.warn("[meta-notif] markAll failed:", e));
  }, [projectKey]);

  const dismissNotif = useCallback((id: string) => {
    setNotifs((prev) => {
      const next = prev.map((n) => (n.id === id ? { ...n, dismissed: true } : n));
      saveNotifs(next);
      return next;
    });
    invoke("dismiss_meta_notification", { id }).catch((e) =>
      console.warn("[meta-notif] dismiss failed:", e));
  }, []);

  const clearAllNotifs = useCallback(() => {
    setNotifs([]);
    saveNotifs([]);
    invoke("clear_meta_notifications", { projectKey }).catch((e) =>
      console.warn("[meta-notif] clear failed:", e));
  }, [projectKey]);

  const routeTo = useCallback((notif: MetaNotification) => {
    const r = notif.route;
    setNotifs((prev) => {
      const next = prev.map((n) => (n.id === notif.id ? { ...n, read: true } : n));
      saveNotifs(next);
      return next;
    });
    invoke("mark_meta_notification_read", { id: notif.id }).catch(() => {});
    if (!r) return;
    if (r.tab) window.dispatchEvent(new CustomEvent("tunaflow:switch-tab", { detail: r.tab }));
    if (r.stage) window.dispatchEvent(new CustomEvent("tunaflow:switch-stage", { detail: r.stage }));
    // Domain-level focus goes through uiRouterSlice rather than a window
    // event — PlansPanel subscribes to the store directly (Finding 1-4).
    if (r.planId) useChatStore.getState().focusPlan(r.planId);
    // `scroll-to-message` window event had no subscribers and has been
    // dropped. If a future UI ever needs to scroll to a specific
    // message we'll add it to uiRouterSlice instead.
    if (!pinned) setOpen(false);
  }, [pinned]);

  /** C — "메타에게 물어보기": 알림 하나를 선택해 메타 채팅 패널을 열면서
   *  해당 맥락을 input 에 자동 주입. 사용자는 엔터만 누르거나 질문을 덧붙여 전송.
   *  실제 메타 LLM 호출은 사용자가 전송할 때 (원칙: 자동 실행 금지). */
  const askMetaAbout = useCallback((notif: MetaNotification) => {
    // 읽음 처리
    setNotifs((prev) => {
      const next = prev.map((n) => (n.id === notif.id ? { ...n, read: true } : n));
      saveNotifs(next);
      return next;
    });
    invoke("mark_meta_notification_read", { id: notif.id }).catch(() => {});
    // 메타 채팅 탭으로 전환 + 컨텍스트 질문 prompt 주입
    const prompt = [
      `알림: **${notif.title}**`,
      notif.summary ? `요약: ${notif.summary}` : "",
      "",
      `위 이벤트에 대해 분석해주시고, 다음 권장 액션을 제안해주세요.`,
    ].filter(Boolean).join("\n");
    setInput(prompt);
    setActiveTab("chat");
    setOpen(true);
    setTimeout(() => inputRef.current?.focus(), 50);
  }, []);

  // Close popup on outside click (pinned stays open)
  useEffect(() => {
    if (!open || pinned) return;
    const handler = (e: MouseEvent) => {
      if (wrapperRef.current && !wrapperRef.current.contains(e.target as Node)) {
        setOpen(false);
      }
    };
    document.addEventListener("mousedown", handler);
    return () => document.removeEventListener("mousedown", handler);
  }, [open, pinned]);

  // Resolve or create Meta conversation on project change
  useEffect(() => {
    setMetaConvId(null);
    setMessages([]);
    getOrCreateMetaConversation(projectKey)
      .then(setMetaConvId)
      .catch((e) => console.warn("[meta] conversation init failed:", e));
  }, [projectKey]);

  // Load messages when Meta conv is ready and panel opens
  const loadMessages = useCallback(async () => {
    if (!metaConvId) return;
    try {
      const msgs = await invoke<Message[]>("list_messages", { conversationId: metaConvId });
      setMessages(msgs);
    } catch (e) {
      console.warn("[meta] load messages failed:", e);
    }
  }, [metaConvId]);

  useEffect(() => {
    if (open && metaConvId) {
      loadMessages();
    }
  }, [open, metaConvId, loadMessages]);

  // Subscribe to streaming events for this conversation
  useEffect(() => {
    if (!metaConvId) return;

    const unlisten: (() => void)[] = [];

    const onChunk = (payload: { messageId: string; text: string }) => {
      setMessages((prev) => {
        const idx = prev.findIndex((m) => m.id === payload.messageId);
        if (idx >= 0) {
          const updated = [...prev];
          updated[idx] = { ...updated[idx], content: payload.text, status: "streaming" };
          return updated;
        }
        return prev;
      });
      setStreamingId(payload.messageId);
    };

    const onCompleted = (payload: { messageId: string; conversationId: string }) => {
      if (payload.conversationId !== metaConvId) return;
      setRunning(false);
      setStreamingId(null);
      loadMessages();
    };

    const onError = (payload: { conversationId: string }) => {
      if (payload.conversationId !== metaConvId) return;
      setRunning(false);
      setStreamingId(null);
      loadMessages();
    };

    Promise.all([
      listen<{ messageId: string; text: string }>("claude:chunk", (e) => onChunk(e.payload)),
      listen<{ messageId: string; conversationId: string }>("agent:completed", (e) => onCompleted(e.payload)),
      listen<{ conversationId: string }>("agent:error", (e) => onError(e.payload)),
    ]).then((fns) => { unlisten.push(...fns); });

    return () => { unlisten.forEach((fn) => fn()); };
  }, [metaConvId, loadMessages]);

  // Auto-scroll
  useEffect(() => {
    messagesEndRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [messages]);

  const isMetaRunning = metaConvId ? runningThreadIds.includes(metaConvId) : false;

  const handleSend = async () => {
    const text = input.trim();
    if (!text || !metaConvId || running || isMetaRunning) return;
    setInput("");
    setRunning(true);

    // Optimistic user message
    const optimisticUser = {
      id: `opt-user-${Date.now()}`,
      conversationId: metaConvId,
      role: "user" as const,
      content: text,
      timestamp: Date.now(),
      status: "done" as const,
    };
    setMessages((prev) => [...prev, optimisticUser]);

    try {
      // Fire start_claude_stream — creates messages internally, returns messageId
      const result = await invoke<{ messageId: string }>("start_claude_stream", {
        input: {
          projectKey,
          conversationId: metaConvId,
          prompt: text,
          agentName: "meta",
        },
      });
      setStreamingId(result.messageId);
      // Optimistically add a streaming placeholder until first chunk arrives
      setMessages((prev) => [
        ...prev,
        {
          id: result.messageId,
          conversationId: metaConvId,
          role: "assistant" as const,
          content: "",
          timestamp: Date.now(),
          status: "streaming" as const,
        },
      ]);
    } catch (e) {
      console.error("[meta] send failed:", e);
      setRunning(false);
      setStreamingId(null);
      loadMessages();
    }
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  };

  const toggleOpen = () => {
    if (pinned) return; // pinned = always open
    if (open) {
      setOpen(false);
      return;
    }
    // Opening: prefer Inbox tab when there are unread, else Chat (preserves
    // prior behavior for empty inbox).
    setActiveTab(unreadCount > 0 ? "inbox" : "chat");
    setOpen(true);
    if (unreadCount === 0) setTimeout(() => inputRef.current?.focus(), 100);
  };

  const handleButtonMouseDown = (e: React.MouseEvent) => {
    // Only left button
    if (e.button !== 0) return;
    e.preventDefault();

    const startX = e.clientX - posRef.current.x;
    const startY = e.clientY - posRef.current.y;
    let moved = false;

    const onMove = (ev: MouseEvent) => {
      const dx = ev.clientX - (startX + posRef.current.x);
      const dy = ev.clientY - (startY + posRef.current.y);
      if (!moved && Math.hypot(dx, dy) < 3) return;
      moved = true;

      const parent = wrapperRef.current?.parentElement;
      const pw = parent?.clientWidth ?? window.innerWidth;
      const ph = parent?.clientHeight ?? window.innerHeight;

      const x = Math.max(0, Math.min(ev.clientX - startX, pw - BUTTON_SIZE));
      const y = Math.max(0, Math.min(ev.clientY - startY, ph - BUTTON_SIZE));
      setPos({ x, y });
    };

    const onUp = () => {
      document.removeEventListener("mousemove", onMove);
      document.removeEventListener("mouseup", onUp);
      document.body.style.cursor = "";
      document.body.style.userSelect = "";
      if (moved) {
        localStorage.setItem("meta-float-pos", JSON.stringify(posRef.current));
      } else {
        toggleOpen();
      }
    };

    document.addEventListener("mousemove", onMove);
    document.addEventListener("mouseup", onUp);
    document.body.style.cursor = "grabbing";
    document.body.style.userSelect = "none";
  };

  const isOpen = open || pinned;

  // Flip popup upward when button is in the lower portion of the container
  const parent = wrapperRef.current?.parentElement;
  const containerH = parent?.clientHeight ?? window.innerHeight;
  const openUpward = pos.y > containerH * 0.55;
  // Flip popup leftward when button is near the right edge
  const containerW = parent?.clientWidth ?? window.innerWidth;
  const openLeft = pos.x + POPUP_W > containerW - 8;

  return (
    <div
      ref={wrapperRef}
      className="absolute z-[60]"
      style={{ left: pos.x, top: pos.y }}
    >
      {/* Floating trigger button */}
      <div className="relative">
        <button
          onMouseDown={handleButtonMouseDown}
          className={cn(
            "w-9 h-9 rounded-full shadow-lg flex items-center justify-center transition-all select-none",
            "border cursor-grab active:cursor-grabbing",
            isOpen
              ? "bg-primary text-primary-foreground border-primary shadow-primary/20"
              : "bg-background border-border/30 text-muted-foreground/60 hover:text-primary hover:border-primary/30 hover:shadow-primary/10",
            !isOpen && unreadCount > 0 && "animate-pulse border-amber-400/60 text-amber-400",
          )}
          title="Meta Agent (드래그로 이동, 길게 눌러 알림함)"
        >
          <Bot className="w-4 h-4" />
        </button>
        {/* Display-only unread badge — not a click target. The whole Bot button
            opens the unified popup; initial tab is chosen by unreadCount. */}
        {unreadCount > 0 && !isOpen && (
          <div
            className="absolute -top-1 -right-1 w-4 h-4 rounded-full bg-amber-400 text-[9px] font-bold text-black flex items-center justify-center pointer-events-none"
            aria-label={`미읽 알림 ${unreadCount}개`}
          >
            {unreadCount > 9 ? "9+" : unreadCount}
          </div>
        )}
      </div>

      {/* Popup chat panel — direction flips based on position */}
      {isOpen && (
        <div
          className="absolute w-[360px] flex flex-col bg-background border border-border/40 rounded-xl shadow-2xl overflow-hidden"
          style={{
            width: POPUP_W,
            height: Math.min(POPUP_H, containerH * 0.65),
            ...(openUpward
              ? { bottom: BUTTON_SIZE + 8 }
              : { top: BUTTON_SIZE + 8 }),
            ...(openLeft
              ? { right: 0 }
              : { left: 0 }),
          }}
        >
          {/* Header */}
          <div className="flex items-center gap-2 px-3 py-2 border-b border-border/30 shrink-0 bg-card/50">
            <Bot className="w-4 h-4 text-primary/70 shrink-0" />
            <span className="flex-1 text-[12px] font-semibold text-foreground">Meta</span>
            <span className="text-[10px] text-muted-foreground/40 mr-1">프로세스 관리자</span>
            <button
              onClick={() => setPinned((v) => !v)}
              className={cn(
                "p-1 rounded hover:bg-accent/50 transition-colors",
                pinned ? "text-primary" : "text-muted-foreground/40 hover:text-muted-foreground"
              )}
              title={pinned ? "핀 해제" : "고정"}
            >
              {pinned ? <Pin className="w-3.5 h-3.5" /> : <PinOff className="w-3.5 h-3.5" />}
            </button>
            {!pinned && (
              <button
                onClick={() => setOpen(false)}
                className="p-1 rounded hover:bg-accent/50 text-muted-foreground/40 hover:text-muted-foreground transition-colors"
              >
                <X className="w-3.5 h-3.5" />
              </button>
            )}
          </div>

          {/* Tab bar — Inbox ↔ Chat */}
          <div className="flex border-b border-border/30 shrink-0 bg-card/30">
            <button
              onClick={() => setActiveTab("inbox")}
              className={cn(
                "flex-1 flex items-center justify-center gap-1.5 py-2 text-[11px] font-medium transition-colors",
                activeTab === "inbox"
                  ? "text-foreground border-b-2 border-primary -mb-px"
                  : "text-muted-foreground/60 hover:text-foreground"
              )}
            >
              <Inbox className="w-3 h-3" />
              알림함
              {unreadCount > 0 && (
                <span className="ml-0.5 px-1 rounded-full bg-amber-400 text-black text-[9px] font-bold">
                  {unreadCount > 9 ? "9+" : unreadCount}
                </span>
              )}
            </button>
            <button
              onClick={() => {
                setActiveTab("chat");
                setTimeout(() => inputRef.current?.focus(), 50);
              }}
              className={cn(
                "flex-1 flex items-center justify-center gap-1.5 py-2 text-[11px] font-medium transition-colors",
                activeTab === "chat"
                  ? "text-foreground border-b-2 border-primary -mb-px"
                  : "text-muted-foreground/60 hover:text-foreground"
              )}
            >
              <Send className="w-3 h-3" />
              대화
            </button>
          </div>

          {/* Body — Inbox list OR Messages, based on activeTab */}
          {activeTab === "inbox" ? (
            <div className="flex flex-col flex-1 min-h-0">
              {notifs.filter((n) => !n.dismissed).length > 0 && (
                <div className="flex items-center gap-2 px-3 h-8 border-b border-border/20 bg-muted/10">
                  <span className="text-[10px] text-muted-foreground flex-1">
                    {notifs.filter((n) => !n.dismissed).length}개
                  </span>
                  {unreadCount > 0 && (
                    <button onClick={markAllRead} className="text-[10px] text-muted-foreground hover:text-foreground">모두 읽음</button>
                  )}
                  <button onClick={clearAllNotifs} className="text-muted-foreground/50 hover:text-destructive p-1" title="전체 삭제">
                    <Trash2 className="w-3 h-3" />
                  </button>
                </div>
              )}
              <div className="flex-1 overflow-y-auto">
                {notifs.filter((n) => !n.dismissed).length === 0 ? (
                  <div className="flex flex-col items-center justify-center h-full gap-2 text-center px-4">
                    <Inbox className="w-7 h-7 text-muted-foreground/20" />
                    <p className="text-[11px] text-muted-foreground/50">알림 없음</p>
                  </div>
                ) : (
                  <ul className="divide-y divide-border/20">
                    {notifs.filter((n) => !n.dismissed).slice(0, 30).map((n) => (
                      <li key={n.id} className={cn("group px-3 py-2 hover:bg-accent/30 transition-colors", !n.read && "bg-amber-500/5")}>
                        <div className="flex items-start gap-2">
                          <div className="flex-1 min-w-0">
                            <p className="text-[12px] font-medium text-foreground truncate">{n.title}</p>
                            {n.summary && <p className="text-[10px] text-muted-foreground line-clamp-2 mt-0.5">{n.summary}</p>}
                            <p className="text-[9px] text-muted-foreground/50 mt-0.5">
                              {new Date(n.createdAt).toLocaleString("ko-KR", { month: "2-digit", day: "2-digit", hour: "2-digit", minute: "2-digit" })}
                              {!n.read && <span className="ml-1.5 text-amber-500">●</span>}
                            </p>
                          </div>
                          <div className="flex items-center gap-1 opacity-0 group-hover:opacity-100 transition-opacity">
                            <button onClick={() => askMetaAbout(n)} className="p-1 rounded hover:bg-primary/10 text-primary" title="메타에게 물어보기">
                              <Bot className="w-3 h-3" />
                            </button>
                            {n.route && (
                              <button onClick={() => routeTo(n)} className="p-1 rounded hover:bg-primary/10 text-primary" title="관련 위치로 이동">
                                <ChevronRight className="w-3 h-3" />
                              </button>
                            )}
                            <button onClick={() => dismissNotif(n.id)} className="p-1 rounded hover:bg-destructive/10 text-muted-foreground hover:text-destructive" title="닫기">
                              <X className="w-3 h-3" />
                            </button>
                          </div>
                        </div>
                      </li>
                    ))}
                  </ul>
                )}
              </div>
            </div>
          ) : (
            <>
              {/* Messages */}
              <div className="flex-1 overflow-y-auto px-3 py-3 space-y-3 min-h-0">
                {messages.length === 0 && (
                  <div className="flex flex-col items-center justify-center h-full gap-2 text-center px-4">
                    <Bot className="w-8 h-8 text-muted-foreground/20" />
                    <p className="text-[12px] text-muted-foreground/40">
                      프로젝트 상태 분석, 이슈 감지, 우선순위 제안을 도와드립니다.
                    </p>
                    <p className="text-[11px] text-muted-foreground/25">
                      "프로젝트 상태 확인해줘" 로 시작해보세요
                    </p>
                  </div>
                )}
                {messages.map((msg) => (
                  <MetaMessage
                    key={msg.id}
                    message={msg}
                    isStreaming={msg.id === streamingId}
                  />
                ))}
                <div ref={messagesEndRef} />
              </div>

              {/* Input */}
              <div className="px-3 py-2 border-t border-border/20 shrink-0 bg-card/30">
                <div className="flex items-end gap-2">
                  <textarea
                    ref={inputRef}
                    value={input}
                    onChange={(e) => setInput(e.target.value)}
                    onKeyDown={handleKeyDown}
                    placeholder="Meta에게 물어보기..."
                    rows={1}
                    className="flex-1 resize-none bg-transparent text-[12px] text-foreground placeholder:text-muted-foreground/30 outline-none border border-border/20 rounded-lg px-3 py-2 min-h-[36px] max-h-[80px] overflow-y-auto"
                    style={{ height: "auto" }}
                    onInput={(e) => {
                      const el = e.currentTarget;
                      el.style.height = "auto";
                      el.style.height = `${Math.min(el.scrollHeight, 80)}px`;
                    }}
                    disabled={running || isMetaRunning}
                  />
                  <button
                    onClick={handleSend}
                    disabled={!input.trim() || running || isMetaRunning}
                    className="p-2 rounded-lg bg-primary/80 hover:bg-primary text-primary-foreground disabled:opacity-30 disabled:cursor-not-allowed transition-colors shrink-0"
                  >
                    {running || isMetaRunning
                      ? <Loader2 className="w-3.5 h-3.5 animate-spin" />
                      : <Send className="w-3.5 h-3.5" />
                    }
                  </button>
                </div>
              </div>
            </>
          )}
        </div>
      )}

    </div>
  );
}

// ─── MetaMessage ─────────────────────────────────────────────────────────────

function MetaMessage({ message, isStreaming }: { message: Message; isStreaming: boolean }) {
  const isUser = message.role === "user";

  return (
    <div className={cn("flex gap-2", isUser && "justify-end")}>
      {!isUser && (
        <div className="w-5 h-5 rounded-full bg-primary/10 flex items-center justify-center shrink-0 mt-0.5">
          <Bot className="w-3 h-3 text-primary/60" />
        </div>
      )}
      <div
        className={cn(
          "max-w-[85%] rounded-xl px-3 py-2 text-[12px] leading-relaxed",
          isUser
            ? "bg-primary/10 text-foreground"
            : "bg-card/60 text-foreground/90"
        )}
      >
        {isUser ? (
          <p className="whitespace-pre-wrap">{message.content}</p>
        ) : (
          <div className={cn(
            "prose prose-sm prose-invert max-w-none",
            "[&_p]:my-1 [&_h2]:text-[13px] [&_h3]:text-[12px]",
            "[&_ul]:my-1 [&_li]:my-0 [&_code]:text-[11px]",
            "[&_pre]:my-2 [&_pre]:text-[11px]",
          )}>
            <ReactMarkdown remarkPlugins={[[remarkGfm, { singleTilde: false }]]}>
              {vizMarkersAll(message.content) || (isStreaming ? "▋" : "")}
            </ReactMarkdown>
          </div>
        )}
      </div>
    </div>
  );
}
