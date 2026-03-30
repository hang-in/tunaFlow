# Long-Term Memory Phase 1 Compression Plan

상태: 중요 / 제안
작성: 2026-03-30

## 목표

Long-term memory 로드맵의 첫 구현으로, recent window를 단순히 늘리는 대신 **오래된 대화를 구조화 요약 memory로 압축**하는 경로를 만든다.

이번 단계의 핵심은:
- recent N 확장이 아니라
- `compressed conversation memory`를 ContextPack 안의 정식 source로 올리는 것이다.

## 왜 지금 필요한가

- 현재 recent messages는 최대 6개 수준이라 멀티에이전트 대화에서 continuity가 빨리 깨진다
- recent window를 10~12개로 늘리는 것은 단기기억 확장일 뿐, 장기기억 해결이 아니다
- 이미 typed compression, ContextPack visibility, author attribution까지 올라왔으므로 이제 오래된 대화를 요약 memory로 승격할 기반이 생겼다

## 이번 단계에서 할 것

### 1. Compression memory 개념 도입

- 최근 메시지 바깥으로 밀린 오래된 대화를
  - 그대로 버리거나 recent N만 늘리지 말고
  - 구조화 요약 memory로 유지한다

권장 형식:
- `Task Overview`
- `Current State`
- `Important Discoveries`
- `Decisions`
- `Open Questions`
- `Context to Preserve`

### 2. Summary cache 또는 동등 구조 추가

- conversation 또는 branch 단위로 compressed memory를 저장하거나 재생성할 수 있는 경로를 둔다
- 원본 메시지는 계속 유지한다
- compressed memory는 working memory 바깥의 보조 source로 다룬다

### 3. ContextPack 통합

- `recent messages`
- `thread inheritance`
- `cross-session`
와 별도로
- `compressed conversation memory`
섹션을 ContextPack 후보로 추가한다

### 4. Marked memory 최소 규칙

- 어떤 대화 구간이 compression 대상으로 승격되었는지 구분할 최소 메타가 필요하다
- 전체 DB 대수술은 하지 않더라도, 적어도:
  - 생성 시점
  - 대상 conversation/branch
  - source range 또는 source count
정도는 남긴다

## 구현 범위

1. compression memory 데이터 구조 초안
2. 오래된 recent context를 요약 memory로 만드는 최소 로직
3. ContextPack assembly에서 이 memory를 포함할 수 있는 경로
4. trace/runtime에서 compressed memory 포함 여부를 확인할 수 있는 최소 visibility

## 비목표

- vector retrieval
- mem0/ReMe 같은 외부 long-term stack 도입
- 완전한 generic memory OS 구축
- project 전역 knowledge graph
- 자동 importance scoring 고도화

## 설계 원칙

### 1. raw recent messages를 대체하지 않는다

- recent window는 계속 working memory로 유지
- compressed memory는 그 바깥의 continuity 보조층

### 2. 원본 메시지는 삭제하지 않는다

- compression은 저장/요약/참조 방식 변화이지
- transcript 자체를 대체하지 않는다

### 3. artifact/plan과 경쟁하지 않는다

- artifact/plan은 structured memory source
- compressed conversation memory는 대화 continuity source
- 둘은 역할이 다르다

### 4. AgentScope 패턴은 차용만 한다

- 전체 framework 도입이 아니라
- `SummarySchema`, `compressed mark`, `working memory 압축` 아이디어만 차용한다

## 먼저 확인할 곳

- `src-tauri/src/commands/agents_helpers/context_pack.rs`
- `src-tauri/src/commands/agents_helpers/compression.rs`
- `src-tauri/src/commands/agents_helpers/context_queries.rs`
- `src-tauri/src/commands/send_common.rs`
- 관련 DB schema/model 파일

## 성공 기준

- 오래된 대화가 recent window 밖으로 밀려도 핵심 continuity가 요약 memory로 유지된다
- recent messages를 무작정 늘리지 않고도 “이전 맥락을 알고 있다”는 응답 품질이 개선된다
- compressed memory가 artifacts/plan/rawq와 다른 memory source로 구분된다

## 후속

이 단계가 끝나면 다음은:

1. structured memory 강화
2. conversation retrieval

순으로 이어진다.
