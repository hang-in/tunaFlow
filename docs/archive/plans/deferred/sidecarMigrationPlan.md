# tunaFlow Sidecar 도입 마스터 계획

작성 목적:
- `tunaFlow`의 엔진 실행 경로를 Rust direct-call 중심 구조에서 `sidecar` 계층 기반 구조로 점진 이행하기 위한 전체 설계 문서다.
- `tunaChat`의 reference를 참고하되, 현재 `tunaFlow`의 `plans / branches / rawq / trace / follow-up / roundtable` 구조를 보존하면서 단계별로 옮기는 것을 목표로 한다.

## 현재 판단

현재 시점에서는 `sidecar`를 즉시 도입하지 않는다.

이유:
- 현재 가장 큰 체감 문제였던 `rawq` 지연과 스트리밍 UX는 direct-call 구조 안에서도 개선 가능했고, 이미 상당 부분 해결 중이다.
- 모델 카탈로그도 현재 CLI 제약상 완전 동적 탐색이 어렵기 때문에, sidecar를 넣는다고 즉시 본질적으로 좋아지지 않는다.
- 지금 sidecar를 넣으면 프로세스 lifecycle, RPC bridge, direct-call과의 이중 경로 관리 비용이 먼저 커질 가능성이 높다.

즉 이 문서는 "지금 바로 구현" 문서가 아니라,
**direct-call 구조로 더 이상 감당하기 어려워질 때를 대비한 설계/이행 문서**로 유지한다.

## sidecar 도입이 필요한 시점

아래 조건 중 둘 이상이 동시에 나타나면 sidecar 도입을 실제로 시작하는 것이 맞다.

1. 엔진별 실행 경로가 더 이상 direct-call로 유지하기 어려울 때
   - Claude / Codex / Gemini / OpenCode의 실행, 취소, timeout, progress 계약이 지나치게 벌어짐

2. 공통 이벤트 모델이 꼭 필요할 때
   - 모든 엔진에서 `started / action / chunk / completed / cancelled` 같은 표준 이벤트를 강하게 맞춰야 할 때

3. 모델 availability 검증과 catalog 갱신을 공통 런타임에서 관리해야 할 때
   - 단순 curated catalog를 넘어, 엔진별 실제 사용 가능 모델 검증과 캐시를 중앙화해야 할 때

4. thread-local queue와 cancel 정책을 프론트만으로 관리하기 어려워질 때
   - 같은 thread 직렬화, 다른 project 병렬화, cancel/drain 정책을 런타임 계층에서 통일해야 할 때

5. roundtable / follow-up / branch 실행 모델을 하나의 orchestration 계층으로 묶어야 할 때
   - 현재 command 단위 구현이 너무 분산되어, 실행 정책을 한곳에서 통제해야 할 때

6. direct-call 구조 때문에 디버깅 비용이 기능 추가 속도를 넘어서기 시작할 때
   - 새 기능보다 런타임 정합성 문제 해결에 드는 비용이 더 커질 때

## 지금 당장 하지 않는 이유

현재 `tunaFlow`는 아래 항목을 direct-call 구조 안에서 먼저 해결하는 편이 더 낫다.

- Claude Context 경량화
- rawq 조건부 실행
- progress-first streaming 보강
- thread-local run queue 정리
- 프로젝트 단위 경계와 branch/thread 모델 정합성 강화

즉 sidecar는 **지금의 1순위 작업이 아니라, 위 개선들로도 더 이상 정리가 안 될 때 시작하는 2차 구조 개선**이다.

## 왜 지금 sidecar가 필요한가

현재 `tunaFlow`는 아래 특성이 섞여 있다.

- 엔진별 CLI 호출이 Rust command 레이어에 직접 퍼져 있음
- 스트리밍 경험이 엔진마다 다름
- 모델 카탈로그, availability, queue, cancel 정책이 흩어져 있음
- 전역 `isRunning` 흔적 때문에 메신저형 UX가 약함
- roundtable, follow-up, branch 실행이 모두 같은 런타임 정책 위에 얹혀야 함

