import { useState, useRef, useEffect } from "react";
import { GitBranch, Copy, Bookmark, Users, Forward, Trash2 } from "lucide-react";

const FOLLOWUP_ENGINES = [
  { id: "claude", label: "Claude" },
  { id: "codex", label: "Codex" },
  { id: "gemini", label: "Gemini" },
];

interface MessageActionsProps {
  messageId: string;
  messageContent: string;
  isUser: boolean;
  onBranch?: (messageId: string) => void;
  onBranchRT?: (messageId: string) => void;
  onMemo?: (messageId: string) => void;
  onFollowup?: (engine: string, content: string) => void;
  onDeletePair?: (messageId: string) => void;
}

export function MessageActions({ messageId, messageContent, isUser, onBranch, onBranchRT, onMemo, onFollowup, onDeletePair }: MessageActionsProps) {
  const [showFollowupMenu, setShowFollowupMenu] = useState(false);
  const followupRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!showFollowupMenu) return;
    const handle = (e: MouseEvent) => {
      if (followupRef.current && !followupRef.current.contains(e.target as Node)) {
        setShowFollowupMenu(false);
      }
    };
    document.addEventListener("mousedown", handle);
    return () => document.removeEventListener("mousedown", handle);
  }, [showFollowupMenu]);

  return (
    <div className="absolute right-3 -top-3 z-10 flex items-center gap-0.5 px-1 py-0.5 rounded-md bg-card border border-border/30 shadow-sm opacity-0 pointer-events-none group-hover:opacity-100 group-hover:pointer-events-auto transition-opacity duration-100">
      {onBranch && (
        <button onClick={() => onBranch(messageId)} title="Thread"
          className="p-1 rounded hover:bg-accent hover:text-foreground text-muted-foreground/50 transition-colors">
          <GitBranch className="w-4 h-4" />
        </button>
      )}
      {onBranchRT && (
        <button onClick={() => onBranchRT(messageId)} title="Roundtable"
          className="p-1 rounded hover:bg-agent-gemini/10 hover:text-agent-gemini text-muted-foreground/50 transition-colors">
          <Users className="w-4 h-4" />
        </button>
      )}
      {onMemo && (
        <button onClick={() => onMemo(messageId)} title="Memo"
          className="p-1 rounded hover:bg-accent hover:text-foreground text-muted-foreground/50 transition-colors">
          <Bookmark className="w-4 h-4" />
        </button>
      )}
      {onFollowup && !isUser && (
        <div className="relative" ref={followupRef}>
          <button onClick={() => setShowFollowupMenu((v) => !v)} title="Forward"
            className="p-1 rounded hover:bg-accent hover:text-foreground text-muted-foreground/50 transition-colors">
            <Forward className="w-4 h-4" />
          </button>
          {showFollowupMenu && (
            <div className="absolute right-0 top-full mt-1 bg-popover border border-border/40 rounded-md shadow-lg p-0.5 min-w-[100px] z-50">
              {FOLLOWUP_ENGINES.map((eng) => (
                <button key={eng.id}
                  onClick={() => { onFollowup(eng.id, messageContent); setShowFollowupMenu(false); }}
                  className="w-full text-left px-2 py-1 rounded text-[10px] text-muted-foreground hover:text-foreground hover:bg-accent transition-colors">
                  → {eng.label}
                </button>
              ))}
            </div>
          )}
        </div>
      )}
      <button onClick={() => navigator.clipboard.writeText(messageContent)} title="Copy"
        className="p-1 rounded hover:bg-accent hover:text-foreground text-muted-foreground/50 transition-colors">
        <Copy className="w-4 h-4" />
      </button>
      {onDeletePair && (
        <button onClick={() => {
          if (window.confirm("이 메시지를 삭제하시겠습니까?")) onDeletePair(messageId);
        }} title="Delete"
          className="p-1 rounded hover:bg-destructive/15 hover:text-destructive text-muted-foreground/50 transition-colors">
          <Trash2 className="w-4 h-4" />
        </button>
      )}
    </div>
  );
}
