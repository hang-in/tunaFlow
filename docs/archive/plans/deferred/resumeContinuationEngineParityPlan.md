# tunaFlow Resume / Continuation 4-Engine Parity 계획

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-29
- 상태: Phase 1-2 완료 (synthetic continuation은 build_normalized_prompt로 이미 구현됨)

## 현재 차이

- Claude: native resume token 존재
- Codex/Gemini/OpenCode: native resume 개념이 없거나 미사용

그래서 같은 conversation이라도 엔진을 바꾸면 연속성 경험이 달라진다.

## 목표

resume parity는 "같은 API"가 아니라 "같은 사용자 경험"으로 정의한다.

즉 사용자는:

1. 이전 대화 맥락을 이어서 보낼 수 있고
2. 앱 재시작 후에도 continuation이 유지되며
3. 엔진별로 대화가 끊겼다고 느끼지 않아야 한다

## parity 기준

### Claude

- native resume token 유지

### Codex/Gemini/OpenCode

- synthetic continuation layer 도입
- recent turns replay
- parent anchor
- stable context summary

## 단계

### Phase 1. continuation contract 정의

- native token이 있으면 사용
- 없으면 synthetic continuation 사용
- frontend는 둘을 같은 "resume supported" 경험으로 다룸

### Phase 2. non-Claude continuation 구현

- replay window
- summarized carry-over
- branch/thread anchor 재사용

### Phase 3. 상태 표시 정리

- "resume token 없음"을 단순 미지원으로 둘지
- "continuation supported via synthetic mode"로 재정의할지 정리

## 검증

1. 앱 재시작 후 4개 엔진 모두 대화 연속성 유지
2. branch follow-up에서 parent context 손실이 줄어듦
3. 문서와 UI가 native/synthetic 차이를 숨기지 않으면서도 UX는 동등

## 현재 상태 (2026-03-29)

### 이미 구현된 것

- **Claude**: native `--resume` token. DB `conversations.resume_token`에 저장. 엔진 불일치 시 폐기.
- **Codex/Gemini/OpenCode**: `build_normalized_prompt()`가 synthetic continuation 역할 수행:
  - 최근 6개 메시지 context replay
  - branch 시 parent context (4개 메시지) + anchor message
  - thread inheritance section
  - cross-session context
  - 앱 재시작 후에도 DB에서 메시지 로드 → context 자동 재조립

### 결론

synthetic continuation은 별도 구현이 아니라 `build_normalized_prompt()`의 기존 context 조립 로직이 그 역할을 한다.
따라서 Phase 1-2는 이미 완료된 상태이며, 추가 코드 변경 없이 문서 정리만 필요.

### 후속

- Phase 3 (상태 표시 정리): UI에서 "resume: native" vs "resume: context replay" 구분 표시 — 현재는 미구현, 필요 시 추가
- provider 비교 테이블에 continuation 방식 명시 — 완료

