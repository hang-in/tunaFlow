# Thread-Local Run Queue 설계

작성자: OpenAI Codex  
작성일: 2026-03-26

## 목적

`tunaFlow`의 실행 상태를 앱 전역 busy 모델에서 벗어나, 메신저/채팅앱처럼 보이는 UX와 터미널 에이전트의 직렬 실행 제약을 동시에 만족하는 구조로 재정의한다.

핵심 원칙:

- 앱 전체는 잠그지 않는다
- thread 단위로만 실행 상태를 가진다
- 같은 thread의 추가 입력은 queue에 적재한다
- 다른 thread의 탐색과 읽기는 항상 가능해야 한다

## 배경

현재 `tunaFlow`는 전역 `isRunning: boolean`을 기준으로 입력과 일부 상호작용을 막는다.

이 구조는 다음 문제를 만든다.

- 한 conversation이 실행 중이면 앱 전체가 busy처럼 느껴짐
- branch/thread도 같이 잠김
- Claude 외 엔진은 스트리밍이 약해 더 멈춘 것처럼 보임

반면 실제 에이전트 런타임은 같은 채팅에서 입력이 추가되면 바로 처리하지 못하고, 안전한 시점에 순차적으로 처리하는 편이 맞다.

즉 필요한 것은:

- UI는 메신저처럼 자유롭게
- 런타임은 thread-local queue로 직렬 처리

## 용어

여기서 thread는 아래 중 하나다.

- main conversation
- branch stream
- roundtable thread

즉 전송 단위는 `project` 전체가 아니라 `thread`다.

## 목표 UX

### 사용자가 느껴야 하는 것

- 현재 대화가 실행 중이어도 다른 대화 클릭 가능
- Sidebar 탐색 가능
- ContextPanel 열람 가능
- branch 전환 가능
- 현재 대화에 추가 메시지를 입력하면 "대기열"로 들어감
- 메시지 전송이 거부되지 않고 받아들여짐

### 같은 thread 안에서는

- 현재 run이 끝나기 전까지 새 입력은 queue
- 완료 후 다음 입력 자동 실행

즉 메신저 UX와 직렬 런타임을 동시에 만족해야 한다.

## 상태 모델

전역 `isRunning` 대신 thread 단위 상태를 둔다.

예시:

```ts
type ThreadRunStatus = "idle" | "running" | "queued" | "cancelling" | "error";

type ThreadRunState = {
  threadId: string;
  status: ThreadRunStatus;
  queueLength: number;
  activeEngine?: string;
  activeModel?: string;
  lastError?: string;
}
```

store 예시:

```ts
runStateByThread: Record<string, ThreadRunState>
outboundQueueByThread: Record<string, QueuedMessage[]>
```

## queue 모델

### 같은 thread

- 상태가 `running`이면 새 입력을 reject하지 않는다
- 대신 queue에 적재
- 현재 run 종료 후 다음 항목 자동 실행

### 다른 thread

- 읽기/탐색은 항상 허용
- 쓰기(새 전송)는 정책 선택 가능
  - 최소 버전: 다른 thread 전송도 허용
  - 보수 버전: 같은 프로젝트 내 동시 실행 제한

1차 구현에서는:

- 다른 thread 탐색/읽기 자유
- 같은 thread queue

까지면 충분하다.

## 터미널 에이전트 제약 반영

터미널 에이전트는 현재 턴이 끝나기 전 새 입력을 즉시 처리하기 어려울 수 있다.

따라서 `tunaFlow`는 아래처럼 동작해야 한다.

- UI는 입력을 즉시 받아들임
- 런타임은 실행 가능한 시점에 전달

즉:

- `accept now`
- `execute when safe`

모델이 맞다.

## UI 표시

### thread-local 상태 표시

현재 thread에만 아래를 표시한다.

- `running`
- `queued 1`
- `cancelling`
- `error`

예:

- `Claude running`
- `1 queued`
- `Cancelling...`

### 입력창 정책

- 현재 thread가 running이어도 textarea는 비활성화하지 않는다
- 전송 시 즉시 queue 적재 가능
- 버튼 라벨 예:
  - `Send`
  - `Queue`

### Sidebar / ContextPanel

- 전역 잠금 금지
- 실행 중에도 클릭 가능

## cancel 정책

1차 권장 정책:

- cancel은 현재 active run만 취소
- queued 메시지는 유지
- 취소 후 queue의 다음 메시지를 자동 실행할지 여부는 정책화

권장 기본:

- cancel 후 queue는 유지
- 사용자가 다시 resume/send 하거나, 자동으로 다음 항목 실행

이 부분은 구현 단순성을 위해 1차에서 "active run 취소 + queue 유지" 정도면 충분하다.

## 구현 순서

### Phase 1

- 전역 `isRunning` 대신 thread-local run state 도입
- 현재 thread만 busy 처리
- Sidebar / ContextPanel 잠금 제거

### Phase 2

- same-thread queue 도입
- running 중 새 메시지는 queue 적재
- queue 길이 표시

### Phase 3

- cancel + queue 정책 정교화
- roundtable / follow-up / branch thread에도 같은 모델 확장

## 완료 기준

1. 한 thread 실행 중에도 다른 대화 탐색 가능
2. 현재 thread에 새 메시지를 보내면 reject가 아니라 queue 적재
3. queue 상태가 UI에 보인다
4. active run 종료 후 queued 입력이 순차 실행된다
5. 전역 busy 느낌이 사라진다

