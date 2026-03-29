# tunaFlow 문서 파일명 규칙 적용 프롬프트

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-30

```md
# tunaFlow Document Naming Rule

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

참고 문서:
- `/Users/d9ng/privateProject/tunaFlow/docs/reference/documentNamingRule_2026-03-30.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/reference/documentVersioningPolicy_2026-03-30.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/reference/documentMetadataSchema_2026-03-30.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/reference/documentationNavigationModel_2026-03-30.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/documentationIaGovernancePlan_2026-03-30.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/documentMetadataAdoptionPlan_2026-03-30.md`

우선 짧게 의견부터 말하라:
- 지금 tunaFlow 문서 파일명이 길어지는 가장 큰 이유가 무엇인지
- 파일명, 문서 title, 메타, index의 역할을 어떻게 분리해야 하는지
- 이번 단계에서 무리한 대규모 rename으로 새면 안 되는 이유

그 다음 실제 작업을 진행하라.

## 목표

1. 문서 파일명 규칙을 기존 문서 거버넌스에 통합한다
2. reference / plan / prompt / brainstorm 문서의 파일명 원칙을 더 명확히 만든다
3. 파일명은 짧게, title/metas/index는 설명적으로 가는 방향을 정리한다

## 권장 방향

- reference는 안정 이름 우선
- plan은 날짜 기반 유지 가능
- prompt는 날짜 폴더 중심
- brainstorm/review/reference-lite는 성격 단어를 파일명에 명확히 포함
- 약자는 표준 약자표가 있을 때만 사용

## 중요

- 이번 단계는 규칙 정리다
- 코드 변경 금지
- 문서 전체 대규모 rename 금지
- 현재 작동 중인 링크를 깨는 이동/개명 금지

## 수정 대상 후보

- `docs/reference/index.md`
- `docs/plans/index.md`
- `docs/prompts/index.md`
- 필요 시 상위 거버넌스 문서

## 검증

- 파일명 규칙이 이전보다 명확해졌는지 설명
- 파일명과 title/metas/index의 역할 분담이 읽히는지 설명
- 이후 문서 생성 시 어떤 규칙을 따를지 사람이 바로 알 수 있는지 설명

## 결과 보고 형식

### A. Opinion
### B. Decision
### C. Files Changed
### D. Naming Model
### E. Verification
### F. Next Recommendation
```
