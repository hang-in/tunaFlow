# tunaFlow TracePanel Runtime Metrics Cards 실행 프롬프트

- 작성자: Claude
- 작성 시각: 2026-03-29
- 카테고리: ui / trace / metrics

```md
# tunaFlow TracePanel Runtime Metrics Cards

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

이번 작업 목표:
TracePanel Phase 2 — 실시간 경과 시간 카운터 + 엔진별 aggregate 분리.

참고 문서:
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/tracePanelRuntimeFirstPlan_2026-03-29.md`

먼저 확인할 파일:
- `src/components/tunaflow/context-panel/TracePanel.tsx`

이번 단계에서 할 일:
1. Active job 카드에 경과 시간 실시간 카운터 (1초 tick)
2. Aggregate stats 아래에 엔진별 breakdown (다중 엔진 사용 시)
3. 단일 엔진만 사용하면 breakdown 미표시

비목표:
- span trace_id 그룹핑
- 새 backend command
- DB 변경

출력 형식:
### A. Opinion
### B. Files Changed
### C. Verification
```
