export interface Project {
  key: string;
  name: string;
  path?: string;
  type: "project" | "discovered" | "chat" | "channel";
  defaultEngine?: string;
  workspaceRoot?: string;
  source: "configured" | "discovered";
  updatedAt: number;
}

export interface Conversation {
  id: string;
  projectKey: string;
  label: string;
  customLabel?: string;
  type: "main" | "branch" | "discussion" | "scratchpad" | "meta";
  mode: "chat" | "roundtable";
  parentId?: string;
  source: "tunadish" | "mattermost" | "slack";
  createdAt: number;
  updatedAt: number;
  engine?: string;
  model?: string;
  persona?: string;
  triggerMode?: "always" | "mentions" | "off";
  totalInputTokens: number;
  totalOutputTokens: number;
  totalCostUsd: number;
  resumeToken?: string;
}

export interface Message {
  id: string;
  conversationId: string;
  role: "user" | "assistant" | "system";
  content: string;
  timestamp: number;
  status: "sending" | "streaming" | "done" | "error";
  progressContent?: string;
  engine?: string;
  model?: string;
  persona?: string;
  /** Runtime-only: set from agent:completed event, not persisted to DB */
  durationMs?: number;
  inputTokens?: number;
  outputTokens?: number;
  costUsd?: number;
}

export interface Branch {
  id: string;
  conversationId: string;
  label: string;
  customLabel?: string;
  status: "active" | "adopted" | "archived" | "discarded";
  checkpointId?: string;
  parentBranchId?: string;
  sessionId?: string;
  gitBranch?: string;
  /** "chat" | "roundtable" */
  mode?: string;
  /** Plan subtask this branch implements */
  subtaskId?: string;
  createdAt: number;
}

export interface Memo {
  id: string;
  messageId: string;
  conversationId: string;
  projectKey: string;
  content: string;
  type: string;
  tags: string;
  createdAt: number;
}

export interface Artifact {
  id: string;
  conversationId?: string;
  branchId?: string;
  subtaskId?: string;
  planId?: string;
  type: string;
  title: string;
  content: string;
  status: "draft" | "approved" | "rejected";
  createdAt: number;
  updatedAt: number;
}

export type SkillLayer = "reference" | "procedural";

export interface SkillDef {
  name: string;
  description: string;
  content: string;
  vendor?: string | null;
  sourcePath?: string | null;
  layer: SkillLayer;
  bindPhases: string[];
}

export interface SkillsSnapshotInfo {
  publishedAt?: string | null;
  totalSkills: number;
  source?: string | null;
}

/** Engine model catalog entry */
export interface EngineModel {
  id: string;
  label: string;
  engine: string;
  recommended: boolean;
  source: string;
}

/** rawq index status returned from backend */
export interface RawqStatus {
  available: boolean;
  indexed: boolean;
  /** "ready" | "built" | "error" | "unavailable" */
  status: string;
  message: string;
  files?: number;
  chunks?: number;
}

/** Unified capability entry from the registry */
export interface ToolCapability {
  name: string;
  /** "skill" | "local_tool" | "mcp_tool" */
  kind: string;
  description: string;
  /** Where this capability was loaded from */
  source: string;
  /** Whether the tool maintains server-side state across calls */
  stateful: boolean;
}

// ─── Command input types ───────────────────────────────────────────────────

export interface CreateProjectInput {
  key: string;
  name: string;
  path?: string;
  type: string;
  defaultEngine?: string;
  workspaceRoot?: string;
  source: string;
}

export interface CreateConversationInput {
  projectKey: string;
  label: string;
  type?: string;
  mode?: string;
  source?: string;
  engine?: string;
  model?: string;
}

export interface CreateUserMessageInput {
  conversationId: string;
  content: string;
}

export interface SendWithClaudeInput {
  projectKey: string;
  conversationId: string;
  userMessageId?: string;
  prompt: string;
  model?: string;
  /** Engine key for backend routing (e.g. "ollama" vs "lmstudio") */
  engine?: string;
  /** Passed directly when no agent is selected */
  systemPrompt?: string;
  /** Agent name → loads docs/agents/{name}.md as system prompt (ContextPack step 1) */
  agentName?: string;
  /** Active skill names — content injected into ContextPack (step 2) */
  activeSkills?: string[];
  /** Conversation IDs for cross-session context (step 3.5) */
  crossSessionIds?: string[];
  /** Persona prompt fragment — injected as persona section in normalized prompt */
  personaFragment?: string;
  /** Profile/persona label — stored in message.persona for visibility */
  personaLabel?: string;
  /** Context mode override: "lite" | "standard" | "full" | undefined (auto) */
  contextModeOverride?: string;
  /** Total context budget cap override (chars). undefined = default (60000) */
  contextBudgetCap?: number;
  /** Serialized user profile JSON — injected as ## User section in ContextPack */
  userProfileJson?: string;
}

export interface RoundtableParticipant {
  name: string;
  model?: string;
  /** "claude" | "codex" | "gemini" | "ollama" | "lmstudio" — defaults to "claude" on backend */
  engine?: string;
  /** Blind verifier — receives topic only, no prior/current transcript */
  blind?: boolean;
  /** RT role — affects output cap directive. "proposer" | "reviewer" | "verifier" | "synthesizer" */
  role?: string;
  /** Explicit output token cap. If not set, derived from role. */
  maxTokens?: number;
}

