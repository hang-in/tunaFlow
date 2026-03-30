# Dependency Migration Phase 4 — react-virtuoso + cmdk

프로젝트: `/Users/d9ng/privateProject/tunaFlow`
모든 응답과 보고는 한국어로 작성하라.

---

## 사전 읽기 (필수)

1. `CLAUDE.md` — 프로젝트 전체 구조, 현재 상태
2. `docs/plans/dependencyAdoptionPlan.md` — 의존성 도입 계획 (Phase 4-3, 4-4)
3. `src/components/tunaflow/ChatPanel.tsx` — 현재 메시지 렌더링 (virtuoso 마이그레이션 대상)
4. `src/components/tunaflow/AppShell.tsx` — 커맨드 팔레트 마운트 위치

---

## 배경

Phase 1-3(의존성 설치)과 Phase 4-1(clipboard), 4-2(sonner)는 완료됨.
react-virtuoso, cmdk는 이미 `npm install` 됨 — 코드 마이그레이션만 남음.

---

## 작업 1: Phase 4-3 — react-virtuoso

### 현재 상태
`ChatPanel.tsx`에서 `messages.map((msg) => <MessageItem ... />)` 로 전체 렌더.
200+ 메시지 시 성능 저하.

### 목표
`<Virtuoso>` 컴포넌트로 교체. 가상 스크롤 + auto-scroll.

### 변경 범위
- `ChatPanel.tsx` 1개 파일만 수정
- `messages.map(...)` → `<Virtuoso data={messages} itemContent={...} />`
- `followOutput="smooth"` 으로 새 메시지 자동 스크롤
- 기존 `messagesEndRef` auto-scroll 제거

### 주의사항
- 메시지 그룹핑(`grouped` prop)이 작동해야 함 — `itemContent` 콜백에서 이전 메시지 참조
- 스트리밍 중 메시지 높이 변화 — Virtuoso가 자동 처리하는지 확인
- 검색 결과 스크롤(`scrollToMessage`) 기능이 있다면 Virtuoso의 `scrollToIndex` 로 대체

### 검증
```bash
npx tsc --noEmit
npx vitest run
# tauri dev → 긴 대화 스크롤 테스트
```

### rollback
`<Virtuoso>` → `messages.map()` 복원

---

## 작업 2: Phase 4-4 — cmdk 커맨드 팔레트

### 현재 상태
에이전트 전환, 프로젝트 전환, 대화 이동이 모두 사이드바 클릭 or 드롭다운.

### 목표
`Cmd+K` (Mac) / `Ctrl+K` (Windows) → 커맨드 팔레트.

### 변경 범위
- `CommandPalette.tsx` **신규 생성** (기존 코드 수정 최소)
- `AppShell.tsx`: `useEffect`로 `Cmd+K` 바인딩 + `<CommandPalette />` 렌더
- 액션 목록:
  - 프로젝트 전환 (`selectProject`)
  - 대화 전환 (`selectConversation`)
  - 엔진 전환 (`setEngine` via store)
  - Settings 열기

### 주의사항
- 기존 UI에 영향 없음 (새 컴포넌트 추가만)
- cmdk는 headless — 스타일링은 tailwind로 직접
- 팔레트 열림/닫힘 상태만 관리

### 검증
```bash
npx tsc --noEmit
npx vitest run
# tauri dev → Cmd+K 동작 확인
```

### rollback
`CommandPalette.tsx` 삭제 + `AppShell.tsx` 바인딩 제거

---

## 절대 하지 말 것

1. Phase 5 (tokio) 작업 하지 말 것 — 별도 세션
2. 기존 동작을 깨뜨리지 말 것 — 각 step 후 빌드+테스트 확인
3. 한 번에 두 작업 동시 수정 금지 — 4-3 커밋 후 4-4 진행
4. ChatPanel 외 다른 컴포넌트에서 Virtuoso 사용하지 말 것 (이번 범위는 ChatPanel만)

---

## 검증 게이트 (매 작업 후)

```bash
cd src-tauri && cargo check
cd src-tauri && cargo test --lib     # 57+ tests
cd .. && npx tsc --noEmit
cd .. && npx vitest run              # 55+ tests
```

## 완료 후

- CLAUDE.md §5, §11 갱신
- 커밋 메시지에 Phase 번호 명시
