# tunaFlow Workspace Panel 재설계 계획

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-26 20:47 KST

## 목적

현재 `tunaFlow`의 우측 `ContextPanel`은 기능이 늘수록 정보가 산만해지고,
사용자가 "지금 무엇을 해야 하는지"보다 "어떤 종류의 데이터가 있나"를 먼저 보게 된다.

이 문서는 우측 패널을 단순 context browser가 아니라,
**현재 작업 단계 중심의 workspace panel**로 재설계하는 방향을 정리한다.

## 현재 구조

실제 코드 기준:

- `src/components/tunaflow/ContextPanel.tsx`
  - 1차 탭
    - `Branches`
    - `Assets`
  - 2차 세그먼트
    - `Artifacts`
    - `Memos`
    - `Skills`
    - `Plans`

하위 패널:

- `BranchesPanel`
- `ArtifactsPanel`
- `MemosPanel`
- `SkillsPanel`
- `PlansPanel`

## 현재 구조의 한계

### 1. 데이터 분류형 구조다

지금 패널은
- branches
- artifacts
- memos
- skills
- plans

처럼 "데이터 종류"를 기준으로 나뉜다.

하지만 harness/workflow 관점에서 사용자가 필요한 것은:

- 지금 plan을 승인해야 하는지
- 리뷰 결과를 봐야 하는지
- 테스트가 실패했는지
- 어떤 artifact가 현재 task의 진실원인지

같은 **작업 단계 정보**다.

### 2. Branch가 우측 패널 루트에 있는 것이 어색하다

branch는 점점 "대화 객체" 성격이 강해지고 있다.

- 일반 branch
- RT branch
- 향후 reviewer thread

이 모두 대화/실행 단위에 가까우므로,
우측 패널 루트보다 중앙 상단 탭이나 drawer 쪽이 더 자연스럽다.

### 3. 기능이 늘수록 과적재된다

앞으로 우측 패널에 들어와야 할 후보:

- Plan
- Review findings
- Tests
- Task brief
- Architect decision
- Artifacts
- Trace
- rawq / graph 상태 일부

이걸 현재 구조 위에 계속 얹으면 산만해질 가능성이 높다.

## 재설계 목표

우측 패널을:

- "데이터 분류 브라우저"

가 아니라

- "현재 작업 단계 중심 workspace panel"

로 바꾼다.

## 기본 원칙

### 1. 우측 패널은 mode 전환형이어야 한다

한 번에 하나의 주 모드만 크게 보여주는 방식이 맞다.

최종 권장 모드:

- `Plan`
- `Review`
- `Test`
- `Artifacts`
- `Trace`

즉 내부적으로는 작은 작업용 탭 구조가 된다.

다만 1차 MVP에서 이 5개를 모두 독립 모드로 여는 것은 비추천이다.
현재 데이터 소스가 실제로 안정적인 모드부터 시작해야 한다.

### 2. branch/thread는 우측 패널에서 빼야 한다

branch/thread는:

- 중앙 상단 탭
- thread/RT drawer

로 가는 것이 맞다.

즉 현재 `BranchesPanel`은 우측 패널의 루트 역할에서 물러나야 한다.

### 3. 우측 패널은 "현재 해야 할 일"을 강조해야 한다

예:

- plan 승인 전이면 `Plan` 모드가 중심
- 리뷰 결과가 오면 `Review` 모드가 중심
- 테스트 실패 시 `Test` 모드가 중심

즉 정적인 정보창이 아니라
현재 workflow 단계에 따라 포커스를 바꾸는 작업 패널이어야 한다.

## 권장 모드 구성

### `Plan`

포함 항목:

- 현재 활성 plan
- subtasks
- owner agent
- approval 상태
- task 시작 액션
- task brief 연결 상태

이 모드는 architect가 plan을 정리하고 developer lane을 여는 중심 모드다.

### `Review`

포함 항목:

- review findings
- reviewer별 결과 요약
- RT review / branch review 결과
- architect decision
- retry / accept / reject 액션

즉 판정과 수습 단계 중심이다.

주의:

현재는 `review-findings` artifact는 있어도
"reviewer별 결과 요약", "retry/accept/reject 액션"이 아직 약할 수 있다.

따라서 1차 MVP에서는 독립 모드로 바로 승격하지 않고,
후속 단계에서 실제 데이터가 모이면 별도 모드로 올리는 것이 안전하다.

### `Test`

포함 항목:

- 마지막 test report
- lint / typecheck / build 상태
- 실패 요약
- 마지막 검증 시각

즉 developer 검증 결과를 보는 곳이다.

주의:

현재 `test-report` artifact는 가능하지만,
lint / typecheck / build 집계가 아직 완전한 패널 데이터 소스로 굳어지지 않았을 수 있다.

따라서 1차 MVP에서는 독립 모드보다는 후속 승격이 더 현실적이다.

### `Artifacts`

포함 항목:

- task brief
- diff summary
- architect decision
- adopted summary
- 일반 artifact
- 관련 memo 일부

