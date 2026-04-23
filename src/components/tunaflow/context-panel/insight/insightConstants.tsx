import { Zap, FlaskConical, Box, Gauge, Lock, Trash2, XCircle, AlertTriangle, Info } from "lucide-react";
import type { InsightCategory, InsightFinding, InsightSeverity } from "@/types";

// label 은 `insight.category.{key}` 로 이관 (PR #167). 여기선 icon/color 만.
export const CATEGORY_META: Record<InsightCategory, { icon: React.ReactNode; color: string }> = {
  stability: { icon: <Zap className="w-3 h-3" />, color: "text-yellow-500" },
  test: { icon: <FlaskConical className="w-3 h-3" />, color: "text-blue-500" },
  architecture: { icon: <Box className="w-3 h-3" />, color: "text-purple-500" },
  performance: { icon: <Gauge className="w-3 h-3" />, color: "text-orange-500" },
  security: { icon: <Lock className="w-3 h-3" />, color: "text-red-500" },
  debt: { icon: <Trash2 className="w-3 h-3" />, color: "text-gray-500" },
};

export const SEVERITY_META: Record<InsightSeverity, { icon: React.ReactNode; cls: string }> = {
  critical: { icon: <XCircle className="w-3 h-3" />, cls: "text-red-500 bg-red-500/10" },
  major: { icon: <AlertTriangle className="w-3 h-3" />, cls: "text-orange-500 bg-orange-500/10" },
  minor: { icon: <Info className="w-3 h-3" />, cls: "text-yellow-500 bg-yellow-500/10" },
  info: { icon: <Info className="w-3 h-3" />, cls: "text-blue-400 bg-blue-400/10" },
};

export type QuadrantKey = "quick-wins" | "strategic" | "fill-ins" | "deprioritize";

export function classifyQuadrant(f: InsightFinding): QuadrantKey {
  const isHighImpact = f.severity === "critical" || f.severity === "major";
  if (f.fixDifficulty === "auto") return isHighImpact ? "quick-wins" : "fill-ins";
  if (f.fixDifficulty === "guided") return isHighImpact ? "strategic" : "fill-ins";
  return "deprioritize"; // manual
}
