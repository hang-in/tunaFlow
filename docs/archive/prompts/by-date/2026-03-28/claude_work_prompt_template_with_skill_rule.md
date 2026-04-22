# tunaFlow Claude 작업 템플릿 프롬프트

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-28
- 카테고리: claude / workflow / template

```md
# tunaFlow 작업 실행 템플릿

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

작업 목표:
- [여기에 이번 작업 목표를 적는다]

중요:
- 작업을 시작하기 전에 현재 작업 유형에 맞는 skill 1~3개를 `~/.tunaflow/skills/`에서 먼저 읽고 그 규칙에 따라 진행하라.
- 모든 skill을 다 읽지 말고, 현재 작업에 맞는 최소 집합만 선택하라.
- 최종 보고에는 어떤 skill을 참고했는지 짧게 적어라.

## 작업 유형별 기본 추천 skill

### 프론트엔드 구현
- `anthropic-frontend-design`
- `microsoft-zustand-store-ts`
- 필요 시 `anthropic-webapp-testing`

### 프론트엔드 리뷰
- `microsoft-frontend-design-review`
- `anthropic-webapp-testing`

### OpenAI/Codex/ChatGPT 관련
- `openai-openai-docs`
- 필요 시 `openai-openai-docs-2`

### Claude/Anthropic 관련
- `anthropic-claude-api`

### MCP/capability/tool integration
- `anthropic-mcp-builder`
- 필요 시 `microsoft-mcp-builder`

## 작업 절차

1. 현재 작업 유형을 먼저 분류한다.
2. 해당 작업에 맞는 skill 1~3개를 선택한다.
3. 선택한 skill의 `SKILL.md`를 먼저 읽는다.
4. 그 규칙을 반영해 코드/문서/검토 작업을 수행한다.
5. 검증 결과를 정리한다.

## 작업 범위

- 요청 범위만 수정
- 무관한 리팩토링 금지
- 확정되지 않은 부분은 TODO 또는 open question으로 남길 것

## 출력 형식

### A. Task Type
### B. Skills Selected
### C. Work Performed
### D. Verification
### E. Remaining Risks / Next Steps
```
