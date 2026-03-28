# Plans

실행 계획, 로드맵, 리팩토링 계획, 테스트 계획 문서.

## 진행 현황 (2026-03-28 기준)

### 완료

| 문서 | 요약 |
|------|------|
| [agentCollaborationPlan](./agentCollaborationPlan.md) | Shared Brief, Findings, Artifact Handoff, Follow-up UX — Phase 1-3 구현 완료 |
| [chatUiMarkdownUpgradePlan](./chatUiMarkdownUpgradePlan.md) | react-markdown + remark-gfm + MarkdownComponents 구현 완료 |
| [claudeContextLightweightPlan](./claudeContextLightweightPlan.md) | ContextMode lite/standard/full + rawq 조건부 skip 구현 완료 |
| [conversationVectorSearchPlan](./conversationVectorSearchPlan.md) | 대화 의미 검색(sentence-transformers/sqlite-vec) 도입 가능성 검토 메모 |
| [engineModelCatalogPlan](./engineModelCatalogPlan.md) | curated catalog + list_engine_models + UI selector 구현 완료 |
| [modelsCommandCatalogPlan](./modelsCommandCatalogPlan.md) | `!models` 명령 + engineModels 통합 구현 완료 |
| [naturalLanguageHandoffPlan](./naturalLanguageHandoffPlan.md) | Phase A 구현 완료 — ENGINE_ALIASES + GOAL_ALIASES + source 우선순위 |
| [progressFirstStreamingPlan](./progressFirstStreamingPlan.md) | progressContent + ProgressBlock/ProgressSummary + stream/done 분리 구현 완료 |
| [rawqIntegrationPlan](./rawqIntegrationPlan.md) | rawq CLI 실제 연동, search/index/status, auto-indexing 구현 완료 |
| [scalabilityRefactorPlan](./scalabilityRefactorPlan.md) | 확장 대비 store/sidebar/input/agents.rs 중심 리팩토링 로드맵 |

| [backgroundAgentExecutionPlan](./backgroundAgentExecutionPlan.md) | Phase 1 구현 완료 — start_* background commands + event-driven frontend + DB SSOT |
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
| [rawqRequiredSidecarPlan](./rawqRequiredSidecarPlan.md) | rawq를 optional fallback이 아니라 필수 sidecar로 다루는 전환 계획 | vendor/sidecar 정착, 배포 경로 정리 |
| [skillRegistryPlan](./skillRegistryPlan.md) | chops 참고 메모 기반 상위 계획 | UI registry, collections, applied skill visibility |
| [2026-03-28_skills_runtime_snapshot_plan](./2026-03-28_skills_runtime_snapshot_plan.md) | _research/_skills → ~/.tunaflow/skills snapshot 발행. Phase 1 완료 | Phase 2 manifest 고도화, Phase 3 앱 번들 |
| [skillsUiVisibilityPlan](./skillsUiVisibilityPlan.md) | Skills UI 가시화 — vendor 그룹핑, 활성 카운트, snapshot 메타 표시 | Phase 1 최소 가시화부터 |
| [engineFeatureParityClassificationPlan](./engineFeatureParityClassificationPlan.md) | 4개 엔진 parity 기준을 P0/P1/P2로 분류하는 기준 문서 | 기능별 개별 parity 실행 필요 |
| [skillsEngineParityPlan](./skillsEngineParityPlan.md) | skill 적용을 4개 엔진 공통으로 맞추는 계획 | 구현 미착수 |
| [contextPackEngineParityPlan](./contextPackEngineParityPlan.md) | full/lite 분리를 줄이고 normalized ContextPack으로 맞추는 계획 | 구현 미착수 |
| [collaborationContextEngineParityPlan](./collaborationContextEngineParityPlan.md) | plan/findings/artifact/thread inheritance parity 계획 | 구현 미착수 |
| [rawqEngineParityPlan](./rawqEngineParityPlan.md) | rawq section과 diagnostics를 4개 엔진 공통으로 맞추는 계획 | 구현 미착수 |
| [streamingEngineParityPlan](./streamingEngineParityPlan.md) | streaming UX/state parity 계획 | 구현 미착수 |
| [tokenCostTrackingEngineParityPlan](./tokenCostTrackingEngineParityPlan.md) | usage/cost 모델 parity 계획 | 구현 미착수 |
| [resumeContinuationEngineParityPlan](./resumeContinuationEngineParityPlan.md) | native/synthetic continuation parity 계획 | 구현 미착수 |
| [chatUiParityWithTunaChatPlan](./chatUiParityWithTunaChatPlan.md) | tunaChat 수준의 채팅 UI/UX로 단계적으로 끌어올리는 상위 계획 | Markdown/file viewer/message density/virtualization 진행 필요 |
| [chatMarkdownCodeblockUpgradePlan_2026-03-29](./chatMarkdownCodeblockUpgradePlan_2026-03-29.md) | 코드블록 헤더 통합, collapse/expand, copy 피드백 | Phase 1 완료 |
| [chatFileViewerIntegrationPlan_2026-03-29](./chatFileViewerIntegrationPlan_2026-03-29.md) | 파일 경로 클릭 + FileViewer 모달 | Phase 1 완료 |
| [tracePanelRuntimeFirstPlan_2026-03-29](./tracePanelRuntimeFirstPlan_2026-03-29.md) | TracePanel을 runtime 대시보드로 전환 | Phase 1 완료 |

