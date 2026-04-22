# tunaFlow 엔진별 모델 카탈로그 / 세부 모델 선택 설계

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-26 09:16 KST

## 목적

현재 `tunaFlow`는 엔진(`claude`, `codex`, `gemini`, `opencode`) 선택은 가능하지만, 실제로 사용 가능한 세부 모델 목록을 현재 버전 기준으로 동적으로 가져오거나 안정적으로 선택하는 구조는 약하다. 일부 기본 모델 문자열은 프론트에 하드코딩되어 있고, 일부 엔진은 모델을 비워 둔 채 CLI 기본값에 의존한다.

이 문서는 `tunaFlow`에 엔진별 모델 카탈로그와 세부 모델 선택 UX를 도입하기 위한 설계 방향을 정리한다.

## 현재 상태

실제 코드 기준:

- 프론트 엔진 목록은 하드코딩
  - `src/components/tunaflow/NewMessageInput.tsx`
  - `src/components/tunaflow/BranchThreadPanel.tsx`
  - `src/components/tunaflow/MessageItem.tsx`
- 기본 모델도 일부 하드코딩
  - `src/lib/constants.ts`
  - Claude: `claude-haiku-4-5-20251001`
  - Gemini: `gemini-2.5-pro`
  - Codex/OpenCode는 명시적 모델 기본값이 약함
- 백엔드는 `model?: string` 전달을 지원
  - `src-tauri/src/commands/agents.rs`
  - `src-tauri/src/commands/roundtable_helpers/executor.rs`

결론:

- 엔진 선택은 가능
- 세부 모델 전달은 구조상 가능
- 하지만 모델 목록과 선택 UX는 아직 제품 수준이 아님

## 목표 상태

사용자는 다음을 할 수 있어야 한다.

1. 엔진별 사용 가능한 모델 목록을 본다.
2. 현재 버전 기준의 모델을 선택한다.
3. 선택한 모델을
   - 일반 대화
   - branch 대화
   - roundtable participant
   에서 사용할 수 있다.
4. 기본 모델과 최근 사용 모델을 구분해서 볼 수 있다.

## 설계 원칙

1. 엔진과 모델을 분리한다.
2. 모델 목록은 가능하면 동적으로 가져온다.
3. 동적 조회가 어려운 엔진은 관리 가능한 catalog fallback을 둔다.
4. 현재 하드코딩 기본값은 초기 fallback으로만 남긴다.
5. UI는 엔진 선택 후 모델 선택이 자연스럽게 이어지게 한다.

## 추천 구조

### 1. backend 모델 카탈로그 계층

예상 command:

- `list_engine_models(engine)`
- `get_engine_model_status(engine)` 선택적

반환 형태 예시:

```json
{
  "engine": "claude",
  "source": "dynamic" | "catalog" | "fallback",
  "models": [
    {
      "id": "claude-haiku-4-5-20251001",
      "label": "Haiku 4.5",
      "recommended": true,
      "available": true
    }
  ]
}
```

### 2. 엔진별 전략

#### Claude

- 가능하면 현재 CLI/환경에서 확인 가능한 방식 우선
- CLI에서 직접 목록 조회가 어렵다면 catalog fallback 필요

#### Codex

- 실제 현재 사용 중인 CLI/설치 상태 기준 확인
- 목록 조회 불가 시 curated catalog

#### Gemini

- 현재 CLI/API에서 목록 조회 가능 여부 확인
- 불가 시 curated catalog

#### OpenCode

- OpenCode가 제공하는 실제 모델 선택 방식 확인
- 목록 조회 불가 시 curated catalog

중요:

- 목록 조회 불가를 실패로만 보지 말고
- `source = fallback`인 curated catalog로라도 UX를 제공
- 단, 어떤 source인지 UI/로그에서 알 수 있으면 더 좋다

## 저장 정책

### 우선 저장 위치

현재 구조를 크게 바꾸지 않는 범위에서:

- conversation 기본 모델
- branch 기본 모델
- roundtable participant model

을 각각 유지할 수 있어야 한다.

권장:

- conversation 생성 시 `engine`, `model` 저장
- branch는 parent 상속 + override 허용
- roundtable participant는 participant 단위 model 유지

## UX 방향

### 일반 대화

- 엔진 드롭다운 옆에 모델 드롭다운 추가
- 엔진 바꾸면 모델 목록 갱신
- 모델이 하나뿐이거나 미지원이면 hidden/disabled 가능

### branch 대화

- 기존 branch thread panel에서 엔진 선택 옆에 모델 선택 추가

### roundtable

- participant별:
  - 엔진
  - 모델
  를 개별 지정 가능하게

### 표시 정책

- 추천 모델 배지
- fallback catalog인 경우 작은 표시
- 실제 현재 conversation의 engine/model이 헤더나 상태바에 보이면 좋음

## 구현 단계

### Phase 1. 카탈로그 계층

- backend `list_engine_models`
- 엔진별 fallback catalog 정의
- 프론트 API 연결

### Phase 2. 일반 대화 / branch 모델 선택

- `NewMessageInput`
- `BranchThreadPanel`

### Phase 3. Roundtable participant 모델 선택

- participant별 모델 지정
- 기본 participant 하드코딩 축소

### Phase 4. 상태/권장값 polish

- 최근 사용 모델
- 추천 모델 표시
- source 표시

## 현재 하드코딩 제거 방향

즉시 전부 제거하지 말고 다음 식으로 축소한다.

1. 현재 하드코딩 기본값은 fallback으로 유지
2. 실제 카탈로그가 있으면 그쪽 우선
3. UI에서는 하드코딩 배열 대신 카탈로그 결과 사용

## 판단

`tunaFlow`는 이미 엔진 멀티플렉싱 구조를 갖고 있으므로, 다음 고도화 포인트는 엔진 자체보다 **세부 모델 선택 체계**다. 이 작업은 UX와 실제 사용성에 직접 연결되며, 현재의 하드코딩 모델 구조를 점진적으로 대체하는 것이 맞다.
