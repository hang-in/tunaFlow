# Plans

실행 계획, 로드맵, 리팩토링 계획, 테스트 계획 문서.

## 진행 현황 (2026-04-11 기준, 세션 20 재분류 반영)

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
| [sidebarThreeSectionPlan](./sidebarThreeSectionPlan.md) | Chats 하위 트리로 RT/Branch 통합, RoundtablesSection/BranchesSection 폐기 |
| [contextPackP0Phase1Plan_2026-03-30](./contextPackP0Phase1Plan_2026-03-30.md) | section visibility + compression + budget UI — 3 phase 모두 구현 완료 (세션 2) |
| [contextPackVisibilityUiPolishPlan_2026-03-30](./contextPackVisibilityUiPolishPlan_2026-03-30.md) | TracePanel/StatusBar context metadata 가시화 구현 완료 (세션 2) |
| [contextPackCompressionAndRawqPostprocessPlan_2026-03-30](./contextPackCompressionAndRawqPostprocessPlan_2026-03-30.md) | compression 품질 + rawq 후처리 Phase 1+2 구현 완료 (세션 2) |
| [contextBudgetControlUiPlan_2026-03-30](./contextBudgetControlUiPlan_2026-03-30.md) | Settings > Runtime context budget mode/cap UI 구현 완료 (세션 2) |
| [contextHubMinimalIntegrationPlan_2026-03-30](./contextHubMinimalIntegrationPlan_2026-03-30.md) | context-hub CLI health/search/get + source policy 구현 완료 (세션 2) |
| [contextHubSidecarIntegrationPlan_2026-03-29](./contextHubSidecarIntegrationPlan_2026-03-29.md) | Phase 1 CLI 연동 구현 완료 (세션 2) |
| [agentIdentityFramingPlan_2026-03-30](./agentIdentityFramingPlan_2026-03-30.md) | `## Identity` 블록 3층 분리 (profile/engine/persona) 구현 완료 (세션 2) |
| [agentIdentityValidationPlan_2026-03-30](./agentIdentityValidationPlan_2026-03-30.md) | 4-engine identity 검증 구현 완료 (세션 2) |
| [messageAuthorAttributionPlan_2026-03-30](./messageAuthorAttributionPlan_2026-03-30.md) | 메시지별 `[assistant:Profile (engine)]` 작성자 태그 구현 완료 (세션 2) |
| [longTermMemoryPhase1CompressionPlan_2026-03-30](./longTermMemoryPhase1CompressionPlan_2026-03-30.md) | conversation_memory v17 + 12+ 메시지 구조화 요약 구현 완료 (세션 2) |
| [runtimeSettingsImplementationPlan_2026-03-30](./runtimeSettingsImplementationPlan_2026-03-30.md) | Settings > Runtime rawq/model/budget/daemon 구현 완료 (세션 2) |
| [settingsShellIaPlan_2026-03-29](./settingsShellIaPlan_2026-03-29.md) | Settings Shell — Agents/Personas/Skills/Runtime 4섹션 구현 완료 (세션 2) |
| [skillsUiVisibilityPlan](./skillsUiVisibilityPlan.md) | vendor 그룹핑 + 검색/필터 + snapshot 메타 표시 구현 완료 (세션 1) |
| [chatMarkdownCodeblockUpgradePlan_2026-03-29](./chatMarkdownCodeblockUpgradePlan_2026-03-29.md) | 코드블록 헤더 + collapse/expand + copy 피드백 Phase 1 구현 완료 (세션 1) |
| [chatFileViewerIntegrationPlan_2026-03-29](./chatFileViewerIntegrationPlan_2026-03-29.md) | 파일 경로 클릭 + FileViewer 모달 Phase 1 구현 완료 (세션 1) |
| [tracePanelRuntimeFirstPlan_2026-03-29](./tracePanelRuntimeFirstPlan_2026-03-29.md) | TracePanel runtime 대시보드 전환 Phase 1 구현 완료 (세션 1) |
| [agentProfilesSettingsMvpPlan_2026-03-29](./agentProfilesSettingsMvpPlan_2026-03-29.md) | Settings > Agents profile 목록/편집/persistence 구현 완료 (세션 1-2) |
| [agentProfileChatInputBindingPlan_2026-03-29](./agentProfileChatInputBindingPlan_2026-03-29.md) | ProfileSelector + 실행 반영 구현 완료 (세션 1-2) |
| [personaRuntimeBindingPlan_2026-03-29](./personaRuntimeBindingPlan_2026-03-29.md) | persona section 주입 + 4-engine parity + applied persona 표시 구현 완료 (세션 2) |
| [appliedAgentConfigVisibilityPlan_2026-03-29](./appliedAgentConfigVisibilityPlan_2026-03-29.md) | message.persona에 profile label → MessageMeta 표시 구현 완료 (세션 2) |
| [artifactManualPromotionMvpPlan_2026-03-30](./artifactManualPromotionMvpPlan_2026-03-30.md) | SaveArtifactDialog 수동 승격 구현 완료 (세션 2) |
| [artifactsTabUsabilityPlan_2026-03-30](./artifactsTabUsabilityPlan_2026-03-30.md) | 필터/정렬/통합 리스트 구현 완료 (세션 2) |
| [artifactDetailViewPlan_2026-03-30](./artifactDetailViewPlan_2026-03-30.md) | content 읽기 + status 변경 + copy/forward/delete 모달 구현 완료 (세션 2) |
| [artifactProvenanceWorkflowPlan_2026-03-30](./artifactProvenanceWorkflowPlan_2026-03-30.md) | source conversation/branch/RT 표시 + jumpToSource 구현 완료 (세션 2) |
| [artifactNavigationActionsPlan_2026-03-30](./artifactNavigationActionsPlan_2026-03-30.md) | 카드 + 모달 subtask link 표시 구현 완료 (세션 2) |
| [chatSearchFtsPlan_2026-03-30](./chatSearchFtsPlan_2026-03-30.md) | FTS5 + SearchBox in CenterPanel 구현 완료 (세션 2) |
| [personaBehaviorValidationPlan_2026-03-30](./personaBehaviorValidationPlan_2026-03-30.md) | 7종 built-in persona promptFragment 검증 완료 (세션 2) |
| [personaCliValidationPlan_2026-03-30](./personaCliValidationPlan_2026-03-30.md) | CLI persona 주입 검증 완료 (세션 2) |
| [handoffTruncationFixPlan_2026-03-30](./handoffTruncationFixPlan_2026-03-30.md) | 핸드오프 컨텍스트 보존 구현 완료 (세션 2) |
| [orchestratedWorkflowPipelinePlan](./orchestratedWorkflowPipelinePlan.md) | V1 Phase A-E 전체 구현 완료 (세션 5). superseded by V2 |
| [dependencyAdoptionPlan](./dependencyAdoptionPlan.md) | Phase 1-4.2 완료 — clipboard/shell/opener/fs/chrono/tokio/virtuoso/cmdk/sonner (세션 4b) |
| [documentationIaGovernancePlan_2026-03-30](./documentationIaGovernancePlan_2026-03-30.md) | 문서 IA/거버넌스 규칙 수립 완료 (세션 2) |
| [contextPackAlgorithmPhase1Plan_2026-03-30](./contextPackAlgorithmPhase1Plan_2026-03-30.md) | Jaccard dedup, markdown 경량화, import collapse, rawq 다해상도 구현 완료 (세션 2-4) |
| [conversationRetrievalPhase1Plan_2026-03-30](./conversationRetrievalPhase1Plan_2026-03-30.md) | FTS5 retrieval + pair/anchor/brief 재조립 구현 완료 (세션 4-7) |
| [conversationRetrievalChunkingPlan_2026-03-30](./conversationRetrievalChunkingPlan_2026-03-30.md) | chunk 단위 retrieval 구현 완료 (세션 4-7) |
| [conversationRetrievalRankingPolishPlan_2026-03-30](./conversationRetrievalRankingPolishPlan_2026-03-30.md) | scoring, dedup, overlap suppression 구현 완료 (세션 4) |
| [unifiedMemoryPolicyPhase1Plan_2026-03-30](./unifiedMemoryPolicyPhase1Plan_2026-03-30.md) | working/structured/compressed/retrieval 통합 selection policy 구현 완료 (세션 4-18) |
| [unifiedMemoryPolicyThresholdTuningPlan_2026-03-30](./unifiedMemoryPolicyThresholdTuningPlan_2026-03-30.md) | retrieval/compressed memory 임계값 튜닝 구현 완료 (세션 3-4) |
| [memoryPolicyTraceSurfacePlan_2026-03-30](./memoryPolicyTraceSurfacePlan_2026-03-30.md) | memory layer active/skipped + auto reason + trace surface 구현 완료 (세션 2-3) |
| [memorySectionBudgetBreakdownPlan_2026-03-30](./memorySectionBudgetBreakdownPlan_2026-03-30.md) | section별 budget 소비량 observability 구현 완료 (세션 2) |
| [topHeavySectionTuningPlan_2026-03-30](./topHeavySectionTuningPlan_2026-03-30.md) | heavy section cap 조정 구현 완료 (세션 2-3) |
| [modeSpecificSectionHeuristicsPlan_2026-03-30](./modeSpecificSectionHeuristicsPlan_2026-03-30.md) | Lite/Std/Full 차등 cap 구현 완료 (세션 2-3) |
| [autoModeHeuristicPolishPlan_2026-03-30](./autoModeHeuristicPolishPlan_2026-03-30.md) | auto scoring/profile 선택 구현 완료 (세션 2-3) |
| [projectFirstStartupUxPlan_2026-03-30](./projectFirstStartupUxPlan_2026-03-30.md) | project-first startup flow 구현 완료 (세션 1-4) |
| [roundtableCompletionOrderPlan_2026-03-30](./roundtableCompletionOrderPlan_2026-03-30.md) | Deliberative RT completion-order collection 구현 완료 (세션 8-9) |
| [roundtableBlindVerifierPhasePlan_2026-03-30](./roundtableBlindVerifierPhasePlan_2026-03-30.md) | blind verifier RT 확장 구현 완료 (세션 8-9) |
| [roundtableParticipantRoleBlindUiPlan_2026-03-30](./roundtableParticipantRoleBlindUiPlan_2026-03-30.md) | RT participant role/blind 설정 UI 구현 완료 (세션 8-9) |
| [roundtableParticipantSurfaceVisibilityPlan_2026-03-30](./roundtableParticipantSurfaceVisibilityPlan_2026-03-30.md) | RT role/blind badge 가시화 구현 완료 (세션 8-9) |

