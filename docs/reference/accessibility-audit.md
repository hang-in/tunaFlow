---
title: Accessibility Audit (Phase 4 Finding 4-1)
updated_at: 2026-04-20
canonical: true
status: active
owner: tunaFlow-core
---

# Accessibility Audit — Beta Readiness

tunaFlow 데스크탑 앱의 접근성 1차 개선 결과와 남은 작업 목록을 정리한다. 전체 WCAG 2.1 AA 준수를 목표로 하되, 베타 단계에서는 **키보드 네비게이션 + 스크린리더 랜드마크 + 포커스 시각화**만 우선 적용한다.

## 1. 이번 패스(PR Phase 4)에서 적용된 항목

### 1-1. Focus-visible 표준화

- 파일: `src/index.css`
- `:where(button, a, [role="button"], [tabindex], input, select, textarea, summary, [contenteditable="true"]):focus-visible`에 일괄 적용
- outline: `2px solid var(--color-primary)`, offset `2px`, 반경 `4px`
- 기존에 Tailwind `ring-*` 클래스를 사용하는 컴포넌트는 그대로 유지된다 (더 특이도 높은 규칙 우선)
- `:focus` 가 아니라 `:focus-visible` 을 쓰므로 마우스 클릭 시에는 outline 이 뜨지 않는다

### 1-2. ARIA Landmarks

| 영역 | 컴포넌트 | role | aria-label |
|------|----------|------|------------|
| 사이드바 | `Sidebar.tsx` | `navigation` | `프로젝트 사이드바` |
| 메인 대화 | `CenterPanel.tsx` | `main` | `메인 대화 영역` |
| 브랜치 드로어 | `BranchThreadPanel.tsx` | `complementary` | `브랜치/라운드테이블 패널` |
| 메타 에이전트 | `MetaFloatingChat.tsx` | `dialog` | `메타 에이전트 창` (aria-modal=false — 드래그 가능한 floating) |

이제 스크린리더가 4개의 명시적 랜드마크로 페이지 구조를 안내할 수 있다.

### 1-3. 기존부터 잘 구현된 영역

- `MessageItem.tsx` — 이미지 alt, 버튼 title 등 대부분의 툴팁 제공
- `SettingsPanel.tsx` — 각 input 에 label 연결
- `AgentAvatar`, `Tooltip` — title 속성 일관되게 사용

## 2. 남은 작업 (Post-Beta)

### 2-1. 추가 landmark 후보

- [ ] `RuntimeStatusBar.tsx` → `role="status"` (aria-live 검토)
- [ ] `TitleBar.tsx` → `role="banner"`
- [ ] `InsightPanel`, `PlansPanel` 등 `role="region"` + label

### 2-2. 스크린리더 라이브 리전

- 스트리밍 응답 영역을 `aria-live="polite"` 로 감싸면 긴 응답이 완료될 때마다 리더가 안내한다
- 다만 동시에 여러 에이전트가 돌 때 speech 간섭이 있을 수 있어 RT 모드는 제외 필요

### 2-3. 키보드 단축키 공식화

현재 `CommandPalette` (Cmd+K) 와 `TextareaInput` 의 Enter/Shift+Enter 외에 전체 단축키 맵이 문서화되지 않음. 4-2 HelpPanel 에 표로 수록 예정.

### 2-4. 대비/컬러

- 현재 다크 테마의 `--prose-muted`, `--prose-faint` 가 작은 폰트에서 WCAG AA 4.5:1 기준을 일부 구간에서 미달할 가능성
- Lighthouse Accessibility 점수 70+ 달성 후, 90+ 를 위해서는 별도 대비 튜닝 필요

## 3. 수동 검증 체크리스트

Phase 4 PR 머지 전에 한 번씩 수동으로 확인한다.

### 3-1. 키보드 네비게이션

- [ ] `Tab` 으로 사이드바 → 메인 입력창 → 탭 메뉴까지 순환 이동 가능
- [ ] 각 포커스 지점에 2px 파란 outline 이 보임
- [ ] 마우스 클릭 시에는 outline 이 뜨지 않음
- [ ] `Cmd+K` 로 CommandPalette 열림, `Esc` 로 닫힘
- [ ] `Esc` 로 드로어, 모달 닫힘

### 3-2. 스크린리더 (VoiceOver / NVDA)

- [ ] Rotor(VO+U) 에서 `navigation`, `main`, `complementary`, `dialog` 4개 랜드마크 표시
- [ ] 각 랜드마크 label 이 한국어로 제대로 읽힘
- [ ] 메시지 리스트에서 `H` 키로 헤딩 점프 가능 (MessageItem 의 agent 이름 + 타임스탬프)

### 3-3. Lighthouse

- Chrome DevTools → Lighthouse → Accessibility 감사
- **목표: 70+ (베타), 90+ (정식 릴리즈)**
- 현재 알려진 차감 사유: 일부 버튼 aria-label 누락, 저대비 텍스트 일부

## 4. 참고 문서

- WCAG 2.1 AA: https://www.w3.org/WAI/WCAG21/quickref/
- MDN Landmark roles: https://developer.mozilla.org/en-US/docs/Web/Accessibility/ARIA/Roles#3._landmark_roles
- Tauri webview 에서도 일반 웹 접근성 API 가 그대로 동작한다
