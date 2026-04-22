# context-hub Search/Get UI Prompt

> 실행 프롬프트 — `contextHubSearchGetUiPlan_2026-03-30.md` 참조

## 지시

Settings > Runtime의 context-hub 카드를 interactive search/get UI로 확장하라.

### 구현 범위

1. 검색 input + 버튼 (hub available 시만 활성화)
2. 결과 리스트 (title, source, snippet, score)
3. 결과 클릭 시 `context_hub_get` 호출 → 문서 내용 표시
4. unavailable 시 비활성화 + 안내

### 제약

- SettingsPanel.tsx 안에서 해결 (새 파일 불필요)
- ContextPack 자동 삽입 안 함
- 기존 health 상태 표시 유지
