# tunaFlow 문서 버전관리 규칙 적용 프롬프트

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-30

```md
# tunaFlow Document Versioning Policy

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

참고 문서:
- `/Users/d9ng/privateProject/tunaFlow/docs/reference/documentVersioningPolicy_2026-03-30.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/reference/documentMetadataSchema_2026-03-30.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/reference/documentationNavigationModel_2026-03-30.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/documentMetadataAdoptionPlan_2026-03-30.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/documentationIaGovernancePlan_2026-03-30.md`

우선 짧게 의견부터 말하라:
- 왜 reference와 plan/prompt를 같은 방식으로 버전관리하면 안 되는지
- 지금 tunaFlow 문서 구조에서 가장 먼저 적용해야 하는 최소 버전관리 규칙이 무엇인지
- 이번 단계에서 전 문서를 한 번에 갈아엎으려 하면 왜 실패하는지

그 다음 실제 작업을 진행하라.

## 목표

1. tunaFlow 문서 버전관리 규칙을 current 문서 구조에 맞게 적용한다
2. reference / plan / prompt / brainstorm 문서의 관리 규칙을 더 명확히 만든다
3. 에이전트가 새 문서를 만들지 기존 문서를 갱신할지 판단하기 쉬운 구조를 만든다

## 권장 방향

- reference는 가능한 한 기존 파일 갱신
- plan/prompt는 작업 단위 새 파일 허용
- brainstorm/reference-lite 문서는 current SSOT가 아님을 명시
- index 중심으로 현재성/관계를 보강

## 중요

- 이번 단계는 문서 거버넌스다
- 코드 변경 금지
- 대규모 파일 이동 금지
- 모든 문서 frontmatter 일괄 적용 금지

## 수정 대상 후보

- `CLAUDE.md`
- `docs/reference/index.md`
- `docs/plans/index.md`
- `docs/prompts/index.md`
- 일부 핵심 reference / plan 문서 상단 상태/관계 정리

## 검증

- reference와 plan/prompt의 관리 규칙이 이전보다 분명해졌는지 설명
- 새 세션 에이전트가 문서 current 여부를 더 쉽게 판단하는지 설명
- 새 문서 생성 vs 기존 문서 갱신 기준이 읽히는지 설명

## 결과 보고 형식

### A. Opinion
### B. Decision
### C. Files Changed
### D. Versioning Model
### E. Verification
### F. Next Recommendation
```
