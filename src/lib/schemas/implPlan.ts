import { z } from "zod";

/**
 * Implementation Plan schema — matches planProposalParser.ts ParsedImplPlan.
 */

export const ImplFileSchema = z.object({
  path: z.string().min(1),
  action: z.string().default("modify"),
});

export const ImplPlanSchema = z.object({
  files: z.array(ImplFileSchema).default([]),
  dependencies: z.array(z.string()).default([]),
  risks: z.array(z.string()).default([]),
});

export type ImplPlanInput = z.infer<typeof ImplPlanSchema>;
