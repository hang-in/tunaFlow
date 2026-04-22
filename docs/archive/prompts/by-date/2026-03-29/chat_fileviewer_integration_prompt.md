# tunaFlow Chat FileViewer Integration 실행 프롬프트

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-29

```md
# tunaFlow Chat FileViewer Integration

프로젝트:
- `/Users/d9ng/privateProject/tunaFlow`

참고 문서:
- `/Users/d9ng/privateProject/tunaFlow/docs/reference/chatUiVsTunaChatGapReview_2026-03-29.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/chatUiParityWithTunaChatPlan.md`
- `/Users/d9ng/privateProject/tunaFlow/docs/plans/chatFileViewerIntegrationPlan_2026-03-29.md`

먼저 의견을 짧게 말하라:
- tunaFlow에서 파일 경로 클릭/preview가 왜 중요한지
- 이번 단계에서 허용할 파일 타입 범위를 어떻게 잡을지

그 다음 실제 작업을 진행하라.

목표:
- 메시지 안 파일 경로 클릭
- markdown/text/code 파일 preview
- 상대경로 resolve

중요:
- 외부 링크와 로컬 파일 링크를 구분할 것
- 보안상 현재 프로젝트 기준 resolve를 우선할 것
- 편집기 기능까지 확장하지 말 것
- 최소한의 viewer 경험만 제공할 것

우선 수정 대상:
- `src/components/tunaflow/chat/MarkdownComponents.tsx`
- 새 FileViewer 관련 컴포넌트/스토어
- `src/components/tunaflow/ChatPanel.tsx` 또는 적절한 상위 컴포넌트

이번 단계에서 하지 말 것:
- 코드블록 대규모 재작업
- virtualization
- command palette

검증:
- 파일 경로 클릭 시 viewer open
- markdown preview
- plain text/code preview

출력 형식:
### A. Opinion
### B. Decision
### C. Files Changed
### D. Supported Path Types
### E. Verification
### F. Next Recommendation
```

