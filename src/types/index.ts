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
  type: "main" | "branch" | "discussion";
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
}

export interface Message {
  id: string;
  conversationId: string;
  role: "user" | "assistant";
  content: string;
  timestamp: number;
  status: "sending" | "streaming" | "done" | "error";
  progressContent?: string;
  engine?: string;
  model?: string;
  persona?: string;
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
  type: string;
  title: string;
  content: string;
  status: "draft" | "approved" | "rejected";
  createdAt: number;
  updatedAt: number;
}

export interface SkillDef {
  name: string;
  description: string;
  content: string;
  vendor?: string | null;
  sourcePath?: string | null;
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
}

export interface RoundtableParticipant {
  name: string;
  model?: string;
  /** "claude" | "codex" | "gemini" | "opencode" — defaults to "claude" on backend */
  engine?: string;
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
}

// ─── Plan types ────────────────────────────────────────────────────────────

export type PlanStatus = "draft" | "active" | "done" | "abandoned";
export type SubtaskStatus = "todo" | "approved" | "in_progress" | "done" | "abandoned";

export interface Plan {
  id: string;
  conversationId: string;
  branchId?: string;
  title: string;
  description?: string;
  expectedOutcome?: string;
  status: PlanStatus;
  createdAt: number;
  updatedAt: number;
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
  createdAt: number;
  updatedAt: number;
}

export interface SubtaskInput {
  title: string;
  details?: string;
}

export interface CreatePlanInput {
  conversationId: string;
  branchId?: string;
  title: string;
  description?: string;
  expectedOutcome?: string;
  subtasks: SubtaskInput[];
}
