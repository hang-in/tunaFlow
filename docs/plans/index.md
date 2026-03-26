# Plans

실행 계획, 로드맵, 리팩토링 계획, 테스트 계획 문서.

## 진행 현황 (2026-03-27 기준)

### 완료

| 문서 | 요약 |
|------|------|
| [agentCollaborationPlan](./agentCollaborationPlan.md) | Shared Brief, Findings, Artifact Handoff, Follow-up UX — Phase 1-3 구현 완료 |
| [chatUiMarkdownUpgradePlan](./chatUiMarkdownUpgradePlan.md) | react-markdown + remark-gfm + MarkdownComponents 구현 완료 |
| [claudeContextLightweightPlan](./claudeContextLightweightPlan.md) | ContextMode lite/standard/full + rawq 조건부 skip 구현 완료 |
| [engineModelCatalogPlan](./engineModelCatalogPlan.md) | curated catalog + list_engine_models + UI selector 구현 완료 |
| [modelsCommandCatalogPlan](./modelsCommandCatalogPlan.md) | `!models` 명령 + engineModels 통합 구현 완료 |
| [naturalLanguageHandoffPlan](./naturalLanguageHandoffPlan.md) | Phase A 구현 완료 — ENGINE_ALIASES + GOAL_ALIASES + source 우선순위 |
| [progressFirstStreamingPlan](./progressFirstStreamingPlan.md) | progressContent + ProgressBlock/ProgressSummary + stream/done 분리 구현 완료 |
| [rawqIntegrationPlan](./rawqIntegrationPlan.md) | rawq CLI 실제 연동, search/index/status, auto-indexing 구현 완료 |
| [scalabilityRefactorPlan](./scalabilityRefactorPlan.md) | 확장 대비 store/sidebar/input/agents.rs 중심 리팩토링 로드맵 |
| [threadLocalRunQueuePlan](./threadLocalRunQueuePlan.md) | runningThreadIds[] + messageQueue[] + thread-aware cancel 구현 완료 |
| [threadContextInheritancePlan](./threadContextInheritancePlan.md) | anchor message + parent turns + RT inheritance Phase 1 구현 완료 |
| [workspacePanelRedesignPlan](./workspacePanelRedesignPlan.md) | 3-mode workspace panel (Plan/Artifacts/Trace) Phase A 구현 완료 |
| [panelDrawerUxPlan](./panelDrawerUxPlan.md) | resizable panels + overlay drawer Phase 1 구현 완료 |
| [sidebarThreeSectionPlan](./sidebarThreeSectionPlan.md) | Projects/Roundtables/Branches/Files 4섹션 구현 완료 |

### 부분 완료

| 문서 | 요약 | 남은 것 |
|------|------|--------|
| [harnessEngineeringAdoptionPlan](./harnessEngineeringAdoptionPlan.md) | Phase 1-5 완료 (artifact types, approval gates, developer/reviewer lane). Phase 6 workspace 가시화 1차 완료 | Review/Test 독립 모드, runtime RBAC |
| [masterTestPlan](./masterTestPlan.md) | Rust unit 27, DB integration 13, Frontend API 13, CI 구축 | E2E smoke, coverage 목표, property test |
| [messagePairDeletionPlan](./messagePairDeletionPlan.md) | 일반 chat에서 user+assistant 인접 메시지 쌍을 함께 삭제하는 계획 |
| [tauri2PluginAdoptionPlan](./tauri2PluginAdoptionPlan.md) | notification, store, dialog, window-state 적용 | clipboard, shell, updater 미적용 |
| [ownerAgentAssignmentPlan](./ownerAgentAssignmentPlan.md) | DB 필드 + PlansPanel UI dropdown 존재 | 자동 할당 로직, agent lane 연동 |
| [projectOnboardingLifecyclePlan](./projectOnboardingLifecyclePlan.md) | folder picker + validation + auto main conv + rawq indexing | workspace scan, template, guided setup |
| [threadModelRoundtableRedesign](./threadModelRoundtableRedesign.md) | branch.mode + RT branch + shadow conversation 구현 | thread-first 모델 통합, reviewer thread |
| [projectScopedConcurrencyPlan](./projectScopedConcurrencyPlan.md) | thread-local queue 구현 (선행 조건 충족) | 프로젝트 간 병렬 UI 표시, cross-project queue |