이 구조는 기능이 늘수록 유지비가 커진다. sidecar 계층을 도입하면 다음을 한곳으로 모을 수 있다.

- 엔진별 command/build/env
- 스트리밍 이벤트 계약
- cancel/timeout
- 모델 목록/availability
- thread-local queue와 향후 project-scoped concurrency

## 목표 원칙

1. 한 번에 전체를 설계한다.
2. 구현은 단계적으로 나눈다.
3. protocol을 먼저 고정하고, 엔진 이관은 나중에 한다.
4. 기존 UI/DB 구조는 최대한 유지한다.
5. `tunaFlow` 특화 기능은 sidecar 도입 중에도 계속 동작해야 한다.

## Non-Goals

- 한 번에 모든 direct-call 경로 제거
- 한 번에 roundtable 전체 재작성
- sidecar 도입과 동시에 thread 모델/branch 모델 전면 교체
- UI 전체 재설계

## 참고 레퍼런스

- `D:\privateProject\tunaChat\client\src\lib\tauriClient.ts`
- `D:\privateProject\tunaChat\client\src-tauri\src\lib.rs`
- `D:\privateProject\tunaChat\sidecar\__main__.py`
- `D:\privateProject\tunaChat\sidecar\router.py`

주의:
- `tunaChat`는 참고 레퍼런스일 뿐이다.
- 그대로 복사하기보다 `tunaFlow`의 현재 구조에 맞는 이식이 필요하다.

## 목표 아키텍처

### 상위 구조

- Frontend
  - 채팅 UI
  - branch / roundtable / follow-up UX
  - run state / queue state 표시
- Rust/Tauri
  - DB, rawq, project, trace, context pack, app state
  - sidecar process lifecycle 관리
  - sidecar RPC bridge
- Sidecar
  - 엔진별 CLI orchestration
  - streaming event emit
  - cancel / timeout / model catalog / availability

### 책임 분리

#### Rust에 남길 것

- conversations / branches / plans / memos / artifacts / trace / rawq / eval
- project context resolution
- ContextPack 조립
- DB persistence
- UI로 전달할 app-level state

#### sidecar로 옮길 것

- `chat`
- `cancel`
- `engine.list`
- `models.validate`
- 향후 `roundtable.run`

## 표준 RPC 제안

### 요청

- `chat`
- `cancel`
- `engine.list`
- `models.validate`
- `roundtable.run` (후속)

### 이벤트

- `started`
- `action`
- `chunk`
- `completed`
- `failed`
- `cancelled`

### 공통 필드

- `request_id`
- `thread_id`
- `project_key`
- `engine`
- `model`
- `resume_token`

## 단계별 구현 계획

### Phase 0. Protocol / Adapter 준비

목표:
- sidecar protocol과 Rust bridge를 먼저 만든다.
- 아직 direct-call 경로는 유지한다.

포함:
- sidecar process spawn
- stdio JSONL 요청/응답 루프
- Rust command에서 sidecar로 메시지 보내는 adapter
- feature flag 또는 config로 direct / sidecar 선택 가능하게

완료 기준:
- sidecar ping 수준 호출 가능
- 앱이 sidecar 유무에 따라 명시적으로 상태를 알 수 있음

### Phase 1. Model Catalog / Availability / `!models`

목표:
- 모델 목록과 availability를 sidecar 책임으로 이동한다.

포함:
- `engine.list`
- `models.validate`
- 앱 시작 시 모델 카탈로그 로드
- `!models`는 sidecar cache 또는 Rust cache를 통해 동일 데이터 표시

완료 기준:
- UI와 `!models`가 같은 source를 사용
- source 구분 가능
- 사용 불가능 모델을 catalog에서 표시/제외 가능

