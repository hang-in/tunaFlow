# tunaFlow Claude 스킬 활성화 실행 프롬프트

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-28
- 카테고리: claude / skills / activation

```md
# tunaFlow 작업 전 스킬 로딩 지시

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

중요:
- 작업을 시작하기 전에 현재 작업에 필요한 skill을 먼저 읽고 그 규칙에 따라 진행하라.
- 모든 skill을 다 읽지 말고, 아래 추천 세트 중 현재 작업과 맞는 최소 집합만 사용하라.
- runtime skill snapshot 경로는 `~/.tunaflow/skills`다.

## 작업 유형별 우선 skill

### 프론트엔드 구현
- `anthropic-frontend-design`
- `microsoft-zustand-store-ts`
- 필요 시 `anthropic-webapp-testing`

### 프론트엔드 리뷰 / 품질 점검
- `microsoft-frontend-design-review`
- `anthropic-webapp-testing`

### OpenAI / Codex / ChatGPT 연동
- `openai-openai-docs`
- 필요 시 `openai-openai-docs-2`

### Claude / Anthropic 관련 구현
- `anthropic-claude-api`

### MCP / capability / tool integration
- `anthropic-mcp-builder`
- 필요 시 `microsoft-mcp-builder`

## 작업 절차

1. 현재 작업 유형을 먼저 분류한다.
2. 그 작업에 맞는 skill 1~3개만 선택한다.
3. 선택한 skill의 `SKILL.md`를 먼저 읽는다.
4. 그 규칙을 따른 상태로 분석/구현/검토를 진행한다.
5. 최종 보고에서 어떤 skill을 참고했는지 짧게 적는다.

## 기본 규칙

- unrelated skill은 켜지 말 것
- Azure 전용 skill은 Azure 작업이 아닐 때 쓰지 말 것
- 프론트 작업이면 UI/design/testing 계열만 우선
- API/provider 작업이면 해당 vendor docs skill만 우선

## tunaFlow에서 자주 쓰는 추천 조합

### 일반 UI 작업
- `anthropic-frontend-design`
- `microsoft-zustand-store-ts`

### UI 작업 + 검증
- `anthropic-frontend-design`
- `anthropic-webapp-testing`
- `microsoft-frontend-design-review`

### OpenAI 기능 작업
- `openai-openai-docs`

### Claude/MCP 기능 작업
- `anthropic-claude-api`
- `anthropic-mcp-builder`

이 규칙에 따라 필요한 skill을 먼저 읽고 작업을 진행하라.
```
