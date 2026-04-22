# tunaFlow 문서 IA / 거버넌스 정리 프롬프트

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-30

```md
# tunaFlow Documentation IA / Governance

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

참고 문서:
- `/Users/d9ng/privateProject/tunaFlow/docs/reference/documentationNavigationModel_2026-03-30.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/documentationIaGovernancePlan_2026-03-30.md`
- `/Users/d9ng/privateProject/tunaFlow/CLAUDE.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/reference/workingRulesForAgents.md`

우선 짧게 의견부터 말하라:
- 지금 문서 구조의 가장 큰 문제를 양이 아니라 어떤 기준 부재로 볼 수 있는지
- 어떤 index부터 먼저 정리하는 것이 효과가 큰지
- 이번 단계에서 대규모 문서 재작성으로 새면 안 되는 이유

그 다음 실제 작업을 진행하라.

## 목표

1. `docs/` 탐색 규칙을 더 명확히 만든다
2. 에이전트가 읽어야 할 문서 순서를 index 중심으로 정리한다
3. 현재 문서와 아카이브 문서를 더 쉽게 구분하게 만든다

## 권장 방향

- 전체 문서를 다 손대지 말고
  `index.md`, 상위 기준 문서, 상태 라벨부터 정리한다
- 역할이 다른 문서군(`reference / plans / prompts / how-to / archive`)을 분명히 설명한다
- 새 세션 에이전트가 3~5개 문서만 읽고도 시작할 수 있게 만든다

## 중요

- 이번 단계는 문서 IA 정리다
- 코드 변경 금지
- 폴더 대규모 이동 금지
- 전체 문서 전면 재작성 금지

## 수정 대상 후보

- `docs/plans/index.md`
- `docs/prompts/index.md`
- `docs/reference/index.md`
- 필요 시 `CLAUDE.md`

## 검증

- 새 세션 에이전트가 어디부터 읽어야 하는지 더 명확해졌는지 설명
- 각 문서군 역할이 index에서 읽히는지 설명
- 아카이브/초안/현재 문서 구분이 이전보다 쉬워졌는지 설명

## 결과 보고 형식

### A. Opinion
### B. Decision
### C. Files Changed
### D. Navigation Model
### E. Verification
### F. Next Recommendation
```
