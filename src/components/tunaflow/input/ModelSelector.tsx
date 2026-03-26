import type { EngineModel } from "@/types";

interface ModelSelectorProps {
  currentModels: EngineModel[];
  selectedModel: string;
  setSelectedModel: (v: string) => void;
}

export function ModelSelector({ currentModels, selectedModel, setSelectedModel }: ModelSelectorProps) {
  if (currentModels.length === 0) return null;

  return (
    <>
      <span className="h-3 w-px bg-border/30" />
      <select
        value={selectedModel}
        onChange={(e) => setSelectedModel(e.target.value)}
        className="bg-transparent rounded px-1 py-0.5 text-[10px] outline-none text-muted-foreground/50 max-w-[120px]"
      >
        {currentModels.map((m) => (
          <option key={m.id} value={m.id}>
            {m.recommended ? "★ " : ""}{m.label}
          </option>
        ))}
      </select>
    </>
  );
}