즉 구조화된 산출물 브라우저 역할이다.

권장 1차:

- `ArtifactsPanel` 본체
- `Memos`는 접이식 섹션으로 포함
- memo scope는 conversation / project 구분이 필요함

### `Trace`

포함 항목:

- run log
- tool calls
- queue / running 상태
- 비용/토큰
- rawq/graph/build 상태 로그

즉 디버깅 및 운영용 모드다.

1차에는 trace_log, running/queue 상태, 간단한 실행 로그 요약부터 시작해도 충분하다.

## 현재 패널과의 매핑

### `PlansPanel`

- 대부분 `Plan` 모드로 이동

### `ArtifactsPanel`

- `Artifacts` 모드의 핵심 콘텐츠로 이동

### `MemosPanel`

두 가지 선택지가 있다.

1. `Artifacts`의 하위 콘텐츠로 흡수
2. `Trace` 또는 `Review` 일부로 분배

하지만 단순 흡수는 주의가 필요하다.

이유:

- Memo는 `project_key` 스코프 성격이 강함
- Artifact는 `conversation_id` / `branch_id` 스코프 성격이 강함

따라서 1차에선:

- `Artifacts` 모드 안에 `Memos` 접이식 섹션
- 필요 시 `conversation / project` scope 전환

형태가 가장 현실적이다.

### `SkillsPanel`

`workspace panel`의 핵심 모드로 보긴 어렵다.

권장:

- 입력 영역 근처의 도구/설정 UI로 이동
- 또는 별도 작은 settings popover로 분리

즉 메인 mode로 승격하지 않는 것이 좋다.

### `BranchesPanel`

권장:

- 우측 패널 루트에서 제거
- 중앙 conversation 헤더 아래 `branch bar`로 재배치

중요:

branch는 여러 개가 될 수 있으므로,
중앙 상단에 탭으로 전부 올리는 것보다
"현재 활성 branch + 펼침 목록" 형태의 branch bar가 더 현실적이다.

## 1차 MVP 구조

### 상단 mode bar

- `Plan`
- `Artifacts`
- `Trace`

후속 승격 대상:

- `Review`
- `Test`

### 패널 본문

현재 활성 mode 하나만 크게 보여준다.

### 보조 표시

- 리뷰 존재 badge
- 테스트 존재 badge
- artifact 개수 badge
- trace 존재 표시

즉 `Review`, `Test`는 1차에선 독립 모드가 아니라
"데이터가 있다"는 신호만 주고,
실제 접근은 `Artifacts` 또는 후속 연결로 처리할 수 있다.

## 상태 전이 규칙

### 수동 전환

사용자가 mode를 직접 바꾼다.

### 추천 전환

향후 도입 가능:

- plan 승인 필요 → `Plan` 추천
- review findings 도착 → `Review` 추천
- test 실패 → `Test` 추천

중요:

자동 강제 전환보다는
"추천 모드 표시"가 더 안전하다.

## 구현 전략

### Phase A

- `ContextPanel`에 mode state 추가
- `Plan / Artifacts / Trace` 3모드만 실제 적용
- `Artifacts` 안에 `Memos` 접이식 섹션 추가
- `Branches` 루트 탭 제거 또는 축소
- `BranchesPanel`은 conversation 헤더 아래 `branch bar`로 이동 준비

### Phase B

- `Review` 모드 추가
- `Test` 모드 추가
- artifact/test/review 집계 연결
- badge / count 표시 강화

### Phase C

- 상황 기반 추천 모드
- badge / count / urgency 표시
- `Skills`를 입력 영역 또는 settings popover로 이동

## 주의사항

### 1. 모든 정보를 한 화면에 다 보여주려 하지 말 것

우측 패널은 제한된 공간이다.
핵심 모드 하나만 집중적으로 보여줘야 한다.

### 2. branch/thread와 혼합하지 말 것

대화 객체는 중앙과 drawer에서 다뤄야 한다.

### 3. 기존 기능 접근성을 해치지 말 것

현재 `PlansPanel`, `ArtifactsPanel`의 핵심 액션은 그대로 살아 있어야 한다.

## 테스트 포인트

- mode 전환이 자연스러운지
- plan 중심 작업 시 `Plan` 모드가 충분한지
- `Artifacts + Memos` 조합이 과도하게 복잡하지 않은지
- `Trace`가 실제 운영 정보로 쓸 만한지
- 기존 패널 기능이 사라지지 않았는지
- 우측 패널이 예전보다 덜 산만한지

## 현재 판정

`ContextPanel` 재설계의 핵심은 패널을 더 늘리는 것이 아니라,
현재 분류형 구조를 workflow형 workspace panel로 바꾸는 것이다.

다만 1차 MVP는 욕심내지 말고:

- `Plan`
- `Artifacts`
- `Trace`

3모드만 실제 모드로 올리고,
`Review`와 `Test`는 데이터 소스가 충분히 연결된 뒤 승격하는 것이 맞다.
