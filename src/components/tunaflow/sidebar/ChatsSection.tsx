import { MessageSquare, Plus, Trash2 } from "lucide-react";
import { TreeRow, SectionHeader } from "./TreeRow";
import { InlineRename } from "../InlineRename";
import type { Conversation } from "@/types";

interface ChatsSectionProps {
  chatsOpen: boolean;
  setChatsOpen: (v: boolean) => void;
  filteredChats: Conversation[];
  selectedConversationId: string | null;
  selectConversation: (id: string) => void;
  renameConversation: (id: string, label: string) => Promise<void>;
  handleCreateChat: (e: React.MouseEvent) => void;
  handleDelete: (id: string, label: string, e: React.MouseEvent) => void;
}

export function ChatsSection({
  chatsOpen, setChatsOpen, filteredChats, selectedConversationId,
  selectConversation, renameConversation, handleCreateChat, handleDelete,
}: ChatsSectionProps) {
  return (
    <>
      <SectionHeader title="Chats" expanded={chatsOpen} onToggle={() => setChatsOpen(!chatsOpen)}
        actions={<button onClick={handleCreateChat} className="p-0.5 rounded text-sidebar-foreground/30 hover:text-sidebar-foreground hover:bg-white/10 transition-colors" title="New chat"><Plus className="w-3 h-3" /></button>} />
      {chatsOpen && (
        <>
          {filteredChats.length === 0 && (
            <TreeRow depth={1} className="cursor-default" icon={<MessageSquare className="w-3.5 h-3.5 text-sidebar-foreground/15" />}
              label={<span className="text-[10px] text-sidebar-foreground/25 italic">No conversations</span>} />
          )}
          {filteredChats.map((conv) => {
            const isActive = conv.id === selectedConversationId;
            return (
              <TreeRow key={conv.id} depth={1} active={isActive}
                icon={<MessageSquare className="w-3.5 h-3.5" />}
                label={<InlineRename value={conv.customLabel ?? conv.label} onSave={(v) => renameConversation(conv.id, v)} inputClassName="text-[10px] w-full" />}
                suffix={isActive ? <span className="w-1.5 h-1.5 rounded-full bg-primary shrink-0 mr-1" /> : undefined}
                actions={<button onClick={(e) => handleDelete(conv.id, conv.customLabel ?? conv.label, e)} className="p-0.5 rounded text-sidebar-foreground/20 hover:text-destructive transition-colors"><Trash2 className="w-3 h-3" /></button>}
                onClick={() => selectConversation(conv.id)} />
            );
          })}
        </>
      )}
    </>
  );
}
