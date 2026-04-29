# Plans — 진행 현황

> **For external readers**: these are active development plans — artifacts of tunaFlow's self-hosting (Plan → Dev → Review) methodology. You do **not** need to read these to use tunaFlow. For product overview see [README](../../README.md). For architecture see [docs/reference/](../reference/).

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
| [designReviewGatePlan](./designReviewGatePlan.md) | **P1** — Plan 승인 시 Architect↔Codex RT 선택 경로 (PlanProposalCard 2버튼 + branch.mode=design_review + plan 문서 adopt) |
| [conventionsContextSyncPlan](./conventionsContextSyncPlan.md) | — |
| [engineServerModePlan](./engineServerModePlan.md) | — |
| [geminiSdkIntegrationPlan](./geminiSdkIntegrationPlan.md) | P1 — Google AI SDK 직접 통합 (CLI 대체, SSE/token/function calling) |
| [i18nPlan](./i18nPlan.md) | P1 — UI 한/영 분리 (react-i18next) + 프롬프트 영어 통일 + ContextPack 응답 언어 주입 |
| [i18nCompletionPlan_2026-04-24](./i18nCompletionPlan_2026-04-24.md) | **P1** — i18n PR A 완결: A2 잔여 60+ 파일을 A2-D/E/F/G + A3-ext 5 슬라이스로 분할, INV-6 추가, 병렬 Developer 2~3 세션 가능 |
| [insightStabilityPlan](./insightStabilityPlan.md) | **P0 (베타 blocker)** — Insight 분석 4 버그 직렬 수정 (rawq OOB / skip 강화 / claude usage 파서 / timeout→status 전이). 4 subtask, 반나절. |
| [liveRuntimeTraceParityValidationPlan_2026-03-30](./liveRuntimeTraceParityValidationPlan_2026-03-30.md) | P1 — 4-engine trace/meta parity 실제 확인 |
| [metaAgentInitialSetupPlan_2026-04-16](./metaAgentInitialSetupPlan_2026-04-16.md) | — |
| [metaAgentOnboardingPlan_2026-04-16](./metaAgentOnboardingPlan_2026-04-16.md) | — |
| [metaAgentPlan](./metaAgentPlan.md) | **P0** — 메타에이전트. 온보딩, 이슈 감지, 우선순위 제안 + Phase 3 (identity analysis trigger) + Phase 4 (background insight worker). INV 6개. Phase 0 부분 구현 (persona_meta, metaConversation.ts, v33 migration) |
| [projectIdentityAnalysisPlan](./projectIdentityAnalysisPlan.md) | **P1** — Artifacts 기반 "Karma" 파이프라인: 6 타입 자동 생성 + plan 3개/volume 10 trigger + identity_summary 분석 + ContextPack 주입 + Insight 정체성 뷰 (metaAgent 의존) |
| [perProjectDatabaseSplitPlan](./perProjectDatabaseSplitPlan.md) | — |
| [preBetaAuditPlan_2026-04-23](./preBetaAuditPlan_2026-04-23.md) | **P0 (베타 blocker)** — 6개 영역 전수감사 결과 (P0 6건 + P1 5건 + Developer 핸드오프 #4) |
| [publicReadinessChecklistPlan](./publicReadinessChecklistPlan.md) | **P0** — OSS 공개 준비 체크리스트 (Phase 0~4 완료, Phase 5 public flip 남음) |
| [sidecarPipelinePlan_2026-04-24](./sidecarPipelinePlan_2026-04-24.md) | **P1** — rawq → CRG 파이프라인 (두 sidecar 결합 섹션). 반나절~1일. 7편 한계 (B) 연결 |
| [windowsBuildPlan_2026-04-24](./windowsBuildPlan_2026-04-24.md) | **P1** — Windows x64 빌드. 1~2일. Developer 핸드오프 포함 |
| [postBetaBacklogPlan_2026-04-24](./postBetaBacklogPlan_2026-04-24.md) | **P2** — Wiki 10편 한계 섹션 "해결 예정" 17 항목 (B-1 ~ B-17) 통합 백로그 |
| [resultReportMarkerCleanupPlan_2026-04-24](./resultReportMarkerCleanupPlan_2026-04-24.md) | **P1** — result/insight 산출물 tunaflow 마커 잔존 정리 (B-16). stripTunaflowMarkers 공용 유틸 + Rust 안전망. Developer 핸드오프 포함 |
| [customEndpointConfigPlan_2026-04-24](./customEndpointConfigPlan_2026-04-24.md) | **P1** — Ollama / LM Studio base URL override UI (첫 외부 이슈 #175 MVP). RunInput custom_base_url + Settings UI + 호출 시점 주입. Developer 핸드오프 포함 |
| [manualVerificationGatePlan_2026-04-24](./manualVerificationGatePlan_2026-04-24.md) | **P1 (ready-to-implement)** — impl-complete 와 Reviewer 사이 사용자 확인 게이트 (B-19 / Issue #176). ⚠️ Manual 파서 + dialog + Rework 경로 재사용. 피드백 반영 (fail 사유 optional 확정). Developer 핸드오프 포함 |
| [rawqGitignoreIndexFixPlan_2026-04-24](./rawqGitignoreIndexFixPlan_2026-04-24.md) | **P0 (hotfix / Issue #180)** — rawq 가 .gitignore 무시 → target/node_modules 인덱싱 → OOM / 시스템 프리즈. ensure_index 에 빌드 산출물 14개 hardcoded exclude + rebuild UI. Developer 핸드오프 포함 |
| [claudeDangerouslySkipPermissionsPlan_2026-04-24](./claudeDangerouslySkipPermissionsPlan_2026-04-24.md) | **P0 (hotfix / Issue #178)** — Claude headless permission 플래그 bypassPermissions → dangerously-skip-permissions. 3 call site (claude.rs L162/L380 + claude_sdk_session.rs L381). Developer 핸드오프 포함 |
| [onboardingCancelLeakFixPlan_2026-04-25](./onboardingCancelLeakFixPlan_2026-04-25.md) | **P1 (Issue #176 follow-up)** — Codex/Gemini 메타 분석 실패 후 "건너뛰기" → 메인창 lock. handleSkip 에 cancel + error state UI 통합 + asyncCancel pipeline audit. Architect 직접 fix |
| [toolStepsRunningStatusFinalizePlan_2026-04-25](./toolStepsRunningStatusFinalizePlan_2026-04-25.md) | **P1 (Issue #187, MERGED)** — long-doc 후 tool steps spinner 무한 회전. saveToolSteps finalize + ToolStepsView fallback 2 layer. PR #188 머지 완료 |
| [reviewRTEntryFailureRollbackPlan_2026-04-25](./reviewRTEntryFailureRollbackPlan_2026-04-25.md) | **P2 (audit follow-up)** — startReviewRT 8 단계 중 어느 단계 실패가 phase rollback 보장 안 하는지 미확인. step-wise catch + retry UX. Developer 핸드오프 포함 |
| [rawqSidecarReleaseGapPlan_2026-04-26](./rawqSidecarReleaseGapPlan_2026-04-26.md) | **P1 (Beta 사용자 보고)** — rawq 미인식 3 케이스 수렴: DMG sidecar 검증 + UX/문서 보강 + CI verify. Developer 핸드오프 포함 |
| [markdownSingleNewlineBreaksPlan_2026-04-26](./markdownSingleNewlineBreaksPlan_2026-04-26.md) | **P1 (Beta 사용자 보고)** — 채팅/로그 single newline collapse → `remark-breaks` 추가 + 4 위치 SSOT 통합. Developer 핸드오프 포함 |
| [claudeResumeSessionTransitionPlan_2026-04-29](./claudeResumeSessionTransitionPlan_2026-04-29.md) | **P0 (hotfix, MERGED v0.1.4-beta)** — claude 2.1.121 `--sdk-url` localhost 차단 정책 회복. sdk-session → `-p --session-id`/`--resume` transport flip. Architect 직접 진행 |
| [resultMdContaminationFixPlan_2026-04-29](./resultMdContaminationFixPlan_2026-04-29.md) | **P1 (MERGED PR #211 / bc34b53)** — reviewer ContextPack 자동 첨부 result.md 제거(P0) + reportSync truncation/self-include 가드(P1) + i18n 정리(P2). 4 task 분리 커밋. FE 381 / Rust 559 통과 |
| [watchdogAndReviewerReadGuardPlan_2026-04-29](./watchdogAndReviewerReadGuardPlan_2026-04-29.md) | **P1 (v0.1.4-beta publish 직전)** — claude.rs watchdog trailing kill 차단 (RAII guard) + REVIEWER_TEMPLATE 에 `*-result.md` read 금지 명시 (Codex 가 Q1=b 로 read tool 사용 자인). 2 task, 합본 PR. Developer 핸드오프 포함 |
| [branchAdoptRollbackPlan_2026-04-25](./branchAdoptRollbackPlan_2026-04-25.md) | **P2 (audit follow-up)** — adoptBranch LLM/DB 부분 적용 위험 (s25 history). DB transaction + LLM retry + 부분 적용 복구 UX. Developer 핸드오프 포함 |
| [planGenerationRollbackPlan_2026-04-25](./planGenerationRollbackPlan_2026-04-25.md) | **P2 (audit follow-up)** — generate_plan_document LLM/DB/file 부분 적용 위험. atomic transaction + 응답 파싱 강건화 + timeout. Developer 핸드오프 포함 |
| [rawqIndexCancelChannelPlan_2026-04-25](./rawqIndexCancelChannelPlan_2026-04-25.md) | **P3 (audit follow-up, low priority)** — rawq subprocess cancel 채널 부재. Child kill on Drop 패턴 권장. Developer 핸드오프 포함 |
| [codexGeminiOnboardingAnalysisFailureInvestigationPlan_2026-04-25](./codexGeminiOnboardingAnalysisFailureInvestigationPlan_2026-04-25.md) | **P2 (Issue #176 sibling)** — Codex/Gemini 메타 분석 자체 실패 원인 조사 + fix. parse_output 강건화 / build_prompt engine-agnostic / JSON 응답 옵션. Developer 핸드오프 포함 |
| [branchInheritsMainSessionPlan_2026-04-25](./branchInheritsMainSessionPlan_2026-04-25.md) | **P1** — brand=ws 모드의 session 키 통합 + ContextPack 낭비 제거. s36 PTY→WS 전환 시 미반영된 사용자 원래 의도 회복 (raw log + ptySessionPolicy 인용). 4 Layer fix. Developer 핸드오프 포함 |
| [userIntentSsotSurfacingPlan_2026-04-25](./userIntentSsotSurfacingPlan_2026-04-25.md) | **P2 (메타)** — Architect 진입 시 conversation DB(sqlite) 의 사용자 메시지 자동 surface. 같은 mismatch 영구 차단. Task A sibling, 머지 후 진행. Developer 핸드오프 포함 |
| [chatPanelMinHeightCascadePlan_2026-04-25](./chatPanelMinHeightCascadePlan_2026-04-25.md) | **P1 (#191 후속)** — PR #192 가 AppShell main flex 만 fix → ChatPanel 내부 (3 위치) cascade 누락. plan→dev 전이 시 푸터 밀림 재발. Audit + flexbox invariant SSOT 문서화. Developer 핸드오프 포함 |
| [tunaflowOutboxArtifactCleanupPlan_2026-04-25](./tunaflowOutboxArtifactCleanupPlan_2026-04-25.md) | **P3 (housekeeping)** — outbox 방식 폐기 (commit 9295062) 후 4 .md 잔재 + .gitignore 누락. git rm + .gitignore 추가. Developer 핸드오프 포함 |
| [metaFloatingChatPosClampPlan_2026-04-25](./metaFloatingChatPosClampPlan_2026-04-25.md) | **P1 (사용자 가시)** — drawer 열림 시 ChatPanel 0px 압축 race. localStorage `meta-float-pos` stale 값이 부모 bounds 밖 → sibling layout race. mount + ResizeObserver 단일 useEffect 로 clamp + persist. Developer 핸드오프 포함 |
| [multiDeveloperActivePlanIsolationPlan_2026-04-25](./multiDeveloperActivePlanIsolationPlan_2026-04-25.md) | **P1** — multi-Developer 동시 작업 시 active plan 1자리 충돌 (Codex 가 다른 Developer 의 plan 진행 시도). 자동 sub-conv 격리 (A) + ContextPack sender 명시 (B) 조합. Developer 핸드오프 포함 |
| [branchCancelSemanticsPlan_2026-04-25](./branchCancelSemanticsPlan_2026-04-25.md) | **P1 (PR #198 follow-up)** — Task A same-session 모델에서 cancel 작동 안 함. stream abort token 도입 + UI cancel 의미 stream-only 로 재정의. Developer 핸드오프 포함 |
| [mainChatBrandRunningGuardPlan_2026-04-25](./mainChatBrandRunningGuardPlan_2026-04-25.md) | **P1 (PR #198 follow-up)** — brand=main session 공유의 부작용. brand 에서 Developer 작업 중일 때 main chat 입력이 같은 process 에 끼어드는 문제. 옵션 B: main mode + brand running 시 input disable + banner + "드로어 열기". Developer 핸드오프 포함 |
| [selfTrustCiTriggerOptimizationPlan_2026-04-25](./selfTrustCiTriggerOptimizationPlan_2026-04-25.md) | **P1 (applied)** — main 직접 push 시 CI skip. 외부 PR + release tag (build.yml) 만 검증. 인지 부담 fragmenting 해소. Revert 절차 명시 |
| [postParityRuntimeValidationSweepPlan_2026-03-30](./postParityRuntimeValidationSweepPlan_2026-03-30.md) | P1 — parity fix 효과 재검증 |
| [realWorkflowMemoryQualityValidationPlan_2026-03-30](./realWorkflowMemoryQualityValidationPlan_2026-03-30.md) | P1 — memory/retrieval 응답 품질 검증 |
| [roleAssignmentCoverageUxPlan](./roleAssignmentCoverageUxPlan.md) | P2 — Settings 역할 커버리지 UX (inferred 저장 명시화 + stale ID 자동 정리 + assertRoleReady 원클릭 적용) |
| [refactorRoadmap_2026-04-20](./refactorRoadmap_2026-04-20.md) | **베타 전 리팩토링 + 안정화 5-Phase 로드맵** (16~19일) — 프로덕션급 베타 기준 |
| [refactorRoadmap_first_prompt](./refactorRoadmap_first_prompt.md) | 새 세션에 붙여넣을 첫 프롬프트 텍스트 |
| [refactorRoadmap_handoff_2026-04-20](./refactorRoadmap_handoff_2026-04-20.md) | 새 세션용 핸드오프 — 프로젝트 철학 / 피할 함정 / Phase 1 Finding 6 진입점 |
| [roundtableRoleTerminologySeparationPlan_2026-03-30](./roundtableRoleTerminologySeparationPlan_2026-03-30.md) | P1 — 프로필 역할 vs RT 토론 역할 분리 |
| [sessionContinuityFixPlan](./sessionContinuityFixPlan.md) | **P0** — claude WS session identity 를 claude session_id 기반으로 재정의. `is_session_continuation` false positive 해결. |
| [runtimeFeatureValidationPlan_2026-03-30](./runtimeFeatureValidationPlan_2026-03-30.md) | P1 — memory/retrieval/auto/budget/RT 실시나리오 검증 |
| [sdkUrlSessionModePlan](./sdkUrlSessionModePlan.md) | — |
| [searchPipelineFromSecallPlan-part2](./searchPipelineFromSecallPlan-part2.md) | P1 — Phase C Part 2: `messages_fts` rebuild + content_tokenized 컬럼 + app-level snippet (depends: PR #127) |
| [skillSelectorAgentPlan](./skillSelectorAgentPlan.md) | — |
| [structuralImprovementPlan](./structuralImprovementPlan.md) | — |
| [systemMessageChannelPlan](./systemMessageChannelPlan.md) | — |
| [toolCallHandlerPlan](./toolCallHandlerPlan.md) | P1 — function calling으로 마커 대체 |
| [userWorldviewInjectionPlan](./userWorldviewInjectionPlan.md) | P1 (**partial**) — Identity 축 (worldview 주입) 만 유지, subtask-01 머지 완료 (PR #144). Interface/Continuity 축은 2026-04-23 `projectIdentityAnalysisPlan` 으로 이관. 자세한 사유는 본 plan 의 superseded_subtasks 섹션 |
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

- 진행 예정/진행 중: **30개**
- 부분 완료: **26개**
- 완료 (archive): **70개**
- 보류 (archive): **13개**
- IA/검토 (archive): **31개**
- **합계**: 168개
