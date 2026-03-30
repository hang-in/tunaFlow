# Message Author Attribution

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

모든 응답과 보고는 한국어로만 작성하라.
일본어/영어 혼용 금지.

먼저 읽을 문서:
- `docs/plans/agentIdentityFramingPlan_2026-03-30.md`
- `docs/plans/messageAuthorAttributionPlan_2026-03-30.md`
- `docs/plans/agentIdentityValidationPlan_2026-03-30.md`
- `docs/reference/promptAuthoringPolicy_2026-03-30.md`

먼저 확인할 파일:
- `src-tauri/src/commands/send_common.rs`
- `src-tauri/src/commands/agents.rs`
- `src/components/tunaflow/chat/MessageMeta.tsx`
- 현재 speaker와 과거 message author가 어디서 섞일 수 있는지 직접 연결된 파일만 추가로 보라

작업 시작 전 짧게 의견을 말하라:
- 왜 self-identification만으로는 부족한지
- 현재 speaker와 과거 message author를 왜 분리해야 하는지

이번 작업 목표:
- 멀티에이전트 대화에서 현재 응답 주체와 과거 메시지 작성자를 분리해서 설명하도록 attribution framing을 보강하라

구현 범위:
1. 현재 speaker와 과거 author를 혼동하게 만드는 공통 prompt/path를 먼저 확인하라
2. 과거 메시지 작성자를 물을 때는 message meta/profile label을 우선 사용하게 하라
3. 현재 speaker는 `지금 답하는 주체`, 과거 author는 `해당 메시지를 작성한 주체`로 분리해서 답하게 하라
4. author 정보가 불명확하면 추측하지 말고 한계를 짧게 설명하게 하라
5. 최소 검증 질문을 포함하라:
   - `방금 전 3개의 대답은 누가 한거야?`
   - `지금 답하는 건 누구야?`
   - `그 답변들도 네가 한 거야?`

비목표:
- DB 스키마 변경
- message meta UI 대형 변경
- 새 agent profile 추가
- 전체 conversation ownership 모델 재설계

구현 원칙:
- self-identification 규칙을 깨지 말고 그 위에 attribution 규칙을 얹어라
- author 메타가 있으면 추측보다 메타를 우선하라
- `같은 세션이므로 다 내 답변이다` 같은 단정은 금지하라

검증:
- `cargo check`
- `tsc --noEmit`
- 가능하면 attribution 질문 시나리오를 최소 1회 이상 확인
- 현재 speaker와 과거 author가 혼동되지 않는지 확인

출력 형식:
### A. Opinion
### B. Files Changed
### C. Attribution Rule
### D. Verification
### E. Residual Gaps