/** RT execution mode:
 * - "sequential"   — within a round each agent sees prior agents' replies (original behavior)
 * - "deliberative" — Round 1 is independent; Round 2+ sees all prior-round answers
 */
export type RtMode = "sequential" | "deliberative";

export interface RoundtableRunInput {
  conversationId: string;
  prompt: string;
  participants: RoundtableParticipant[];
  /** Number of rounds (1-3, default 1) */
  rounds?: number;
  /** Execution mode (default "sequential") */
  mode?: RtMode;
  /** When true, a synthesizer participant runs after the round to aggregate
   *  reviewer verdicts. Requires ≥2 reviewer roles; silently skipped otherwise. */
  autoSynthesize?: boolean;
}

export interface CreateBranchInput {
  conversationId: string;
  label?: string;
  checkpointId?: string;
  parentBranchId?: string;
  mode?: string;
  subtaskId?: string;
}

export interface AdoptBranchInput {
  branchId: string;
  conversationId: string;
}

export interface CreateMemoInput {
  messageId: string;
  conversationId: string;
  projectKey: string;
  content: string;
  type?: string;
  tags?: string;
}

export interface CreateArtifactInput {
  conversationId?: string;
  branchId?: string;
  subtaskId?: string;
  planId?: string;
  type: string;
  title: string;
  content: string;
}

export interface UpdateArtifactStatusInput {
  id: string;
  status: "draft" | "approved" | "rejected";
}

// ─── Agent Profile types ──────────────────────────────────────────────────

export interface AgentProfile {
  id: string;
  label: string;
  engine: string;
  model?: string;
  personaId?: string;
  defaultSkills: string[];
}

export interface Persona {
  id: string;
  name: string;
  role: string;
  summary: string;
  builtIn: boolean;
  priorities: string[];
  behaviors: string[];
  constraints: string[];
  tone: string;
  outputStyle: string;
  promptFragment: string;
  /** Skills recommended for this persona role — merged with profile.defaultSkills on apply */
  recommendedSkills?: string[];
}

// ─── Plan types ────────────────────────────────────────────────────────────

export type PlanStatus = "draft" | "active" | "done" | "abandoned";
export type PlanPhase = "drafting" | "subtask_review" | "approval" | "implementation" | "review" | "done" | "rework";
export type SubtaskStatus = "todo" | "approved" | "in_progress" | "done" | "abandoned";

export interface Plan {
  id: string;
  conversationId: string;
  branchId?: string;
  title: string;
  description?: string;
  expectedOutcome?: string;
  status: PlanStatus;
  phase: PlanPhase;
  architectEngine?: string;
  developerEngine?: string;
  reviewerEngines?: string;
  implementationBranchId?: string;
  reviewBranchId?: string;
  /** File-path-safe slug, unique per project */
  slug?: string;
  /** Follow-up plan lineage — links to the predecessor plan */
  parentPlanId?: string;
  revision: number;
  versionMajor: number;
  versionMinor: number;
  createdAt: number;
  updatedAt: number;
}

export interface PlanEvent {
  id: string;
  planId: string;
  eventType: string;
  actor?: string;
  detail?: string;
  createdAt: number;
}

export interface PlanSubtask {
  id: string;
  planId: string;
  idx: number;
  title: string;
  details?: string;
  status: SubtaskStatus;
  outcome?: string;
  ownerAgent?: string;
  lastUpdatedBy?: string;
  /** JSON array of subtask indices this depends on */
  dependsOn?: number[];
  /** Parallel execution group label */
  parallelGroup?: string;
  createdAt: number;
  updatedAt: number;
}

export interface SubtaskInput {
  title: string;
  details?: string;
}

export interface FailureLesson {
  id: string;
  projectKey: string;
  planId?: string;
  filePath?: string;
  pattern?: string;
  finding: string;
  resolution?: string;
  createdAt: number;
}

// ── Insight ─────────────────────────────────────────────────

export type InsightCategory = "stability" | "test" | "architecture" | "performance" | "security" | "debt";
export type InsightSeverity = "critical" | "major" | "minor" | "info";
export type InsightFixDifficulty = "auto" | "guided" | "manual";
export type InsightFindingStatus = "open" | "selected" | "in_progress" | "resolved" | "dismissed";
export type InsightSessionStatus = "pending" | "analyzing" | "completed" | "failed";

export interface InsightSession {
  id: string;
  projectKey: string;
  status: InsightSessionStatus;
  categories?: string; // JSON array
  testOutput?: string;
  summary?: string;
  createdAt: number;
  completedAt?: number;
}

export interface InsightFinding {
  id: string;
  sessionId: string;
  projectKey: string;
  category: InsightCategory;
  severity: InsightSeverity;
  fixDifficulty: InsightFixDifficulty;
  title: string;
  description: string;
  filePath?: string;
  lineNumber?: number;
  snippet?: string;
  estimatedFiles?: number;
  resolution?: string;
  planId?: string;
  status: InsightFindingStatus;
  createdAt: number;
}

export interface InsightReport {
  id: string;
  sessionId: string;
  projectKey: string;
  type: "category" | "meta";
  category?: string;
  content: string;
  createdAt: number;
}

export interface InsightAgentConfig {
  engine: string;
  model: string;
  systemPrompt: string;
  presetId: string; // "balanced" | "thorough" | "security-focused" | "custom"
}

export interface CreatePlanInput {
  conversationId: string;
  branchId?: string;
  title: string;
  description?: string;
  expectedOutcome?: string;
  subtasks: SubtaskInput[];
}
