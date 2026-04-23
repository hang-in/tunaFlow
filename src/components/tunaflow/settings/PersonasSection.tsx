import { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { cn } from "@/lib/utils";
import { Plus, Trash2 } from "lucide-react";
import { getSetting, setSetting } from "@/lib/appStore";
import { DEFAULT_PERSONAS } from "@/lib/defaultPersonas";
import type { Persona } from "@/types";

export function PersonasSection() {
  const { t } = useTranslation("settings");
  const [personas, setPersonas] = useState<Persona[]>([]);
  const [selectedId, setSelectedId] = useState<string | null>(null);

  useEffect(() => {
    getSetting<Persona[]>("personas", DEFAULT_PERSONAS).then((p) => {
      setPersonas(p);
      if (p.length > 0) setSelectedId(p[0].id);
    });
  }, []);

  const save = (next: Persona[]) => {
    setPersonas(next);
    setSetting("personas", next);
  };

  const selected = personas.find((p) => p.id === selectedId);

  const updateField = <K extends keyof Persona>(field: K, value: Persona[K]) => {
    if (!selectedId) return;
    save(personas.map((p) => p.id === selectedId ? { ...p, [field]: value } : p));
  };

  const updateArrayField = (field: "priorities" | "behaviors" | "constraints", idx: number, value: string) => {
    if (!selected) return;
    const arr = [...selected[field]];
    arr[idx] = value;
    updateField(field, arr);
  };

  const addToArray = (field: "priorities" | "behaviors" | "constraints") => {
    if (!selected) return;
    updateField(field, [...selected[field], ""]);
  };

  const removeFromArray = (field: "priorities" | "behaviors" | "constraints", idx: number) => {
    if (!selected) return;
    updateField(field, selected[field].filter((_, i) => i !== idx));
  };

  return (
    <div>
      <h2 className="text-[14px] font-[550] text-foreground mb-1">{t("personas.heading")}</h2>
      <p className="text-[12px] text-muted-foreground mb-4">{t("personas.description")}</p>

      <div className="flex gap-4 min-h-[300px]">
        <div className="w-[160px] shrink-0 space-y-1">
          {personas.map((p) => (
            <button key={p.id} onClick={() => setSelectedId(p.id)}
              className={cn("w-full text-left px-3 py-2 rounded-lg transition-colors",
                selectedId === p.id ? "bg-background text-foreground" : "text-muted-foreground hover:bg-background/50")}>
              <span className="text-[12px] font-medium block truncate">{p.name}</span>
              <span className="text-[10px] text-muted-foreground/50 block truncate">{p.role}</span>
            </button>
          ))}
        </div>

        {selected ? (
          <div className="flex-1 min-w-0 space-y-3 overflow-y-auto max-h-[400px] pr-1">
            <div className="flex gap-3">
              <div className="flex-1">
                <label className="text-[11px] text-muted-foreground mb-1 block">Name</label>
                <input value={selected.name} onChange={(e) => updateField("name", e.target.value)}
                  disabled={selected.builtIn}
                  className="w-full bg-background rounded-lg px-3 py-1.5 text-[13px] font-medium outline-none border border-border/30 focus:border-ring/40 disabled:opacity-50" />
              </div>
              <div className="flex-1">
                <label className="text-[11px] text-muted-foreground mb-1 block">Role</label>
                <input value={selected.role} onChange={(e) => updateField("role", e.target.value)}
                  className="w-full bg-background rounded-lg px-3 py-1.5 text-[12px] outline-none border border-border/30 focus:border-ring/40" />
              </div>
            </div>

            <div>
              <label className="text-[11px] text-muted-foreground mb-1 block">Summary</label>
              <input value={selected.summary} onChange={(e) => updateField("summary", e.target.value)}
                className="w-full bg-background rounded-lg px-3 py-1.5 text-[12px] outline-none border border-border/30 focus:border-ring/40" />
            </div>

            <div className="flex gap-3">
              <div className="flex-1">
                <label className="text-[11px] text-muted-foreground mb-1 block">Tone</label>
                <select value={selected.tone} onChange={(e) => updateField("tone", e.target.value)}
                  className="w-full bg-background rounded-lg px-3 py-1.5 text-[12px] outline-none border border-border/30 focus:border-ring/40 cursor-pointer">
                  <option value="direct">Direct</option>
                  <option value="analytical">Analytical</option>
                  <option value="critical">Critical</option>
                  <option value="formal">Formal</option>
                </select>
              </div>
              <div className="flex-1">
                <label className="text-[11px] text-muted-foreground mb-1 block">Output Style</label>
                <select value={selected.outputStyle} onChange={(e) => updateField("outputStyle", e.target.value)}
                  className="w-full bg-background rounded-lg px-3 py-1.5 text-[12px] outline-none border border-border/30 focus:border-ring/40 cursor-pointer">
                  <option value="structured">Structured</option>
                  <option value="brief">Brief</option>
                  <option value="checklist">Checklist</option>
                  <option value="diff_first">Diff First</option>
                </select>
              </div>
            </div>

            {(["priorities", "behaviors", "constraints"] as const).map((field) => (
              <div key={field}>
                <label className="text-[11px] text-muted-foreground mb-1 block capitalize">{field} ({selected[field].length})</label>
                <div className="space-y-1">
                  {selected[field].map((item, i) => (
                    <div key={i} className="flex gap-1">
                      <input value={item} onChange={(e) => updateArrayField(field, i, e.target.value)}
                        className="flex-1 bg-background rounded px-2 py-1 text-[11px] outline-none border border-border/30 focus:border-ring/40" />
                      <button onClick={() => removeFromArray(field, i)}
                        className="p-1 rounded text-muted-foreground/30 hover:text-destructive transition-colors shrink-0">
                        <Trash2 className="w-3 h-3" />
                      </button>
                    </div>
                  ))}
                  <button onClick={() => addToArray(field)}
                    className="text-[10px] text-primary/60 hover:text-primary transition-colors">
                    <Plus className="w-3 h-3 inline mr-0.5" />Add
                  </button>
                </div>
              </div>
            ))}

            <div>
              <label className="text-[11px] text-muted-foreground mb-1 block">Prompt Fragment</label>
              <textarea value={selected.promptFragment} onChange={(e) => updateField("promptFragment", e.target.value)}
                rows={3}
                className="w-full bg-background rounded-lg px-3 py-2 text-[11px] font-mono outline-none border border-border/30 focus:border-ring/40 resize-none" />
            </div>

            {selected.builtIn && (
              <p className="text-[10px] text-muted-foreground/30 italic">Built-in persona. Name cannot be changed.</p>
            )}
          </div>
        ) : (
          <div className="flex-1 flex items-center justify-center text-muted-foreground/30 text-[13px]">
            Select a persona
          </div>
        )}
      </div>
    </div>
  );
}