### 보류

| 문서 | 이유 |
|------|------|
| [sidecarMigrationPlan](./sidecarMigrationPlan.md) | direct-call로 충분. 필요 시 재검토 |
| [rawqCodeReviewGraphIntegrationPlan](./rawqCodeReviewGraphIntegrationPlan.md) | code-review-graph 미도입. rawq 단독 운영 |
| [rawqAutomationPlan](./rawqAutomationPlan.md) | rawq 기본 연동 완료. 자동화는 후순위 |
| [gitAwareBranchModelPlan](./gitAwareBranchModelPlan.md) | git 필드 준비만 됨. 실제 git 연동은 후순위 |
| [opusRefactorPlan](./opusRefactorPlan.md) | 초기 대규모 리팩토링 계획. 점진 구조 개선으로 대체 |
| [messageSearchAdoptionPlan](./messageSearchAdoptionPlan.md) | FTS 스키마만 존재. 현재 프로젝트 중심 로컬 필터로 충분 |

### 진행 예정

| 문서 | 우선순위 |
|------|---------|
| [planBasedFollowupPlan](./planBasedFollowupPlan.md) | PlansPanel Forward 버튼은 구현됨. subtask 단위 자동 dispatch는 미구현 |
| [contextPackTraceabilityPlan](./contextPackTraceabilityPlan.md) | 에이전트 응답에 어떤 ContextPack이 사용되었는지 추적하는 구조. trace_log 확장 + UI 표시 |

## 문서 목록

- [agentCollaborationPlan](./agentCollaborationPlan.md)
- [chatUiMarkdownUpgradePlan](./chatUiMarkdownUpgradePlan.md)
- [claudeContextLightweightPlan](./claudeContextLightweightPlan.md)
- [contextPackTraceabilityPlan](./contextPackTraceabilityPlan.md)
- [engineModelCatalogPlan](./engineModelCatalogPlan.md)
- [gitAwareBranchModelPlan](./gitAwareBranchModelPlan.md)
- [harnessEngineeringAdoptionPlan](./harnessEngineeringAdoptionPlan.md)
- [masterTestPlan](./masterTestPlan.md)
- [messagePairDeletionPlan](./messagePairDeletionPlan.md)
- [messageSearchAdoptionPlan](./messageSearchAdoptionPlan.md)
- [modelsCommandCatalogPlan](./modelsCommandCatalogPlan.md)
- [naturalLanguageHandoffPlan](./naturalLanguageHandoffPlan.md)
- [ownerAgentAssignmentPlan](./ownerAgentAssignmentPlan.md)
- [panelDrawerUxPlan](./panelDrawerUxPlan.md)
- [planBasedFollowupPlan](./planBasedFollowupPlan.md)
- [projectOnboardingLifecyclePlan](./projectOnboardingLifecyclePlan.md)
- [projectScopedConcurrencyPlan](./projectScopedConcurrencyPlan.md)
- [progressFirstStreamingPlan](./progressFirstStreamingPlan.md)
- [opusRefactorPlan](./opusRefactorPlan.md)
- [rawqAutomationPlan](./rawqAutomationPlan.md)
- [rawqCodeReviewGraphIntegrationPlan](./rawqCodeReviewGraphIntegrationPlan.md)
- [rawqIntegrationPlan](./rawqIntegrationPlan.md)
- [roundtableCreationConfigPlan](./roundtableCreationConfigPlan.md)
- [scalabilityRefactorPlan](./scalabilityRefactorPlan.md)
- [sidebarThreeSectionPlan](./sidebarThreeSectionPlan.md)
- [sidecarMigrationPlan](./sidecarMigrationPlan.md)
- [tauri2PluginAdoptionPlan](./tauri2PluginAdoptionPlan.md)
- [threadContextInheritancePlan](./threadContextInheritancePlan.md)
- [threadLocalRunQueuePlan](./threadLocalRunQueuePlan.md)
- [threadModelRoundtableRedesign](./threadModelRoundtableRedesign.md)
- [workspacePanelRedesignPlan](./workspacePanelRedesignPlan.md)