### 보류

| 문서 | 이유 |
|------|------|
| [chatVirtualizationPlan](./chatVirtualizationPlan.md) | 긴 대화 성능 문제 체감 시 착수. 현재 메시지 수 기준 불필요 |

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
| [agentDaemonRoadmapPlan](./agentDaemonRoadmapPlan.md) | Phase 1-2 완료 (background worker + job registry). Phase 3 daemon extraction 진행 예정 |
| [contextBudgetScalingPlan](./contextBudgetScalingPlan.md) | background execution 안정화 후 베타에서 단계적 context budget 상향 실험 |
| [planBasedFollowupPlan](./planBasedFollowupPlan.md) | PlansPanel Forward 버튼은 구현됨. subtask 단위 자동 dispatch는 미구현 |

## 문서 목록

- [agentCollaborationPlan](./agentCollaborationPlan.md)
- [agentDaemonRoadmapPlan](./agentDaemonRoadmapPlan.md)
- [chatUiMarkdownUpgradePlan](./chatUiMarkdownUpgradePlan.md)
- [claudeContextLightweightPlan](./claudeContextLightweightPlan.md)
- [conversationVectorSearchPlan](./conversationVectorSearchPlan.md)
- [contextPackTraceabilityPlan](./contextPackTraceabilityPlan.md)
- [contextBudgetScalingPlan](./contextBudgetScalingPlan.md)
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
- [rawqRequiredSidecarPlan](./rawqRequiredSidecarPlan.md)
- [roundtableCreationConfigPlan](./roundtableCreationConfigPlan.md)
- [scalabilityRefactorPlan](./scalabilityRefactorPlan.md)
- [skillRegistryPlan](./skillRegistryPlan.md)
- [engineFeatureParityClassificationPlan](./engineFeatureParityClassificationPlan.md)
- [skillsEngineParityPlan](./skillsEngineParityPlan.md)
- [contextPackEngineParityPlan](./contextPackEngineParityPlan.md)
- [collaborationContextEngineParityPlan](./collaborationContextEngineParityPlan.md)
- [rawqEngineParityPlan](./rawqEngineParityPlan.md)
- [streamingEngineParityPlan](./streamingEngineParityPlan.md)
- [tokenCostTrackingEngineParityPlan](./tokenCostTrackingEngineParityPlan.md)
- [resumeContinuationEngineParityPlan](./resumeContinuationEngineParityPlan.md)
- [chatUiParityWithTunaChatPlan](./chatUiParityWithTunaChatPlan.md)
- [chatMarkdownCodeblockUpgradePlan_2026-03-29](./chatMarkdownCodeblockUpgradePlan_2026-03-29.md)
- [chatFileViewerIntegrationPlan_2026-03-29](./chatFileViewerIntegrationPlan_2026-03-29.md)
- [backgroundAgentExecutionPlan](./backgroundAgentExecutionPlan.md)
- [sidebarThreeSectionPlan](./sidebarThreeSectionPlan.md)
- [sidecarMigrationPlan](./sidecarMigrationPlan.md)
- [tauri2PluginAdoptionPlan](./tauri2PluginAdoptionPlan.md)
- [threadContextInheritancePlan](./threadContextInheritancePlan.md)
- [threadLocalRunQueuePlan](./threadLocalRunQueuePlan.md)
- [threadModelRoundtableRedesign](./threadModelRoundtableRedesign.md)
- [workspacePanelRedesignPlan](./workspacePanelRedesignPlan.md)