### 부분 완료

| 문서 | 요약 | 남은 것 |
|------|------|--------|
| [harnessEngineeringAdoptionPlan](./harnessEngineeringAdoptionPlan.md) | Phase 1-6 workspace 가시화 완료 | Review/Test 독립 모드, runtime RBAC |
| [masterTestPlan](./masterTestPlan.md) | Rust 188 + Frontend 175 = 363 tests | E2E smoke, coverage 목표, property test |
| [messagePairDeletionPlan](./messagePairDeletionPlan.md) | user+assistant 인접 메시지 쌍 삭제 계획 | 구현 미확인 |
| [tauri2PluginAdoptionPlan](./tauri2PluginAdoptionPlan.md) | notification/store/dialog/window-state/clipboard/shell 적용 | updater 미적용 |
| [ownerAgentAssignmentPlan](./ownerAgentAssignmentPlan.md) | DB 필드 + PlansPanel UI dropdown 존재 | 자동 할당 로직, agent lane 연동 |
| [projectOnboardingLifecyclePlan](./projectOnboardingLifecyclePlan.md) | folder picker + validation + auto main conv + rawq + scaffolding | workspace scan, template, guided setup |
| [threadModelRoundtableRedesign](./threadModelRoundtableRedesign.md) | branch.mode + RT branch + shadow conv + 드로어 RT + 사이드바 통합 | reviewer thread |
| [gitSyncBranchModelPlan_2026-03-29](./gitSyncBranchModelPlan_2026-03-29.md) | branch↔git 동기화 모델 설계. adopt=merge, delete=pointer-only | git CLI 연동, .tunaflow/ 구조 |
| [projectScopedConcurrencyPlan](./projectScopedConcurrencyPlan.md) | thread-local queue 구현 | 프로젝트 간 병렬 UI, cross-project queue |
| [rawqRequiredSidecarPlan](./rawqRequiredSidecarPlan.md) | sidecar bundle + daemon startup + fs watcher 구현 | 배포 경로 정리 |
| [skillRegistryPlan](./skillRegistryPlan.md) | 4-layer 스킬 + 레지스트리 + 스킬팩 구현 (세션 10) | collections UX 고도화 |
| [2026-03-28_skills_runtime_snapshot_plan](./2026-03-28_skills_runtime_snapshot_plan.md) | Phase 1 snapshot 발행 완료 | Phase 2 manifest 고도화, Phase 3 앱 번들 |
| [engineFeatureParityClassificationPlan](./engineFeatureParityClassificationPlan.md) | 4-engine parity 분류 기준 + Claude parity fix 완료 | 기능별 개별 parity 실행 |
| [chatUiParityWithTunaChatPlan](./chatUiParityWithTunaChatPlan.md) | Pretendard 3-tier 폰트, max-w-4xl, 아바타 인라인 (세션 14) | message density 고도화 |
| [agentProfileUsagePolishPlan_2026-03-29](./agentProfileUsagePolishPlan_2026-03-29.md) | profile summary 기본 표시 | custom 규칙 가시화, RT 표시 polish |
| [contextHubSearchGetUiPlan_2026-03-30](./contextHubSearchGetUiPlan_2026-03-30.md) | Settings UI + chops ContextPack 자동 주입 (세션 7) | 수동 UI polish |
| [contextPackAlgorithmImprovementsPlan](./contextPackAlgorithmImprovementsPlan.md) | P2 동적 예산 배분(allocate_budgets) 구현 | P1 Jaccard dedup, P3-P5 미착수 |
| [agentDaemonRoadmapPlan](./agentDaemonRoadmapPlan.md) | Phase 1-2 완료 (background worker + job registry) | Phase 3 daemon extraction |
| [planBasedFollowupPlan](./planBasedFollowupPlan.md) | PlansPanel Forward 버튼 구현 | subtask 단위 자동 dispatch |
| [workflowPipelineV2Plan](./workflowPipelineV2Plan.md) | V2 6-stage 설계 + Phase 1 진행 (subtask_review, DevProgress) | 세부 UX 미완성 |
| [workflowStabilizationPlan](./workflowStabilizationPlan.md) | Phase 1 프롬프트 양식 부분 구현, Phase 3-1 idle timeout | Phase 2, 3-2~3-3 미구현 |
| [workflowDocumentV2Plan](./workflowDocumentV2Plan.md) | Architect 직접 작성 + semantic versioning 문서 모델 | 구현 미착수 |
| [longTermMemoryRoadmapPlan_2026-03-30](./longTermMemoryRoadmapPlan_2026-03-30.md) | Phase 1 compressed + Phase 2 retrieval/vector + Phase 3 session discovery 완료 (세션 7-18) + 자동 트리거 배선 완료 (세션 20) | Phase 4 cross-project recall |
| [compressedMemoryOperationalPolishPlan_2026-03-30](./compressedMemoryOperationalPolishPlan_2026-03-30.md) | provenance/model_used/force_recompress 구현 (세션 7) | UI 가시화 미확인 |
| [structuredMemorySourceStrengtheningPlan_2026-03-30](./structuredMemorySourceStrengtheningPlan_2026-03-30.md) | budget weight 재조정 (structured > conversational, 세션 18) | 역할 분리 Trace 표시 |

