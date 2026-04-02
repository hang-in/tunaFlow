# abtop 코드베이스 분석 및 tunaFlow 적용 아이디어

> Status: idea
> Created: 2026-04-01
> 대상: `/Users/d9ng/privateProject/_research/_util/abtop`
> 관점: tunaFlow에 직접 이식할 가치가 있는 패턴만 추출

---

## 1. 한 줄 결론

`abtop`에서 tunaFlow에 가져올 만한 핵심은 **에이전트 런타임 관측/진단 모델**이다.

즉 가져올 것은:

- shared process scan
- 공통 session diagnostics 모델
- orphan port / child process 추적
- rate limit side-channel 수집

이고,

가져오지 않을 것은:

- TUI 자체
- 외부 collector 중심 구조 전체
- agent discovery 방식 전체

이다.

---

## 2. abtop가 실제로 잘하는 것

`abtop`는 “AI coding agent용 htop”에 가깝다.

핵심 기능:

- Claude/Codex 세션 자동 발견
- 세션별 token/context/status/current task 표시
- child process / port 추적
- orphan port 탐지
- git dirty 상태 표시
- rate limit 정보 표시

참고 파일:

- `README.md`
- `src/app.rs`
- `src/model/session.rs`
- `src/collector/mod.rs`
- `src/collector/process.rs`
- `src/collector/rate_limit.rs`

즉 이 프로젝트의 중심은 **agent execution runtime observability**다.

---

## 3. 구조적으로 좋은 부분

### 3.1 SharedProcessData 패턴

핵심 파일:

- `src/collector/mod.rs`

핵심 아이디어:

- `ps`
- children map
- open ports

를 매 collector가 따로 읽지 않고, tick마다 한 번만 읽어서 공유한다.

이 패턴의 장점:

- 중복 시스템 호출 감소
- 여러 agent collector가 같은 process snapshot을 공유
- orphan port나 descendant CPU 같은 계산을 일관되게 수행 가능

### tunaFlow 적용 가치

높음.

tunaFlow도 앞으로:

- runtime diagnostics
- background worker/daemon
- orphan process 확인
- project별 child process 추적

을 더 키우려면 process snapshot 공유 패턴이 필요하다.

---

### 3.2 AgentSession 표준 모델

핵심 파일:

- `src/model/session.rs`

핵심 아이디어:

엔진마다 수집 방식은 달라도, 최종 UI에는 공통 세션 모델을 준다.

예:

- `status`
- `model`
- `context_percent`
- `total_input_tokens`
- `total_output_tokens`
- `git_branch`
- `git_added/git_modified`
- `children`
- `current_tasks`

### tunaFlow 적용 가치

높음.

tunaFlow는 trace/runtime 정보가 여러 군데 흩어져 있고,
엔진별 사용량/상태 차이도 아직 surface마다 다르게 보일 수 있다.

따라서 내부적으로도 다음 같은 표준 snapshot 모델이 유용하다.

예:

```ts
interface AgentRuntimeSnapshot {
  engine: string;
  conversationId: string;
  status: "running" | "waiting" | "done" | "error";
  currentTask?: string;
  contextMode?: string;
  contextPercent?: number;
  inputTokens?: number;
  outputTokens?: number;
  costUsd?: number | null;
  gitBranch?: string | null;
  gitDirty?: boolean;
  childProcesses?: RuntimeChildProcess[];
  orphanPorts?: RuntimeOrphanPort[];
}
```

이런 표준 모델이 있으면:

- RuntimeStatusBar
- TracePanel
- 나중의 daemon/diagnostics

를 같은 데이터 언어로 묶기 쉬워진다.

---

### 3.3 Orphan port / child process 추적

핵심 파일:

- `src/collector/process.rs`
- `src/collector/mod.rs`

핵심 아이디어:

- 에이전트 세션이 죽었는데
- child process가 포트를 계속 잡고 있으면
- orphan port로 식별

이는 AI coding agent 환경에서 실제로 자주 생기는 문제다.

### tunaFlow 적용 가치

매우 높음.

tunaFlow는:

- dev server
- test watcher
- local tool
- shell command

를 agent가 띄우는 워크플로우가 많기 때문에,
“누가 띄운 프로세스인지”와 “아직 살아 있어야 하는지”를 보는 진단층이 유용하다.

가능한 surface:

- `TracePanel` diagnostics card
- `Runtime` settings/status area
- 나중의 `agent daemon` status page

---

### 3.4 Rate limit side-channel 수집

핵심 파일:

- `src/collector/rate_limit.rs`

핵심 아이디어:

모델 응답 안에서만 토큰을 보는 게 아니라:

- Claude hook file
- Codex cache file

같은 별도 채널로 quota 정보를 읽는다.

### tunaFlow 적용 가치

중상.

tunaFlow는 현재:

- token/cost usage

는 꽤 보강됐지만,

- 계정 quota
- 5시간/일/주 단위 소진율

같은 운영 정보는 약하다.

이는 특히 다중 agent를 오래 돌리는 사용자에게 의미가 있다.

---

## 4. tunaFlow에 가져오지 않는 것이 좋은 부분

