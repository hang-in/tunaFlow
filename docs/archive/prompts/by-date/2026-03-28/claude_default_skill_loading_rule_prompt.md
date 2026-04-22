# tunaFlow Claude 기본 skill loading rule 프롬프트

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-28
- 카테고리: claude / workflow / skills

```md
# tunaFlow Claude 기본 작업 규칙: 필요한 skill 먼저 읽기

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

이 프롬프트는 Claude에게 기본 작업 규칙을 주는 용도다.

핵심 규칙:
- 작업을 시작하기 전에 현재 작업 유형에 맞는 skill 1~3개를 먼저 고른다
- 선택한 skill의 `SKILL.md`를 먼저 읽고 그 규칙에 따라 분석/구현/검토를 진행한다
- 관련 없는 skill은 읽지 않는다
- 모든 skill을 다 읽으려 하지 않는다

runtime skill snapshot:
- `~/.tunaflow/skills`

작업 유형별 기본 추천:

### 프론트엔드 구현
- `anthropic-frontend-design`
- `microsoft-zustand-store-ts`
- 필요 시 `anthropic-webapp-testing`

### 프론트엔드 리뷰
- `microsoft-frontend-design-review`
- `anthropic-webapp-testing`

### OpenAI / Codex / ChatGPT 관련
- `openai-openai-docs`
- 필요 시 `openai-openai-docs-2`

### Claude / Anthropic 관련
- `anthropic-claude-api`

### MCP / capability / tool integration
- `anthropic-mcp-builder`
- 필요 시 `microsoft-mcp-builder`

작업 절차:
1. 작업 유형 분류
2. 필요한 skill 1~3개 선택
3. 해당 `SKILL.md` 먼저 읽기
4. 그 규칙에 따라 작업
5. 최종 보고에 어떤 skill을 참고했는지 짧게 적기

금지:
- unrelated Azure skill을 습관적으로 켜는 것
- 모든 vendor skill을 다 읽는 것
- skill 없이 바로 구현을 시작하는 것

출력 형식:
### A. Task Type
### B. Skills Selected
### C. Why These Skills
### D. Work Performed
### E. Verification
```
