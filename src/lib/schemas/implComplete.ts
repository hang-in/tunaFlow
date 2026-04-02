import { z } from "zod";

/**
 * Implementation Complete schema — matches tool_handler.rs mark_implementation_complete.
 */

export const ImplCompleteSchema = z.object({
  summary: z.string().min(1),
});

export type ImplCompleteInput = z.infer<typeof ImplCompleteSchema>;