### 4.1 TUI 자체

핵심 파일:

- `src/ui/mod.rs`

`abtop`의 TUI는 완성도가 높지만, tunaFlow는 Tauri GUI 앱이다.

가져올 것은:

- 레이아웃 사고방식
- 정보 밀도

정도이지,

ratatui 패널/위젯 구조 자체를 이식할 가치는 낮다.

---

### 4.2 세션 discovery 방식 전체

`abtop`는 외부 도구이기 때문에:

- ps
- lsof
- 로그 파일 파싱

으로 “돌아가는 세션을 발견”해야 한다.

반면 tunaFlow는 이미:

- conversation
- trace
- runtime queue
- DB
- event

를 내부적으로 알고 있다.

따라서 discovery 자체를 그대로 가져오면 오히려 중복이 커진다.

가져올 것은:

- discovery 결과 모델
- diagnostics 패턴

이지,

discovery 구현 전체는 아니다.

---

### 4.3 collector 전체 이식

Claude/Codex collector는 `abtop`라는 외부 관찰 도구에 맞는 코드다.

tunaFlow는:

- 내부 실행 정보가 더 정확한 경우가 많고
- 외부 collector는 보조적 진단에만 쓰는 편이 맞다.

즉 collector 구현 전체를 가져오지 말고,
원리만 차용해야 한다.

---

## 5. tunaFlow에 실제로 맞는 적용 아이디어

### P1. Runtime Diagnostics 계층

가장 현실적인 첫 적용.

새로운 진단 모듈을 만들어:

- child process count
- listening ports
- orphan risk
- git dirty
- current task
- context %

를 한 번에 반환한다.

가능한 파일:

- `src-tauri/src/commands/runtime_diagnostics.rs`

가능한 명령:

- `get_runtime_diagnostics`
- `list_runtime_children`
- `list_orphan_ports`

이건 agent-first 철학과도 잘 맞는다.

---

### P1. Shared Process Scan 백엔드

`abtop`처럼:

- process info
- children map
- ports

를 한 tick/요청에서 한 번만 읽는 모듈을 두는 게 좋다.

가능한 구조:

```rust
pub struct SharedProcessData {
    pub process_info: HashMap<u32, ProcInfo>,
    pub children_map: HashMap<u32, Vec<u32>>,
    pub ports: HashMap<u32, Vec<u16>>,
}
```

이후 여러 diagnostics command가 이걸 재사용할 수 있다.

---

### P1. Agent Runtime Snapshot 표준화

지금 tunaFlow는:

- TracePanel
- RuntimeStatusBar
- background run state

가 각자 필요한 정보만 부분적으로 보고 있다.

`abtop`처럼 공통 세션 모델을 두면:

- 같은 데이터를 여러 surface에서 재사용 가능
- engine별 parity 확인이 쉬움
- 나중의 daemon extraction에도 유리

---

### P2. Quota/Rate Limit 관측

사용 가치가 있지만,
우선순위는 orphan/process diagnostics보다 낮다.

이유:

- 현재 tunaFlow는 token/cost 자체는 꽤 잘 보인다
- quota 정보는 추가 운영 편의성에 가깝다

즉 P2 정도가 적절하다.

---

## 6. 현재 tunaFlow와의 궁합 평가

### 잘 맞는 점

- agent-first 제품 철학
- 여러 agent/engine을 동시에 돌리는 구조
- background execution 증가
- trace/runtime surface 이미 존재
- git-aware workflow 존재

### 주의할 점

- tunaFlow는 외부 관찰 도구가 아니라 내부 오케스트레이터다
- diagnostics가 코어 workflow를 압도하면 안 된다
- `abtop`식 정보량을 그대로 UI에 넣으면 과밀해질 수 있다

즉 적용 방향은:

- `abtop 전체를 가져오기`

가 아니라

- `tunaFlow의 Runtime/Trace에 필요한 진단층만 흡수하기`

가 맞다.

---

## 7. 권장 결론

`abtop` 분석에서 tunaFlow에 가장 가치 있는 것은:

1. shared process scan
2. session/runtime snapshot 표준화
3. orphan port / child process diagnostics
4. 그 다음 quota/rate-limit 관측

이다.

반대로:

- TUI 자체
- 외부 collector 전체
- discovery 방식 전체

는 tunaFlow에 바로 적용할 대상이 아니다.

---

## 8. 제안되는 다음 단계

만약 실제로 도입을 검토한다면 순서는 이게 맞다.

### Step 1

`Runtime Diagnostics` 아이디어 문서 또는 plan 문서 작성

### Step 2

백엔드에 shared process snapshot + orphan port detection 최소 구현

### Step 3

TracePanel 또는 Runtime surface에 최소 diagnostics 카드 추가

### Step 4

필요 시 quota/rate-limit 연동 검토

---

## 9. 최종 한 줄 평가

`abtop`는 tunaFlow에 “또 다른 UI”를 주는 참고 레포가 아니라,
**에이전트 런타임을 어떻게 관측하고 진단할지에 대한 아주 좋은 참고 구현**이다.
