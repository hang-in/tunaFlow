import { useState, useEffect } from "react";
import { invoke } from "@tauri-apps/api/core";
import type { Branch } from "@/types";

export function useProjectBranches(conversations: { id: string }[], storeBranches: Branch[], renameCounter: number) {
  const [dbBranches, setDbBranches] = useState<Branch[]>([]);
  const convIdKey = conversations.filter((c) => !c.id.startsWith("branch:")).map((c) => c.id).join(",");
  // Re-fetch when store branches change (covers deleteBranch from drawer)
  const storeBranchKey = storeBranches.map((b) => b.id).join(",");
  useEffect(() => {
    const ids = convIdKey.split(",").filter(Boolean);
    if (!ids.length) { setDbBranches([]); return; }
    Promise.all(ids.map((id) => invoke<Branch[]>("list_branches", { conversationId: id }).catch(() => [] as Branch[])))
      .then((r) => setDbBranches(r.flat()));
  }, [convIdKey, renameCounter, storeBranchKey]);
  const sm = new Map(storeBranches.map((b) => [b.id, b]));
  const merged = dbBranches.map((db) => { const u = sm.get(db.id); return u ? { ...db, customLabel: u.customLabel, status: u.status } : db; });
  for (const sb of storeBranches) { if (!dbBranches.some((db) => db.id === sb.id)) merged.push(sb); }
  return merged;
}
