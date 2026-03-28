# tunaFlow Token/Cost Tracking 4-Engine Parity 계획

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-29
- 상태: Phase 1-2 부분 완료 (frontend N/A 표시 완료, backend DB 레벨 unavailable 구분은 후속)

> **현재 한계:** DB에는 0과 unavailable이 구분되지 않음.
> Frontend에서 engine 기반으로 N/A를 표시하지만, `trace_log`/`messages` 테이블에는 `usage_status` 같은 컬럼이 없어 쿼리 시 구분 불가.
> 후속 작업: `usage_status ENUM (exact, estimated, unavailable)` 컬럼 추가 (DB migration 필요).

## 현재 차이

- Claude: usage 비교적 풍부
- Codex: usage 기록 존재
- Gemini: partial
- OpenCode: 없음

이 상태에서는 엔진별 운영 가시성이 맞지 않는다.

## 목표

최소한 다음 항목은 4개 엔진에 공통으로 남겨야 한다.

1. input tokens
2. output tokens
3. total cost 또는 cost unavailable reason
4. message/conversation 누적 usage

## 원칙

1. provider가 cost를 직접 주면 그대로 사용
2. provider가 안 주면 추정치 또는 unavailable 상태를 명확히 기록
3. "없음"과 "미구현"을 구분한다

## 단계

### Phase 1. 공통 usage model 정리

- exact / estimated / unavailable 상태 정의
- DB 및 frontend 표기 규칙 통일

### Phase 2. Gemini/OpenCode 보강

- Gemini partial를 exact/estimated로 명확히 표시
- OpenCode는 최소 unavailable reason부터 기록

### Phase 3. UI parity

- 엔진별 usage 배지/상세 표기를 같은 틀로 정리

## 검증

1. 4개 엔진 모두 usage record가 남음
2. exact/estimated/unavailable가 구분됨
3. conversation 누적 합산이 엔진별로 깨지지 않음

