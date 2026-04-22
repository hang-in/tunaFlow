# tunaFlow Sidebar Workspace Hierarchy 실행 프롬프트

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-29

```md
# tunaFlow Sidebar Workspace Hierarchy

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

참고 문서:
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/sidebarWorkspaceHierarchyPlan_2026-03-29.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/reference/codexProjectReference_2026-03-29.md`

우선 짧게 의견부터 말하라:
- 현재 Sidebar에서 프로젝트와 채팅 위계가 왜 헷갈리는지
- 이번 단계에서 반드시 고쳐야 할 점
- 이번에 같이 묶지 말아야 할 점

그 다음 실제 작업을 진행하라.

## 목표 구조

```text
[📁 Workspace ▼]
  프로젝트 목록
  + Add project

CHATS
  Main
    b1
    Test
    원탁회의...

ARTIFACTS
FILES
```

## 목표

1. 프로젝트 선택을 Sidebar 최상단 드롭다운으로 분리
2. `+ Add project`를 드롭다운 내부 액션으로 이동
3. Chats를 "선택된 프로젝트의 하위 트리"처럼 보이게 정리

## 수정 대상 후보

- `src/components/tunaflow/Sidebar.tsx`
- `src/components/tunaflow/sidebar/ChatsSection.tsx`
- 필요 시 새 sidebar 컴포넌트

## 중요

- 이번 세션은 Sidebar 위계 정리 1차다
- 전체 Sidebar 전면 재설계로 범위를 넓히지 말 것
- Skills의 최종 위치 문제는 이번 단계에서 해결하지 말 것
- Artifacts / Files 트리는 이번 단계에서 최소 수정만 할 것

## 검증

- `tsc --noEmit`
- 선택된 프로젝트가 드롭다운으로 명확히 보이는지
- Add project가 드롭다운 내부에 있는지
- Chats가 프로젝트 하위 트리처럼 보이는지

## 결과 보고 형식

### A. Opinion
### B. Decision
### C. Files Changed
### D. Hierarchy Changes
### E. Verification
### F. Next Recommendation
```