### Phase 2. Single-Engine Chat 이관

목표:
- Claude 한 엔진부터 sidecar로 이관한다.

포함:
- `chat`
- `chunk`
- `completed`
- `cancel`
- resume token 흐름 유지

완료 기준:
- Claude 경로가 sidecar를 통해 안정적으로 동작
- trace / context pack / DB persistence 유지

### Phase 3. Multi-Engine 이관

목표:
- Codex / Gemini / OpenCode를 순차 이관한다.

포함:
- 엔진별 runner
- model 전달
- 실패/timeout 정책 통일

완료 기준:
- 모든 엔진이 동일 프로토콜로 실행
- direct-call 경로는 더 이상 기본 경로가 아님

### Phase 4. Run State / Queue 연동

목표:
- sidecar 이벤트와 `threadLocalRunQueuePlan`을 결합한다.

포함:
- thread-local run state
- same-thread queue
- cancel 정책 정리

완료 기준:
- 메신저형 UX
- 앱 전체 busy 제거
- 같은 thread는 직렬, 다른 project는 병렬 확장 가능

### Phase 5. Roundtable / Follow-up / Branch 실행 모델 통합

목표:
- roundtable과 follow-up도 sidecar protocol 기반으로 통합한다.

포함:
- `roundtable.run`
- branch thread 실행 통합
- follow-up handoff를 sidecar 요청 형식에 맞춤

완료 기준:
- 대화 / 브랜치 / RT가 같은 실행 모델 위에서 동작

## 단계별 위험

### Big-bang 전환 위험

- 스트리밍 계약이 한번에 깨질 수 있음
- cancel과 queue 정책이 동시에 흔들릴 수 있음
- roundtable / follow-up / trace / rawq 연결점이 복합적으로 깨질 수 있음
- 회귀 원인 분리가 어려움

따라서:
- 설계는 한 번에
- 이행은 단계적으로

## 검증 원칙

각 단계마다 최소한 아래를 확인한다.

1. 기존 DB persistence 유지 여부
2. ContextPack 품질 유지 여부
3. cancel 동작
4. chunk / completed 순서 보장
5. trace 기록 유지 여부
6. 같은 thread 직렬성 유지 여부

## 단계별 실행 체크리스트

### 시작 전

- 현재 direct-call 경로 목록 고정
- 엔진별 입출력/이벤트 계약 문서화
- sidecar bridge 에러 처리 원칙 정의

### Phase 1 이후

- 모델 카탈로그 source 일치 확인
- `!models`와 UI 출력 일치 확인

### Phase 2 이후

- Claude 단일 경로 완전 검증
- direct-call 대비 UX regression 확인

### Phase 3 이후

- 전체 엔진 동일 패턴 검증

### Phase 4 이후

- 앱 전체 busy 제거 확인
- same-thread queue 동작 확인

## 실행 프롬프트 템플릿

아래 템플릿을 각 phase 실행 시 기반으로 쓴다.

```md
# tunaFlow sidecar Phase X 구현

프로젝트:
- `D:\\privateProject\\tunaFlow`

참고 문서:
- `D:\\privateProject\\tunaFlow\\docs\\plans\\sidecarMigrationPlan.md`

이번 작업 목표는:
Phase X 범위만 구현하는 것이다.

중요:
- 실제 코드 기준으로만 작업
- 이번 단계 범위 밖 확장 금지
- 모든 응답과 보고는 한국어로만 작성하라

작업 후 반드시:
- 변경 파일
- protocol 변화
- 기존 direct-call 대비 차이
- 검증 결과
- 남은 리스크
를 정리하라.
```

## 최종 판단

sidecar 도입은 필요하다.
다만 `tunaChat` reference가 있다고 해서 한 번에 전체 교체하는 방식은 위험하다.
`tunaFlow`는 이미 기능 축이 많으므로, protocol을 먼저 설계하고 단계별로 이행하는 것이 맞다.
