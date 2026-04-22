# context-hub Minimal Integration Plan

상태: 제안
작성: 2026-03-30

## 목표

`context-hub`를 placeholder UI가 아니라 실제로 동작하는 최소 공급층으로 붙인다.

## 현재 전제

- `ContextPack` visibility / compression / budget control까지 완료
- `context-hub`는 tunaFlow 내부 재구현 대상이 아니라 sidecar/CLI/MCP 성격의 외부 런타임으로 다룬다
- source 정책은 `bundled/local/private only`가 기본이다

## 1차 범위

### Phase 1A: Health

- `context-hub` 존재 여부 확인
- 버전 또는 사용 가능 상태 확인
- Runtime 또는 진단 surface에서 최소 상태 확인 가능

### Phase 1B: Search

- 제한된 source 범위에서 `search`
- query → result list 반환
- 아직 자동 ContextPack 주입은 하지 않음

### Phase 1C: Get

- 검색 결과 선택 후 `get`
- 문서/지식 본문 가져오기
- 결과를 사용자가 확인 가능

## 비목표

- Knowledge Sources shell 재도입
- public source 자동 조회
- automatic fetch
- ContextPack 자동 삽입
- flow agent orchestration

## 권장 UX

- 1차는 Settings보다 Runtime/diagnostics 또는 개발용 surface에서 상태 확인
- 본격적인 제품 UI보다 “실제 연결 경로 존재”를 먼저 증명

## 권장 구현 방식

- CLI/sidecar 호출 우선
- search/get command를 명시적으로 노출
- source 정책 위반 시 호출 차단

## 성공 기준

- `context-hub` health check 가능
- 허용된 source에서 search 가능
- search result 하나를 get으로 가져올 수 있음
- public auto-fetch는 여전히 금지됨

## 메모

이 단계는 knowledge UI를 여는 것이 아니라, `context-hub`를 실제 공급층으로 연결할 최소 경로를 여는 것이다.
