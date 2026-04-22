# tunaFlow Chat Markdown / Codeblock Upgrade Plan

- 작성자: OpenAI Codex
- 작성 시각: 2026-03-29
- 상태: 제안

## 목표

`tunaFlow`의 Markdown 렌더링을 "지원됨" 수준에서
"코드 대화에 최적화된 채팅 UI" 수준으로 올린다.

## 현재 상태

- `react-markdown + remark-gfm` 사용
- Prism syntax highlighting
- copy button
- language badge

## 부족한 점

1. 긴 코드블록 접기/펼치기 없음
2. 코드블록 헤더 정보 밀도 낮음
3. copy feedback 미세함
4. 표/blockquote/link 외 커스텀 상호작용 부족

## 단계

### Step 1. Code header 강화

- 언어
- 줄 수
- copy 상태 표시

### Step 2. Collapse / expand

- 긴 코드블록 자동 접기
- overlay gradient
- 펼치기/접기 버튼

### Step 3. Highlight quality 점검

- Prism 유지 vs Shiki 전환 판단
- 일단 대규모 전환보다 UX 개선 우선

### Step 4. Markdown typography tuning

- prose spacing
- heading/list/table rhythm
- compact variant와 일반 variant 차이 재정리

## 권장 원칙

- 첫 단계에서 Shiki까지 한 번에 밀지 않아도 된다
- 먼저 현재 컴포넌트 구조 안에서 UX를 올린다
- 렌더 비용이 커지지 않도록 streaming과 완료 상태를 분리해 유지한다

## 검증

1. 긴 코드 응답에서 읽기 경험이 개선됨
2. collapse/expand 동작 안정적
3. 기존 streaming UX와 충돌 없음

