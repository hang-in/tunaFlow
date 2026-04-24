/**
 * Assembles `SendWithClaudeInput` for the `start_*_stream` backend commands.
 * Shared between the main-chat and branch-drawer paths so that ContextPack
 * inputs (budget override, user profile, workflow skills, persona, crossSession
 * anchors) are evaluated identically.
 *
 * Caveat: `engine` is intentionally optional. The main-chat path forwards it
 * so the backend can route across providers, whereas the branch path leaves
 * it unset (backend picks the engine from the invoked command). Preserving
 * that asymmetry is a behavior-preserving constraint of the refactor.
 */
import { invoke } from "@tauri-apps/api/core";
import { getSetting } from "@/lib/appStore";
import type { SendWithClaudeInput } from "@/types";

export interface BuildSendInputParams {
  projectKey: string;
  conversationId: string;
  prompt: string;
  model?: string;
  /** Main-chat only — branch path omits this field. */
  engine?: string;
  /** Main-chat only. */
  systemPrompt?: string;
  personaFragment?: string;
  personaLabel?: string;
  crossSessionIds?: string[];
  /** ChatState.getEffectiveSkills — resolves phase-based workflow skill set. */
  getEffectiveSkills: (phase: string | null, prompt: string) => string[];
  opts?: { userMessageId?: string; imagePaths?: string[] };
}

interface BudgetConfig {
  mode: string;
  totalCap: number;
}

const BUDGET_DEFAULT: BudgetConfig = { mode: "auto", totalCap: 60000 };

export async function buildSendInput(p: BuildSendInputParams): Promise<SendWithClaudeInput> {
  const budgetCfg = await getSetting<BudgetConfig>("contextBudgetConfig", BUDGET_DEFAULT);
  const userProfile = await getSetting<object | null>("userProfile", null).catch(() => null);
  const planPhase = await invoke<string | null>("get_active_plan_phase", {
    conversationId: p.conversationId,
  }).catch(() => null);
  const activeSkills = p.getEffectiveSkills(planPhase, p.prompt);

  // Issue #175 — forward UI base URL override for openai-compat engines.
  // Empty string is equivalent to "no override" so the backend falls back to
  // env var / hardcoded default; only non-empty trimmed value is forwarded.
  let customBaseUrl: string | undefined;
  if (p.engine === "ollama" || p.engine === "lmstudio") {
    const raw = await getSetting<string>(`engineEndpoint:${p.engine}`, "");
    const trimmed = raw.trim();
    if (trimmed) customBaseUrl = trimmed;
  }

  return {
    projectKey: p.projectKey,
    conversationId: p.conversationId,
    prompt: p.prompt,
    model: p.model,
    ...(p.engine !== undefined ? { engine: p.engine } : {}),
    ...(p.systemPrompt !== undefined ? { systemPrompt: p.systemPrompt } : {}),
    activeSkills,
    crossSessionIds: p.crossSessionIds,
    personaFragment: p.personaFragment,
    personaLabel: p.personaLabel,
    contextModeOverride: budgetCfg.mode === "auto" ? undefined : budgetCfg.mode,
    contextBudgetCap: budgetCfg.totalCap === 60000 ? undefined : budgetCfg.totalCap,
    userProfileJson: userProfile ? JSON.stringify(userProfile) : undefined,
    ...(p.opts?.userMessageId ? { userMessageId: p.opts.userMessageId } : {}),
    ...(p.opts?.imagePaths && p.opts.imagePaths.length > 0
      ? { imagePaths: p.opts.imagePaths }
      : {}),
    ...(customBaseUrl ? { customBaseUrl } : {}),
  };
}
