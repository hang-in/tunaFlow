# tunaFlow Artifact 수동 승격 MVP 프롬프트

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-30

```md
# tunaFlow Artifact Manual Promotion MVP

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

참고 문서:
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/artifactManualPromotionMvpPlan_2026-03-30.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/artifactsAsMainTabAndMemoAssistPlan_2026-03-29.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/reference/implementationStatus.md`

우선 짧게 의견부터 말하라:
- 지금 단계에서 자동 승격보다 수동 승격이 먼저 맞는 이유
- 메시지 액션에서 artifact 저장을 어떤 UX로 여는 게 가장 자연스러운지
- 이번 단계에서 review/test 자동화나 artifact editor 확장으로 새면 안 되는 이유

그 다음 실제 작업을 진행하라.

## 목표

1. assistant 메시지에서 `Save as Artifact` 액션 제공
2. title / type / content를 최소한으로 조정할 수 있는 생성 UI 제공
3. 저장 후 Artifacts 탭에서 바로 확인 가능하게 연결

## 권장 방향

- assistant 메시지 action row에 secondary action으로 추가
- compact modal 중심
- content 기본값은 메시지 본문
- type은 작은 문서형 집합으로 시작

## 수정 대상 후보

- `src/components/tunaflow/message/MessageActions.tsx`
- artifact 생성 modal/sheet 컴포넌트
- `assetSlice.createArtifact`
- Artifacts 탭 반영 경로

## 중요

- 이번 단계는 수동 승격 MVP다
- 자동 승격 금지
- review/test artifact 자동 생성 금지
- artifact 대형 편집기 도입 금지
- 파일 export 금지

## 검증

- `tsc --noEmit`
- assistant 메시지에서 artifact 저장 흐름이 동작하는지
- 저장 후 Artifacts 탭에서 확인 가능한지
- 어떤 type 집합으로 시작했는지 설명

## 결과 보고 형식

### A. Opinion
### B. Decision
### C. Files Changed
### D. Promotion Flow
### E. Verification
### F. Next Recommendation
```
