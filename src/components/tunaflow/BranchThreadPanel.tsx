import { useRef, useEffect, useState, useMemo } from "react";
import { X, Check, GitBranch, Maximize2, SendHorizonal, ChevronDown } from "lucide-react";
import { AgentAvatar } from "./AgentAvatar";
import { cn, normalizeEngine, AGENT_DOT_COLORS, AGENT_DISPLAY_NAMES, formatTimestamp } from "@/lib/utils";
import { useChatStore } from "@/stores/chatStore";
import { MessageItem } from "./MessageItem";
import { InlineRename } from "./InlineRename";

type Engine = "claude" | "codex" | "gemini" | "opencode";
const ENGINE_LIST: { id: Engine; label: string }[] = [
  { id: "claude", label: "Claude" },
  { id: "codex", label: "Codex" },
  { id: "gemini", label: "Gemini" },
  { id: "opencode", label: "OpenCode" },
];

export function BranchThreadPanel() {
  const {
    threadBranchId,
    threadMessages,
    threadBranchLabel,
    threadParentMessage,
    selectedConversationId,
    isRunning,
    runningThreadIds,
    closeThread,
    adoptBranch,
    openBranchStream,
    sendThreadMessage,
    renameBranch,
    engineModels,
  } = useChatStore();

  const bottomRef = useRef<HTMLDivElement>(null);
  const textareaRef = useRef<HTMLTextAreaElement>(null);
  const [text, setText] = useState("");
  const [engine, setEngine] = useState<Engine>("claude");
  const [selectedModel, setSelectedModel] = useState<string>("");
  const [showEnginePicker, setShowEnginePicker] = useState(false);
  const enginePickerRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!showEnginePicker) return;
    const handle = (e: MouseEvent) => {
      if (enginePickerRef.current && !enginePickerRef.current.contains(e.target as Node)) {
        setShowEnginePicker(false);
      }
    };
    document.addEventListener("mousedown", handle);
    return () => document.removeEventListener("mousedown", handle);
  }, [showEnginePicker]);

  const currentModels = useMemo(
    () => engineModels.filter((m) => m.engine === engine),
    [engineModels, engine],
  );

  useEffect(() => {
    const rec = currentModels.find((m) => m.recommended);
    setSelectedModel(rec?.id ?? currentModels[0]?.id ?? "");
  }, [engine, currentModels.length]);

  useEffect(() => {
    bottomRef.current?.scrollIntoView({ behavior: "smooth" });
  }, [threadMessages]);

  useEffect(() => {
    const el = textareaRef.current;
    if (!el) return;
    el.style.height = "auto";
    el.style.height = `${Math.min(el.scrollHeight, 140)}px`;
  }, [text]);

  if (!threadBranchId) return null;

  const handleSend = async () => {
    const prompt = text.trim();
    if (!prompt) return;
    setText("");
    await sendThreadMessage(prompt, engine, selectedModel || undefined);
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === "Enter" && !e.shiftKey) {
      e.preventDefault();
      handleSend();
    }
  };

  const handleAdopt = async () => {
    if (!selectedConversationId) return;
    await adoptBranch(threadBranchId, selectedConversationId);
    closeThread();
  };

  const handleOpenFull = async () => {
    closeThread();
    await openBranchStream(threadBranchId);
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
      {/* Header */}
      <div className="flex items-center gap-2.5 px-3.5 h-10 border-b border-border/40 shrink-0">
        <div className="flex items-center gap-1.5 flex-1 min-w-0">
          <GitBranch className="w-3 h-3 text-primary/60 shrink-0" />
          <h2 className="text-[12px] font-medium text-foreground truncate min-w-0">
            {threadBranchId ? (
              <InlineRename value={threadBranchLabel ?? ""} onSave={(v) => renameBranch(threadBranchId, v)} inputClassName="text-[11px] w-full" />
            ) : threadBranchLabel}
          </h2>
          <span className="text-[8px] font-medium px-1 py-0.5 rounded uppercase tracking-wider shrink-0 text-primary/50 bg-primary/6">
            Branch
          </span>
        </div>
        <div className="flex items-center gap-0.5 shrink-0">
          <button onClick={handleAdopt} title="Adopt" className="flex items-center gap-0.5 px-1.5 py-0.5 rounded text-[9px] font-medium text-primary/70 hover:bg-primary/8 transition-colors">
            <Check className="w-2.5 h-2.5" /> Adopt
          </button>
          <button onClick={handleOpenFull} title="Full view" className="p-1 rounded text-muted-foreground/50 hover:text-foreground hover:bg-accent transition-colors">
            <Maximize2 className="w-3 h-3" />
          </button>
          <button onClick={closeThread} title="Close" className="p-1 rounded text-muted-foreground/50 hover:text-foreground hover:bg-accent transition-colors">
            <X className="w-3 h-3" />
          </button>
        </div>
      </div>

      {/* Parent anchor */}
      {threadParentMessage && (
        <div className="flex gap-2.5 px-3.5 py-2 border-b border-border/30 bg-accent/10 shrink-0">
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
            <GitBranch className="w-4 h-4" />
            <p>No replies yet</p>
          </div>
        ) : (
          <div className="py-2 space-y-0.5">
            {threadMessages.map((msg) => (
              <MessageItem key={msg.id} message={msg} showActions={false} />
            ))}
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

      {/* Input */}
      <div className="px-3 pb-3 pt-1.5 shrink-0">
        <div className="rounded-lg border border-border/30 bg-card/50 focus-within:border-ring/30 transition-colors">
          {/* Toolbar */}
          <div className="flex items-center gap-1 px-2 pt-1.5 pb-1 border-b border-border/20">
            <div className="relative" ref={enginePickerRef}>
              <button
                onClick={() => setShowEnginePicker(!showEnginePicker)}
                className="flex items-center gap-1 text-[9px] text-muted-foreground/50 hover:text-foreground transition-colors px-1 py-0.5 rounded hover:bg-accent/50"
              >
                <span className={cn("w-1.5 h-1.5 rounded-full", `bg-agent-${engine}`)} />
                <span className="font-medium">{ENGINE_LIST.find((e) => e.id === engine)?.label}</span>
                <ChevronDown className="w-2 h-2" />
              </button>
              {showEnginePicker && (
                <div className="absolute bottom-full left-0 mb-1 bg-popover border border-border/30 rounded-md shadow-lg p-0.5 min-w-[100px] z-50">
                  {ENGINE_LIST.map((eng) => (
                    <button
                      key={eng.id}
                      onClick={() => { setEngine(eng.id); setShowEnginePicker(false); }}
                      className={cn(
                        "w-full flex items-center gap-1.5 px-2 py-1 rounded text-[9px] transition-colors",
                        engine === eng.id ? "text-foreground bg-accent" : "text-muted-foreground hover:text-foreground hover:bg-accent"
                      )}
                    >
                      <span className={cn("w-1.5 h-1.5 rounded-full shrink-0", engine === eng.id ? `bg-agent-${eng.id}` : "bg-muted")} />
                      {eng.label}
                    </button>
                  ))}
                </div>
              )}
            </div>
            {currentModels.length > 0 && (
              <>
                <span className="h-2.5 w-px bg-border/20" />
                <select
                  value={selectedModel}
                  onChange={(e) => setSelectedModel(e.target.value)}
                  className="bg-transparent rounded px-0.5 py-0 text-[9px] outline-none text-muted-foreground/40 max-w-[100px]"
                >
                  {currentModels.map((m) => (
                    <option key={m.id} value={m.id}>
                      {m.recommended ? "★ " : ""}{m.label}
                    </option>
                  ))}
                </select>
              </>
            )}
          </div>

          <textarea
            ref={textareaRef}
            value={text}
            onChange={(e) => setText(e.target.value)}
            onKeyDown={handleKeyDown}
            placeholder="Continue thread… (↵)"
            rows={1}
            className="w-full px-2.5 py-1.5 text-[12px] bg-transparent resize-none outline-none text-foreground placeholder:text-muted-foreground/30 leading-relaxed"
          />
          <div className="flex items-center gap-1 px-2 pb-1.5 pt-0.5">
            <span className="text-[8px] text-muted-foreground/25 font-mono">↵</span>
            <span className="flex-1" />
            <button
              onClick={handleSend}
              disabled={!text.trim()}
              className={cn(
                "flex items-center gap-1 px-2 py-0.5 rounded text-[9px] font-medium transition-colors",
                text.trim()
                  ? "bg-primary/90 text-primary-foreground hover:bg-primary"
                  : "bg-muted text-muted-foreground/30 cursor-not-allowed"
              )}
            >
              <SendHorizonal className="w-2.5 h-2.5" />
              Send
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
