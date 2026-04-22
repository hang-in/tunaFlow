# tunaFlow 대화 벡터 검색 도입 검토 메모

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-27 21:10 KST

## 목적

현재 `tunaFlow`는:

- 최근 메시지 window
- parent conversation recent turns
- cross-session rows
- plans / findings / artifacts
- rawq code context

를 조합해 ContextPack을 만든다.

이 문서는 여기에 **대화 의미 기반 검색(vector retrieval)** 을 추가할 경우의 구조를
미리 검토해 두는 메모다.

중요:

- 이번 문서는 **도입 계획 참고용**
- 아직 구현 대상으로 확정된 것은 아님
- 현재 제품 우선순위보다 앞서지는 않음

## 현재 구조 요약

### DB

핵심 스키마:

- `projects`
- `conversations`
- `messages`
- `branches`
- `memos`
- `artifacts`
- `trace_log`
- `messages_fts` (FTS5 스키마만 존재)

참고:

- `messages_fts`는 아직 스키마만 있고 트리거/실사용은 없다.

### ContextPack

현재 ContextPack은 주로 아래에 의존한다.

- 최근 N개 메시지
- branch parent recent turns
- thread anchor message
- plans
- artifacts
- findings
- rawq
- cross-session context

즉 현재는 **최근성(recentness) + 명시적 구조물** 중심이고,
의미 기반 회수는 없다.

## 문제 정의

최근성 기반만 쓰면:

1. 예전에 중요한 결정을 한 대화가 최근 window 밖으로 밀리기 쉽다
2. 긴 conversation에서 같은 주제가 반복돼도 자동 회수가 어렵다
3. branch / follow-up / RT에서 “예전에 비슷한 얘기를 했던 부분”을 다시 찾으려면 수동 탐색이 필요하다

rawq는 코드 탐색에는 좋지만,
**대화 내용 자체의 의미 검색** 은 하지 못한다.

## 벡터 검색이 맡을 역할

`tunaFlow`에 벡터 검색이 들어간다면 역할은 명확해야 한다.

- 코드 검색 대체 아님
- recent messages 대체 아님
- **대화 기억 보조층**

즉 위치는:

- rawq = 코드 기억
- vector retrieval = 대화 기억

## 권장 데이터 구조

메시지 단건 임베딩보다 **turn/chunk 단위 임베딩**이 적합하다.

권장 테이블:

### `conversation_chunks`

- `id`
- `project_key`
- `conversation_id`
- `kind`
  - `turn`
  - `branch_anchor`
  - `roundtable_brief`
  - `artifact_summary`
- `root_message_id`
- `text`
- `created_at`
- `updated_at`

### `conversation_chunk_vectors`

- `chunk_id`
- `embedding`

또는 `sqlite-vec` 사용 시 vec virtual table로 분리.

## 왜 chunk가 맞는가

`messages` 그대로 임베딩하면:

- user와 assistant가 분리돼
- 검색 결과가 반쪽짜리로 나올 수 있다.

반면 chunk로 묶으면:

- 질문 + 응답 한 쌍
- branch anchor + 직후 응답
- RT brief 요약

처럼 바로 ContextPack에 넣을 수 있는 단위가 된다.

## ContextPack 통합 위치

벡터 검색은 기존 recent window를 대체하면 안 된다.

권장 위치:

- `## Relevant prior conversation`

삽입 순서:

1. thread anchor / parent turns
2. current recent context
3. **vector-retrieved prior chunks**
4. plan / findings / artifacts / rawq

권장 규칙:

- 현재 prompt를 쿼리로 사용
- 현재 프로젝트 범위만 검색
- top 3~5 chunk
- 현재 recent messages와 중복되는 chunk는 제외

즉 제품 원칙상 **항상 현재 프로젝트 범위 검색** 이어야 한다.

## sentence-transformers + sqlite-vec 검토

### 장점

- 의미 기반 회수가 쉬움
- 긴 대화에서도 중요한 예전 turn 회수 가능
- follow-up / branch / RT 품질 향상 가능성

### 단점

- 인덱싱 파이프라인이 새로 필요
- 메시지 삭제/branch/adopt/RT brief 생성과 정합성 관리 필요
- 임베딩 모델 배포 문제
- ContextPack 노이즈가 늘어날 위험

## Python 사이드카 필요 여부

### 1. Python sidecar 사용

`sentence-transformers/all-MiniLM-L6-v2`를 그대로 쓰려면
Python이 가장 현실적이다.

장점:

- 구현 난이도 낮음
- 모델 생태계가 익숙함
- 추후 rerank 실험도 쉬움

단점:

- sidecar/worker 추가
- 현재 direct-call 구조와 거리가 생김
- 운영 복잡도 증가

### 2. Rust 내부 처리

Python 없이 Rust/ONNX로 임베딩을 계산하는 방법도 있다.

장점:

- Tauri backend 안에서 끝남
- 프로세스 추가 없음

단점:

- 구현 난이도 상승
- MiniLM-L6-v2를 그대로 쓰는 것보다 도입 부담이 큼

### 3. sqlite-vec 자체

`sqlite-vec`를 쓰려면:

- SQLite extension 로딩/링킹
- 현재 `rusqlite` bundled 환경과의 정합성

을 따로 검토해야 한다.

즉 임베딩 계산 문제와 별개의 이슈가 하나 더 생긴다.

## 현실적인 도입 순서

현재 기준으로는 아래 순서가 가장 안전하다.

### Phase 0 — 조사 메모

이 문서 단계.

### Phase 1 — 구조 실험

- `conversation_chunks` 설계
- project-scoped retrieval만 허용
- 벡터 저장은 일반 SQLite 테이블
- 검색은 Rust 쪽 brute-force cosine 또는 단순 비교

목적:

- retrieval 품질과 ContextPack noise를 먼저 확인

### Phase 2 — 임베딩 경로 확정

둘 중 하나 선택:

- Python worker + sentence-transformers
- Rust/ONNX 기반 임베딩

### Phase 3 — sqlite-vec 검토

Phase 1/2 결과가 좋을 때만 검토.

즉 `sqlite-vec`는 바로 1순위가 아니라,
**일반 구조가 맞는지 검증한 뒤 최적화 단계**로 보는 게 낫다.

## 현재 판단

### 도입 가치

높음.

특히:

- 긴 대화
- plan follow-up
- branch / RT 재진입
- 예전 합의/결정 회수

에 이점이 있다.

### 현재 우선순위

즉시 구현 우선순위는 아님.

이유:

- 인덱싱/삭제/정합성 관리 비용이 큼
- 아직 최근성 + artifacts + rawq 조합만으로도 해결 가능한 영역이 많음
- 먼저 실제 pain point가 충분히 반복되는지 확인할 가치가 있음

## 결론

`sentence-transformers + sqlite-vec`는 흥미롭고 도입 가치가 있지만,
현재 `tunaFlow`에는 바로 넣기보다:

1. chunk 단위 대화 기억 구조를 먼저 설계하고
2. project-scoped retrieval만 실험하고
3. 임베딩 경로(Python vs Rust)를 확정한 뒤
4. 마지막에 `sqlite-vec`를 검토

하는 순서가 적합하다.

