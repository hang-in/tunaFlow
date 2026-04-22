# tunaFlow Settings > Runtime 우선 구현 계획

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-30
- 상태: 초안

## 목적

현재 placeholder인 `Settings > Runtime`을
실제로 사용 가능한 설정/진단 화면으로 바꾼다.

이번 단계는 `Knowledge Sources`보다 먼저,
사용자가 당장 의미 있게 확인하고 조정할 수 있는 runtime 정보부터 제품으로 올리는 데 목적이 있다.

## 핵심 판단

지금은 새로운 placeholder를 늘리는 것보다,
이미 존재하는 runtime 개념을 하나의 설정 화면으로 정리하는 것이 더 가치가 크다.

특히 아래 항목은 이미 제품 내부에 개념이나 구현이 일부 존재한다.

- rawq 상태
- model catalog
- context budget
- daemon / background runtime 관련 상태

따라서 `Runtime`은 더 이상 placeholder로 두지 말고,
1차 진단/설정 셸로 바꾸는 것이 맞다.

## 현재 상태

이미 있는 것:

- `Settings > Runtime` placeholder
- `rawqStatus` store/state
- `RuntimeStatusBar` 하단 표시
- model catalog 관련 command / store
- context budget 관련 계획 문서와 일부 guardrail 구조

아직 없는 것:

- Runtime에서 이를 한 번에 보는 UI
- rawq 상태와 진단 정보의 상세 보기
- model catalog refresh / availability 확인 진입점
- context budget 상태 설명 또는 조정 UI

## 목표

### 1. Runtime을 실제 섹션으로 만든다

최소 포함:

- rawq
- Model Catalog
- Context Budget
- Background / Daemon

### 2. 각 영역은 “지금 무엇을 볼 수 있는지”가 분명해야 한다

원칙:

- 구현된 것은 진짜 상태/행동으로 노출
- 미구현은 계획처럼 보이지 않게 제한적으로 표기

### 3. 사용자가 지금 당장 할 수 있는 행동이 있어야 한다

예:

- rawq 상태 확인
- model catalog 새로고침
- context budget 현재 정책 확인

이번 단계에서는 작은 액션이 몇 개만 있어도 충분하다.

## 권장 섹션 구성

### A. rawq

보여줄 것:

- status
- message
- indexed files/chunks
- 현재 프로젝트 기준 진단

가능한 액션:

- refresh status
- 필요 시 index build 진입점

### B. Model Catalog

보여줄 것:

- engine별 모델 로드 상태
- 마지막 refresh 상태

가능한 액션:

- refresh catalog

### C. Context Budget

보여줄 것:

- 현재 policy 설명
- context mode / truncation 개념
- 지금은 편집 가능 여부를 명확히 표시

이번 단계에서는 읽기 전용이어도 괜찮다.

### D. Background / Daemon

보여줄 것:

- background execution 구조 설명
- rawq daemon / background worker 관련 상태 또는 개념

주의:

- 실제 데이터가 부족하면 과장하지 말고 설명 + 현재 상태로 제한

## 구현 범위

- `SettingsPanel.tsx`
- 필요 시 관련 selector / runtime status helper
- frontend에서 이미 접근 가능한 상태 재사용
- 최소 command 연결

## 비목표

- context-hub 도입
- flow agent 구현
- full runtime control center
- daemon orchestration 대규모 리팩토링

## 완료 기준

1. `Settings > Runtime`이 더 이상 placeholder가 아니다
2. rawq / model catalog / context budget / daemon이 분리된 섹션으로 보인다
3. 사용자가 runtime 상태를 제품 안에서 확인할 수 있다

