# tunaFlow rawq + code-review-graph 병행 적용 계획

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-26 19:43 KST
- **상태: 보류** — rawq 필수 sidecar 전환(`rawqRequiredSidecarPlan.md`) 완료 후 재검토.
  이번 단계에서 code-review-graph는 통합 대상이 아니다.

## 목적

`tunaFlow`는 이미 `rawq`를 통해 코드 검색 레이어를 갖고 있다. 여기에 `D:\privateProject\_research\_util\code-review-graph`를 함께 붙이면, 검색과 구조 분석을 분리한 더 강한 코드 컨텍스트 계층을 만들 수 있다.

이 문서는 두 도구를 함께 쓸 때의 역할 분담, 도입 원칙, 단계별 적용 순서를 정리한다.

## 전제

### rawq

- 자연어/키워드 기반 코드 검색
- 관련 파일, 스니펫, 개념 위치 탐색
- semantic search 성격이 강함

### code-review-graph

확인 근거:

- `D:\privateProject\_research\_util\code-review-graph\README.md`
- `D:\privateProject\_research\_util\code-review-graph\docs\architecture.md`
- `D:\privateProject\_research\_util\code-review-graph\docs\schema.md`

확인된 핵심:

- Tree-sitter 기반 구조 그래프를 `.code-review-graph/graph.db` SQLite에 저장
- 함수, 클래스, 타입, import, call, inheritance, test coverage 관계를 추적
- `impact radius`, `review context`, `semantic_search_nodes`, `query_graph` 같은 구조 질의에 강함
- full build 후에는 incremental update/watch가 가능함

결론:

- `rawq`는 "관련 코드를 빨리 찾는 도구"
- `code-review-graph`는 "구조와 영향도를 파악하는 도구"

즉 둘은 대체 관계가 아니라 보완 관계다.

## 왜 함께 쓸 가치가 있는가

현재 `tunaFlow`의 컨텍스트 병목은 "무조건 많은 걸 넣는 것"이지 "도구가 부족한 것"만은 아니다.

따라서 방향은:

1. `rawq`로 관련 코드 후보를 찾고
2. 필요할 때만 `code-review-graph`로 구조/영향도를 좁혀서 보고
3. 둘 다 항상 넣지 않고 질문 유형에 따라 다르게 쓰는 것

이 방식이 가장 효율적이다.

## 역할 분리 원칙

### rawq가 맡을 것

- "어디에 있지?" 계열 탐색
- 기능/개념/키워드 기반 관련 파일 탐색
- 빠른 코드 snippet 수집
- 일반 채팅에서의 가벼운 코드 검색

예:

- "인증 로직 어디 있어?"
- "rawq 상태 UI 관련 코드 보여줘"
- "branch adopt summary 구현 위치가 어디지?"

### code-review-graph가 맡을 것

- "무엇이 연결되어 있지?" 계열 구조 분석
- 영향 범위(blast radius)
- caller/callee/import/test 관계 추적
- 리뷰/리팩토링/변경 영향도 질문

예:

- "이 함수 바꾸면 어디까지 영향 가?"
- "이 파일 변경의 테스트 영향 범위는?"
- "이 branch 변경을 리뷰할 때 꼭 봐야 할 호출 체인은?"

## 핵심 원칙

### 1. 둘 다 항상 자동 주입하지 않는다

가장 중요하다.

`rawq`와 `code-review-graph`를 매 요청마다 같이 돌리면 다시 Claude full context 병목이 생긴다.

따라서:

- 기본 일반 대화: `rawq`도 조건부
- 구조/리뷰 질문: `code-review-graph` 조건부
- 둘 다 always-on 금지

### 2. 검색 레이어와 구조 레이어를 분리한다

컨텍스트를 한 덩어리로 합치지 말고 개념적으로 분리한다.

- Search Layer: rawq
- Structure Layer: code-review-graph

나중에 prompt를 구성할 때도 별도 섹션으로 넣는 것이 좋다.

### 3. code-review-graph는 review/refactor/impact 질문부터 시작한다

처음부터 일반 채팅 전반에 붙이지 말고, 아래 상황에만 제한 적용하는 것이 맞다.

- 코드 리뷰
- 리팩토링 검토
- 변경 영향도 확인
- branch adopt 전 검토
- RT에서 구조 비교 토론

### 4. 설치/빌드 상태는 프로젝트 단위로 관리한다

`code-review-graph`는 repo root 아래 `.code-review-graph/graph.db`를 만든다.

