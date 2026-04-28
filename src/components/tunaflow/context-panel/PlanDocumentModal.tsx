import { useState, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { invoke } from "@tauri-apps/api/core";
import ReactMarkdown from "react-markdown";
import { REMARK_PLUGINS } from "@/lib/markdownPlugins";
import { X, Clock, ClipboardList, ChevronDown, ChevronRight, User, FileText } from "lucide-react";
import { cn } from "@/lib/utils";
import { useChatStore } from "@/stores/chatStore";
import type { Plan, PlanEvent, PlanSubtask } from "@/types";
import { getPlanSlug } from "@/lib/workflowOrchestration";
import * as planApi from "@/lib/api/plans";
import { PLAN_PHASE_CFG, SUBTASK_STATUS_CFG } from "./plans/constants";
import { markdownComponents } from "../chat/MarkdownComponents";

interface PlanDocumentModalProps {
  plan: Plan;
  onClose: () => void;
}

export function PlanDocumentModal({ plan, onClose }: PlanDocumentModalProps) {
  const { t } = useTranslation("workflow");
  const [subtasks, setSubtasks] = useState<PlanSubtask[]>([]);
  const [events, setEvents] = useState<PlanEvent[]>([]);
  const [loading, setLoading] = useState(true);
  const [expandedSubtask, setExpandedSubtask] = useState<string | null>(null);
  const [planFileContent, setPlanFileContent] = useState<string | null>(null);
  const [taskFileContents, setTaskFileContents] = useState<Record<number, string>>({});

  useEffect(() => {
    setLoading(true);
    Promise.all([
      planApi.listSubtasks(plan.id),
      planApi.listPlanEvents(plan.id),
    ]).then(([sts, evs]) => {
      setSubtasks(sts);
      setEvents(evs);
    }).catch(() => {}).finally(() => setLoading(false));

    // Try to load plan document from filesystem
    (async () => {
      try {
        const projectKey = useChatStore.getState().selectedProjectKey;
        if (!projectKey) return;
        const project = await invoke("get_project", { key: projectKey }) as { path?: string };
        if (!project?.path) return;

        const slug = getPlanSlug(plan);
        const planPath = `${project.path}/docs/plans/${slug}.md`;

        // Read main plan file
        try {
          const content = await invoke<{ content: string }>("read_text_file", { filePath: planPath, projectPath: project.path });
          setPlanFileContent(content.content);
        } catch { /* file doesn't exist yet */ }

        // Read task files
        const tasks: Record<number, string> = {};
        for (let i = 1; i <= 50; i++) {
          const taskPath = `${project.path}/docs/plans/${slug}-task-${String(i).padStart(2, "0")}.md`;
          try {
            const content = await invoke<{ content: string }>("read_text_file", { filePath: taskPath, projectPath: project.path });
            tasks[i] = content.content;
          } catch { break; } // stop at first missing file
        }
        setTaskFileContents(tasks);
      } catch { /* silent */ }
    })();
  }, [plan.id]);

  const phaseCfg = PLAN_PHASE_CFG[plan.phase];

  // Filter events related to a specific subtask index
  const subtaskEvents = (idx: number) =>
    events.filter((ev) => ev.detail?.includes(`subtask ${idx + 1}`));

  return (
    <div className="fixed inset-0 z-50 flex items-center justify-center bg-black/50" onClick={onClose}>
      <div className="bg-popover border border-border rounded-xl shadow-2xl w-[640px] max-h-[85vh] overflow-hidden flex flex-col" onClick={(e) => e.stopPropagation()}>
        {/* Header */}
        <div className="flex items-center gap-2 px-5 py-3 border-b border-border/40 shrink-0">
          <ClipboardList className="w-4 h-4 text-primary/60" />
          <span className="text-sm font-medium text-foreground flex-1">{plan.title}</span>
          {(plan.versionMajor > 1 || plan.versionMinor > 0) && (
            <span className="text-[9px] font-mono text-muted-foreground/50 px-1.5 rounded bg-accent/50">v{plan.versionMajor}.{plan.versionMinor}</span>
          )}
          {planFileContent && (
            <span className="text-[8px] text-status-approved/50">{t("plan_document.status_file_based")}</span>
          )}
          <span className={cn("text-[9px] font-semibold px-1.5 py-0.5 rounded-full border", phaseCfg.cls)}>
            {phaseCfg.label}
          </span>
          <button onClick={onClose} className="p-1 rounded text-muted-foreground/50 hover:text-foreground hover:bg-accent transition-colors">
            <X className="w-4 h-4" />
          </button>
        </div>

        {/* Body */}
        <div className="flex-1 overflow-y-auto px-5 py-4 space-y-4">
          {loading ? (
            <p className="text-xs text-muted-foreground">Loading...</p>
          ) : (
            <>
              {/* Plan file content — rendered as markdown if available */}
              {planFileContent ? (
                <div className="prose prose-invert max-w-none text-[11px] leading-relaxed [&>*:first-child]:mt-0 [&>*:last-child]:mb-0 [&_h1]:text-[13px] [&_h2]:text-[12px] [&_h3]:text-[11px] [&_h1]:mt-3 [&_h2]:mt-2 [&_h3]:mt-1.5 [&_p]:my-1 [&_ul]:my-1 [&_li]:my-0.5 [&_code]:text-[10px]">
                  <ReactMarkdown remarkPlugins={REMARK_PLUGINS} components={markdownComponents}>
                    {planFileContent}
                  </ReactMarkdown>
                </div>
              ) : (
                <>
                  {plan.description && (
                    <div>
                      <h4 className="text-[10px] text-muted-foreground/60 uppercase tracking-wide mb-1">{t("plan_document.section_description")}</h4>
                      <p className="text-xs text-foreground/80 leading-relaxed whitespace-pre-wrap">{plan.description}</p>
                    </div>
                  )}
                  {plan.expectedOutcome && (
                    <div>
                      <h4 className="text-[10px] text-muted-foreground/60 uppercase tracking-wide mb-1">{t("plan_document.section_expected_outcome")}</h4>
                      <p className="text-xs text-foreground/80 leading-relaxed whitespace-pre-wrap">{plan.expectedOutcome}</p>
                    </div>
                  )}
                </>
              )}

              {/* Subtasks — clickable to expand work instruction */}
              <div>
                <h4 className="text-[10px] text-muted-foreground/60 uppercase tracking-wide mb-2">
                  {t("plan_document.section_subtasks", { count: subtasks.length })}
                </h4>
                <div className="space-y-1.5">
                  {subtasks.map((st, i) => {
                    const isExpanded = expandedSubtask === st.id;
                    const hasDetails = !!st.details?.trim();
                    const stEvents = subtaskEvents(i);
                    const statusCfg = SUBTASK_STATUS_CFG[st.status];

                    return (
                      <div key={st.id} className={cn(
                        "rounded-md border transition-colors",
                        isExpanded ? "border-primary/30 bg-primary/[0.03]" : "border-border/40 bg-card/50",
                      )}>
                        {/* Summary row — clickable */}
                        <button
                          onClick={() => setExpandedSubtask(isExpanded ? null : st.id)}
                          className="w-full flex items-start gap-2 p-3 text-left hover:bg-accent/20 transition-colors rounded-md"
                        >
                          <span className="mt-0.5 shrink-0 text-muted-foreground/40">
                            {isExpanded ? <ChevronDown className="w-3 h-3" /> : <ChevronRight className="w-3 h-3" />}
                          </span>
                          <span className="text-[10px] text-muted-foreground/40 font-mono shrink-0 mt-0.5 w-4 text-right">{i + 1}.</span>
                          <div className="flex-1 min-w-0">
                            <div className="flex items-center gap-1.5">
                              <span className="text-[11px] font-medium text-foreground">
                                {(taskFileContents[i + 1] && (() => { const m = taskFileContents[i + 1].match(/^#\s+(.+)$/m); return m?.[1]?.trim(); })()) || st.title}
                              </span>
                              <span className={cn("text-[8px] font-semibold px-1 py-0 rounded-full border shrink-0", statusCfg.cls)}>
                                {statusCfg.label}
                              </span>
                            </div>
                            {!isExpanded && hasDetails && (
                              <p className="text-[10px] text-muted-foreground/50 mt-0.5 line-clamp-1">{st.details}</p>
                            )}
                            {!isExpanded && !hasDetails && (
                              <p className="text-[10px] text-amber-600/40 italic mt-0.5">{t("plan_document.empty_instructions")}</p>
                            )}
                          </div>
                        </button>

                        {/* Expanded: full work instruction */}
                        {isExpanded && (
                          <div className="px-3 pb-3 pt-0 ml-9 space-y-2.5 border-t border-border/20 mt-0">
                            {/* Work instruction */}
                            <div className="pt-2">
                              <div className="flex items-center gap-1 mb-1">
                                <FileText className="w-3 h-3 text-primary/50" />
                                <span className="text-[9px] text-muted-foreground/60 uppercase tracking-wide">{t("plan_document.section_task_instructions")}</span>
                              </div>
                              {taskFileContents[i + 1] ? (
                                <div className="rounded bg-card/80 border border-border/30 px-3 py-2 prose prose-invert max-w-none text-[11px] leading-relaxed [&>*:first-child]:mt-0 [&>*:last-child]:mb-0 [&_h1]:text-[13px] [&_h2]:text-[12px] [&_h3]:text-[11px] [&_h1]:mt-3 [&_h2]:mt-2 [&_h3]:mt-1.5 [&_p]:my-1 [&_ul]:my-1 [&_li]:my-0.5 [&_code]:text-[10px]">
                                  <ReactMarkdown remarkPlugins={REMARK_PLUGINS} components={markdownComponents}>
                                    {taskFileContents[i + 1]}
                                  </ReactMarkdown>
                                </div>
                              ) : hasDetails ? (
                                <div className="rounded bg-card/80 border border-border/30 px-3 py-2">
                                  <p className="text-[11px] text-foreground/80 leading-relaxed whitespace-pre-wrap">{st.details}</p>
                                </div>
                              ) : (
                                <p className="text-[10px] text-amber-600/50 italic">{t("plan_document.empty_task_instructions")}</p>
                              )}
                            </div>

                            {/* Metadata */}
                            <div className="flex items-center gap-3 text-[9px] text-muted-foreground/50">
                              {st.ownerAgent && (
                                <span className="flex items-center gap-0.5">
                                  <User className="w-2.5 h-2.5" />{st.ownerAgent}
                                </span>
                              )}
                              {st.lastUpdatedBy && (
                                <span>{t("plan_document.last_updated_by", { actor: st.lastUpdatedBy })}</span>
                              )}
                            </div>

                            {/* Related revision history */}
                            {stEvents.length > 0 && (
                              <div>
                                <div className="flex items-center gap-1 mb-1">
                                  <Clock className="w-2.5 h-2.5 text-muted-foreground/30" />
                                  <span className="text-[8px] text-muted-foreground/40 uppercase tracking-wide">{t("plan_document.section_history")}</span>
                                </div>
                                <div className="space-y-0.5 border-l border-border/20 pl-2">
                                  {stEvents.map((ev) => {
                                    const d = new Date(ev.createdAt * 1000);
                                    const ts = `${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")} ${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
                                    return (
                                      <div key={ev.id} className="text-[9px] text-muted-foreground/50">
                                        <span className="text-muted-foreground/30 font-mono">{ts}</span>
                                        {" "}{ev.eventType.replace(/_/g, " ")}
                                        {ev.actor && <span className="text-foreground/30"> ({ev.actor})</span>}
                                      </div>
                                    );
                                  })}
                                </div>
                              </div>
                            )}
                          </div>
                        )}
                      </div>
                    );
                  })}
                </div>
              </div>

              {/* Full Revision History */}
              {events.length > 0 && (
                <div>
                  <h4 className="text-[10px] text-muted-foreground/60 uppercase tracking-wide mb-2 flex items-center gap-1">
                    <Clock className="w-3 h-3" />{t("plan_document.section_revision_history")}
                  </h4>
                  <div className="space-y-1 border-l-2 border-border/30 pl-3">
                    {events.map((ev) => {
                      const d = new Date(ev.createdAt * 1000);
                      const ts = `${d.getFullYear()}-${String(d.getMonth() + 1).padStart(2, "0")}-${String(d.getDate()).padStart(2, "0")} ${String(d.getHours()).padStart(2, "0")}:${String(d.getMinutes()).padStart(2, "0")}`;
                      return (
                        <div key={ev.id} className="text-[10px] text-muted-foreground/60">
                          <span className="text-muted-foreground/30 font-mono">{ts}</span>
                          {" — "}
                          <span>{ev.eventType.replace(/_/g, " ")}</span>
                          {ev.actor && <span className="text-foreground/40"> ({ev.actor})</span>}
                          {ev.detail && <span className="text-muted-foreground/40"> — {ev.detail.slice(0, 100)}</span>}
                        </div>
                      );
                    })}
                  </div>
                </div>
              )}
            </>
          )}
        </div>
      </div>
    </div>
  );
}
