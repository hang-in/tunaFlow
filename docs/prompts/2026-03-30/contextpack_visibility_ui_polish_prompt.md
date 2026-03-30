# ContextPack Visibility UI Polish Prompt

> 실행 프롬프트 — `contextPackVisibilityUiPolishPlan_2026-03-30.md` 참조

## 지시

4-engine context metadata가 trace_log에 저장되고 있다. 이 정보를 사용자에게 읽기 쉽게 보여주는 UI polish를 수행하라.

### 범위

1. TracePanel 히스토리 카드: context 섹션 가독성 대폭 개선 (폰트 크기, pill badge, truncated 경고)
2. TracePanel aggregate: 최근 context mode 한 줄 요약 추가
3. RuntimeStatusBar: 마지막 context mode 약어 표시

### 제약

- context budget slider, retrieval 구조 변경 범위 밖
- 새 파일 생성 불가 — 기존 TracePanel.tsx, RuntimeStatusBar.tsx만 수정
- 스타일은 기존 Linear lch 색상 시스템 유지
