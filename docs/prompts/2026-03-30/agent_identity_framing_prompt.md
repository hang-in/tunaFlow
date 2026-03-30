# Agent Identity Framing

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

모든 응답과 보고는 한국어로만 작성하라.
일본어/영어 혼용 금지.

먼저 읽을 문서:
- `docs/plans/agentIdentityFramingPlan_2026-03-30.md`
- `docs/reference/promptAuthoringPolicy_2026-03-30.md`
- `docs/how-to/tunaflow_persona_baseline_6.md`

먼저 확인할 파일:
- `src-tauri/src/commands/agents.rs`
- `src-tauri/src/commands/send_common.rs`
- `src-tauri/src/commands/roundtable_helpers/prompt.rs`
- `src/components/tunaflow/input/NewMessageInput.tsx`
- `src/components/tunaflow/chat/MessageMeta.tsx`
- agent profile / persona / runtime prompt assembly와 직접 연결된 파일만 추가로 열어라

skill 지시:
- 관련 skill이 있더라도 필요한 것만 최소로 사용하라
- 과도한 skill 로딩 금지
- 이번 작업은 identity framing 규칙이 핵심이므로, skill은 보조 수단일 때만 사용하라

작업 시작 전 짧게 의견을 말하라:
- 왜 지금 identity framing 규칙이 필요한지
- profile / engine / persona를 왜 분리해서 설명해야 하는지
- 이번 단계에서 무엇을 하지 말아야 하는지

이번 작업 목표:
- 에이전트가 자연어 응답에서 자기 정체성을 설명할 때 `profile`, `engine`, `persona`를 일관되게 구분하도록 공통 framing 규칙을 반영하라

구현 범위:
1. 현재 self-identification이 실제로 어떤 공통 prompt 경로를 통해 결정되는지 먼저 확인하라
2. `agent profile`을 1순위 자기소개로 두고, `engine`은 필요 시 2순위 정보로 설명하게 하는 규칙을 반영하라
3. persona는 자기 이름처럼 답하지 않도록 하고, 필요 시 role/policy 정보로만 설명하게 하라
4. 사용자가 잘못된 모델 이름으로 부를 때 짧고 일관되게 정정하도록 하라
5. 4개 엔진 모두 같은 규칙을 따르게 하되, 엔진별 분기보다 공통 조립 경로를 우선 사용하라
6. 가능하면 최소 검증 시나리오를 남겨라:
   - `너 코덱스야?`
   - `지금 누가 답하고 있어?`
   - `엔진이 뭐야?`

비목표:
- 새 agent profile 추가
- profile naming 체계 재설계
- message meta UI 대형 변경
- 전체 persona 시스템 리팩토링
- 영어/일본어 identity 별칭 확장

구현 원칙:
- 가능하면 공통 prompt assembly 한 곳에서 해결하라
- 이미 있는 applied profile/persona 표시 체계와 충돌하지 말라
- 사용자가 보게 되는 1급 이름은 profile이라는 원칙을 유지하라
- `Claude Code(opencode)` 같은 혼합 표현은 만들지 말라

검증:
- `cargo check`
- `tsc --noEmit`
- 가능한 범위에서 identity 질문 시나리오를 최소 1회 이상 확인
- profile / engine / persona가 한 문장 안에서 혼동되지 않는지 확인

출력 형식:
### A. Opinion
### B. Decision
### C. Files Changed
### D. Identity Rule
### E. Verification
### F. Residual Gaps
