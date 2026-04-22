# tunaFlow Chat Virtualization 실행 프롬프트

- 작성자: Claude
- 작성 시각: 2026-03-29
- 카테고리: chat / performance / virtualization

```md
# tunaFlow Chat Virtualization 구현

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

이번 작업 목표:
긴 대화(200+ 메시지)에서 렌더링 성능과 스크롤 안정성을 확보하라.

중요:
- 작업 시작 전에 Opinion을 먼저 말하라:
  1. 현재 성능 병목이 실제로 존재하는지
  2. react-virtuoso vs 직접 구현 판단
  3. 기존 grouped/streaming/auto-scroll 동작과의 호환 방안

참고 문서:
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/chatVirtualizationPlan.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/chatUiParityWithTunaChatPlan.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/reference/chatUiVsTunaChatGapReview_2026-03-29.md`

먼저 확인할 파일:
- `/Users/d9ng/privateProject/tunaFlow/src/components/tunaflow/ChatPanel.tsx`
- `/Users/d9ng/privateProject/tunaFlow/src/components/tunaflow/MessageItem.tsx`

이번 단계에서 할 일:
1. ChatPanel의 메시지 영역을 virtualized 렌더링으로 전환
2. auto-scroll 동작 유지 (streaming 중 하단 고정)
3. grouped 메시지 호환성 유지
4. sticky input 레이아웃 유지

비목표:
- DB 페이징 / lazy loading
- 메시지 구조 변경
- 마크다운 렌더링 변경
- FileViewer 변경

완료 기준:
1. 500개 메시지에서 스크롤 버벅임 없음
2. streaming 중 auto-scroll 정상 동작
3. grouped 메시지 표시 유지
4. 기존 기능(branch badge, hover actions, RT view) 정상 동작

출력 형식:
### A. Opinion
### B. Decision
### C. Files Changed
### D. Performance Comparison
### E. Verification
### F. Residual Risks
```
