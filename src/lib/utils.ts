import { clsx, type ClassValue } from "clsx";
import { twMerge } from "tailwind-merge";

export function cn(...inputs: ClassValue[]) {
  return twMerge(clsx(inputs));
}

export type AgentEngine = "claude" | "codex" | "gemini" | "opencode";

export const AGENT_COLORS: Record<AgentEngine, string> = {
  claude: "text-agent-claude border-agent-claude/30 bg-agent-claude/10",
  codex: "text-agent-codex border-agent-codex/30 bg-agent-codex/10",
  gemini: "text-agent-gemini border-agent-gemini/30 bg-agent-gemini/10",
  opencode: "text-agent-opencode border-agent-opencode/30 bg-agent-opencode/10",
};

export const AGENT_DOT_COLORS: Record<AgentEngine, string> = {
  claude: "bg-agent-claude",
  codex: "bg-agent-codex",
  gemini: "bg-agent-gemini",
  opencode: "bg-agent-opencode",
};

export const AGENT_DISPLAY_NAMES: Record<AgentEngine, string> = {
  claude: "Claude",
  codex: "Codex",
  gemini: "Gemini",
  opencode: "OpenCode",
};

export function formatTimestamp(ts: number): string {
  return new Date(ts).toLocaleTimeString("en-US", {
    hour: "2-digit",
    minute: "2-digit",
  });
}

/** Normalize engine string to known engine ID. "claude-code" → "claude" etc. */
export function normalizeEngine(s: string | undefined): AgentEngine | null {
  if (!s) return null;
  if (s === "claude" || s === "claude-code") return "claude";
  if (s === "codex") return "codex";
  if (s === "gemini") return "gemini";
  if (s === "opencode") return "opencode";
  return null;
}

export function isKnownEngine(s: string | undefined): s is AgentEngine {
  return normalizeEngine(s) !== null;
}

/** Agent name text color classes */
export const AGENT_TEXT_COLORS: Record<AgentEngine, string> = {
  claude: "text-agent-claude",
  codex: "text-agent-codex",
  gemini: "text-agent-gemini",
  opencode: "text-agent-opencode",
};
