import { cn } from "@/lib/utils";
import { useChatStore } from "@/stores/chatStore";
import { MessageSquare, GitBranch, Users, X } from "lucide-react";

/**
 * Chat object tab bar — shows main conversation + currently open branch/RT.
 * Sits above the ChatPanel header. Max 2 tabs (main + 1 branch).
 */
export function ChatObjectTabs() {
  const {
    selectedConversationId,
    conversations,
    activeBranchId,
    threadBranchId,
    threadBranchLabel,
    branches,
    closeThread,
    closeBranchStream,
  } = useChatStore();

  if (!selectedConversationId) return null;

  const currentConv = conversations.find((c) => c.id === selectedConversationId);

  // Determine which branch is "open" (either full-view or drawer)
  const openBranchId = activeBranchId || threadBranchId;
  const openBranch = openBranchId ? branches.find((b) => b.id === openBranchId) : null;
  const branchLabel = openBranch
    ? (openBranch.customLabel ?? openBranch.label)
    : threadBranchLabel ?? null;
  const isRT = openBranch?.mode === "roundtable";

  // Active state: if any branch is open, branch tab is active; otherwise main
  const branchActive = !!openBranchId;
  const mainActive = !branchActive;

  // If no branch open, don't render tab bar (save vertical space)
  if (!openBranchId) return null;

  const handleMainClick = () => {
    // Close any open branch/drawer to return to main
    if (threadBranchId) closeThread();
    if (activeBranchId) closeBranchStream();
  };

  const handleBranchClose = (e: React.MouseEvent) => {
    e.stopPropagation();
    if (threadBranchId) closeThread();
    if (activeBranchId) closeBranchStream();
  };

  // Main conversation label
  const mainLabel = currentConv?.customLabel ?? currentConv?.label ?? "Conversation";
  // If in branch full-view, the "main" conversation is the parent — show as Architect
  const mainDisplayLabel = activeBranchId ? "Architect" : mainLabel;

  return (
    <div className="flex items-center gap-0.5 px-2 h-8 bg-card/30 border-b border-border/30 shrink-0">
      {/* Main tab */}
      <button
        onClick={handleMainClick}
        className={cn(
          "flex items-center gap-1.5 px-2.5 py-1 rounded-t text-[10px] font-medium transition-colors max-w-[160px]",
          mainActive
            ? "bg-background text-foreground border-b-2 border-primary/40"
            : "text-muted-foreground/60 hover:text-foreground hover:bg-accent/40"
        )}
      >
        <MessageSquare className="w-3 h-3 shrink-0" />
        <span className="truncate">{mainDisplayLabel}</span>
      </button>

      {/* Branch/RT tab */}
      {openBranchId && branchLabel && (
        <div
          className={cn(
            "flex items-center gap-1 px-2.5 py-1 rounded-t text-[10px] font-medium transition-colors max-w-[180px] group cursor-default",
            branchActive
              ? "bg-background text-foreground border-b-2 border-primary/40"
              : "text-muted-foreground/60 hover:text-foreground hover:bg-accent/40"
          )}
        >
          {isRT ? (
            <Users className="w-3 h-3 shrink-0 text-agent-gemini/70" />
          ) : (
            <GitBranch className="w-3 h-3 shrink-0 text-primary/60" />
          )}
          <span className="truncate">{branchLabel}</span>
          {isRT && (
            <span className="text-[7px] font-medium text-agent-gemini/50 bg-agent-gemini/8 px-0.5 rounded shrink-0">
              RT
            </span>
          )}
          <button
            onClick={handleBranchClose}
            className="shrink-0 ml-0.5 p-0.5 rounded opacity-0 group-hover:opacity-100 text-muted-foreground hover:text-foreground hover:bg-accent transition-all"
            title="Close"
          >
            <X className="w-2.5 h-2.5" />
          </button>
        </div>
      )}
    </div>
  );
}
