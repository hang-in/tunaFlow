# `!models` 기반 모델 카탈로그 설계

작성자: OpenAI Codex  
작성일: 2026-03-26

## 목적

`tunaFlow`의 엔진별 모델 선택을 단순 하드코딩 UI가 아니라, 하나의 공통 모델 카탈로그 계층으로 정리한다.

핵심은 다음 두 가지다.

1. 앱 시작 시 모델 목록을 받아 캐시한다.
2. `!models`와 모델 선택 UI가 같은 목록을 사용한다.

즉 `!models`를 별도 기능으로 만들지 않고, `모델 카탈로그 SSOT` 위에 텍스트 명령과 UI를 같이 얹는다.

## 배경

현재 `tunaFlow`는:

- 엔진 목록이 프론트에 하드코딩되어 있음
- 일부 기본 모델도 상수로 박혀 있음
- `model?: string` 전달 구조는 있으나, 목록 공급 계층은 없음

반면 `tunaDish`는:

- `!models`를 `chat.send("!models")`로 보낼 수 있음
- 실제 모델 목록은 `project.context.result.available_engines` 또는 `engine.list.result`를 통해 공급
- UI는 그 캐시된 `availableEngines`를 사용

즉 `tunaDish` 패턴의 핵심은 `실시간 CLI 탐색`이 아니라 `공통 모델 목록 공급 계층`이다.

## 목표

`tunaFlow`에서 최소한 아래가 가능해야 한다.

1. 앱 시작 시 엔진별 모델 목록 로드
2. 필요 시 수동 갱신
3. 일반 대화/브랜치 대화 UI에서 같은 목록 사용
4. `!models` 명령도 같은 목록을 사용
5. 선택된 모델이 실제 send 경로로 전달

## 제안 구조

### 1. 공통 모델 카탈로그 계층

백엔드에 공통 command를 둔다.

- `list_engine_models()`
- 또는 `refresh_engine_models()`

반환 형태 예시:

```json
{
  "engines": {
    "claude": ["claude-sonnet-4-5", "claude-haiku-4-5"],
    "gemini": ["gemini-2.5-pro", "gemini-2.5-flash"],
    "codex": ["o3", "o4-mini"],
    "opencode": []
  },
  "source": "curated",
  "updatedAt": 1760000000
}
```

여기서 중요한 것은:

- UI가 엔진별 모델 목록을 한 구조로 소비할 수 있어야 하고
- `!models`도 같은 구조를 텍스트로 보여줘야 한다는 점이다.

### 2. 공급 시점

실시간 탐색은 강제하지 않는다.

권장 시점:

1. 앱 시작 시 1회 로드
2. 프로젝트/세션 전환 시 필요하면 재사용
3. 사용자가 `!models --refresh` 또는 refresh 버튼을 누르면 재조회

즉 기본은 `load-on-start + cache`다.

### 3. 프론트 저장 위치

store에 아래 중 하나를 둔다.

- `engineCatalog`
- `availableEngines`

권장 구조:

```ts
type EngineCatalog = {
  source: "curated" | "dynamic";
  updatedAt?: number;
  engines: Record<string, string[]>;
}
```

### 4. `!models`의 역할

`!models`는 모델을 직접 계산하는 명령이 아니라,
현재 캐시된 모델 카탈로그를 텍스트로 보여주는 명령이어야 한다.

권장 동작:

- `!models`
  - 현재 캐시된 목록 출력
- `!models --refresh`
  - backend 재조회 후 결과 출력

즉 `!models`는 카탈로그의 조회/노출 인터페이스다.

## 현실적인 source 전략

현재 CLI들만으로 완전한 동적 모델 목록 조회는 제한적일 가능성이 높다.
따라서 초기 단계에서는 `curated source`를 허용하는 것이 현실적이다.

중요:

- curated라도 `source: "curated"`를 명시해야 한다
- 나중에 실제 동적 조회가 가능해지면 backend source만 교체할 수 있어야 한다

즉 목표는 `완전 동적`이 아니라 `교체 가능한 모델 카탈로그 계층`이다.

## UX 방향

### 일반 대화 / 브랜치 대화

- 엔진 선택 드롭다운
- 모델 선택 드롭다운
- 둘 다 같은 카탈로그 사용

### `!models`

예시 출력:

