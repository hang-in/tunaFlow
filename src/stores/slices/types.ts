/**
 * Shared types for chatStore slices.
 * Re-exports the full ChatState so each slice can reference `get()`.
 */
import type { StoreApi } from "zustand";
import type {
  Project,
  Conversation,
  Message,
  Branch,
  Memo,
  Artifact,
  SkillDef,
  CreateProjectInput,
  CreateConversationInput,
  CreateBranchInput,
  SendWithClaudeInput,
  RoundtableRunInput,
  RoundtableParticipant,
  RtMode,
  AdoptBranchInput,
  CreateMemoInput,
  CreateArtifactInput,
  UpdateArtifactStatusInput,
  RawqStatus,
  EngineModel,
  AgentProfile,
} from "../../types";

// Re-export for convenience
export type {
  Project,
  Conversation,
  Message,
  Branch,
  Memo,
  Artifact,
  SkillDef,
  CreateProjectInput,
  CreateConversationInput,
  CreateBranchInput,
  SendWithClaudeInput,
  RoundtableRunInput,
  RoundtableParticipant,
  RtMode,
  AdoptBranchInput,
  CreateMemoInput,
  CreateArtifactInput,
  UpdateArtifactStatusInput,
  RawqStatus,
  EngineModel,
  AgentProfile,
};

/** Queued send action for same-thread serial execution */
export interface QueuedAction {
  threadId: string;
  label: string;
  execute: () => Promise<void>;
}

export interface ChatState {
  projects: Project[];
  selectedProjectKey: string | null;
  conversations: Conversation[];
  selectedConversationId: string | null;
  messages: Message[];
  branches: Branch[];
  /** Conversation thread IDs currently executing agent calls (supports multi-project parallel) */
  runningThreadIds: string[];
  /** Same-thread message queue — drained sequentially after active run completes */
  messageQueue: QueuedAction[];
  error: string | null;
  /** Branch stream mode — set when user "opens" a branch for chatting */
  activeBranchId: string | null;
  /** Conversation to restore when closing the branch stream */
  parentConversationId: string | null;
  /** Thread drawer state — sliding panel showing branch messages anchored to a parent message */
  threadBranchId: string | null;
  threadBranchConvId: string | null;
  threadMessages: Message[];
  threadBranchLabel: string | null;
  threadParentMessage: Message | null;
  memos: Memo[];
  artifacts: Artifact[];
  skills: SkillDef[];
  activeSkills: string[];
  crossSessionIds: string[];
  rawqStatus: RawqStatus | null;
  projectLoading: string | null;
  engineModels: EngineModel[];
  /** Pending handoff source set by UI actions (artifact forward, plan forward, etc.) */
  handoffSource: { type: string; content: string } | null;
  /** Message ID to scroll to and highlight (set by memo click, cleared after scroll) */
  scrollToMessageId: string | null;
  /** Current persona prompt fragment — set by profile/persona selection, included in agent requests */
  personaFragment: string | null;
  /** Current profile/persona label for message meta visibility */
  personaLabel: string | null;
  /** Agent profiles — shared between Settings and NewMessageInput */
  agentProfiles: AgentProfile[];
  selectedProfileId: string | null;
  /** Per-conversation engine/profile memory */
  _convEngineMap: Record<string, { profileId: string | null; engine: string; model?: string }>;
  loadProfiles: () => Promise<void>;
  saveProfiles: (profiles: AgentProfile[]) => void;
  selectProfile: (profileId: string | null) => void;
  saveConversationEngine: (conversationId: string, state: { profileId: string | null; engine: string; model?: string }) => void;
  getConversationEngine: (conversationId: string) => { profileId: string | null; engine: string; model?: string } | null;

  setHandoffSource: (source: { type: string; content: string } | null) => void;
  _startRun: (threadId: string) => void;
  _endRun: (threadId: string) => void;
  _enqueue: (threadId: string, label: string, execute: () => Promise<void>) => void;
  loadProjects: () => Promise<void>;
  loadEngineModels: (refresh?: boolean) => Promise<void>;
  createProject: (input: CreateProjectInput) => Promise<void>;
  hideProject: (key: string) => Promise<void>;
  selectProject: (key: string) => Promise<void>;
  createConversation: (input: CreateConversationInput) => Promise<Conversation>;
  deleteConversation: (id: string) => Promise<void>;
  selectConversation: (id: string) => Promise<void>;
  sendMessage: (prompt: string, model?: string, systemPrompt?: string) => Promise<void>;
  sendWithEngine: (engine: string, prompt: string, model?: string, systemPrompt?: string) => Promise<void>;
  sendFollowup: (engine: string, sourceType: string, sourceContent: string, goal?: string) => Promise<void>;
  sendRoundtable: (prompt: string, participants: RoundtableParticipant[], mode?: RtMode) => Promise<void>;
  sendRoundtableFollowup: (prompt: string, participants: RoundtableParticipant[], mode?: RtMode) => Promise<void>;
  loadBranches: (conversationId: string) => Promise<void>;
  createBranch: (conversationId: string, checkpointId?: string, label?: string, mode?: string, parentBranchId?: string) => Promise<void>;
  deleteBranch: (branchId: string) => Promise<void>;
  renameConversation: (id: string, customLabel: string) => Promise<void>;
  deleteMessagePair: (messageId: string) => Promise<void>;
  renameBranch: (branchId: string, customLabel: string) => Promise<void>;
  linkGitBranch: (branchId: string, gitBranch: string | null) => Promise<void>;
  adoptBranch: (branchId: string, conversationId: string) => Promise<void>;
  openBranchStream: (branchId: string) => Promise<void>;
  closeBranchStream: () => Promise<void>;
  openThread: (branchId: string) => Promise<void>;
  closeThread: () => void;
  sendThreadMessage: (prompt: string, engine?: string, model?: string) => Promise<void>;
  sendThreadRoundtable: (prompt: string, participants: RoundtableParticipant[], mode?: RtMode) => Promise<void>;
  sendThreadRoundtableFollowup: (prompt: string, participants: RoundtableParticipant[], mode?: RtMode) => Promise<void>;
  cancelOperation: (threadId?: string) => Promise<void>;
  toggleCrossSession: (conversationId: string) => void;
  loadSkills: () => Promise<void>;
  toggleSkill: (name: string) => void;
  loadMemos: () => Promise<void>;
  createMemo: (messageId: string, content: string) => Promise<void>;
  deleteMemo: (id: string) => Promise<void>;
  loadArtifacts: () => Promise<void>;
  createArtifact: (input: CreateArtifactInput) => Promise<void>;
  updateArtifactStatus: (id: string, status: "draft" | "approved" | "rejected") => Promise<void>;
  deleteArtifact: (id: string) => Promise<void>;
}

/** Zustand setter / getter types for slices */
export type SetState = StoreApi<ChatState>["setState"];
export type GetState = StoreApi<ChatState>["getState"];

/** A slice creator returns a partial ChatState (state + actions) */
export type SliceCreator<T> = (set: SetState, get: GetState) => T;
