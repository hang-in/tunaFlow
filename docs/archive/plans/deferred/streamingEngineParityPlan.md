# tunaFlow Streaming 4-Engine Parity 계획

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-29
- 상태: Phase 1-2 완료 (Codex JSONL streaming + frontend chunk listener + invoke parity)

## 현재 차이

- Claude: streaming 지원
- Gemini: streaming 지원
- Codex: one-shot 중심
- OpenCode: one-shot 중심

사용자는 엔진을 바꾸는 순간 응답 경험이 달라진다.

## 목표

모든 엔진에서 최소한의 streaming UX를 동일하게 제공한다.

여기서 parity의 기준은:

1. placeholder assistant message
2. partial content 업데이트
3. cancel 동작
4. completion/failure 상태 전이

## 구현 전략

### Option A. provider-native streaming 우선

가능하면 provider가 제공하는 streaming을 쓴다.

### Option B. synthetic streaming fallback

native streaming이 없으면:

- subprocess chunk polling
- line-buffered relay
- completion 전 partial progress event

등으로 UX를 맞춘다.

## 단계

### Phase 1. 상태 모델 통일

- streaming / completed / failed / cancelled 상태 정의를 엔진 공통으로 정리

### Phase 2. Codex/OpenCode parity

- one-shot 결과를 최소한 partial-like UX로 보이게 할지 결정
- 가능하면 provider-native 또는 wrapper 기반 incremental relay 추가

### Phase 3. frontend event parity

- 동일한 event contract로 UI를 갱신
- thinking/progress placeholder도 통일

## 검증

1. 4개 엔진 모두 동일 상태 전이 모델 사용
2. 사용자는 엔진 교체 시 streaming 유무 차이를 거의 느끼지 않음
3. cancel / failure UX가 엔진별로 다르지 않음

