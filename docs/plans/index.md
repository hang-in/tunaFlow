# Plans — 진행 현황

> 갱신: 2026-04-22 (docs/reorg-phase-a 작업)
> 완료·보류·초안 문서는 `docs/archive/plans/{completed,deferred,misc}/`로 이동.

## 📂 구조

- `docs/plans/` — **현재 진행 중** plans (active + partial)
- `docs/archive/plans/completed/` — 완료된 plans (구현 반영됨)
- `docs/archive/plans/deferred/` — 보류된 plans (후순위)
- `docs/archive/plans/misc/` — IA/검토/가이드 초안

## 🟢 진행 예정 / 진행 중

| 문서 | 설명 |
|------|------|
| [betaReadinessPlan](./betaReadinessPlan.md) | — |
| [betaReleaseReadinessPlan](./betaReleaseReadinessPlan.md) | — |
| [betaRtUpgradeSprintPlan_2026-04-15](./betaRtUpgradeSprintPlan_2026-04-15.md) | — |
| [cicdReleasePlan](./cicdReleasePlan.md) | — |
| [contextBudgetScalingPlan](./contextBudgetScalingPlan.md) | P2 — 단계적 context budget 상향 실험 |
| [conventionsContextSyncPlan](./conventionsContextSyncPlan.md) | — |
| [engineServerModePlan](./engineServerModePlan.md) | — |
| [geminiSdkIntegrationPlan](./geminiSdkIntegrationPlan.md) | P1 — Google AI SDK 직접 통합 (CLI 대체, SSE/token/function calling) |
| [i18nPlan](./i18nPlan.md) | P1 — UI 한/영 분리 (react-i18next) + 프롬프트 영어 통일 + ContextPack 응답 언어 주입 |
| [liveRuntimeTraceParityValidationPlan_2026-03-30](./liveRuntimeTraceParityValidationPlan_2026-03-30.md) | P1 — 4-engine trace/meta parity 실제 확인 |
| [metaAgentInitialSetupPlan_2026-04-16](./metaAgentInitialSetupPlan_2026-04-16.md) | — |
| [metaAgentOnboardingPlan_2026-04-16](./metaAgentOnboardingPlan_2026-04-16.md) | — |
| [metaAgentPlan](./metaAgentPlan.md) | **P0** — 메타에이전트. 온보딩, 이슈 감지, 우선순위 제안. 모든 프로젝트 대상 |
| [perProjectDatabaseSplitPlan](./perProjectDatabaseSplitPlan.md) | — |
| [postParityRuntimeValidationSweepPlan_2026-03-30](./postParityRuntimeValidationSweepPlan_2026-03-30.md) | P1 — parity fix 효과 재검증 |
| [realWorkflowMemoryQualityValidationPlan_2026-03-30](./realWorkflowMemoryQualityValidationPlan_2026-03-30.md) | P1 — memory/retrieval 응답 품질 검증 |
| [refactorRoadmap_2026-04-20](./refactorRoadmap_2026-04-20.md) | **베타 전 리팩토링 + 안정화 5-Phase 로드맵** (16~19일) — 프로덕션급 베타 기준 |
| [refactorRoadmap_first_prompt](./refactorRoadmap_first_prompt.md) | 새 세션에 붙여넣을 첫 프롬프트 텍스트 |
| [refactorRoadmap_handoff_2026-04-20](./refactorRoadmap_handoff_2026-04-20.md) | 새 세션용 핸드오프 — 프로젝트 철학 / 피할 함정 / Phase 1 Finding 6 진입점 |
| [roundtableRoleTerminologySeparationPlan_2026-03-30](./roundtableRoleTerminologySeparationPlan_2026-03-30.md) | P1 — 프로필 역할 vs RT 토론 역할 분리 |
| [runtimeFeatureValidationPlan_2026-03-30](./runtimeFeatureValidationPlan_2026-03-30.md) | P1 — memory/retrieval/auto/budget/RT 실시나리오 검증 |
| [sdkUrlSessionModePlan](./sdkUrlSessionModePlan.md) | — |
| [searchPipelineFromSecallPlan-part2](./searchPipelineFromSecallPlan-part2.md) | P1 — Phase C Part 2: `messages_fts` rebuild + content_tokenized 컬럼 + app-level snippet (depends: PR #127) |
| [skillSelectorAgentPlan](./skillSelectorAgentPlan.md) | — |
| [structuralImprovementPlan](./structuralImprovementPlan.md) | — |
| [systemMessageChannelPlan](./systemMessageChannelPlan.md) | — |
| [toolCallHandlerPlan](./toolCallHandlerPlan.md) | P1 — function calling으로 마커 대체 |
| [traceOverhaulPlan_2026-04-16](./traceOverhaulPlan_2026-04-16.md) | — |

## 🟡 부분 완료 — 추가 구현 필요

| 문서 | 설명 |
|------|------|
| [2026-03-28_skills_runtime_snapshot_plan](./2026-03-28_skills_runtime_snapshot_plan.md) | Phase 1 snapshot 발행 완료 |
| [agentDaemonRoadmapPlan](./agentDaemonRoadmapPlan.md) | Phase 1-2 완료 (background worker + job registry) |
| [agentProfileUsagePolishPlan_2026-03-29](./agentProfileUsagePolishPlan_2026-03-29.md) | profile summary 기본 표시 |
| [chatUiParityWithTunaChatPlan](./chatUiParityWithTunaChatPlan.md) | Pretendard 3-tier 폰트, max-w-4xl, 아바타 인라인 (세션 14) |
| [compressedMemoryOperationalPolishPlan_2026-03-30](./compressedMemoryOperationalPolishPlan_2026-03-30.md) | provenance/model_used/force_recompress 구현 (세션 7) |
| [contextHubSearchGetUiPlan_2026-03-30](./contextHubSearchGetUiPlan_2026-03-30.md) | Settings UI + chops ContextPack 자동 주입 (세션 7) |
| [contextPackAlgorithmImprovementsPlan](./contextPackAlgorithmImprovementsPlan.md) | P2 동적 예산 배분(allocate_budgets) 구현 |
| [engineFeatureParityClassificationPlan](./engineFeatureParityClassificationPlan.md) | 4-engine parity 분류 기준 + Claude parity fix 완료 |
| [gitSyncBranchModelPlan_2026-03-29](./gitSyncBranchModelPlan_2026-03-29.md) | branch↔git 동기화 모델 설계. adopt=merge, delete=pointer-only |
| [harnessEngineeringAdoptionPlan](./harnessEngineeringAdoptionPlan.md) | Phase 1-6 workspace 가시화 완료 |
| [longTermMemoryRoadmapPlan_2026-03-30](./longTermMemoryRoadmapPlan_2026-03-30.md) | Phase 1 compressed + Phase 2 retrieval/vector + Phase 3 session discovery 완료 (세션 7-18) + 자동 트리거 배선 완료 (세션 20) |
| [masterTestPlan](./masterTestPlan.md) | Rust 188 + Frontend 175 = 363 tests |
| [messagePairDeletionPlan](./messagePairDeletionPlan.md) | user+assistant 인접 메시지 쌍 삭제 계획 |
| [ownerAgentAssignmentPlan](./ownerAgentAssignmentPlan.md) | DB 필드 + PlansPanel UI dropdown 존재 |
| [planBasedFollowupPlan](./planBasedFollowupPlan.md) | PlansPanel Forward 버튼 구현 |
| [projectOnboardingLifecyclePlan](./projectOnboardingLifecyclePlan.md) | folder picker + validation + auto main conv + rawq + scaffolding |
| [projectScopedConcurrencyPlan](./projectScopedConcurrencyPlan.md) | thread-local queue 구현 |
| [promptRegressionEvalPlan](./promptRegressionEvalPlan.md) | — |
| [rawqRequiredSidecarPlan](./rawqRequiredSidecarPlan.md) | sidecar bundle + daemon startup + fs watcher 구현 |
| [skillRegistryPlan](./skillRegistryPlan.md) | 4-layer 스킬 + 레지스트리 + 스킬팩 구현 (세션 10) |
| [structuredMemorySourceStrengtheningPlan_2026-03-30](./structuredMemorySourceStrengtheningPlan_2026-03-30.md) | budget weight 재조정 (structured > conversational, 세션 18) |
| [tauri2PluginAdoptionPlan](./tauri2PluginAdoptionPlan.md) | notification/store/dialog/window-state/clipboard/shell 적용 |
| [threadModelRoundtableRedesign](./threadModelRoundtableRedesign.md) | branch.mode + RT branch + shadow conv + 드로어 RT + 사이드바 통합 |
| [workflowDocumentV2Plan](./workflowDocumentV2Plan.md) | Architect 직접 작성 + semantic versioning 문서 모델 |
| [workflowPipelineV2Plan](./workflowPipelineV2Plan.md) | V2 6-stage 설계 + Phase 1 진행 (subtask_review, DevProgress) |
| [workflowStabilizationPlan](./workflowStabilizationPlan.md) | Phase 1 프롬프트 양식 부분 구현, Phase 3-1 idle timeout |

## ✅ 완료 — `docs/archive/plans/completed/`

70개 plan 아카이브됨. [목록 보기](../archive/plans/completed/)

## ⏸ 보류 — `docs/archive/plans/deferred/`

| 문서 | 사유 |
|------|------|
| [chatVirtualizationPlan](../archive/plans/deferred/chatVirtualizationPlan.md) | 긴 대화 성능 문제 체감 시 착수 (react-virtuoso 의존성은 도입 완료) |
| [collaborationContextEngineParityPlan](../archive/plans/deferred/collaborationContextEngineParityPlan.md) | plan/findings/artifact parity 기본 달성. 세밀 조정 후순위 |
| [contextPackEngineParityPlan](../archive/plans/deferred/contextPackEngineParityPlan.md) | normalized ContextPack 통합 완료. 세밀 parity는 후순위 |
| [gitAwareBranchModelPlan](../archive/plans/deferred/gitAwareBranchModelPlan.md) | git 필드 준비만 됨. 실제 git 연동은 후순위 |
| [messageSearchAdoptionPlan](../archive/plans/deferred/messageSearchAdoptionPlan.md) | FTS5 구현 완료. 전문 검색 확장은 후순위 |
| [rawqAutomationPlan](../archive/plans/deferred/rawqAutomationPlan.md) | rawq 기본 연동 완료. 자동화는 후순위 |
| [rawqCodeReviewGraphIntegrationPlan](../archive/plans/deferred/rawqCodeReviewGraphIntegrationPlan.md) | code-review-graph 미도입. rawq 단독 운영 |
| [rawqEngineParityPlan](../archive/plans/deferred/rawqEngineParityPlan.md) | rawq section 4-engine 공통 완료. diagnostics parity 후순위 |
| [resumeContinuationEngineParityPlan](../archive/plans/deferred/resumeContinuationEngineParityPlan.md) | Claude native + non-Claude context replay 완료. 세밀 조정 후순위 |
| [sidecarMigrationPlan](../archive/plans/deferred/sidecarMigrationPlan.md) | direct-call로 충분. 필요 시 재검토 |
| [skillsEngineParityPlan](../archive/plans/deferred/skillsEngineParityPlan.md) | 키워드 매칭 선택적 주입 완료. 세밀 parity는 후순위 |
| [streamingEngineParityPlan](../archive/plans/deferred/streamingEngineParityPlan.md) | streaming UX 기본 parity 달성. typing indicator 통일 후순위 |
| [tokenCostTrackingEngineParityPlan](../archive/plans/deferred/tokenCostTrackingEngineParityPlan.md) | usage/cost 모델 parity 후순위 |

## 📦 IA/검토/가이드 초안 — `docs/archive/plans/misc/`

31개 초안 아카이브됨. [목록 보기](../archive/plans/misc/)

## 📊 통계

- 진행 예정/진행 중: **27개**
- 부분 완료: **26개**
- 완료 (archive): **70개**
- 보류 (archive): **13개**
- IA/검토 (archive): **31개**
- **합계**: 167개
