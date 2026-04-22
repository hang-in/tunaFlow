# tunaFlow rawq 자동화 운영 계획

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-26 08:38 KST

## 목적

`tunaFlow`에서 rawq를 실제로 도입한다면, 사용자가 rawq를 직접 관리하지 않아도 되도록 운영 흐름을 자동화해야 한다. 이 문서는 rawq의 자동 실행, 자동 인덱싱, 업데이트 대응을 하나의 운영 계획으로 정리한다.

## 원칙

1. rawq 실행은 자동이어야 한다.
2. 인덱싱은 사용자 수동 개입 없이 가능한 한 자동이어야 한다.
3. 업데이트는 자동 감지까지는 허용하지만, 자동 적용은 신중해야 한다.
4. rawq 자체 로직은 `tunaFlow` 안에 재구현하지 않는다.
5. `tunaFlow`는 rawq를 호출하고 상태를 관리하는 얇은 adapter만 가진다.

## 목표 상태

사용자는 다음을 신경 쓰지 않아도 된다.

- rawq 바이너리 경로
- 인덱스가 있는지 여부
- 언제 재인덱싱해야 하는지
- rawq 버전이 오래됐는지 여부

대신 `tunaFlow`가 아래를 자동으로 처리한다.

- 프로젝트 열 때 rawq 상태 확인
- agent 호출 전 필요한 code context 자동 확보
- stale index 자동 재생성 또는 백그라운드 재인덱싱
- rawq 업데이트 가능 여부 감지

## 1. 자동 실행

### 목표

agent 호출 시 rawq가 있으면 자동으로 사용한다.

### 권장 동작

- `ContextPack` 조립 시 자동 rawq search
- rawq 바이너리 탐색 순서 (현재 구현 기준):
  1. `RAWQ_BIN` 환경변수
  2. bundled sidecar (`src-tauri/binaries/rawq-<target-triple>`)
  3. 개발용 로컬 빌드 경로
  4. PATH의 `rawq` (개발 보조 경로)

### 운영 모드

현재 구현 (2026-03-28):

- rawq 실패 시 `build_rawq_section()`이 `None`을 반환하고 해당 섹션만 빠짐
- 요청 자체가 실패하지는 않음 (에러는 stderr에 기록)
- strict mode는 미구현

향후 옵션:

- strict mode: rawq 실패 시 요청 자체 실패 (도입 검증용). 현재는 계획만 존재.

### 완료 기준

- 사용자는 별도 명령 없이 rawq 검색 결과를 자동으로 받는다. — **완료**

## 2. 자동 인덱싱

### 목표

인덱스가 없거나 오래됐을 때 자동으로 복구한다.

### 권장 흐름

#### 프로젝트 열기

- `rawq index status <project_path>` 확인
- 인덱스 없음:
  - 자동 `rawq index build <project_path>`

#### 프로젝트 사용 중

- 파일 변경 감지
- 즉시 재인덱싱하지 않고 debounce
- 마지막 변경 후 일정 시간 idle이면 백그라운드 재인덱싱

#### agent 호출 직전

- 인덱스가 stale이면:
  - 빠른 재확인
  - 필요 시 백그라운드 재인덱싱 시작

### 왜 저장할 때마다 바로 재인덱싱하지 않는가

- 너무 자주 돌면 무겁다
- 대형 프로젝트에서 UX가 나빠진다

따라서 권장 전략은:

- 최초 1회 자동 build
- 이후는 변경 누적 후 자동 rebuild
- 호출 직전 stale 확인

### 완료 기준

- 새 프로젝트에서 수동 인덱싱 없이 rawq 사용 가능
- 코드 변경 후에도 인덱스가 장시간 낡은 상태로 방치되지 않음

## 3. rawq 업데이트 대응

### 목표

rawq 레포가 업데이트되어도 `tunaFlow`가 갑자기 깨지지 않도록 한다.

### 원칙

- 자동 감지: 예
- 자동 적용: 기본적으로 아니오

### 이유

rawq 업데이트 시 바뀔 수 있는 것:

- CLI 옵션
- `--json` 출력 스키마
- 인덱스 포맷
- daemon 동작

이걸 무조건 자동 반영하면 `tunaFlow` adapter가 깨질 수 있다.

### 권장 흐름

1. 현재 사용 중인 rawq 버전/커밋 기록
2. 주기적으로 또는 앱 시작 시 버전 확인
3. 기준보다 새 버전이 있으면 "업데이트 가능" 표시
4. 사용자가 명시적으로 업데이트 실행
5. 업데이트 후 self-check 수행
   - `rawq --version`
   - `rawq search --help`
   - `rawq search ... --json`
   - 필요 시 `rawq index status`

### 완료 기준

- 업데이트 가능 여부는 자동으로 알 수 있다.
- 업데이트 적용은 통제 가능하다.
- 적용 후 호환성 검증 루틴이 있다.

## 4. 추천 구현 순서

### Phase A. 자동 실행 안정화

- rawq binary resolution — **완료** (4단계 탐색)
- search 자동 실행 — **완료** (ContextPack 조립 시 자동)
- strict mode — 미구현 (향후 옵션)

### Phase B. 자동 인덱싱

- index status command
- project open 시 index 확인
- 최초 자동 build
- debounce 재인덱싱

### Phase C. 업데이트 감지

- rawq version 표시
- 기준 버전 비교
- self-check

## 5. tunaFlow에서 예상 수정 위치

- `src-tauri/src/agents/rawq.rs`
- `src-tauri/src/commands/agents_helpers/context_pack.rs`
- 향후:
  - rawq 상태 command
  - project open/select 흐름
  - ContextPanel 상태 표시

## 6. 운영 판단

rawq는 사용자가 직접 조작하는 기능이 아니라, `tunaFlow`가 자동으로 관리하는 코드 컨텍스트 엔진에 가깝게 가는 것이 맞다.

즉 최종 목표는:

- 검색: 자동
- 인덱싱: 자동
- 업데이트: 자동 감지 + 통제된 반영

이다.
