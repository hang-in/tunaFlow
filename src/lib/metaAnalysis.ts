/**
 * Meta Analysis — Tier 2 엔진 선택 + 자동 분석 트리거.
 *
 * **원칙**: 사용자가 쉽게 선택 가능하도록 간단한 토글 UI. 저비용 엔진(Haiku / Gemini Flash)
 * 기반으로 주간 요약 · 실패 패턴 · artifact 요약을 자동 생성.
 */
import { getSetting, setSetting } from "@/lib/appStore";

export type MetaAnalysisEngine = "off" | "claude-haiku" | "gemini-flash" | "auto";

export interface MetaAnalysisConfig {
  /** 기본 엔진. "auto" 는 프로젝트 스택 기반 자동 선택 */
  engine: MetaAnalysisEngine;
  /** 자동 트리거 on/off */
  autoTrigger: boolean;
  /** Tier 2 트리거 임계값 */
  thresholds: {
    reviewPassedCount: number;  // 누적 N 건 도달 시 주간 요약
    reviewFailedCount: number;  // 누적 N 건 도달 시 실패 패턴 분석
    artifactCount: number;       // 누적 N 건 도달 시 artifacts 요약
    idleDays: number;            // N 일 경과 시 "다음 우선순위" 제안
  };
}

export const DEFAULT_CONFIG: MetaAnalysisConfig = {
  engine: "claude-haiku",
  autoTrigger: true,
  thresholds: {
    reviewPassedCount: 10,
    reviewFailedCount: 5,
    artifactCount: 10,
    idleDays: 7,
  },
};

export async function loadMetaConfig(): Promise<MetaAnalysisConfig> {
  const saved = await getSetting<MetaAnalysisConfig | null>("metaAnalysisConfig", null);
  if (!saved) return DEFAULT_CONFIG;
  return {
    ...DEFAULT_CONFIG,
    ...saved,
    thresholds: { ...DEFAULT_CONFIG.thresholds, ...(saved.thresholds ?? {}) },
  };
}

export async function saveMetaConfig(config: MetaAnalysisConfig): Promise<void> {
  await setSetting("metaAnalysisConfig", config);
}

/** "auto" 모드에서 프로젝트 스택 기반으로 실제 엔진 결정.
 *  규칙(간단): TypeScript/Python 프로젝트 → claude-haiku (구조 파싱 안정),
 *  Rust/Go/Java → gemini-flash (대용량 diff 저렴). */
export function resolveAutoEngine(
  stackKeywords: string[] = [],
): Exclude<MetaAnalysisEngine, "auto" | "off"> {
  const hasLargeDiffStack = stackKeywords.some((k) =>
    ["rust", "go", "java", "cpp", "kotlin"].includes(k.toLowerCase())
  );
  return hasLargeDiffStack ? "gemini-flash" : "claude-haiku";
}

/** 엔진 key 를 backend engine/model 로 변환. backend 의 `start_codex_run` 등이 쓰는 값과 맞춤. */
export function toBackendEngine(key: Exclude<MetaAnalysisEngine, "auto" | "off">): {
  engine: string; model: string;
} {
  switch (key) {
    case "claude-haiku":
      return { engine: "claude", model: "claude-haiku-4-5-20251001" };
    case "gemini-flash":
      return { engine: "gemini", model: "gemini-2.5-flash" };
  }
}
