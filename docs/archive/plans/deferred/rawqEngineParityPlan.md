# tunaFlow rawq Context 4-Engine Parity 계획

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-29
- 상태: Phase 1 완료 (build_normalized_prompt 통합)

## 현재 차이

rawq 자체는 프로젝트 공통 기능이지만,
실제 prompt injection은 full context 경로에 묶여 있어 Claude 편향이 있다.

즉 rawq index와 search는 공통이어도,
검색 결과를 어떤 엔진이 받는지는 다르다.

## 목표

rawq 기반 코드 컨텍스트는 4개 엔진 모두 동등하게 주입되어야 한다.

## 원칙

1. rawq 검색 실행 기준은 provider와 무관해야 한다
2. 검색 결과 section 포맷은 공통이어야 한다
3. provider별로 rawq를 끄는 예외를 두지 않는다

## 단계

### Phase 1. rawq section을 공통 ContextPack으로 이동

- `build_rawq_section()` 사용 위치를 Claude 편향 경로에서 분리
- 모든 send/start 경로에서 같은 판단 기준 사용

### Phase 2. diagnostics parity

- rawq not found / no index / no results / timed out 상태가 4개 엔진 모두 동일하게 보이도록 정리

### Phase 3. UI 설명 정리

- rawq는 특정 엔진 강화 기능이 아니라 프로젝트 공통 컨텍스트라고 명시

## 검증

1. 같은 프로젝트/같은 프롬프트에서 rawq section inclusion 여부가 4개 엔진 동일
2. rawq failure state가 엔진별로 다르게 숨겨지지 않음
3. trace meta에 rawq section 유무가 기록됨

