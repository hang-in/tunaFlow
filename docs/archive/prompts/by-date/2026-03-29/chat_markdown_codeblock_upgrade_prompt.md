# tunaFlow Chat Markdown / Codeblock Upgrade 실행 프롬프트

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-29

```md
# tunaFlow Chat Markdown / Codeblock Upgrade

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

참고 문서:
- `/Users/d9ng/privateProject/tunaFlow/docs/reference/chatUiVsTunaChatGapReview_2026-03-29.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/chatUiParityWithTunaChatPlan.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/chatMarkdownCodeblockUpgradePlan_2026-03-29.md`

먼저 의견을 짧게 말하라:
- 현재 tunaFlow 채팅 Markdown/codeblock UX에서 가장 부족한 점 3개
- 이번 단계에서 꼭 같이 묶어야 할 것과 묶지 말아야 할 것

그 다음 실제 작업을 진행하라.

목표:
- 코드블록 헤더 강화
- 긴 코드블록 collapse/expand
- copy feedback 개선
- markdown typography 미세 조정

중요:
- tunaChat를 그대로 복붙하지 말 것
- 기존 branch / follow-up / streaming UX를 깨지 말 것
- 대규모 unrelated refactor 금지
- 필요 이상으로 새 라이브러리 늘리지 말 것

우선 수정 대상:
- `src/components/tunaflow/chat/MarkdownComponents.tsx`
- 필요 시 `src/components/tunaflow/MessageItem.tsx`

이번 단계에서 하지 말 것:
- FileViewer 도입
- virtualization
- 입력 영역 quick actions

검증:
- 긴 코드블록 렌더링 확인
- collapse/expand 동작
- copy feedback

출력 형식:
### A. Opinion
### B. Decision
### C. Files Changed
### D. UX Improvements
### E. Verification
### F. Next Recommendation
```

