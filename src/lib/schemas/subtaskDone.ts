import { z } from "zod";

/**
 * Subtask Done schema — matches tool_handler.rs mark_subtask_done.
 */

export const SubtaskDoneSchema = z.object({
  subtask_number: z.number().int().min(1),
  summary: z.string().optional(),
});

export type SubtaskDoneInput = z.infer<typeof SubtaskDoneSchema>;
