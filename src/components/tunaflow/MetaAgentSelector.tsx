import { useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { CheckCircle2, Loader2, AlertTriangle, ChevronDown, ExternalLink } from "lucide-react";
import { useTranslation } from "react-i18next";
import { cn } from "@/lib/utils";

// ─── Types ───────────────────────────────────────────────────────────────────

interface AgentDetection {
  engine: string;                  // claude / codex / gemini / ollama / lmstudio
  kind: "cli" | "http";
  installed: boolean;
  version?: string | null;
  path?: string | null;
  endpoint?: string | null;
  models: string[];
  note?: string | null;
}

interface SelectedConfig {
  engine: string;
  model: string;
  endpoint?: string;
}

interface Props {
  onProceed: (config: SelectedConfig) => void;
  onSkip: () => void;
  projectName: string;
}

// ─── Engine metadata (for display) ───────────────────────────────────────────

interface EngineMeta {
  label: string;
  installHintKey: string; // dialog.meta_agent.* key (falls back to plain installHint when provided)
  installHint?: string;
  docLink?: string;
  defaultEndpoint?: string;
}

const ENGINE_META: Record<string, EngineMeta> = {
  claude:   { label: "Claude",    installHintKey: "", installHint: "npm install -g @anthropic-ai/claude-code", docLink: "https://docs.claude.com/en/docs/claude-code" },
  codex:    { label: "Codex",     installHintKey: "", installHint: "npm install -g @openai/codex-cli",        docLink: "https://openai.com/index/codex" },
  gemini:   { label: "Gemini",    installHintKey: "", installHint: "npm install -g @google/gemini-cli",        docLink: "https://ai.google.dev/gemini-api/docs/cli" },
  ollama:   { label: "Ollama",    installHintKey: "ollama_install_hint",   defaultEndpoint: "http://localhost:11434" },
  lmstudio: { label: "LM Studio", installHintKey: "lmstudio_install_hint", defaultEndpoint: "http://localhost:1234/v1" },
};

// Default model candidates for CLI engines (no live enumeration).
const CLI_DEFAULT_MODELS: Record<string, string[]> = {
  claude: ["claude-opus-4-6", "claude-sonnet-4-6", "claude-haiku-4-5"],
  codex:  ["gpt-5-codex", "gpt-4o-codex"],
  gemini: ["gemini-2.5-pro", "gemini-2.5-flash"],
};

// ─── Component ───────────────────────────────────────────────────────────────

export function MetaAgentSelector({ onProceed, onSkip, projectName }: Props) {
  const { t } = useTranslation("dialog");
  const [detections, setDetections] = useState<AgentDetection[] | null>(null);
  const [ollamaEndpoint, setOllamaEndpoint] = useState("http://localhost:11434");
  const [lmstudioEndpoint, setLmstudioEndpoint] = useState("http://localhost:1234/v1");

  const [selectedEngine, setSelectedEngine] = useState<string | null>(null);
  const [modelByEngine, setModelByEngine] = useState<Record<string, string>>({});

  const [skipConfirm, setSkipConfirm] = useState(false);
  const debounceRef = useRef<number | null>(null);

  // Initial + on-endpoint-change detection
  const runDetect = async (oEp: string, lEp: string) => {
    setDetections(null);
    try {
      const result = await invoke<AgentDetection[]>("detect_available_agents", {
        ollamaEndpoint: oEp,
        lmstudioEndpoint: lEp,
      });
      setDetections(result);

      // Auto-pick a default model per engine (preserve existing choice if still valid)
      setModelByEngine((prev) => {
        const next = { ...prev };
        for (const d of result) {
          if (!next[d.engine]) {
            const list = d.models.length > 0 ? d.models : (CLI_DEFAULT_MODELS[d.engine] ?? []);
            if (list.length > 0) next[d.engine] = list[0];
          }
        }
        return next;
      });
    } catch (e) {
      console.error("[agent-detect]", e);
      setDetections([]);
    }
  };

  useEffect(() => {
    runDetect(ollamaEndpoint, lmstudioEndpoint);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const onEndpointChange = (engine: "ollama" | "lmstudio", value: string) => {
    if (engine === "ollama") setOllamaEndpoint(value);
    else setLmstudioEndpoint(value);

    if (debounceRef.current) window.clearTimeout(debounceRef.current);
    debounceRef.current = window.setTimeout(() => {
      const o = engine === "ollama" ? value : ollamaEndpoint;
      const l = engine === "lmstudio" ? value : lmstudioEndpoint;
      runDetect(o, l);
    }, 600);
  };

  const canProceed = useMemo(() => {
    if (!selectedEngine) return false;
    const model = modelByEngine[selectedEngine];
    if (!model) return false;
    const det = detections?.find((d) => d.engine === selectedEngine);
    if (!det) return false;
    if (det.kind === "cli") return det.installed;
    // http: require installed (endpoint reachable) AND at least one model
    return det.installed && det.models.length > 0;
  }, [selectedEngine, modelByEngine, detections]);

  const handleProceed = () => {
    if (!canProceed || !selectedEngine) return;
    const det = detections!.find((d) => d.engine === selectedEngine)!;
    const endpoint = det.kind === "http"
      ? (selectedEngine === "ollama" ? ollamaEndpoint : lmstudioEndpoint)
      : undefined;
    onProceed({
      engine: selectedEngine,
      model: modelByEngine[selectedEngine],
      endpoint,
    });
  };

  return (
    <>
      <div className="px-6 pt-5 pb-4 border-b border-border">
        <h2 className="text-sm font-semibold text-foreground">{t("meta_agent.title")}</h2>
        <p className="text-[11px] text-muted-foreground mt-0.5">
          {t("meta_agent.description_prefix")}
          <span className="font-mono">{projectName}</span>
          {t("meta_agent.description_suffix")}
        </p>
      </div>

      <div className="flex-1 overflow-y-auto min-h-0 px-6 py-4 space-y-3">
        {detections === null && (
          <div className="flex items-center gap-2 text-[11px] text-muted-foreground py-4">
            <Loader2 className="w-3.5 h-3.5 animate-spin" />
            {t("meta_agent.detecting")}
          </div>
        )}

        {detections !== null && detections.map((d) => {
          const meta = ENGINE_META[d.engine];
          const modelList = d.models.length > 0 ? d.models : (CLI_DEFAULT_MODELS[d.engine] ?? []);
          const isSelected = selectedEngine === d.engine;
          const selectable = d.installed && modelList.length > 0;

          return (
            <div
              key={d.engine}
              className={cn(
                "rounded-lg border p-3 transition-colors",
                isSelected ? "border-primary bg-primary/5" : "border-border hover:border-border/80",
                !selectable && "opacity-60"
              )}
            >
              <label className="flex items-start gap-3 cursor-pointer">
                <input
                  type="radio"
                  name="meta-agent"
                  checked={isSelected}
                  disabled={!selectable}
                  onChange={() => setSelectedEngine(d.engine)}
                  className="mt-1 accent-primary"
                />
                <div className="flex-1 min-w-0 space-y-1.5">
                  <div className="flex items-center gap-2">
                    <span className="text-[12px] font-semibold text-foreground">{meta?.label ?? d.engine}</span>
                    {d.installed ? (
                      <span className="inline-flex items-center gap-1 text-[9px] px-1.5 py-0.5 rounded bg-green-500/10 text-green-500 font-medium">
                        <CheckCircle2 className="w-2.5 h-2.5" /> detected
                      </span>
                    ) : (
                      <span className="inline-flex items-center gap-1 text-[9px] px-1.5 py-0.5 rounded bg-muted/40 text-muted-foreground/70 font-medium">
                        <AlertTriangle className="w-2.5 h-2.5" /> not found
                      </span>
                    )}
                    {d.version && <span className="text-[9px] text-muted-foreground/60 font-mono">{d.version}</span>}
                  </div>

                  {/* CLI details */}
                  {d.kind === "cli" && d.path && (
                    <div className="text-[10px] text-muted-foreground/70 font-mono truncate">{d.path}</div>
                  )}

                  {/* HTTP endpoint editor */}
                  {d.kind === "http" && (
                    <div className="flex items-center gap-2">
                      <span className="text-[10px] text-muted-foreground/60 shrink-0">Endpoint</span>
                      <input
                        type="text"
                        value={d.engine === "ollama" ? ollamaEndpoint : lmstudioEndpoint}
                        onChange={(e) => onEndpointChange(d.engine as "ollama" | "lmstudio", e.target.value)}
                        className="flex-1 text-[10px] font-mono bg-background border border-border/60 rounded px-2 py-1 focus:outline-none focus:border-primary/60"
                      />
                    </div>
                  )}

                  {/* Model selector */}
                  {selectable && modelList.length > 0 && (
                    <div className="flex items-center gap-2">
                      <span className="text-[10px] text-muted-foreground/60 shrink-0">Model</span>
                      <div className="relative flex-1">
                        <select
                          value={modelByEngine[d.engine] ?? modelList[0]}
                          onChange={(e) => setModelByEngine((prev) => ({ ...prev, [d.engine]: e.target.value }))}
                          onClick={() => { if (selectable) setSelectedEngine(d.engine); }}
                          className="w-full text-[10px] font-mono bg-background border border-border/60 rounded px-2 py-1 pr-6 appearance-none focus:outline-none focus:border-primary/60"
                        >
                          {modelList.map((m) => (<option key={m} value={m}>{m}</option>))}
                        </select>
                        <ChevronDown className="w-3 h-3 text-muted-foreground/50 absolute right-1.5 top-1/2 -translate-y-1/2 pointer-events-none" />
                      </div>
                    </div>
                  )}

                  {/* Install hint when not found */}
                  {!d.installed && meta && (meta.installHint || meta.installHintKey) && (
                    <div className="text-[10px] text-muted-foreground/70 mt-1 space-y-0.5">
                      <div>
                        {t("meta_agent.install_label")}{" "}
                        <span className="font-mono text-muted-foreground">
                          {meta.installHintKey === "ollama_install_hint"
                            ? t("meta_agent.ollama_install_hint")
                            : meta.installHintKey === "lmstudio_install_hint"
                            ? t("meta_agent.lmstudio_install_hint")
                            : meta.installHint}
                        </span>
                      </div>
                      {meta.docLink && (
                        <a
                          href={meta.docLink}
                          target="_blank"
                          rel="noreferrer"
                          className="inline-flex items-center gap-1 text-primary/70 hover:text-primary"
                        >
                          <ExternalLink className="w-2.5 h-2.5" />
                          {t("meta_agent.install_guide")}
                        </a>
                      )}
                    </div>
                  )}

                  {/* Failure note */}
                  {!d.installed && d.note && d.kind === "http" && (
                    <div className="text-[10px] text-muted-foreground/50 italic">{d.note}</div>
                  )}
                </div>
              </label>
            </div>
          );
        })}
      </div>

      <div className="px-6 py-4 border-t border-border flex justify-between items-center">
        <button
          onClick={() => setSkipConfirm(true)}
          className="text-[11px] text-muted-foreground hover:text-foreground transition-colors"
        >
          {t("meta_agent.skip_button")}
        </button>
        <button
          onClick={handleProceed}
          disabled={!canProceed}
          className={cn(
            "px-4 py-1.5 rounded-lg text-[11px] font-medium transition-colors",
            canProceed
              ? "bg-primary text-primary-foreground hover:bg-primary/90"
              : "bg-muted text-muted-foreground/40 cursor-not-allowed"
          )}
        >
          {t("meta_agent.proceed_button")}
        </button>
      </div>

      {skipConfirm && (
        <SkipWithBasicOverlay
          onBack={() => setSkipConfirm(false)}
          onConfirm={onSkip}
        />
      )}
    </>
  );
}

// ─── Skip-with-basic-scaffold confirmation ───────────────────────────────────

function SkipWithBasicOverlay({ onBack, onConfirm }: { onBack: () => void; onConfirm: () => void }) {
  const { t } = useTranslation("dialog");
  return (
    <div className="absolute inset-0 z-20 flex items-center justify-center bg-background/80 backdrop-blur-sm rounded-xl">
      <div className="w-[400px] bg-background border border-border rounded-lg shadow-xl p-5 space-y-4">
        <h3 className="text-sm font-semibold text-foreground">{t("meta_agent.skip_overlay_title")}</h3>

        <div className="space-y-3 text-[11px] text-muted-foreground leading-relaxed">
          <div>
            <p className="text-foreground/80 font-medium mb-1">{t("meta_agent.skip_overlay_with_heading")}</p>
            <ul className="space-y-1 pl-3 list-disc list-outside marker:text-primary/50">
              <li>
                {t("meta_agent.skip_overlay_with_item_1_before")}
                <span className="text-foreground/70">{t("meta_agent.skip_overlay_with_item_1_highlight")}</span>
                {t("meta_agent.skip_overlay_with_item_1_after")}
              </li>
              <li>
                {t("meta_agent.skip_overlay_with_item_2_before")}
                <span className="text-foreground/70">{t("meta_agent.skip_overlay_with_item_2_highlight")}</span>
                {t("meta_agent.skip_overlay_with_item_2_after")}
              </li>
            </ul>
          </div>
          <div>
            <p className="text-foreground/80 font-medium mb-1">{t("meta_agent.skip_overlay_without_heading")}</p>
            <ul className="space-y-1 pl-3 list-disc list-outside marker:text-muted-foreground/50">
              <li>
                <span className="font-mono text-foreground/70">{t("meta_agent.skip_overlay_without_item_1_highlight")}</span>
                {t("meta_agent.skip_overlay_without_item_1_after")}
              </li>
              <li>{t("meta_agent.skip_overlay_without_item_2")}</li>
              <li>{t("meta_agent.skip_overlay_without_item_3")}</li>
            </ul>
          </div>
        </div>

        <div className="flex justify-between pt-1">
          <button
            onClick={onBack}
            className="px-3 py-1.5 rounded text-[11px] text-muted-foreground hover:text-foreground hover:bg-accent/50 transition-colors"
          >
            {t("meta_agent.skip_overlay_back")}
          </button>
          <button
            onClick={onConfirm}
            className="px-3 py-1.5 rounded text-[11px] font-medium text-foreground hover:bg-accent/50 transition-colors"
          >
            {t("meta_agent.skip_overlay_confirm")}
          </button>
        </div>
      </div>
    </div>
  );
}