```text
사용 가능한 모델 목록

- claude
  - claude-sonnet-4-5
  - claude-haiku-4-5
- gemini
  - gemini-2.5-pro
  - gemini-2.5-flash
- codex
  - o3
  - o4-mini

source: curated
```

### Roundtable

이번 단계에서는 participant별 상세 편집까지 바로 가지 않아도 된다.
다만 roundtable 기본 participant 모델도 같은 카탈로그를 참조하도록 정리할 수 있어야 한다.

## 구현 순서

### Phase 1

- backend `list_engine_models`
- 프론트 store에 `engineCatalog`
- 앱 시작 시 모델 목록 로드

### Phase 2

- 일반 대화 / 브랜치 대화 모델 선택 UI를 카탈로그 기반으로 전환
- 하드코딩 기본값은 fallback default가 아니라 `초기 선택값` 수준으로만 축소

### Phase 3

- `!models` 명령 추가
- `!models --refresh` 지원
- 필요 시 roundtable 설정 연결

## 완료 기준

아래가 되면 1차 완료로 본다.

1. 앱 시작 시 엔진별 모델 목록이 store에 들어간다
2. 일반 대화와 브랜치 대화가 같은 목록을 사용한다
3. `!models`가 같은 카탈로그를 텍스트로 보여준다
4. 선택 모델이 실제 send 경로로 전달된다
5. 카탈로그 source가 명시된다

## Opus 실행 프롬프트

아래 프롬프트를 그대로 사용하면 된다.

```md
# tunaFlow `!models` + 공통 모델 카탈로그 구현

프로젝트:
- `D:\privateProject\tunaFlow`

참고 문서:
- `D:\privateProject\tunaFlow\docs\plans\modelsCommandCatalogPlan.md`

이번 작업 목표는:
엔진별 모델 목록을 하나의 공통 카탈로그 계층으로 정리하고,
그 목록을 UI와 `!models` 명령이 함께 사용하도록 구현하는 것이다.

중요:
- 실제 코드 기준으로만 작업
- 기존 멀티엔진 구조 유지
- `!models`를 별도 임시 기능으로 만들지 말고 공통 카탈로그를 먼저 만들 것
- 모든 응답과 보고는 한국어로만 작성하라

## 먼저 확인할 파일

### 프론트
- `D:\privateProject\tunaFlow\src\components\tunaflow\NewMessageInput.tsx`
- `D:\privateProject\tunaFlow\src\components\tunaflow\BranchThreadPanel.tsx`
- `D:\privateProject\tunaFlow\src\stores\chatStore.ts`
- `D:\privateProject\tunaFlow\src\types\index.ts`
- `D:\privateProject\tunaFlow\src\lib\constants.ts`

### 백엔드
- `D:\privateProject\tunaFlow\src-tauri\src\commands\agents.rs`
- `D:\privateProject\tunaFlow\src-tauri\src\lib.rs`
- 엔진 adapter 파일들

## 구현 요구사항

1. backend에 공통 모델 카탈로그 command 추가
   - 예: `list_engine_models()`
2. 앱 시작 시 모델 목록을 불러와 store에 저장
3. 일반 대화 / 브랜치 대화 UI가 같은 카탈로그를 사용하게 수정
4. `!models` 명령을 추가해 현재 카탈로그를 텍스트로 보여주게 구현
5. 가능하면 `!models --refresh`도 지원
6. 선택된 모델이 실제 send 경로로 전달되게 유지

## source 정책

현재 단계에서는 curated source 허용.
단:
- source를 반드시 명시할 것 (`curated` / `dynamic`)
- UI 또는 `!models` 출력에서 source를 확인 가능하게 할 것

## 하지 말 것

- 새 settings 페이지 추가
- 완전 실시간 모델 탐색 강제
- 문서 대규모 정리
- 엔진 구조 전면 재설계

## 검증

작업 후 반드시 아래를 설명하라.

1. 모델 카탈로그를 어디서 어떻게 로드하는지
2. source가 무엇인지
3. `!models`가 어떤 데이터를 보여주는지
4. 일반 대화 / 브랜치 UI가 어떻게 같은 카탈로그를 쓰는지
5. 실제 send 경로로 모델이 어떻게 전달되는지
6. 타입체크/빌드/가능한 검증 결과

## 출력 형식

### A. Changes Made
### B. Files Modified
### C. Model Catalog Flow
### D. `!models` Flow
### E. Verification
### F. Remaining Risks

바로 코드 수정까지 진행하라.
```
