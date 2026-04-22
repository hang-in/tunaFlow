# tunaFlow Artifacts 메인 탭 + Memo 보조 UX 프롬프트

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-29

```md
# tunaFlow Artifacts Main Tab + Memo Assist UX

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

참고 문서:
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/artifactsAsMainTabAndMemoAssistPlan_2026-03-29.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/reference/codexProjectReference_2026-03-29.md`

우선 짧게 의견부터 말하라:
- 왜 Artifacts는 메인 탭이어야 하고 Memo는 보조 UX여야 하는지
- 이번 단계에서 반드시 고쳐야 할 것
- 이번에 같이 묶지 말아야 할 것

그 다음 실제 작업을 진행하라.

## 목표

1. `Artifacts`를 메인 탭 구조로 승격
2. `Memo`를 탭이 아닌 보조 진입점으로 이동
3. `Memo`와 `Artifact`의 역할 차이가 UI에서 읽히게 만들기

## 권장 방향

- 메인 탭: `Chat / Plan / Artifacts / Review / Test`
- Memo: 작은 아이콘 진입 + list/popover/drawer

## 수정 대상 후보

- `src/components/tunaflow/CenterPanel.tsx`
- `src/components/tunaflow/ChatPanel.tsx`
- `src/components/tunaflow/Sidebar.tsx`
- `src/components/tunaflow/context-panel/ArtifactsPanel.tsx`
- `src/components/tunaflow/context-panel/MemosPanel.tsx`
- 필요 시 새 Memo trigger/list 컴포넌트

## 중요

- 이번 단계는 정보 구조와 진입 방식 정리다
- Artifact 자동 승격은 이번에 하지 말 것
- Skills 최종 위치 문제와 섞지 말 것
- Trace 구조 재작업과 섞지 말 것

## 검증

- `tsc --noEmit`
- Artifacts가 메인 탭에 있는지
- Memo가 탭이 아니라 보조 아이콘/리스트로 들어가는지
- UI만 봐도 Memo와 Artifact의 역할 차이가 읽히는지

## 결과 보고 형식

### A. Opinion
### B. Decision
### C. Files Changed
### D. UX Changes
### E. Verification
### F. Next Recommendation
```

