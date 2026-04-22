# tunaFlow ContextPack 4-Engine Parity 계획

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-29
- 상태: Phase 1 완료 (build_normalized_prompt 통합)

## 현재 차이

문서와 코드 모두 `ContextPack full`이 Claude 전용에 가깝다.

- Claude: richer assembly
- Codex/Gemini/OpenCode: lite context prefix

이 상태에서는 엔진을 바꾸는 순간 정보량 자체가 달라진다.

## 목표

모든 엔진이 같은 논리적 ContextPack을 받도록 맞춘다.

여기서 "같다"의 의미는:

1. 같은 데이터 소스를 사용
2. 같은 section 분류를 사용
3. provider별 포맷만 얇게 다를 수 있음

## 공통 섹션

1. project context
2. recent conversation context
3. plan section
4. findings section
5. artifact handoff
6. skills section
7. rawq section
8. cross-session section
9. thread inheritance

## 단계

### Phase 1. normalized context payload 정의

- provider별 prompt 문자열 조립 전에 공통 구조체 또는 조립 함수 정의
- Claude 전용 분기보다 상위 계층에서 section presence를 결정

### Phase 2. non-Claude 경로 확장

- Codex/Gemini/OpenCode 경로가 lite prefix 대신 normalized payload를 사용
- 길이 제한은 provider별 guardrail로만 조정

### Phase 3. provider 비교 표 갱신

- `implementationStatus.md`에서 full/lite 분리 표기를 줄이고
- 실제 parity 상태를 새 기준으로 정리

## 리스크

- non-Claude CLI의 입력 길이 허용치 차이
- 과도한 prompt 길이로 인한 latency 증가

## 검증

1. 동일 대화에서 section presence가 4개 엔진 모두 일치
2. provider 차이는 길이/포맷 정도로 제한
3. trace에 포함 section 목록이 기록됨

