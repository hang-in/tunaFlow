# tunaFlow 문서 메타 도입 계획

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-30
- 상태: 초안

## 목적

`docs/` 전반에 공통 메타를 붙이는 기준과 적용 순서를 정한다.

이번 문서의 목표는 모든 문서를 한 번에 고치는 것이 아니라,
어디부터 어떤 메타를 붙이면 탐색성이 가장 빨리 좋아지는지 정하는 것이다.

## 핵심 판단

메타 도입은 전면 적용보다
`핵심 문서군부터 최소 메타를 붙이는 방식`이 맞다.

이유:

1. 문서 수가 많다
2. 모든 문서에 모든 메타를 붙이면 비용이 크다
3. 효과가 큰 문서군은 이미 정해져 있다

## 우선 적용 대상

### 1순위

- `docs/reference/*.md`
- `docs/plans/index.md`
- `docs/prompts/index.md`
- `CLAUDE.md`

이유:

- 새 세션 에이전트가 가장 먼저 읽는 문서군이기 때문

### 2순위

- 현재 활성 작업의 `plans/*.md`
- 대응하는 `prompts/*.md`

### 3순위

- `how-to/*.md`
- archive 전환 대상 문서

## 1차 최소 메타

처음에는 아래만 붙인다.

- `title`
- `type`
- `status`
- `updated_at`
- `summary`
- `canonical`
- `related`

## 2차 메타

문서군이 안정되면 추가:

- `read_before`
- `paired_plan` / `paired_prompt`
- `last_verified_at`
- `superseded_by`

## 적용 규칙

### reference

- `canonical` 중요
- `ssot_level`은 2차 도입 가능

### plan

- `status`, `related`, `canonical` 우선

### prompt

- `paired_plan`, `target_agent`, `expected_output`은 2차

## 비목표

- 모든 문서 제목/파일명 변경
- 문서 전체 재작성
- archive 대규모 이동

## 완료 기준

1. 핵심 문서군이 최소 메타를 가진다
2. index가 메타를 바탕으로 읽기 순서를 안내한다
3. 에이전트가 낡은 문서와 기준 문서를 더 잘 구분한다

