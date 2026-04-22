# tunaFlow Claude Code 스킬 활성화 가이드

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-28
- 상태: 운영 가이드

## 목적

runtime snapshot이 `~/.tunaflow/skills`에 발행된 이후에는,
Claude Code가 tunaFlow 작업을 할 때 필요한 스킬을 먼저 읽고 그 규칙에 맞춰 진행하도록 유도할 수 있다.

이 문서는 tunaFlow 작업 유형별로 어떤 스킬을 우선 활성화하는 것이 좋은지 정리한다.

## 전제

- tunaFlow의 런타임 스킬 경로는 `~/.tunaflow/skills`
- 실제 prompt 주입은 tunaFlow의 `activeSkills`를 통해 이뤄짐
- 스킬은 많이 켜는 것보다 **작업에 맞는 최소 집합**만 켜는 것이 낫다

## 핵심 원칙

1. 모든 작업에 모든 스킬을 켜지 않는다.
2. 작업 카테고리에 맞는 1~3개만 선택한다.
3. Claude에게는 "먼저 해당 스킬을 읽고, 그 규칙에 따라 구현/검토하라"고 지시한다.
4. 스킬이 현재 작업과 무관하면 켜지 않는다.

## tunaFlow에서 우선 추천하는 스킬 묶음

### 1. 프론트엔드 구현

권장:

- `anthropic-frontend-design`
- `microsoft-zustand-store-ts`
- `anthropic-webapp-testing`

사용 시점:

- React 컴포넌트 작성
- UI 레이아웃 변경
- Zustand store 변경
- 입력/패널/메시지 UI 개선

### 2. 프론트엔드 리뷰 / 품질 점검

권장:

- `microsoft-frontend-design-review`
- `anthropic-webapp-testing`

사용 시점:

- 구현 결과 리뷰
- 접근성/반응형/UI 품질 점검
- “이 화면이 충분히 좋은지 검토” 같은 요청

### 3. OpenAI / Codex / ChatGPT 연동 작업

권장:

- `openai-openai-docs`
- `openai-openai-docs-2`

사용 시점:

- OpenAI API 사용
- Codex/ChatGPT/OpenAI 모델 관련 최신 공식 가이드 확인
- OpenAI 연동 기능 변경

### 4. Claude / Anthropic 연동 작업

권장:

- `anthropic-claude-api`

사용 시점:

- Claude Code / Claude API 관련 동작 변경
- Anthropic provider 경로 수정

### 5. MCP / 툴 연동 작업

권장:

- `anthropic-mcp-builder`
- `microsoft-mcp-builder`

사용 시점:

- MCP 툴 정의
- capability registry 확장
- 새 tool integration 설계

### 6. 테스트 / 브라우저 검증

권장:

- `anthropic-webapp-testing`

사용 시점:

- 수동 검증 절차 작성
- UI 테스트 보강
- smoke test 관점 점검

## tunaFlow 기준 추천 기본 조합

### A. 일반 UI 작업

- `anthropic-frontend-design`
- `microsoft-zustand-store-ts`

### B. UI 작업 + 검증

- `anthropic-frontend-design`
- `anthropic-webapp-testing`
- `microsoft-frontend-design-review`

### C. OpenAI 기능 작업

- `openai-openai-docs`
- 필요 시 `openai-openai-docs-2`

### D. Claude/MCP 기능 작업

- `anthropic-claude-api`
- `anthropic-mcp-builder`

## 비권장 패턴

- vendor와 무관한 대형 Azure 스킬을 기본으로 켜는 것
- unrelated skill을 습관적으로 많이 켜는 것
- 프론트 작업에 문서/infra 스킬까지 한꺼번에 넣는 것

## Claude에게 줄 지시 형태

권장 문장 예시:

- "작업 시작 전에 `anthropic-frontend-design`, `microsoft-zustand-store-ts`를 먼저 읽고 그 규칙에 따라 진행하라."
- "OpenAI 연동 수정이므로 `openai-openai-docs`를 우선 참고하고, 공식 문서 기준으로 판단하라."
- "MCP 관련 작업이므로 `anthropic-mcp-builder`를 먼저 읽고 capability 설계를 맞춰라."

## 최종 판단

tunaFlow에서 Claude Code가 더 잘 작업하게 하려면,
모든 스킬을 한꺼번에 넣는 것이 아니라 **작업 유형별 추천 세트**를 프롬프트에서 명시해 주는 방식이 가장 현실적이다.
