import { useEffect, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { CheckCircle2, Circle, Loader2, AlertCircle } from "lucide-react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { cn } from "@/lib/utils";
import { useChatStore } from "@/stores/chatStore";
import { MetaAgentSelector } from "./MetaAgentSelector";
import { markdownComponents } from "./chat/MarkdownComponents";

// eslint-disable-next-line @typescript-eslint/no-explicit-any
const REMARK_PLUGINS: any[] = [[remarkGfm, { singleTilde: false }]];

// ─── Types ────────────────────────────────────────────────────────────────────

interface StepPayload { step: number; label: string; done: boolean; }
interface PreviewPayload { claude_md: string; ref_index: string; has_existing_claude_md: boolean; }
interface ErrorPayload { message: string; }

// Flow: agent_select → loading → preview → done
// Overlays (cancel_confirm / skip_confirm) render on top of the host state.
type ModalState = "agent_select" | "loading" | "preview" | "cancel_confirm" | "skip_confirm" | "done";
type PreviewTab = "claude_md" | "ref_index";

interface Step { label: string; done: boolean; active: boolean; }

interface AgentConfig { engine: string; model: string; endpoint?: string; }

// ─── Component ───────────────────────────────────────────────────────────────

export function ProjectOnboardingModal() {
  const onboardingProject = useChatStore((s) => s.onboardingProject);
  const clearOnboardingProject = useChatStore((s) => s.clearOnboardingProject);

  const [modalState, setModalState] = useState<ModalState>("agent_select");
  const [steps, setSteps] = useState<Step[]>([
    { label: "프로젝트 스캔 중...", done: false, active: true },
    { label: "기존 문서 분석 중...", done: false, active: false },
    { label: "AI가 정리 중...", done: false, active: false },
  ]);
  const [preview, setPreview] = useState<PreviewPayload | null>(null);
  const [activeTab, setActiveTab] = useState<PreviewTab>("claude_md");
  const [error, setError] = useState<string | null>(null);
  const cleanupRef = useRef<(() => void)[]>([]);

  // Reset to initial "agent_select" phase when a new project arrives.
  useEffect(() => {
    if (!onboardingProject) return;
    setModalState("agent_select");
    setSteps([
      { label: "프로젝트 스캔 중...", done: false, active: true },
      { label: "기존 문서 분석 중...", done: false, active: false },
      { label: "AI가 정리 중...", done: false, active: false },
    ]);
    setPreview(null);
    setError(null);
    return () => { cleanupRef.current.forEach((u) => u()); cleanupRef.current = []; };
  }, [onboardingProject]);

  // Kick off analysis once the user confirms a meta-agent selection.
  const handleProceed = async (cfg: AgentConfig) => {
    if (!onboardingProject) return;
    setModalState("loading");
    setError(null);

    const unlistenStep = await listen<StepPayload>("project:onboarding:step", (e) => {
      const { step, label, done } = e.payload;
      setSteps((prev) => prev.map((s, i) => {
        if (i === step - 1) return { label, done, active: !done };
        if (i === step && !done) return { ...s, active: false };
        return s;
      }));
    });

    const unlistenPreview = await listen<PreviewPayload>("project:onboarding:preview", (e) => {
      setPreview(e.payload);
      setModalState("preview");
    });

    const unlistenError = await listen<ErrorPayload>("project:onboarding:error", (e) => {
      setError(e.payload.message);
    });

    cleanupRef.current = [unlistenStep, unlistenPreview, unlistenError];

    invoke("analyze_project_for_onboarding", {
      projectPath: onboardingProject.path,
      projectName: onboardingProject.name,
      engine: cfg.engine,
      model: cfg.model,
      endpoint: cfg.endpoint,
    }).catch((e) => setError(String(e)));
  };

  // Skip from the selector: default scaffolding (createProject) already ran,
  // so we just dismiss the modal.
  const handleSkipFromSelector = () => {
    cleanupRef.current.forEach((u) => u());
    setModalState("done");
    clearOnboardingProject();
  };

  const handleCancel = () => {
    invoke("cancel_project_onboarding").catch(() => {});
    cleanupRef.current.forEach((u) => u());
    setModalState("done");
    clearOnboardingProject();
  };

  const handleSkip = () => {
    cleanupRef.current.forEach((u) => u());
    setModalState("done");
    clearOnboardingProject();
  };

  const handleApply = async () => {
    if (!preview || !onboardingProject) return;
    try {
      await invoke("apply_project_onboarding", {
        projectPath: onboardingProject.path,
        claudeMdContent: preview.claude_md,
        refIndexContent: preview.ref_index,
      });
    } catch (e) {
      setError(String(e));
      return;
    }
    setModalState("done");
    clearOnboardingProject();
  };

  if (!onboardingProject || modalState === "done") return null;

  return (
    <div className="fixed inset-0 z-[100] flex items-center justify-center">
      {/* Backdrop */}
      <div className="absolute inset-0 bg-black/50" />

      {/* Modal */}
      <div className="relative z-10 w-[560px] max-h-[85vh] bg-background border border-border rounded-xl shadow-2xl flex flex-col overflow-hidden">

        {/* ── Agent select state (initial phase) ── */}
        {modalState === "agent_select" && (
          <MetaAgentSelector
            projectName={onboardingProject.name}
            onProceed={handleProceed}
            onSkip={handleSkipFromSelector}
          />
        )}

        {/* ── Loading state ── */}
        {(modalState === "loading" || modalState === "cancel_confirm") && (
          <>
            <div className="px-6 pt-5 pb-4 border-b border-border">
              <h2 className="text-sm font-semibold text-foreground">프로젝트 분석 중</h2>
              <p className="text-[11px] text-muted-foreground mt-0.5">{onboardingProject.name}</p>
            </div>

            <div className="px-6 py-5 space-y-3 flex-1">
              {steps.map((step, i) => (
                <div key={i} className="flex items-center gap-3">
                  {step.done ? (
                    <CheckCircle2 className="w-4 h-4 text-green-500 shrink-0" />
                  ) : step.active ? (
                    <Loader2 className="w-4 h-4 text-primary shrink-0 animate-spin" />
                  ) : (
                    <Circle className="w-4 h-4 text-muted-foreground/30 shrink-0" />
                  )}
                  <span className={cn(
                    "text-[12px]",
                    step.done ? "text-foreground/60" : step.active ? "text-foreground" : "text-muted-foreground/40"
                  )}>
                    {step.label}
                  </span>
                </div>
              ))}

              {error && (
                <div className="flex items-start gap-2 mt-3 p-3 rounded-lg bg-destructive/10 border border-destructive/20">
                  <AlertCircle className="w-4 h-4 text-destructive shrink-0 mt-0.5" />
                  <div>
                    <p className="text-[11px] font-medium text-destructive">분석 실패</p>
                    <p className="text-[10px] text-destructive/70 mt-0.5">{error}</p>
                    <p className="text-[10px] text-muted-foreground mt-1">건너뛰기를 눌러 빈 템플릿으로 시작할 수 있습니다.</p>
                  </div>
                </div>
              )}

              {!error && (
                <p className="text-[10px] text-muted-foreground/50 pt-1">
                  잠시 후 결과를 보여드립니다
                </p>
              )}
            </div>

            <div className="px-6 py-4 border-t border-border flex justify-between items-center">
              {error ? (
                <>
                  <button
                    onClick={() => setModalState("skip_confirm")}
                    className="text-[11px] text-muted-foreground hover:text-foreground transition-colors"
                  >
                    건너뛰기
                  </button>
                  <button
                    onClick={handleCancel}
                    className="px-3 py-1.5 text-[11px] font-medium text-destructive/80 hover:text-destructive transition-colors"
                  >
                    닫기
                  </button>
                </>
              ) : (
                <button
                  onClick={() => setModalState("cancel_confirm")}
                  className="ml-auto text-[11px] text-muted-foreground hover:text-foreground transition-colors"
                >
                  취소
                </button>
              )}
            </div>

            {/* Cancel confirm overlay */}
            {modalState === "cancel_confirm" && (
              <CancelConfirmOverlay
                onBack={() => setModalState("loading")}
                onConfirm={handleCancel}
              />
            )}
          </>
        )}

        {/* ── Preview state ── */}
        {(modalState === "preview" || modalState === "skip_confirm") && preview && (
          <>
            <div className="px-6 pt-5 pb-3 border-b border-border">
              <h2 className="text-sm font-semibold text-foreground">분석 완료</h2>
              <p className="text-[11px] text-muted-foreground mt-0.5">
                이렇게 정리하시겠습니까? 내용을 확인 후 적용하세요.
              </p>
            </div>

            {/* Tabs */}
            <div className="flex border-b border-border px-6">
              {(["claude_md", "ref_index"] as PreviewTab[]).map((tab) => (
                <button
                  key={tab}
                  onClick={() => setActiveTab(tab)}
                  className={cn(
                    "px-3 py-2 text-[11px] font-medium border-b-2 transition-colors -mb-px",
                    activeTab === tab
                      ? "border-primary text-foreground"
                      : "border-transparent text-muted-foreground hover:text-foreground"
                  )}
                >
                  {tab === "claude_md" ? "CLAUDE.md" : "docs/reference/index.md"}
                </button>
              ))}
            </div>

            {/* Content — render as Markdown so the preview matches how the
                 file will actually look in the editor / Docs viewer. */}
            <div className="flex-1 overflow-y-auto min-h-0">
              <div className="prose prose-invert prose-chat prose-sm max-w-none px-6 py-4 text-[12px] leading-relaxed [&>*:first-child]:mt-0 [&>*:last-child]:mb-0">
                <ReactMarkdown remarkPlugins={REMARK_PLUGINS} components={markdownComponents}>
                  {activeTab === "claude_md" ? preview.claude_md : preview.ref_index}
                </ReactMarkdown>
              </div>
            </div>

            <div className="px-6 py-4 border-t border-border">
              <div className="flex justify-between items-start">
                <div>
                  <button
                    onClick={() => setModalState("skip_confirm")}
                    className="text-[11px] text-muted-foreground hover:text-foreground transition-colors"
                  >
                    건너뛰기
                  </button>
                  <p className="text-[9px] text-muted-foreground/40 mt-0.5">
                    빈 템플릿 파일만 생성됩니다
                  </p>
                </div>
                <button
                  onClick={handleApply}
                  className="px-4 py-1.5 rounded-lg bg-primary text-primary-foreground text-[11px] font-medium hover:bg-primary/90 transition-colors"
                >
                  적용하기
                </button>
              </div>
            </div>

            {/* Skip confirm overlay */}
            {modalState === "skip_confirm" && (
              <SkipConfirmOverlay
                onBack={() => setModalState("preview")}
                onConfirm={handleSkip}
              />
            )}
          </>
        )}
      </div>
    </div>
  );
}

// ─── Sub-dialogs ─────────────────────────────────────────────────────────────

function CancelConfirmOverlay({ onBack, onConfirm }: { onBack: () => void; onConfirm: () => void }) {
  return (
    <div className="absolute inset-0 z-20 flex items-center justify-center bg-background/80 backdrop-blur-sm rounded-xl">
      <div className="w-[360px] bg-background border border-border rounded-lg shadow-xl p-5 space-y-4">
        <h3 className="text-sm font-semibold text-foreground">분석을 취소하시겠습니까?</h3>

        <div className="space-y-2 text-[11px]">
          <p className="text-muted-foreground font-medium">취소하면 다음 상태로 남습니다:</p>
          <ul className="space-y-1.5 pl-1">
            <li className="flex items-start gap-2 text-foreground/60">
              <span className="text-green-500 mt-0.5">✓</span>
              프로젝트 폴더 구조는 이미 생성됨
              <span className="text-[9px] text-muted-foreground/50">(docs/plans/, docs/reference/ 등)</span>
            </li>
            <li className="flex items-start gap-2 text-amber-500/80">
              <span className="mt-0.5">⚠</span>
              <span>CLAUDE.md는 빈 템플릿으로 남음 — 에이전트가 프로젝트 맥락 없이 시작</span>
            </li>
            <li className="flex items-start gap-2 text-amber-500/80">
              <span className="mt-0.5">⚠</span>
              <span>기존 문서 인덱스가 만들어지지 않음</span>
            </li>
          </ul>
          <p className="text-[10px] text-muted-foreground/50 pt-1">
            💡 설정 &gt; 프로젝트에서 나중에 다시 분석을 실행할 수 있습니다
          </p>
        </div>

        <div className="flex justify-between pt-1">
          <button
            onClick={onBack}
            className="px-3 py-1.5 rounded text-[11px] text-muted-foreground hover:text-foreground hover:bg-accent/50 transition-colors"
          >
            돌아가기
          </button>
          <button
            onClick={onConfirm}
            className="px-3 py-1.5 rounded text-[11px] font-medium text-destructive hover:bg-destructive/10 transition-colors"
          >
            그래도 취소
          </button>
        </div>
      </div>
    </div>
  );
}

function SkipConfirmOverlay({ onBack, onConfirm }: { onBack: () => void; onConfirm: () => void }) {
  return (
    <div className="absolute inset-0 z-20 flex items-center justify-center bg-background/80 backdrop-blur-sm rounded-xl">
      <div className="w-[380px] bg-background border border-border rounded-lg shadow-xl p-5 space-y-4">
        <h3 className="text-sm font-semibold text-foreground">AI 분석 결과를 건너뛰시겠습니까?</h3>

        <div className="space-y-2 text-[11px] text-muted-foreground">
          <p>대신 아래 파일이 빈 양식으로 생성됩니다:</p>
          <ul className="space-y-1 pl-2 font-mono text-foreground/60">
            <li>· CLAUDE.md (섹션 구조만 있음)</li>
            <li>· docs/reference/index.md (빈 목록)</li>
          </ul>
          <p className="pt-1">
            에이전트는 프로젝트를 처음 실행할 때 직접 코드를 탐색해 맥락을 파악합니다.
          </p>
          <p className="text-[10px] text-muted-foreground/50">
            rawq 인덱싱은 백그라운드에서 계속 진행됩니다
          </p>
        </div>

        <div className="flex justify-between pt-1">
          <button
            onClick={onBack}
            className="px-3 py-1.5 rounded text-[11px] text-muted-foreground hover:text-foreground hover:bg-accent/50 transition-colors"
          >
            돌아가기
          </button>
          <button
            onClick={onConfirm}
            className="px-3 py-1.5 rounded text-[11px] font-medium text-muted-foreground hover:text-foreground hover:bg-accent/50 transition-colors"
          >
            건너뛰기
          </button>
        </div>
      </div>
    </div>
  );
}
