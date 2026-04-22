# tunaFlow Artifacts 탭 사용성 개선 프롬프트

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-30

```md
# tunaFlow Artifacts Tab Usability Pass

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

참고 문서:
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/artifactsTabUsabilityPlan_2026-03-30.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/artifactManualPromotionMvpPlan_2026-03-30.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/artifactsAsMainTabAndMemoAssistPlan_2026-03-29.md`

우선 짧게 의견부터 말하라:
- 지금 단계에서 검색보다 filter/sort/detail이 먼저 맞는 이유
- Artifacts 탭을 문서 허브처럼 느끼게 하려면 어떤 최소 UX가 필요한지
- 이번 단계에서 자동 승격이나 artifact-plan 연결로 새면 안 되는 이유

그 다음 실제 작업을 진행하라.

## 목표

1. Artifacts 탭에 빠른 필터 추가
2. 기본 정렬을 개선
3. artifact 상세 보기(expand 또는 modal) 추가
4. artifact의 type/status/created-at 같은 기본 메타를 더 잘 보이게 한다

## 권장 방향

- 우선 local state 기반 filter/sort
- 기본 정렬은 최신순
- 상세 보기는 가볍게
- 긴 content는 카드에서 truncate, 상세 보기에서 전체 노출

## 수정 대상 후보

- `src/components/tunaflow/context-panel/ArtifactsPanel.tsx`
- 필요 시 detail modal 컴포넌트

## 중요

- 이번 단계는 usability pass다
- full-text search 금지
- 자동 승격 금지
- artifact-plan 자동 변환 금지
- export/publish 금지

## 검증

- `tsc --noEmit`
- Artifacts 탭에서 filter/sort/detail이 동작하는지
- 저장된 artifact를 이전보다 더 쉽게 다시 찾을 수 있는지 설명

## 결과 보고 형식

### A. Opinion
### B. Decision
### C. Files Changed
### D. UX Model
### E. Verification
### F. Next Recommendation
```
