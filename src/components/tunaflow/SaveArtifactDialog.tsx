import { useState } from "react";
import { X } from "lucide-react";
import { useChatStore } from "@/stores/chatStore";

const ARTIFACT_TYPES = [
  { id: "note", label: "Note" },
  { id: "code", label: "Code" },
  { id: "spec", label: "Spec" },
  { id: "plan", label: "Plan" },
  { id: "task-brief", label: "Task Brief" },
  { id: "review-findings", label: "Review Findings" },
  { id: "architect-decision", label: "Architect Decision" },
  { id: "test-report", label: "Test Report" },
];

interface SaveArtifactDialogProps {
  open: boolean;
  onClose: () => void;
  initialContent: string;
}

export function SaveArtifactDialog({ open, onClose, initialContent }: SaveArtifactDialogProps) {
  const selectedConversationId = useChatStore((s) => s.selectedConversationId);
  const createArtifact = useChatStore((s) => s.createArtifact);

  const [title, setTitle] = useState("");
  const [type, setType] = useState("note");
  const [content, setContent] = useState(initialContent);
  const [saving, setSaving] = useState(false);

  // Reset form when dialog opens with new content
  const [lastContent, setLastContent] = useState("");
  if (open && initialContent !== lastContent) {
    setContent(initialContent);
    setTitle(initialContent.split("\n")[0]?.replace(/^#+\s*/, "").slice(0, 60) || "");
    setLastContent(initialContent);
  }

  if (!open) return null;

  const handleSave = async () => {
    if (!title.trim() || !content.trim() || !selectedConversationId) return;
    setSaving(true);
    try {
      await createArtifact({
        conversationId: selectedConversationId,
        type,
        title: title.trim(),
        content: content.trim(),
      });
      onClose();
      setTitle("");
      setContent("");
    } finally {
      setSaving(false);
    }
  };

  return (
    <div className="fixed inset-0 z-[70] flex items-center justify-center">
      <div className="absolute inset-0 bg-black/30" onClick={onClose} />

      <div className="relative bg-card border border-border/40 rounded-xl shadow-2xl w-[500px] max-h-[70vh] overflow-hidden flex flex-col">
        {/* Header */}
        <div className="flex items-center px-4 h-11 shrink-0">
          <span className="text-[13px] font-medium text-foreground flex-1">Save as Artifact</span>
          <button onClick={onClose} className="p-1 rounded hover:bg-accent text-muted-foreground hover:text-foreground transition-colors">
            <X className="w-4 h-4" />
          </button>
        </div>

        <div className="p-4 space-y-3 overflow-y-auto">
          {/* Title */}
          <div>
            <label className="text-[11px] text-muted-foreground mb-1 block">Title</label>
            <input
              value={title}
              onChange={(e) => setTitle(e.target.value)}
              placeholder="Artifact title"
              autoFocus
              className="w-full bg-background rounded-lg px-3 py-2 text-[13px] outline-none border border-border/30 focus:border-ring/40"
            />
          </div>

          {/* Type */}
          <div>
            <label className="text-[11px] text-muted-foreground mb-1 block">Type</label>
            <select
              value={type}
              onChange={(e) => setType(e.target.value)}
              className="w-full bg-background rounded-lg px-3 py-2 text-[12px] outline-none border border-border/30 focus:border-ring/40 cursor-pointer"
            >
              {ARTIFACT_TYPES.map((t) => (
                <option key={t.id} value={t.id}>{t.label}</option>
              ))}
            </select>
          </div>

          {/* Content */}
          <div>
            <label className="text-[11px] text-muted-foreground mb-1 block">Content</label>
            <textarea
              value={content}
              onChange={(e) => setContent(e.target.value)}
              rows={8}
              className="w-full bg-background rounded-lg px-3 py-2 text-[12px] font-mono outline-none border border-border/30 focus:border-ring/40 resize-none"
            />
          </div>

          {/* Actions */}
          <div className="flex items-center gap-2 pt-1">
            <span className="flex-1" />
            <button onClick={onClose} className="px-3 py-1.5 rounded-lg text-[12px] text-muted-foreground hover:bg-accent transition-colors">
              Cancel
            </button>
            <button
              onClick={handleSave}
              disabled={saving || !title.trim() || !content.trim()}
              className="px-4 py-1.5 rounded-lg text-[12px] font-medium bg-primary/15 text-primary hover:bg-primary/25 transition-colors disabled:opacity-40"
            >
              {saving ? "Saving…" : "Save"}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}
