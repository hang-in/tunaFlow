# Ideas — 브레인스토밍 / 탐색 문서

> 갱신: 2026-04-22
> 성격: 구현 확정된 plan으로 가기 전의 탐색·설계·분석 메모. `status: idea` 가 기본.
> 완료·해결된 idea는 `docs/archive/ideas/completed/` 로 이동.

## 📄 활성 ideas (56개)

| 문서 | 요약 |
|------|------|
| [abtopAnalysisForTunaFlow](./abtopAnalysisForTunaFlow.md) | `abtop`에서 tunaFlow에 가져올 만한 핵심은 **에이전트 런타임 관측/진단 모델**이다. |
| [agentSkillsReferenceIdea](./agentSkillsReferenceIdea.md) | `agent-skills`는 tunaFlow의 **전체 아키텍처 레퍼런스**로는 맞지 않다. |
| [architectEnhancementIdea](./architectEnhancementIdea.md) | — |
| [artifactAndFailureLearningIdea](./artifactAndFailureLearningIdea.md) | | 타입 | DB 테이블 | 실제 사용 | |
| [artifactsTabDesignReviewIdea](./artifactsTabDesignReviewIdea.md) | ```sql |
| [bgeM3QuantizationAndAcceleratorIdea](./bgeM3QuantizationAndAcceleratorIdea.md) | | 항목 | 값 | |
| [blog-contextpack-draft](./blog-contextpack-draft.md) | tunaFlow는 Claude, Codex, Gemini, OpenCode 같은 CLI 에이전트를 하나의 앱에서 오케스트레이션하는 데스크톱 클라이언트입니다. 에이전트 하나만 쓸 때는 비교적 단순합니 |
| [ciExecutionLoopIdea](./ciExecutionLoopIdea.md) | — |
| [ciMultiOsPlan](./ciMultiOsPlan.md) | - `.github/workflows/ci.yml`: windows-latest 단일, Node 20, actions v4 |
| [claudeCodePatternsForTunaFlow](./claudeCodePatternsForTunaFlow.md) | Claude Code의 아키텍처 패턴 중 tunaFlow 고도화에 적용 가치가 있는 항목을 정리한다. 각 항목에 tunaFlow 현재 상태, 적용 방안, 예상 효과를 기술한다. |
| [clawSoulsPersonaSpecIdea](./clawSoulsPersonaSpecIdea.md) | **AI 에이전트 페르소나 공유 플랫폼**. 마크다운 파일 묶음(Soul)으로 에이전트의 성격/행동/스타일을 정의하고 공유하는 오픈 스펙. 80+ 큐레이션된 Soul. Apache 2.0. |
| [clawTeamAnalysis](./clawTeamAnalysis.md) | ClawTeam은 홍콩대학교 데이터과학 연구실(HKUDS)의 멀티에이전트 오케스트레이션 프레임워크. tmux + 파일시스템 기반으로 CLI 에이전트(Claude Code, Codex 등)를 스폰하고 |
| [codeAgentOrchestraReferenceIdea](./codeAgentOrchestraReferenceIdea.md) | | 글의 패턴 | tunaFlow 구현 | 비고 | |
| [codeReviewGraphIntegrationIdea](./codeReviewGraphIntegrationIdea.md) | Developer/Reviewer에게 **코드 변경의 영향 범위, 호출 관계, 테스트 매핑**을 제공. |
| [codeReviewRefactoringIdea](./codeReviewRefactoringIdea.md) | | # | 수정 | 상태 | |
| [computerUseE2eTestIdea](./computerUseE2eTestIdea.md) | tunaFlow는 Tauri 데스크톱 앱이라 Playwright/Cypress 같은 웹 테스트 프레임워크로 e2e 커버가 안 된다. 기존 smoke test는 깨진 상태이고, Tauri WebVie |
| [contextPackTieringIdea](./contextPackTieringIdea.md) | tunaFlow는 "다중 에이전트 오케스트레이션 클라이언트(AOC)". 핵심 가치인 멀티에이전트를 쓰려면 현재 토큰 비용이 너무 높음. |
| [customTitlebarContextMenuIdea](./customTitlebarContextMenuIdea.md) | - **타이틀바**: Tauri 기본 네이티브 타이틀바. 앱 이름만 표시. 커스텀 기능 없음. |
| [designSystemIdea](./designSystemIdea.md) | | 문제 | 수치 | |
| [externalReferenceCatalog_2026-04-11](./externalReferenceCatalog_2026-04-11.md) | | 대상 | 성격 | 출처 | 분석 기준 시점 | 문서 | |
| [guardrailImprovementIdeas](./guardrailImprovementIdeas.md) | Claude Code는 6-layer 안전 파이프라인을 구현하고 있다: |
| [hermesAgentPatternsIdea](./hermesAgentPatternsIdea.md) | NousResearch의 자기 개선형 멀티 에이전트 프레임워크. OpenAI SDK 기반 멀티 프로바이더, 멀티 플랫폼(Telegram/Discord/Slack/WhatsApp/Signal), SQ |
| [httpApiTestInfraIdea](./httpApiTestInfraIdea.md) | 현재 tunaFlow의 Tauri command는 **앱 내부에서만 호출 가능**합니다. |
| [insightTabDesign](./insightTabDesign.md) | - Review/Test 탭이 Artifacts 탭의 필터링된 뷰에 불과 (데이터 중복) |
| [insightWorkflowIdea](./insightWorkflowIdea.md) | 1. Insight 분석 결과가 DB에만 있고, **에이전트가 접근할 수 없음** — 아키텍트에게 "Insight 탭 볼 수 있어?"라고 물으면 "볼 수 없다"고 답함 |
| [knowledgeLayerArchitectureIdea](./knowledgeLayerArchitectureIdea.md) | ```rust |
| [larksuiteCliArchitectureReferenceIdea](./larksuiteCliArchitectureReferenceIdea.md) | - 작성: 2026-04-06 |
| [litertLmIntegrationIdea](./litertLmIntegrationIdea.md) | Google `LiteRT-LM` (구 TFLite-LM) — WebGPU/Wasm 환경에서 LLM 온디바이스 추론에 최적화된 런타임. 주로 Gemma 계열과 소형 LLM 대상. |
| [mexContextScaffoldIdea](./mexContextScaffoldIdea.md) | AI 에이전트를 위한 **프로젝트 컨텍스트 스캐폴드 관리 도구**. 에이전트가 세션 간에 프로젝트를 "기억"할 수 있도록 구조화된 마크다운 문서를 유지하고, 문서와 실제 코드의 **drift(불일치 |
| [mobileArchitectureIdea](./mobileArchitectureIdea.md) | — |
| [modernSqliteFeaturesIdea](./modernSqliteFeaturesIdea.md) | ```rust |
| [onboardingMetaAgentIdea](./onboardingMetaAgentIdea.md) | — |
| [openHarnessLightRagReferenceIdea](./openHarnessLightRagReferenceIdea.md) | Claude Code의 Python 클린룸 구현. 43+ 도구, 54 CLI 명령, 114 테스트. Protocol 기반 LLM 추상화로 Anthropic/OpenAI/Ollama 런타임 스왑 가능 |
| [projectDocumentRagIdea](./projectDocumentRagIdea.md) | tunaFlow에는 이미 4개의 지식 검색 경로가 있습니다: |
| [projectMetaAgentIdea](./projectMetaAgentIdea.md) | | 작업 | 현재 누가 하는가 | 메타 에이전트가 하면 | |
| [projectPerWindowIdea](./projectPerWindowIdea.md) | 현재 단일 앱 인스턴스에서 프로젝트 드롭다운으로 전환하는 구조인데: |
| [ptyFullIntegrationPlan](./ptyFullIntegrationPlan.md) | | 항목 | 상태 | |
| [ptyInteractiveIdea](./ptyInteractiveIdea.md) | — |
| [ptySessionPolicy](./ptySessionPolicy.md) | 1. **채팅 = Claude 세션 1:1** — tunaFlow의 Conversation 하나가 Claude Code의 Session 하나에 대응 |
| [rawqGraphEvolutionStrategyIdea](./rawqGraphEvolutionStrategyIdea.md) | | | rawq | code-review-graph | |
| [referenceRepoReviewV2Idea](./referenceRepoReviewV2Idea.md) | tunaFlow가 세션 5 이후 크게 진척됨: |
| [removeGlobalProfileStateIdea](./removeGlobalProfileStateIdea.md) | — |
| [reviewerWorkflowEnhancementsIdea](./reviewerWorkflowEnhancementsIdea.md) | dev → review 워크플로우의 reviewer 단계를 사용자 통제 + 토큰 효율 + 검증 품질 셋 다 개선하기 위한 아이디어 모음. 디버깅 중에 떠올랐고 잊지 않으려 일단 정리. |
| [reworkSubtaskTargetingIdea](./reworkSubtaskTargetingIdea.md) | Review에서 서브태스크 1개가 실패 → Rework → Developer가 **전체 서브태스크를 다시 구현**. |
| [rtAlgorithmEnhancementIdeas](./rtAlgorithmEnhancementIdeas.md) | — |
| [sdkAsInterfaceLayerIdea](./sdkAsInterfaceLayerIdea.md) | `sdkIntegrationIdea.md`는 SDK를 "유료 API를 직접 호출하는 수단"으로 봤다. 이 문서는 **API 키 없이도 SDK에서 얻을 수 있는 가치**에 집중한다. |
| [sdkIntegrationIdea](./sdkIntegrationIdea.md) | — |
| [seCallVectorStorageIdea](./seCallVectorStorageIdea.md) | seCall의 벡터 스토리지 선택지: |
| [smallModelStressTesterIdea](./smallModelStressTesterIdea.md) | **소형 모델(3B active)이 설계 취약점을 정직하게 노출한다.** |
| [speedyClaudeToolOptimizationIdea](./speedyClaudeToolOptimizationIdea.md) | 터미널에서 파일 탐색/검색/치환에 기본 도구(find, grep, sed)를 쓰면 느리다. Modern CLI 도구로 10-64x 개선 가능. |
| [tabSidebarRestructureIdea](./tabSidebarRestructureIdea.md) | — |
| [techPostSeriesIdea](./techPostSeriesIdea.md) | | # | 제목 | 핵심 | 톤 | |
| [terminalInChatIdea](./terminalInChatIdea.md) | PTY 에이전트 실행 중 채팅 메시지 버블 대신 **xterm.js 터미널을 채팅 영역 인라인으로 표시**하고, |
| [traceEnhancementAbtopIdea](./traceEnhancementAbtopIdea.md) | 매 에이전트 호출마다 `trace_log`에 기록: |
| [vectorDbAndRetrievalAlgorithmsIdea](./vectorDbAndRetrievalAlgorithmsIdea.md) | — |
| [workflowGraphEnhancementIdea](./workflowGraphEnhancementIdea.md) | 현재 워크플로우 파이프라인: |

## 📦 완료된 ideas — `docs/archive/ideas/completed/`

- [chatReadabilityImprovementIdea](../archive/ideas/completed/chatReadabilityImprovementIdea.md)
- [embeddingLatencyOptimizationIdea](../archive/ideas/completed/embeddingLatencyOptimizationIdea.md)
