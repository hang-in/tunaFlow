# Prompts

Opus/Claude에 전달하는 실행 지시문, hand-off 문서, 초기 구현 프롬프트.

- [dataModelPrompt](./dataModelPrompt.md): 데이터 모델 정리/생성용 프롬프트
- [firstImplementationPrompt](./firstImplementationPrompt.md): 초기 구현 지시문
- [handoffMaster](./handoffMaster.md): 신규 작업자 온보딩/핸드오프 문서
- [harnessEngineeringAdoptionPrompt](./harnessEngineeringAdoptionPrompt.md): tunaFlow에 harness engineering을 단계적으로 적용하기 위한 상세 실행 프롬프트
- [messagePairDeletionPrompt](./messagePairDeletionPrompt.md): 일반 chat conversation에서 user+assistant 인접 메시지 쌍을 삭제하는 실행 프롬프트
- [messageSearchAdoptionPrompt](./messageSearchAdoptionPrompt.md): tunaDish의 FTS 검색 UX를 참고해 tunaFlow의 Rust DB 레이어에 메시지 검색을 붙이는 실행 프롬프트
- [panelDrawerUxPrompt](./panelDrawerUxPrompt.md): 패널 리사이즈, overlay thread/RT drawer, 우측 workspace panel 재설계를 위한 실행 프롬프트
- [rawqIntegrationPrompt](./rawqIntegrationPrompt.md): tunaFlow에 실제 rawq CLI를 도입하기 위한 Phase 1 실행 프롬프트
- [roundtableCreationConfigPrompt](./roundtableCreationConfigPrompt.md): RT 생성 dialog에서 고른 participant/mode/model을 첫 실행에 실제 연결하는 프롬프트
- [scalabilityRefactorPrompt](./scalabilityRefactorPrompt.md): 확장 대비 리팩토링을 `chatStore`부터 단계적으로 진행하는 실행 프롬프트
- [sidebarThreeSectionPrompt](./sidebarThreeSectionPrompt.md): 좌측 Sidebar를 Chats / Branches & RT / Files 3섹션 구조로 재구성하는 Phase 1 실행 프롬프트
- [threadContextInheritancePrompt](./threadContextInheritancePrompt.md): thread와 RT에 parent anchor/source/recent turns를 상속시키는 1차 구현 프롬프트
- [workspacePanelRedesignPrompt](./workspacePanelRedesignPrompt.md): ContextPanel을 3모드 우선의 workflow형 workspace panel로 재구성하는 Phase 1 실행 프롬프트
