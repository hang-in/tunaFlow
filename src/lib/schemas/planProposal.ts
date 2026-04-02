import { z } from "zod";

/**
 * Plan Proposal schema — matches tool_handler.rs submit_plan_proposal
 * and planProposalParser.ts ParsedPlanProposal.
 */

export const SubtaskSchema = z.object({
  title: z.string().min(1),
  details: z.string().optional(),
});

export const PlanProposalSchema = z.object({
  title: z.string().min(1),
  description: z.string().min(1),
  expected_outcome: z.string().optional().default(""),
  subtasks: z.array(SubtaskSchema).min(1, "최소 1개의 subtask 필요"),
  constraints: z.array(z.string()).optional().default([]),
  non_goals: z.array(z.string()).optional().default([]),
});

export type PlanProposalInput = z.infer<typeof PlanProposalSchema>;

/** Convert zod-validated input to ParsedPlanProposal shape */
export function toParsedPlanProposal(
  input: PlanProposalInput,
  raw: string,
): {
  title: string;
  description: string;
  expectedOutcome: string;
  subtasks: { title: string; details?: string }[];
  constraints: string[];
  nonGoals: string[];
  raw: string;
} {
  return {
    title: input.title,
    description: input.description,
    expectedOutcome: input.expected_outcome ?? "",
    subtasks: input.subtasks,
    constraints: input.constraints ?? [],
    nonGoals: input.non_goals ?? [],
    raw,
  };
}
