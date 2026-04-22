import { useEffect, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { toast } from "sonner";
import { Globe, FileText } from "lucide-react";

const WORLDVIEW_MAX_TOKENS = 500;

const DEFAULT_WORLDVIEW_TEMPLATE = `# User Worldview

## Ontology
(기본 세계관 — 직접 작성)

## Engagement preference
(agent 와의 협업 방식 선호 — 직접 작성)
`;

// guardrail::estimate_tokens 와 동일한 휴리스틱 (ascii/4 + cjk*2/3).
function estimateTokens(text: string): number {
  let ascii = 0;
  let cjk = 0;
  for (const ch of text) {
    const code = ch.codePointAt(0) ?? 0;
    const isCjk =
      (code >= 0x4e00 && code <= 0x9fff) ||
      (code >= 0x3040 && code <= 0x30ff) ||
      (code >= 0xac00 && code <= 0xd7af);
    if (isCjk) cjk += 1;
    else ascii += 1;
  }
  return Math.floor(ascii / 4) + Math.floor((cjk * 2) / 3);
}

export function WorldviewSettings() {
  const [content, setContent] = useState<string>("");
  const [saved, setSaved] = useState<boolean>(true);
  const [loading, setLoading] = useState<boolean>(true);
  const [enabled, setEnabled] = useState<boolean>(true);
  const [appliedPath, setAppliedPath] = useState<string | null>(null);

  useEffect(() => {
    (async () => {
      try {
        const [wv, path, en] = await Promise.all([
          invoke<string | null>("get_worldview", { input: { projectPath: null } }),
          invoke<string | null>("get_worldview_path", { input: { projectPath: null } }),
          invoke<boolean>("get_worldview_enabled"),
        ]);
        setContent(wv ?? "");
        setAppliedPath(path);
        setEnabled(en);
        setSaved(true);
      } catch (err) {
        console.error("[worldview] load failed", err);
        toast.error("Worldview 로드 실패");
      } finally {
        setLoading(false);
      }
    })();
  }, []);

  const tokens = estimateTokens(content);
  const overLimit = tokens > WORLDVIEW_MAX_TOKENS;

  const handleSave = async () => {
    try {
      await invoke("set_worldview", { input: { content, projectPath: null } });
      setSaved(true);
      const newPath = await invoke<string | null>("get_worldview_path", {
        input: { projectPath: null },
      });
      setAppliedPath(newPath);
      toast.success("Worldview 저장됨 — 다음 요청부터 적용");
    } catch (err) {
      console.error("[worldview] save failed", err);
      toast.error(`저장 실패: ${err}`);
    }
  };

  const handleLoadDefault = () => {
    setContent(DEFAULT_WORLDVIEW_TEMPLATE);
    setSaved(false);
  };

  const handleToggleEnabled = async (next: boolean) => {
    try {
      await invoke("set_worldview_enabled", { input: { enabled: next } });
      setEnabled(next);
      toast.success(next ? "Worldview 주입 활성화" : "Worldview 주입 비활성화");
    } catch (err) {
      console.error("[worldview] toggle failed", err);
      toast.error(`토글 실패: ${err}`);
    }
  };

  return (
    <div className="space-y-5">
      <div>
        <h2 className="text-[14px] font-[550] text-foreground mb-1 flex items-center gap-2">
          <Globe className="w-4 h-4" />
          User Worldview
        </h2>
        <p className="text-[12px] text-muted-foreground leading-relaxed">
          에이전트가 매 요청 시 ContextPack 의 identity 바로 앞에서 참조하는 사용자 stance 문서입니다.
          최대 {WORLDVIEW_MAX_TOKENS} tokens.
        </p>
      </div>

      {/* Toggle */}
      <label className="flex items-center gap-2 text-[12px] text-foreground/80 cursor-pointer">
        <input
          type="checkbox"
          checked={enabled}
          onChange={(e) => handleToggleEnabled(e.target.checked)}
          className="accent-primary"
        />
        Worldview 주입 활성화
      </label>

      {/* Editor */}
      <div className="space-y-2">
        <textarea
          value={content}
          disabled={loading}
          onChange={(e) => {
            setContent(e.target.value);
            setSaved(false);
          }}
          placeholder="(비어있음 — 기본 문구 로드 또는 직접 작성)"
          className="w-full h-72 font-mono text-[12px] bg-background border border-border/40 rounded-md px-3 py-2 text-foreground placeholder:text-muted-foreground/40 focus:outline-none focus:border-ring/50 resize-none"
        />
        <div className="flex items-center justify-between text-[11px]">
          <span className={overLimit ? "text-red-500 font-medium" : "text-muted-foreground/70"}>
            {tokens} / {WORLDVIEW_MAX_TOKENS} tokens
            {overLimit ? " — 앞부분만 주입됨" : ""}
          </span>
          {appliedPath && (
            <span className="text-muted-foreground/60 inline-flex items-center gap-1 truncate max-w-[60%]">
              <FileText className="w-3 h-3 shrink-0" />
              <span className="truncate" title={appliedPath}>{appliedPath}</span>
            </span>
          )}
        </div>
      </div>

      {/* Actions */}
      <div className="flex items-center gap-2">
        <button
          onClick={handleSave}
          disabled={saved || loading}
          className="px-3 py-1.5 text-[12px] font-medium rounded-md bg-primary text-primary-foreground hover:bg-primary/90 disabled:opacity-50 disabled:cursor-not-allowed transition-colors"
        >
          저장
        </button>
        <button
          onClick={handleLoadDefault}
          disabled={loading}
          className="px-3 py-1.5 text-[12px] font-medium rounded-md text-foreground/70 hover:text-foreground hover:bg-accent transition-colors"
        >
          기본 문구 로드
        </button>
      </div>
    </div>
  );
}