### 보류

| 문서 | 이유 |
|------|------|
| [chatVirtualizationPlan](./chatVirtualizationPlan.md) | 긴 대화 성능 문제 체감 시 착수 (react-virtuoso 의존성은 도입 완료) |
| [sidecarMigrationPlan](./sidecarMigrationPlan.md) | direct-call로 충분. 필요 시 재검토 |
| [rawqCodeReviewGraphIntegrationPlan](./rawqCodeReviewGraphIntegrationPlan.md) | code-review-graph 미도입. rawq 단독 운영 |
| [rawqAutomationPlan](./rawqAutomationPlan.md) | rawq 기본 연동 완료. 자동화는 후순위 |
| [gitAwareBranchModelPlan](./gitAwareBranchModelPlan.md) | git 필드 준비만 됨. 실제 git 연동은 후순위 |
| [messageSearchAdoptionPlan](./messageSearchAdoptionPlan.md) | FTS5 구현 완료. 전문 검색 확장은 후순위 |
| [skillsEngineParityPlan](./skillsEngineParityPlan.md) | 키워드 매칭 선택적 주입 완료. 세밀 parity는 후순위 |
| [contextPackEngineParityPlan](./contextPackEngineParityPlan.md) | normalized ContextPack 통합 완료. 세밀 parity는 후순위 |
| [collaborationContextEngineParityPlan](./collaborationContextEngineParityPlan.md) | plan/findings/artifact parity 기본 달성. 세밀 조정 후순위 |
| [rawqEngineParityPlan](./rawqEngineParityPlan.md) | rawq section 4-engine 공통 완료. diagnostics parity 후순위 |
| [streamingEngineParityPlan](./streamingEngineParityPlan.md) | streaming UX 기본 parity 달성. typing indicator 통일 후순위 |
| [tokenCostTrackingEngineParityPlan](./tokenCostTrackingEngineParityPlan.md) | usage/cost 모델 parity 후순위 |
| [resumeContinuationEngineParityPlan](./resumeContinuationEngineParityPlan.md) | Claude native + non-Claude context replay 완료. 세밀 조정 후순위 |

