# tunaFlow TracePanel Runtime-First 실행 프롬프트

- 작성자: Claude
- 작성 시각: 2026-03-29
- 카테고리: ui / trace / runtime

```md
# tunaFlow TracePanel Runtime-First 재설계

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

이번 작업 목표:
TracePanel을 "trace 이력 뷰어"에서 "런타임 상태 대시보드"로 전환하라.

참고 문서:
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/tracePanelRuntimeFirstPlan_2026-03-29.md`

먼저 확인할 파일:
- `src/components/tunaflow/context-panel/TracePanel.tsx`
- `src/stores/slices/runtimeSlice.ts` (runningThreadIds, messageQueue)

이번 단계에서 할 일:
1. Runtime 섹션을 최상단으로 이동하고 시각적으로 강화
2. Active jobs 카드를 확대하고 경과 시간 표시
3. rawq/skills 상태를 한눈에 보이게
4. Trace history를 접기/펼치기로 축소
5. 실행 중 auto-refresh 추가

비목표:
- DB 스키마 변경
- 새 backend command 추가
- trace 데이터 구조 변경

출력 형식:
### A. Opinion
### B. Files Changed
### C. Layout Changes
### D. Verification
```
