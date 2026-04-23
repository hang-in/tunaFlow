import { useEffect, useMemo, useRef, useState } from "react";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import { CheckCircle2, Circle, Loader2, AlertCircle } from "lucide-react";
import ReactMarkdown from "react-markdown";
import remarkGfm from "remark-gfm";
import { useTranslation } from "react-i18next";
import { cn } from "@/lib/utils";
import { useChatStore } from "@/stores/chatStore";
import { MetaAgentSelector } from "./MetaAgentSelector";
import { markdownComponents } from "./chat/MarkdownComponents";
import {
  applyInitialSetup,
  normalizeInitialSetup,
  type InitialSetupPayload,
  type InitialSetupSelection,
} from "@/lib/initialSetupApply";

// eslint-disable-next-line @typescript-eslint/no-explicit-any
const REMARK_PLUGINS: any[] = [[remarkGfm, { singleTilde: false }]];

// ─── Types ────────────────────────────────────────────────────────────────────

interface StepPayload { step: number; label: string; done: boolean; }
interface PreviewPayload {
  claude_md: string;
  ref_index: string;
  has_existing_claude_md: boolean;
  initial_setup?: unknown;
}
interface ErrorPayload { message: string; }

// Flow: agent_select → loading → preview → done
// Overlays (cancel_confirm / skip_confirm) render on top of the host state.
type ModalState = "agent_select" | "loading" | "preview" | "cancel_confirm" | "skip_confirm" | "done";
type PreviewTab = "claude_md" | "ref_index" | "initial_setup";

interface Step { label: string; done: boolean; active: boolean; }

interface AgentConfig { engine: string; model: string; endpoint?: string; }

// ─── Component ───────────────────────────────────────────────────────────────