즉 `tunaFlow`에서는 프로젝트별로 아래를 관리해야 한다.

- 설치 가능 여부
- graph DB 존재 여부
- 마지막 업데이트 시각
- update/watch 상태

이것도 `rawq`처럼 프로젝트 컨텍스트의 일부로 봐야 한다.

## tunaFlow에 적용할 때의 권장 구조

### 프로젝트 도구 상태

프로젝트마다 아래 두 도구 상태를 따로 가진다.

- rawq
  - available
  - indexed
  - indexing
- code-review-graph
  - available
  - built
  - updating

### 컨텍스트 모드 예시

- `lite`
  - graph 사용 안 함
  - rawq도 대부분 생략
- `standard`
  - 필요 시 rawq
  - graph 없음
- `review`
  - rawq 선택적
  - code-review-graph 사용
- `full`
  - 정말 필요한 경우에만 두 레이어 모두 허용

즉 `code-review-graph`는 `Claude full context`의 기본 요소가 아니라, 특정 모드의 선택적 도구여야 한다.

## 단계별 도입 계획

### Phase 1. 설치/상태/빌드 레이어

목표:

- 프로젝트에서 `code-review-graph` 사용 가능 여부 확인
- graph DB 존재 여부 확인
- build/update/status wrapper 추가

권장 범위:

- `code-review-graph status`
- `code-review-graph build`
- `code-review-graph update`

완료 기준:

- 프로젝트 단위로 graph 상태를 확인할 수 있다
- 첫 build를 실행할 수 있다

### Phase 2. 최소 구조 질의 레이어

목표:

- review/impact 질문에 필요한 최소 질의만 붙인다

권장 질의:

- impact radius
- review context
- caller/callee/import/test 관련 기본 query

완료 기준:

- 특정 질문 유형에서 rawq 대신 또는 rawq 보조로 graph 결과를 가져올 수 있다

### Phase 3. rawq + graph 조합 규칙

목표:

- 두 도구를 언제 같이 쓸지 규칙화

권장 규칙:

- 검색 질문: rawq 우선
- 구조 질문: graph 우선
- 코드 리뷰: graph 우선 + 필요 시 rawq snippet 보강
- adopt/review 전: graph impact 확인

완료 기준:

- 질문 유형 또는 context mode에 따라 어느 레이어를 쓸지 분기된다

### Phase 4. UI/가시성

목표:

- 프로젝트 UI에서 graph 상태 노출
- review 관련 작은 진입점 제공

예:

- Sidebar 프로젝트 상태에 graph badge
- ContextPanel에 graph status/build/update
- 나중에 blast radius preview

이번 단계는 후순위다.

## tunaFlow에 붙일 때 주의할 점

### 1. rawq와 graph를 같은 문제에 동시에 남용하지 말 것

둘 다 정보를 많이 줄 수 있지만, 목적이 다르다.

`rawq 결과 + graph 결과 + findings + artifacts + plan`을 한 번에 넣으면 다시 prompt가 무거워진다.

### 2. graph는 review 도구이지 기본 채팅 도구가 아니다

`code-review-graph` README도 코드 리뷰와 영향 분석에 초점이 맞춰져 있다.

따라서 기본 chat send 경로에 항상 붙이는 것은 비추천이다.

### 3. watch/update는 나중에

처음부터 watch까지 자동화하지 말고:

1. status
2. build
3. on-demand update

순으로 가는 것이 안전하다.

### 4. 프로젝트 루트와 git repo 루트 관계를 고려해야 한다

`code-review-graph`는 사실상 repo root 기준 도구다.

따라서 프로젝트 path가 git repo root와 다를 수 있으면:

- repo root 탐지
- graph build 대상 경로 정규화

를 나중에 고려해야 한다.

## 추천 적용 순서

1. 설계 문서 유지
2. Phase 1: 상태/빌드 wrapper만 추가
3. Phase 2: review/impact 질문용 최소 질의 연결
4. Phase 3: rawq와 graph의 사용 분기 규칙 추가
5. 마지막에만 UI 확장

## 현재 판정

`code-review-graph`는 `rawq`와 함께 붙일 가치가 충분하다.

다만 올바른 방향은:

- `rawq`를 대체하는 것이 아니라
- `구조 분석 레이어`로 추가하고
- 기본 채팅에 항상 주입하지 말고
- review/refactor/impact 중심으로 제한 적용하는 것

즉 지금은 도입을 검토할 만한 시점이지만, 첫 구현은 작게 시작하는 것이 맞다.
