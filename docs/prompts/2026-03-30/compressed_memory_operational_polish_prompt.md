# Compressed Memory Operational Polish

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

모든 응답과 보고는 한국어로만 작성하라.
일본어/영어 혼용 금지.

먼저 읽을 문서:
- `docs/plans/longTermMemoryRoadmapPlan_2026-03-30.md`
- `docs/plans/longTermMemoryPhase1CompressionPlan_2026-03-30.md`
- `docs/plans/compressedMemoryOperationalPolishPlan_2026-03-30.md`
- `docs/reference/promptAuthoringPolicy_2026-03-30.md`

먼저 확인할 파일:
- `src-tauri/src/commands/conversation_memory.rs`
- `src-tauri/src/commands/send_common.rs`
- 관련 schema/model 파일
- compressed memory visibility와 연결되는 frontend/runtime 파일

작업 시작 전 짧게 의견을 말하라:
- 왜 지금 compressed memory는 구현됐지만 아직 운영 가능한 상태는 아닌지
- 이번 단계에서 무엇을 하지 말아야 하는지

이번 작업 목표:
- compressed memory의 생성/상태/provenance/재압축 정책을 더 명확히 보여주고, 최소 운영 품질을 보강하라

구현 범위:
1. compressed memory 상태를 최소한 아래 수준으로 구분하라:
   - not_generated
   - fresh
   - stale
   - failed
2. created_at / updated_at / source_count 등 provenance 메타를 사용자가 확인 가능하게 하라
3. stale 판정과 재압축 규칙이 현재 기준에서 더 명확히 드러나게 하라
4. trace/runtime 또는 유사한 진단 표면에서 compressed memory 포함 여부와 상태를 볼 수 있게 하라
5. 최소 검증 시나리오를 포함하라:
   - 12+ 메시지 후 생성
   - 새 메시지 추가 후 stale
   - 재압축 후 상태 회복

비목표:
- vector retrieval
- long-term memory 전체 재설계
- generic memory editor
- 대형 신규 탭 추가
- global memory graph

구현 원칙:
- recent window를 대체하지 말라
- artifact/plan과 경쟁시키지 말라
- hidden background 기능이 아니라 운영 가능한 진단 가능한 기능으로 올리는 데 집중하라

검증:
- `cargo check`
- `tsc --noEmit`
- 관련 테스트가 있으면 실행
- compressed memory 상태 변화가 설명 가능하게 보이는지 확인

출력 형식:
### A. Opinion
### B. Decision
### C. Files Changed
### D. State Model
### E. Verification
### F. Residual Gaps
