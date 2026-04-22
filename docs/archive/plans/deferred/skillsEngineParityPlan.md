# tunaFlow Skills 4-Engine Parity 계획

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-29
- 상태: Phase 1 완료 (build_normalized_prompt 통합)

## 현재 차이

현재 active skills는 주로 Claude 경로에서만 강하게 의미가 있다.

- `activeSkills`는 store에 존재
- `build_skills_section()`은 존재
- 하지만 non-Claude 경로는 lite enriched prompt를 사용해 skills section이 동일하게 주입되지 않는다

즉 UI에서 skill을 켜도 4개 엔진에 동등 적용된다고 보기 어렵다.

## 목표

모든 엔진에서 skill selection이 같은 개념으로 동작해야 한다.

1. skill 선택 UI는 공통
2. skill content 주입은 4개 엔진 모두 동일
3. 실제 어떤 skill이 주입됐는지 trace/debug 가능

## 구현 원칙

1. 엔진별 별도 skill 해석 로직을 만들지 않는다
2. backend에서 공통 skill section을 조립한다
3. provider adapter는 동일한 normalized prompt payload를 받는다

## 단계

### Phase 1. 공통 skill injection 경로 통합

- Claude 외 경로에도 skills section 포함
- `send_common.rs`와 각 provider command 경로를 점검
- "lite path라서 skills 제외" 전제를 제거

### Phase 2. trace/debug 가시화

- trace log 또는 context meta에 applied skills 기록
- "선택됨"과 "실제 주입됨"을 구분해 표시

### Phase 3. 상태 문서/UI 정리

- 엔진별 차등 설명 제거
- applied skill visibility를 문서와 UI에 반영

## 검증

1. 같은 대화, 같은 active skills에서 4개 엔진 모두 동일 skill section 포함
2. trace 또는 debug 출력에서 applied skills 확인 가능
3. UI 설명이 실제 구현과 일치