export function ProjectOnboardingModal() {
  const { t } = useTranslation("dialog");
  const onboardingProject = useChatStore((s) => s.onboardingProject);
  const clearOnboardingProject = useChatStore((s) => s.clearOnboardingProject);

  const [modalState, setModalState] = useState<ModalState>("agent_select");
  const [steps, setSteps] = useState<Step[]>([
    { label: t("onboarding.step_scan"), done: false, active: true },
    { label: t("onboarding.step_analyze"), done: false, active: false },
    { label: t("onboarding.step_summarize"), done: false, active: false },
  ]);
  const [preview, setPreview] = useState<PreviewPayload | null>(null);
  const [activeTab, setActiveTab] = useState<PreviewTab>("claude_md");
  const [error, setError] = useState<string | null>(null);
  const cleanupRef = useRef<(() => void)[]>([]);

  // Initial-setup selections (only populated when preview.initial_setup parses)
  const [profileSelection, setProfileSelection] = useState<Set<number>>(new Set());
  const [skillSelection, setSkillSelection] = useState<Set<string>>(new Set());
  const [applyWorkflow, setApplyWorkflow] = useState(true);

  // Store access — needed by applyInitialSetup
  const agentProfiles = useChatStore((s) => s.agentProfiles);
  const activeSkills = useChatStore((s) => s.activeSkills);
  const skills = useChatStore((s) => s.skills);
  const saveProfiles = useChatStore((s) => s.saveProfiles);
  const acceptRecommendedSkills = useChatStore((s) => s.acceptRecommendedSkills);

  const normalizedInitialSetup = useMemo<InitialSetupPayload | null>(() => {
    if (!preview?.initial_setup) return null;
    return normalizeInitialSetup(preview.initial_setup);
  }, [preview]);

  // When a new preview arrives, pre-check everything that looks valid — users
  // can uncheck to opt out. Matches plan §4 UX expectation.
  useEffect(() => {
    if (!normalizedInitialSetup) {
      setProfileSelection(new Set());
      setSkillSelection(new Set());
      setApplyWorkflow(true);
      return;
    }
    const installedSkillNames = new Set(skills.map((s) => s.name));
    setProfileSelection(new Set(normalizedInitialSetup.agent_profiles?.map((_, i) => i) ?? []));
    setSkillSelection(new Set((normalizedInitialSetup.skills ?? []).filter((n) => installedSkillNames.has(n))));
    setApplyWorkflow(!!normalizedInitialSetup.workflow);
  }, [normalizedInitialSetup, skills]);

  // Reset to initial "agent_select" phase when a new project arrives.
  useEffect(() => {
    if (!onboardingProject) return;
    setModalState("agent_select");
    setSteps([
      { label: t("onboarding.step_scan"), done: false, active: true },
      { label: t("onboarding.step_analyze"), done: false, active: false },
      { label: t("onboarding.step_summarize"), done: false, active: false },
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
      // Apply initial-setup selections (best effort — individual failures are
      // logged but don't block the onboarding).
      if (normalizedInitialSetup) {
        const selection: InitialSetupSelection = {
          profileIndices: profileSelection,
          skills: skillSelection,
          applyWorkflow,
        };
        try {
          await applyInitialSetup(normalizedInitialSetup, selection, {
            currentProfiles: agentProfiles,
            saveProfiles,
            currentActiveSkills: activeSkills,
            setActiveSkills: (names) => acceptRecommendedSkills(names),
            availableSkillNames: new Set(skills.map((s) => s.name)),
          });
        } catch (e) {
          console.error("[onboarding] initialSetup apply failed:", e);
        }
      }
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
              <h2 className="text-sm font-semibold text-foreground">{t("onboarding.analyzing_title")}</h2>
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
                    <p className="text-[11px] font-medium text-destructive">{t("onboarding.analysis_failed_title")}</p>
                    <p className="text-[10px] text-destructive/70 mt-0.5">{error}</p>
                    <p className="text-[10px] text-muted-foreground mt-1">{t("onboarding.analysis_failed_hint")}</p>
                  </div>
                </div>
              )}

              {!error && (
                <p className="text-[10px] text-muted-foreground/50 pt-1">
                  {t("onboarding.wait_hint")}
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
                    {t("onboarding.skip_button")}
                  </button>
                  <button
                    onClick={handleCancel}
                    className="px-3 py-1.5 text-[11px] font-medium text-destructive/80 hover:text-destructive transition-colors"
                  >
                    {t("onboarding.close_button")}
                  </button>
                </>
              ) : (
                <button
                  onClick={() => setModalState("cancel_confirm")}
                  className="ml-auto text-[11px] text-muted-foreground hover:text-foreground transition-colors"
                >
                  {t("onboarding.cancel_button")}
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
              <h2 className="text-sm font-semibold text-foreground">{t("onboarding.analysis_done_title")}</h2>
              <p className="text-[11px] text-muted-foreground mt-0.5">
                {t("onboarding.analysis_done_desc")}
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
              {normalizedInitialSetup && (
                <button
                  onClick={() => setActiveTab("initial_setup")}
                  className={cn(
                    "px-3 py-2 text-[11px] font-medium border-b-2 transition-colors -mb-px",
                    activeTab === "initial_setup"
                      ? "border-primary text-foreground"
                      : "border-transparent text-muted-foreground hover:text-foreground"
                  )}
                >
                  {t("onboarding.tab_initial_setup")}
                </button>
              )}
            </div>

            {/* Content — render as Markdown so the preview matches how the
                 file will actually look in the editor / Docs viewer. */}
            <div className="flex-1 overflow-y-auto min-h-0">
              {activeTab === "initial_setup" && normalizedInitialSetup ? (
                <InitialSetupPanel
                  payload={normalizedInitialSetup}
                  installedSkillNames={new Set(skills.map((s) => s.name))}
                  profileSelection={profileSelection}
                  setProfileSelection={setProfileSelection}
                  skillSelection={skillSelection}
                  setSkillSelection={setSkillSelection}
                  applyWorkflow={applyWorkflow}
                  setApplyWorkflow={setApplyWorkflow}
                />
              ) : (
                <div className="prose prose-invert prose-chat prose-sm max-w-none px-6 py-4 text-[12px] leading-relaxed [&>*:first-child]:mt-0 [&>*:last-child]:mb-0">
                  <ReactMarkdown remarkPlugins={REMARK_PLUGINS} components={markdownComponents}>
                    {activeTab === "claude_md" ? preview.claude_md : preview.ref_index}
                  </ReactMarkdown>
                </div>
              )}
            </div>

            <div className="px-6 py-4 border-t border-border">
              <div className="flex justify-between items-start">
                <div>
                  <button
                    onClick={() => setModalState("skip_confirm")}
                    className="text-[11px] text-muted-foreground hover:text-foreground transition-colors"
                  >
                    {t("onboarding.skip_button")}
                  </button>
                  <p className="text-[9px] text-muted-foreground/40 mt-0.5">
                    {t("onboarding.empty_template_hint")}
                  </p>
                </div>
                <button
                  onClick={handleApply}
                  className="px-4 py-1.5 rounded-lg bg-primary text-primary-foreground text-[11px] font-medium hover:bg-primary/90 transition-colors"
                >
                  {t("onboarding.apply_button")}
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
  const { t } = useTranslation("dialog");
  return (
    <div className="absolute inset-0 z-20 flex items-center justify-center bg-background/80 backdrop-blur-sm rounded-xl">
      <div className="w-[360px] bg-background border border-border rounded-lg shadow-xl p-5 space-y-4">
        <h3 className="text-sm font-semibold text-foreground">{t("onboarding.cancel_confirm_title")}</h3>

        <div className="space-y-2 text-[11px]">
          <p className="text-muted-foreground font-medium">{t("onboarding.cancel_confirm_lead")}</p>
          <ul className="space-y-1.5 pl-1">
            <li className="flex items-start gap-2 text-foreground/60">
              <span className="text-green-500 mt-0.5">✓</span>
              {t("onboarding.cancel_confirm_folder_created")}
              <span className="text-[9px] text-muted-foreground/50">{t("onboarding.cancel_confirm_folder_detail")}</span>
            </li>
            <li className="flex items-start gap-2 text-amber-500/80">
              <span className="mt-0.5">⚠</span>
              <span>{t("onboarding.cancel_confirm_claude_md_empty")}</span>
            </li>
            <li className="flex items-start gap-2 text-amber-500/80">
              <span className="mt-0.5">⚠</span>
              <span>{t("onboarding.cancel_confirm_no_index")}</span>
            </li>
          </ul>
          <p className="text-[10px] text-muted-foreground/50 pt-1">
            {t("onboarding.cancel_confirm_later_tip")}
          </p>
        </div>

        <div className="flex justify-between pt-1">
          <button
            onClick={onBack}
            className="px-3 py-1.5 rounded text-[11px] text-muted-foreground hover:text-foreground hover:bg-accent/50 transition-colors"
          >
            {t("onboarding.back_button")}
          </button>
          <button
            onClick={onConfirm}
            className="px-3 py-1.5 rounded text-[11px] font-medium text-destructive hover:bg-destructive/10 transition-colors"
          >
            {t("onboarding.cancel_anyway")}
          </button>
        </div>
      </div>
    </div>
  );
}

function InitialSetupPanel({
  payload,
  installedSkillNames,
  profileSelection,
  setProfileSelection,
  skillSelection,
  setSkillSelection,
  applyWorkflow,
  setApplyWorkflow,
}: {
  payload: InitialSetupPayload;
  installedSkillNames: Set<string>;
  profileSelection: Set<number>;
  setProfileSelection: (s: Set<number>) => void;
  skillSelection: Set<string>;
  setSkillSelection: (s: Set<string>) => void;
  applyWorkflow: boolean;
  setApplyWorkflow: (v: boolean) => void;
}) {
  const { t } = useTranslation("dialog");
  const toggleProfile = (i: number) => {
    const next = new Set(profileSelection);
    if (next.has(i)) next.delete(i);
    else next.add(i);
    setProfileSelection(next);
  };
  const toggleSkill = (name: string) => {
    const next = new Set(skillSelection);
    if (next.has(name)) next.delete(name);
    else next.add(name);
    setSkillSelection(next);
  };

  return (
    <div className="px-6 py-4 space-y-4 text-[12px]">
      {payload.rationale && (
        <div className="rounded-md border border-border/60 bg-accent/30 p-2.5 text-[11px] text-muted-foreground leading-relaxed">
          <span className="font-medium text-foreground">{t("onboarding.initial_setup_rationale")} </span>
          {payload.rationale}
        </div>
      )}

      {/* Agent Profiles */}
      {payload.agent_profiles && payload.agent_profiles.length > 0 && (
        <section>
          <h3 className="text-[11px] font-semibold text-foreground mb-1.5">Agent Profiles</h3>
          <ul className="space-y-1">
            {payload.agent_profiles.map((p, i) => (
              <li key={i}>
                <label className="flex items-center gap-2 cursor-pointer hover:bg-accent/50 rounded px-1.5 py-1">
                  <input
                    type="checkbox"
                    checked={profileSelection.has(i)}
                    onChange={() => toggleProfile(i)}
                    className="w-3.5 h-3.5"
                  />
                  <span className="capitalize font-medium text-foreground">{p.role}</span>
                  <span className="text-muted-foreground">{p.engine}</span>
                  {p.model && <span className="text-[10px] text-muted-foreground/60">({p.model})</span>}
                  {p.persona_id && <span className="text-[10px] text-muted-foreground/50 ml-auto">{p.persona_id}</span>}
                </label>
              </li>
            ))}
          </ul>
        </section>
      )}

      {/* Skills */}
      {payload.skills && payload.skills.length > 0 && (
        <section>
          <h3 className="text-[11px] font-semibold text-foreground mb-1.5">Recommended Skills</h3>
          <ul className="space-y-1">
            {payload.skills.map((name) => {
              const installed = installedSkillNames.has(name);
              return (
                <li key={name}>
                  <label className={cn(
                    "flex items-center gap-2 rounded px-1.5 py-1",
                    installed ? "cursor-pointer hover:bg-accent/50" : "opacity-50 cursor-not-allowed",
                  )}>
                    <input
                      type="checkbox"
                      disabled={!installed}
                      checked={skillSelection.has(name)}
                      onChange={() => installed && toggleSkill(name)}
                      className="w-3.5 h-3.5"
                    />
                    <span className={installed ? "text-foreground" : "text-muted-foreground"}>{name}</span>
                    {!installed && (
                      <span className="text-[10px] text-muted-foreground/60 ml-auto">not installed</span>
                    )}
                  </label>
                </li>
              );
            })}
          </ul>
        </section>
      )}

      {/* Workflow */}
      {payload.workflow && Object.keys(payload.workflow).length > 0 && (
        <section>
          <h3 className="text-[11px] font-semibold text-foreground mb-1.5">Workflow Defaults</h3>
          <label className="flex items-start gap-2 cursor-pointer hover:bg-accent/50 rounded px-1.5 py-1">
            <input
              type="checkbox"
              checked={applyWorkflow}
              onChange={(e) => setApplyWorkflow(e.target.checked)}
              className="w-3.5 h-3.5 mt-0.5"
            />
            <div className="space-y-0.5 text-[11px]">
              {payload.workflow.review_track && (
                <div><span className="text-muted-foreground">{t("onboarding.initial_setup_workflow_review_track")}</span> <span className="text-foreground">{payload.workflow.review_track}</span></div>
              )}
              {payload.workflow.context_mode && (
                <div><span className="text-muted-foreground">{t("onboarding.initial_setup_workflow_context")}</span> <span className="text-foreground">{payload.workflow.context_mode}</span></div>
              )}
              {payload.workflow.rt_participants && payload.workflow.rt_participants.length > 0 && (
                <div><span className="text-muted-foreground">{t("onboarding.initial_setup_workflow_rt")}</span> <span className="text-foreground">{payload.workflow.rt_participants.join(", ")}</span></div>
              )}
            </div>
          </label>
        </section>
      )}
    </div>
  );
}

function SkipConfirmOverlay({ onBack, onConfirm }: { onBack: () => void; onConfirm: () => void }) {
  const { t } = useTranslation("dialog");
  return (
    <div className="absolute inset-0 z-20 flex items-center justify-center bg-background/80 backdrop-blur-sm rounded-xl">
      <div className="w-[380px] bg-background border border-border rounded-lg shadow-xl p-5 space-y-4">
        <h3 className="text-sm font-semibold text-foreground">{t("onboarding.skip_confirm_title")}</h3>

        <div className="space-y-2 text-[11px] text-muted-foreground">
          <p>{t("onboarding.skip_confirm_lead")}</p>
          <ul className="space-y-1 pl-2 font-mono text-foreground/60">
            <li>{t("onboarding.skip_confirm_claude_md")}</li>
            <li>{t("onboarding.skip_confirm_ref_index")}</li>
          </ul>
          <p className="pt-1">
            {t("onboarding.skip_confirm_agent_note")}
          </p>
          <p className="text-[10px] text-muted-foreground/50">
            {t("onboarding.skip_confirm_rawq_note")}
          </p>
        </div>

        <div className="flex justify-between pt-1">
          <button
            onClick={onBack}
            className="px-3 py-1.5 rounded text-[11px] text-muted-foreground hover:text-foreground hover:bg-accent/50 transition-colors"
          >
            {t("onboarding.back_button")}
          </button>
          <button
            onClick={onConfirm}
            className="px-3 py-1.5 rounded text-[11px] font-medium text-muted-foreground hover:text-foreground hover:bg-accent/50 transition-colors"
          >
            {t("onboarding.skip_button")}
          </button>
        </div>
      </div>
    </div>
  );
}