### 진행 예정

| 문서 | 우선순위 |
|------|---------|
| [metaAgentPlan](./metaAgentPlan.md) | **P0** — 메타에이전트. 온보딩, 이슈 감지, 우선순위 제안. 모든 프로젝트 대상 |
| [geminiSdkIntegrationPlan](./geminiSdkIntegrationPlan.md) | P1 — Google AI SDK 직접 통합 (CLI 대체, SSE/token/function calling) |
| [toolCallHandlerPlan](./toolCallHandlerPlan.md) | P1 — function calling으로 마커 대체 |
| [roundtableRoleTerminologySeparationPlan_2026-03-30](./roundtableRoleTerminologySeparationPlan_2026-03-30.md) | P1 — 프로필 역할 vs RT 토론 역할 분리 |
| [runtimeFeatureValidationPlan_2026-03-30](./runtimeFeatureValidationPlan_2026-03-30.md) | P1 — memory/retrieval/auto/budget/RT 실시나리오 검증 |
| [postParityRuntimeValidationSweepPlan_2026-03-30](./postParityRuntimeValidationSweepPlan_2026-03-30.md) | P1 — parity fix 효과 재검증 |
| [liveRuntimeTraceParityValidationPlan_2026-03-30](./liveRuntimeTraceParityValidationPlan_2026-03-30.md) | P1 — 4-engine trace/meta parity 실제 확인 |
| [realWorkflowMemoryQualityValidationPlan_2026-03-30](./realWorkflowMemoryQualityValidationPlan_2026-03-30.md) | P1 — memory/retrieval 응답 품질 검증 |
| [contextBudgetScalingPlan](./contextBudgetScalingPlan.md) | P2 — 단계적 context budget 상향 실험 |
| [i18nPlan](./i18nPlan.md) | P1 — UI 한/영 분리 (react-i18next) + 프롬프트 영어 통일 + ContextPack 응답 언어 주입 |
| [refactorRoadmap_2026-04-20](./refactorRoadmap_2026-04-20.md) | **베타 전 리팩토링 + 안정화 5-Phase 로드맵** (16~19일) — 프로덕션급 베타 기준 |
| [refactorRoadmap_handoff_2026-04-20](./refactorRoadmap_handoff_2026-04-20.md) | 새 세션용 핸드오프 — 프로젝트 철학 / 피할 함정 / Phase 1 Finding 6 진입점 |
| [refactorRoadmap_first_prompt](./refactorRoadmap_first_prompt.md) | 새 세션에 붙여넣을 첫 프롬프트 텍스트 |

### 기타 (IA/검토/가이드)

