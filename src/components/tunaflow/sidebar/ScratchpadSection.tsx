import { useState } from "react";
import { useTranslation } from "react-i18next";
import { cn } from "@/lib/utils";
import { Lightbulb, Plus, Trash2, ChevronRight, Pencil } from "lucide-react";
import { useChatStore } from "@/stores/chatStore";
import { ask } from "@tauri-apps/plugin-dialog";
import type { Conversation } from "@/types";
import { SidebarContextMenu, type ContextMenuState } from "./SidebarContextMenu";

interface ScratchpadSectionProps {
  scratchpads: Conversation[];
  selectedConversationId: string | null;
  selectConversation: (id: string) => Promise<void>;
  renameConversation: (id: string, label: string) => Promise<void>;
}

export function ScratchpadSection({
  scratchpads,
  selectedConversationId,
  selectConversation,
  renameConversation,
}: ScratchpadSectionProps) {
  const { t } = useTranslation("sidebar");
  const [expanded, setExpanded] = useState(true);
  const createConversation = useChatStore((s) => s.createConversation);
  const deleteConversation = useChatStore((s) => s.deleteConversation);
  const selectedProjectKey = useChatStore((s) => s.selectedProjectKey);
  const runningThreadIds = useChatStore((s) => s.runningThreadIds);
  const [ctxMenu, setCtxMenu] = useState<ContextMenuState | null>(null);

  const handleAdd = async () => {
    if (!selectedProjectKey) return;
    const label = `Scratch ${scratchpads.length + 1}`;
    const conv = await createConversation({
      projectKey: selectedProjectKey,
      label,
      type: "scratchpad",
      mode: "chat",
      source: "user",
    });
    if (conv) await selectConversation(conv.id);
  };

  const handleDelete = async (id: string) => {
    const ok = await ask(t("confirm.scratchpad_delete_body"), { kind: "warning", title: t("confirm.scratchpad_delete_title") });
    if (ok) await deleteConversation(id);
  };

  const openCtx = (e: React.MouseEvent, sp: Conversation) => {
    e.preventDefault();
    e.stopPropagation();
    setCtxMenu({
      x: e.clientX,
      y: e.clientY,
      items: [
        {
          label: t("action.rename"),
          icon: <Pencil className="w-3.5 h-3.5" />,
          onClick: () => {
            const val = window.prompt(t("prompt.rename_new_name"), sp.label ?? "");
            if (val) renameConversation(sp.id, val);
          },
        },
        { separator: true, label: "", onClick: () => {} },
        {
          label: t("action.delete"),
          icon: <Trash2 className="w-3.5 h-3.5" />,
          danger: true,
          onClick: () => handleDelete(sp.id),
        },
      ],
    });
  };

  return (
    <div className="mt-2">
      {/* Section header */}
      <div className="flex items-center justify-between px-1 py-1">
        <button
          onClick={() => setExpanded(!expanded)}
          className="flex items-center gap-1.5 text-[10px] font-semibold text-muted-foreground/50 uppercase tracking-wider hover:text-muted-foreground/70 transition-colors"
        >
          <ChevronRight className={cn("w-3 h-3 transition-transform", expanded && "rotate-90")} />
          <Lightbulb className="w-3 h-3" />
          {t("section.scratchpad")}
        </button>
        <button
          onClick={handleAdd}
          className="p-0.5 rounded text-muted-foreground/30 hover:text-foreground hover:bg-accent/40 transition-colors"
          title={t("action.new_scratchpad")}
        >
          <Plus className="w-3 h-3" />
        </button>
      </div>

      {/* Scratchpad list */}
      {expanded && (
        <div className="space-y-0.5 mt-0.5">
          {scratchpads.length === 0 && (
            <p className="text-[10px] text-muted-foreground/30 px-3 py-1">{t("empty.no_scratchpads")}</p>
          )}
          {scratchpads.map((sp) => {
            const isSelected = sp.id === selectedConversationId;
            const isRunning = runningThreadIds.includes(sp.id);
            return (
              <button
                key={sp.id}
                onClick={() => selectConversation(sp.id)}
                onContextMenu={(e) => openCtx(e, sp)}
                className={cn(
                  "group w-full flex items-center gap-2 px-3 py-1.5 rounded-md text-left text-[11px] transition-colors select-none",
                  isSelected
                    ? "bg-accent/40 text-foreground"
                    : "text-foreground/60 hover:bg-accent/20 hover:text-foreground/80"
                )}
              >
                <Lightbulb className="w-3 h-3 shrink-0 text-amber-500/50" />
                <span className="flex-1 truncate">{sp.label}</span>
                {isRunning && <span className="w-1.5 h-1.5 rounded-full bg-status-approved animate-pulse shrink-0" />}
                <div
                  role="button"
                  tabIndex={0}
                  onClick={(e) => { e.stopPropagation(); handleDelete(sp.id); }}
                  onKeyDown={(e) => { if (e.key === "Enter") { e.stopPropagation(); handleDelete(sp.id); } }}
                  className="p-0.5 rounded opacity-0 group-hover:opacity-100 text-muted-foreground/30 hover:text-destructive transition-all cursor-pointer"
                >
                  <Trash2 className="w-2.5 h-2.5" />
                </div>
              </button>
            );
          })}
        </div>
      )}

      {ctxMenu && <SidebarContextMenu menu={ctxMenu} onClose={() => setCtxMenu(null)} />}
    </div>
  );
}
