/**
 * Tool step data model — parsed from `__STEP__:{json}` progress events.
 *
 * Used for:
 * 1. Streaming UI — live progress display in chat
 * 2. Lazy-loaded display — stored in progressContent (JSON), not used for search/ContextPack
 */

export interface ToolStep {
  type: "thinking" | "tool_use" | "tool_result" | "command" | "file_change";
  name: string;
  input: string;
  status: "running" | "done" | "error";
  ts: number; // epoch ms
}

const STEP_PREFIX = "__STEP__:";

/** Check if a progress event text is a structured tool step */
export function isToolStep(text: string): boolean {
  return text.startsWith(STEP_PREFIX);
}

/** Parse a `__STEP__:{json}` progress text into a ToolStep */
export function parseToolStep(text: string): ToolStep | null {
  if (!text.startsWith(STEP_PREFIX)) return null;
  try {
    const json = JSON.parse(text.slice(STEP_PREFIX.length));
    return {
      type: json.type ?? "tool_use",
      name: json.name ?? "Tool",
      input: json.input ?? "",
      status: json.status ?? "done",
      ts: Date.now(),
    };
  } catch {
    return null;
  }
}

/** Serialize tool steps for progressContent storage */
export function serializeSteps(steps: ToolStep[]): string {
  return JSON.stringify(steps);
}

/** Deserialize tool steps from progressContent */
export function deserializeSteps(json: string): ToolStep[] {
  try {
    const arr = JSON.parse(json);
    if (Array.isArray(arr) && arr.length > 0 && arr[0].type) return arr;
    return [];
  } catch {
    return [];
  }
}

/** Format step name + input for display */
export function formatStep(step: ToolStep): string {
  if (step.input) {
    return `${step.name}: ${step.input}`;
  }
  return step.name;
}

/** Status icon for a tool step */
export function stepIcon(step: ToolStep): string {
  switch (step.status) {
    case "running": return "⠋";
    case "done": return "✓";
    case "error": return "✗";
  }
}