| 문서 | 설명 |
|------|------|
| [agentSkillPersonaIaPlan_2026-03-29](./agentSkillPersonaIaPlan_2026-03-29.md) | agent/persona/settings IA 초안 — 대부분 구현 반영됨 |
| [sidebarWorkspaceHierarchyPlan_2026-03-29](./sidebarWorkspaceHierarchyPlan_2026-03-29.md) | 사이드바 위계 1차 계획 — 구현 반영됨 |
| [artifactsAsMainTabAndMemoAssistPlan_2026-03-29](./artifactsAsMainTabAndMemoAssistPlan_2026-03-29.md) | Artifacts 메인 탭 승격 계획 — 구현 반영됨 |
| [personaBaselineReviewPlan_2026-03-29](./personaBaselineReviewPlan_2026-03-29.md) | persona baseline 비교 검토 — 세션 2에서 반영 |
| [chopsContextHubTunaFlowIntegrationIaPlan_2026-03-29](./chopsContextHubTunaFlowIntegrationIaPlan_2026-03-29.md) | chops/context-hub/tunaFlow 통합 IA 초안 |
| [settingsSkillsKnowledgeSourcesPlan_2026-03-29](./settingsSkillsKnowledgeSourcesPlan_2026-03-29.md) | Settings Skills/Knowledge Sources 분리 초안 |
| [knowledgeSourcesSettingsShellPlan_2026-03-30](./knowledgeSourcesSettingsShellPlan_2026-03-30.md) | Knowledge Sources 제품 셸 계획 |
| [documentMetadataAdoptionPlan_2026-03-30](./documentMetadataAdoptionPlan_2026-03-30.md) | 문서 메타 단계적 도입 계획 |
| [personaVsHandoffValidationPlan_2026-03-30](./personaVsHandoffValidationPlan_2026-03-30.md) | persona vs handoff 검증 — 세션 2에서 반영 |
| [codeHygienePassPlan_2026-03-30](./codeHygienePassPlan_2026-03-30.md) | 코드 위생 패스 — 세션 3에서 부분 반영 |
| [promptQualityAdoptionPlan_2026-03-30](./promptQualityAdoptionPlan_2026-03-30.md) | 프롬프트 품질 도입 계획 |
| [contextHubExplicitHandoffPlan_2026-03-30](./contextHubExplicitHandoffPlan_2026-03-30.md) | context-hub 명시적 핸드오프 |
| [contextStackReevaluationPlan_2026-03-30](./contextStackReevaluationPlan_2026-03-30.md) | context 스택 재평가 |
| [deferredTechReevaluationPlan_2026-03-30](./deferredTechReevaluationPlan_2026-03-30.md) | 보류 기술 재평가 |
| [tokenCostDbParityPlan_2026-03-30](./tokenCostDbParityPlan_2026-03-30.md) | token/cost DB 레벨 parity |
| [evaluationUiConnectionPlan_2026-03-30](./evaluationUiConnectionPlan_2026-03-30.md) | Evaluation UI 연결 계획 |
| [evaluationUnderTestPlan_2026-03-30](./evaluationUnderTestPlan_2026-03-30.md) | Evaluation 테스트 계획 |
| [evaluationRunCreationUiPlan_2026-03-30](./evaluationRunCreationUiPlan_2026-03-30.md) | Evaluation Run 생성 UI |
| [evaluationRunExecutionLinkagePlan_2026-03-30](./evaluationRunExecutionLinkagePlan_2026-03-30.md) | Evaluation Run 실행 연결 |
| [evaluationRunExecutionRealWiringPlan_2026-03-30](./evaluationRunExecutionRealWiringPlan_2026-03-30.md) | Evaluation Run 실제 배선 |
| [evaluationUsabilityPassPlan_2026-03-30](./evaluationUsabilityPassPlan_2026-03-30.md) | Evaluation 사용성 패스 |
| [gitSyncPhase1Plan_2026-03-30](./gitSyncPhase1Plan_2026-03-30.md) | Git sync Phase 1 |
| [gitBranchLinkVisibilityPlan_2026-03-30](./gitBranchLinkVisibilityPlan_2026-03-30.md) | Git branch link 가시화 |
| [gitBranchDefaultingPlan_2026-03-30](./gitBranchDefaultingPlan_2026-03-30.md) | Git branch 기본값 |
| [gitSyncPhase2GuardedActionsPlan_2026-03-30](./gitSyncPhase2GuardedActionsPlan_2026-03-30.md) | Git sync Phase 2 guarded actions |
| [roundtableCreationConfigPlan](./roundtableCreationConfigPlan.md) | RT 생성 설정 계획 |
| [contextPackTraceabilityPlan](./contextPackTraceabilityPlan.md) | ContextPack 추적성 계획 |
| [codebaseRefactoringProposal](./codebaseRefactoringProposal.md) | 코드베이스 리팩토링 제안서 |
| [2026-03-28_claude_skill_activation_guide](./2026-03-28_claude_skill_activation_guide.md) | Claude Code 스킬 활성화 운영 가이드 |
| [opusRefactorPlan](./opusRefactorPlan.md) | 초기 대규모 리팩토링 — 점진 개선으로 대체, 참고용 |

## 문서 목록

