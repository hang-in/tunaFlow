# tunaFlow Collaboration Context 4-Engine Parity 계획

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-29
- 상태: Phase 1 완료 (build_normalized_prompt 통합)

## 범위

이 문서는 아래 협업 컨텍스트 섹션을 다룬다.

1. plan summary
2. findings summary
3. artifact handoff
4. thread inheritance
5. cross-session summary

## 현재 차이

이 섹션들은 Claude 경로에서 더 풍부하게 조립된다.
non-Claude 경로는 lite recent context 중심이라 협업 기능의 효과가 약하다.

## 목표

agent collaboration 기능은 엔진을 바꿔도 같은 수준으로 유지되어야 한다.

즉:

- 같은 plan owner 정보
- 같은 findings
- 같은 artifact handoff
- 같은 parent thread inheritance

가 4개 엔진에 공통 적용되어야 한다.

## 단계

### Phase 1. section source 정규화

- 각 collaboration section의 source query를 공통 함수로 정리
- provider command에서 중복 조립을 줄임

### Phase 2. 4-engine injection 통합

- non-Claude 경로에도 동일 section 포함
- branch / roundtable / follow-up 경로 모두 점검

### Phase 3. UX/trace alignment

- 사용자가 "현재 어떤 협업 컨텍스트가 붙었는지" 확인 가능하게 함
- trace meta나 debug inspector에 section 목록 표기

## 검증

1. plan-followup 대화에서 4개 엔진 모두 같은 plan/findings/artifact 데이터를 받음
2. branch inheritance가 엔진 교체와 무관하게 유지
3. roundtable/worker follow-up 결과 품질 차이가 줄어듦

