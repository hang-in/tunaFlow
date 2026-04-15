export interface EngineConfig {
  command: string;
  engineKey: string;
  label: string;
  hasChunkEvent: boolean;
}

export const ENGINE_CONFIGS: Record<string, EngineConfig> = {
  claude:   { command: "start_claude_stream",        engineKey: "claude-code", label: "Claude initializing...",   hasChunkEvent: true },
  codex:    { command: "start_codex_run",            engineKey: "codex",       label: "Codex starting...",        hasChunkEvent: true },
  gemini:   { command: "start_gemini_stream",        engineKey: "gemini",      label: "Gemini initializing...",   hasChunkEvent: true },
  ollama:   { command: "start_openai_compat_stream", engineKey: "ollama",      label: "Ollama initializing...",   hasChunkEvent: true },
  lmstudio: { command: "start_openai_compat_stream", engineKey: "lmstudio",    label: "LM Studio initializing...", hasChunkEvent: true },
};