- [2026-03-28_claude_skill_activation_guide](./2026-03-28_claude_skill_activation_guide.md)
- [2026-03-28_skills_runtime_snapshot_plan](./2026-03-28_skills_runtime_snapshot_plan.md)
- [agentCollaborationPlan](./agentCollaborationPlan.md)
- [agentDaemonRoadmapPlan](./agentDaemonRoadmapPlan.md)
- [agentIdentityFramingPlan_2026-03-30](./agentIdentityFramingPlan_2026-03-30.md)
- [agentIdentityValidationPlan_2026-03-30](./agentIdentityValidationPlan_2026-03-30.md)
- [agentProfileChatInputBindingPlan_2026-03-29](./agentProfileChatInputBindingPlan_2026-03-29.md)
- [agentProfilesSettingsMvpPlan_2026-03-29](./agentProfilesSettingsMvpPlan_2026-03-29.md)
- [agentProfileUsagePolishPlan_2026-03-29](./agentProfileUsagePolishPlan_2026-03-29.md)
- [agentSkillPersonaIaPlan_2026-03-29](./agentSkillPersonaIaPlan_2026-03-29.md)
- [appliedAgentConfigVisibilityPlan_2026-03-29](./appliedAgentConfigVisibilityPlan_2026-03-29.md)
- [artifactDetailViewPlan_2026-03-30](./artifactDetailViewPlan_2026-03-30.md)
- [artifactManualPromotionMvpPlan_2026-03-30](./artifactManualPromotionMvpPlan_2026-03-30.md)
- [artifactNavigationActionsPlan_2026-03-30](./artifactNavigationActionsPlan_2026-03-30.md)
- [artifactProvenanceWorkflowPlan_2026-03-30](./artifactProvenanceWorkflowPlan_2026-03-30.md)
- [artifactsAsMainTabAndMemoAssistPlan_2026-03-29](./artifactsAsMainTabAndMemoAssistPlan_2026-03-29.md)
- [artifactsTabUsabilityPlan_2026-03-30](./artifactsTabUsabilityPlan_2026-03-30.md)
- [autoModeHeuristicPolishPlan_2026-03-30](./autoModeHeuristicPolishPlan_2026-03-30.md)
- [backgroundAgentExecutionPlan](./backgroundAgentExecutionPlan.md)
- [chatFileViewerIntegrationPlan_2026-03-29](./chatFileViewerIntegrationPlan_2026-03-29.md)
- [chatMarkdownCodeblockUpgradePlan_2026-03-29](./chatMarkdownCodeblockUpgradePlan_2026-03-29.md)
- [chatSearchFtsPlan_2026-03-30](./chatSearchFtsPlan_2026-03-30.md)
- [chatUiMarkdownUpgradePlan](./chatUiMarkdownUpgradePlan.md)
- [chatUiParityWithTunaChatPlan](./chatUiParityWithTunaChatPlan.md)
- [chatVirtualizationPlan](./chatVirtualizationPlan.md)
- [chopsContextHubTunaFlowIntegrationIaPlan_2026-03-29](./chopsContextHubTunaFlowIntegrationIaPlan_2026-03-29.md)
- [claudeContextLightweightPlan](./claudeContextLightweightPlan.md)
- [codebaseRefactoringProposal](./codebaseRefactoringProposal.md)
- [codeHygienePassPlan_2026-03-30](./codeHygienePassPlan_2026-03-30.md)
- [collaborationContextEngineParityPlan](./collaborationContextEngineParityPlan.md)
- [compressedMemoryOperationalPolishPlan_2026-03-30](./compressedMemoryOperationalPolishPlan_2026-03-30.md)
- [contextBudgetControlUiPlan_2026-03-30](./contextBudgetControlUiPlan_2026-03-30.md)
- [contextBudgetScalingPlan](./contextBudgetScalingPlan.md)
- [contextHubExplicitHandoffPlan_2026-03-30](./contextHubExplicitHandoffPlan_2026-03-30.md)
- [contextHubMinimalIntegrationPlan_2026-03-30](./contextHubMinimalIntegrationPlan_2026-03-30.md)
- [contextHubSearchGetUiPlan_2026-03-30](./contextHubSearchGetUiPlan_2026-03-30.md)
- [contextHubSidecarIntegrationPlan_2026-03-29](./contextHubSidecarIntegrationPlan_2026-03-29.md)
- [contextPackAlgorithmImprovementsPlan](./contextPackAlgorithmImprovementsPlan.md)
- [contextPackAlgorithmPhase1Plan_2026-03-30](./contextPackAlgorithmPhase1Plan_2026-03-30.md)
- [contextPackCompressionAndRawqPostprocessPlan_2026-03-30](./contextPackCompressionAndRawqPostprocessPlan_2026-03-30.md)
- [contextPackEngineParityPlan](./contextPackEngineParityPlan.md)
- [contextPackP0Phase1Plan_2026-03-30](./contextPackP0Phase1Plan_2026-03-30.md)
- [contextPackTraceabilityPlan](./contextPackTraceabilityPlan.md)
- [contextPackVisibilityUiPolishPlan_2026-03-30](./contextPackVisibilityUiPolishPlan_2026-03-30.md)
- [contextStackReevaluationPlan_2026-03-30](./contextStackReevaluationPlan_2026-03-30.md)
- [conversationRetrievalChunkingPlan_2026-03-30](./conversationRetrievalChunkingPlan_2026-03-30.md)
- [conversationRetrievalPhase1Plan_2026-03-30](./conversationRetrievalPhase1Plan_2026-03-30.md)
- [conversationRetrievalRankingPolishPlan_2026-03-30](./conversationRetrievalRankingPolishPlan_2026-03-30.md)
- [conversationVectorSearchPlan](./conversationVectorSearchPlan.md)
- [deferredTechReevaluationPlan_2026-03-30](./deferredTechReevaluationPlan_2026-03-30.md)
- [dependencyAdoptionPlan](./dependencyAdoptionPlan.md)
- [documentationIaGovernancePlan_2026-03-30](./documentationIaGovernancePlan_2026-03-30.md)
- [documentMetadataAdoptionPlan_2026-03-30](./documentMetadataAdoptionPlan_2026-03-30.md)
- [engineFeatureParityClassificationPlan](./engineFeatureParityClassificationPlan.md)
- [engineModelCatalogPlan](./engineModelCatalogPlan.md)
- [evaluationRunCreationUiPlan_2026-03-30](./evaluationRunCreationUiPlan_2026-03-30.md)
- [evaluationRunExecutionLinkagePlan_2026-03-30](./evaluationRunExecutionLinkagePlan_2026-03-30.md)
- [evaluationRunExecutionRealWiringPlan_2026-03-30](./evaluationRunExecutionRealWiringPlan_2026-03-30.md)
- [evaluationUiConnectionPlan_2026-03-30](./evaluationUiConnectionPlan_2026-03-30.md)
- [evaluationUnderTestPlan_2026-03-30](./evaluationUnderTestPlan_2026-03-30.md)
- [evaluationUsabilityPassPlan_2026-03-30](./evaluationUsabilityPassPlan_2026-03-30.md)
- [betaReleaseReadinessPlan](./betaReleaseReadinessPlan.md)
- [betaRtUpgradeSprintPlan_2026-04-15](./betaRtUpgradeSprintPlan_2026-04-15.md) — 베타 직전 RT 고도화 sprint (Quick/Deep 2트랙 + 루브릭 + Agent-as-Judge + README)
- [cicdReleasePlan](./cicdReleasePlan.md)
- [geminiSdkIntegrationPlan](./geminiSdkIntegrationPlan.md)
- [gitAwareBranchModelPlan](./gitAwareBranchModelPlan.md)
- [gitBranchDefaultingPlan_2026-03-30](./gitBranchDefaultingPlan_2026-03-30.md)
- [gitBranchLinkVisibilityPlan_2026-03-30](./gitBranchLinkVisibilityPlan_2026-03-30.md)
- [gitSyncBranchModelPlan_2026-03-29](./gitSyncBranchModelPlan_2026-03-29.md)
- [gitSyncPhase1Plan_2026-03-30](./gitSyncPhase1Plan_2026-03-30.md)
- [gitSyncPhase2GuardedActionsPlan_2026-03-30](./gitSyncPhase2GuardedActionsPlan_2026-03-30.md)
- [handoffTruncationFixPlan_2026-03-30](./handoffTruncationFixPlan_2026-03-30.md)
- [harnessEngineeringAdoptionPlan](./harnessEngineeringAdoptionPlan.md)
- [knowledgeSourcesSettingsShellPlan_2026-03-30](./knowledgeSourcesSettingsShellPlan_2026-03-30.md)
- [liveRuntimeTraceParityValidationPlan_2026-03-30](./liveRuntimeTraceParityValidationPlan_2026-03-30.md)
- [longTermMemoryPhase1CompressionPlan_2026-03-30](./longTermMemoryPhase1CompressionPlan_2026-03-30.md)
- [longTermMemoryRoadmapPlan_2026-03-30](./longTermMemoryRoadmapPlan_2026-03-30.md)
- [masterTestPlan](./masterTestPlan.md)
- [memoryPolicyTraceSurfacePlan_2026-03-30](./memoryPolicyTraceSurfacePlan_2026-03-30.md)
- [memorySectionBudgetBreakdownPlan_2026-03-30](./memorySectionBudgetBreakdownPlan_2026-03-30.md)
- [messageAuthorAttributionPlan_2026-03-30](./messageAuthorAttributionPlan_2026-03-30.md)
- [messagePairDeletionPlan](./messagePairDeletionPlan.md)
- [messageSearchAdoptionPlan](./messageSearchAdoptionPlan.md)
- [modelsCommandCatalogPlan](./modelsCommandCatalogPlan.md)
- [modeSpecificSectionHeuristicsPlan_2026-03-30](./modeSpecificSectionHeuristicsPlan_2026-03-30.md)
- [naturalLanguageHandoffPlan](./naturalLanguageHandoffPlan.md)
- [opusRefactorPlan](./opusRefactorPlan.md)
- [orchestratedWorkflowPipelinePlan](./orchestratedWorkflowPipelinePlan.md)
- [ownerAgentAssignmentPlan](./ownerAgentAssignmentPlan.md)
- [panelDrawerUxPlan](./panelDrawerUxPlan.md)
- [personaBaselineReviewPlan_2026-03-29](./personaBaselineReviewPlan_2026-03-29.md)
- [personaBehaviorValidationPlan_2026-03-30](./personaBehaviorValidationPlan_2026-03-30.md)
- [personaCliValidationPlan_2026-03-30](./personaCliValidationPlan_2026-03-30.md)
- [personaRuntimeBindingPlan_2026-03-29](./personaRuntimeBindingPlan_2026-03-29.md)
- [personaVsHandoffValidationPlan_2026-03-30](./personaVsHandoffValidationPlan_2026-03-30.md)
- [planBasedFollowupPlan](./planBasedFollowupPlan.md)
- [postParityRuntimeValidationSweepPlan_2026-03-30](./postParityRuntimeValidationSweepPlan_2026-03-30.md)
- [progressFirstStreamingPlan](./progressFirstStreamingPlan.md)
- [projectFirstStartupUxPlan_2026-03-30](./projectFirstStartupUxPlan_2026-03-30.md)
- [projectOnboardingLifecyclePlan](./projectOnboardingLifecyclePlan.md)
- [projectScopedConcurrencyPlan](./projectScopedConcurrencyPlan.md)
- [promptQualityAdoptionPlan_2026-03-30](./promptQualityAdoptionPlan_2026-03-30.md)
- [rawqAutomationPlan](./rawqAutomationPlan.md)
- [refactorRoadmap_2026-04-20](./refactorRoadmap_2026-04-20.md)
- [refactorRoadmap_handoff_2026-04-20](./refactorRoadmap_handoff_2026-04-20.md)
- [refactorRoadmap_first_prompt](./refactorRoadmap_first_prompt.md)
- [rawqCodeReviewGraphIntegrationPlan](./rawqCodeReviewGraphIntegrationPlan.md)
- [rawqEngineParityPlan](./rawqEngineParityPlan.md)
- [rawqIntegrationPlan](./rawqIntegrationPlan.md)
- [rawqRequiredSidecarPlan](./rawqRequiredSidecarPlan.md)
- [realWorkflowMemoryQualityValidationPlan_2026-03-30](./realWorkflowMemoryQualityValidationPlan_2026-03-30.md)
- [resumeContinuationEngineParityPlan](./resumeContinuationEngineParityPlan.md)
- [roundtableBlindVerifierPhasePlan_2026-03-30](./roundtableBlindVerifierPhasePlan_2026-03-30.md)
- [roundtableCompletionOrderPlan_2026-03-30](./roundtableCompletionOrderPlan_2026-03-30.md)
- [roundtableCreationConfigPlan](./roundtableCreationConfigPlan.md)
- [roundtableParticipantRoleBlindUiPlan_2026-03-30](./roundtableParticipantRoleBlindUiPlan_2026-03-30.md)
- [roundtableParticipantSurfaceVisibilityPlan_2026-03-30](./roundtableParticipantSurfaceVisibilityPlan_2026-03-30.md)
- [roundtableRoleTerminologySeparationPlan_2026-03-30](./roundtableRoleTerminologySeparationPlan_2026-03-30.md)
- [runtimeFeatureValidationPlan_2026-03-30](./runtimeFeatureValidationPlan_2026-03-30.md)
- [runtimeSettingsImplementationPlan_2026-03-30](./runtimeSettingsImplementationPlan_2026-03-30.md)
- [scalabilityRefactorPlan](./scalabilityRefactorPlan.md)
- [settingsShellIaPlan_2026-03-29](./settingsShellIaPlan_2026-03-29.md)
- [settingsSkillsKnowledgeSourcesPlan_2026-03-29](./settingsSkillsKnowledgeSourcesPlan_2026-03-29.md)
- [sidebarThreeSectionPlan](./sidebarThreeSectionPlan.md)
- [sidebarWorkspaceHierarchyPlan_2026-03-29](./sidebarWorkspaceHierarchyPlan_2026-03-29.md)
- [sidecarMigrationPlan](./sidecarMigrationPlan.md)
- [skillRegistryPlan](./skillRegistryPlan.md)
- [skillSelectorAgentPlan](./skillSelectorAgentPlan.md)
- [skillsEngineParityPlan](./skillsEngineParityPlan.md)
- [skillsUiVisibilityPlan](./skillsUiVisibilityPlan.md)
- [streamingEngineParityPlan](./streamingEngineParityPlan.md)
- [structuredMemorySourceStrengtheningPlan_2026-03-30](./structuredMemorySourceStrengtheningPlan_2026-03-30.md)
- [tauri2PluginAdoptionPlan](./tauri2PluginAdoptionPlan.md)
- [threadContextInheritancePlan](./threadContextInheritancePlan.md)
- [threadLocalRunQueuePlan](./threadLocalRunQueuePlan.md)
- [threadModelRoundtableRedesign](./threadModelRoundtableRedesign.md)
- [tokenCostDbParityPlan_2026-03-30](./tokenCostDbParityPlan_2026-03-30.md)
- [tokenCostTrackingEngineParityPlan](./tokenCostTrackingEngineParityPlan.md)
- [toolCallHandlerPlan](./toolCallHandlerPlan.md)
- [topHeavySectionTuningPlan_2026-03-30](./topHeavySectionTuningPlan_2026-03-30.md)
- [tracePanelRuntimeFirstPlan_2026-03-29](./tracePanelRuntimeFirstPlan_2026-03-29.md)
- [unifiedMemoryPolicyPhase1Plan_2026-03-30](./unifiedMemoryPolicyPhase1Plan_2026-03-30.md)
- [unifiedMemoryPolicyThresholdTuningPlan_2026-03-30](./unifiedMemoryPolicyThresholdTuningPlan_2026-03-30.md)
- [workflowDocumentV2Plan](./workflowDocumentV2Plan.md)
- [workflowPipelineV2Plan](./workflowPipelineV2Plan.md)
- [workflowStabilizationPlan](./workflowStabilizationPlan.md)
- [workspacePanelRedesignPlan](./workspacePanelRedesignPlan.md)
