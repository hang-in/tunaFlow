# FRONTEND_ARCHITECTURE.md — 프론트엔드 아키텍처 문서

> **status: archived** — 이 문서는 세션 1 시점(2026-03-25)의 3패널 레이아웃을 기준으로 작성됨.
> 현재 코드는 5-tab CenterPanel, slice 기반 store, ProjectStartup 등 크게 달라졌으므로 참고용으로만 사용.
> 현행 아키텍처는 CLAUDE.md §4, §7 참조.
>
> **최종 갱신**: 2025-03-25 (코드 기준 검증)

---

## 1. 앱 진입 구조

```
index.html          ← <body class="dark"> + #root
  └─ src/main.tsx   ← React 부트스트랩 + index.css import
      └─ src/App.tsx        ← AppShell 래퍼
          └─ AppShell.tsx   ← 3패널 레이아웃 + 초기화 (auto project create)
```

- **라우팅**: 없음. 단일 페이지 구조. `App.tsx` → `AppShell` 직접 렌더링.
- **CSS 프레임워크**: Tailwind CSS v4 (`@tailwindcss/vite` 플러그인).
- **경로 alias**: `@/` → `src/` (vite.config.ts + tsconfig.json).

근거: `src/main.tsx`, `src/App.tsx`, `vite.config.ts`, `tsconfig.json`

---

## 2. 레이아웃 구조

```
┌──────────────────────────────────────────────────────────────────┐
│                        AppShell.tsx                                │
│  ┌──────────┐  ┌──────────────────────────────┐  ┌────────────┐  │
│  │          │  │        ChatPanel.tsx          │  │           │  │
│  │ Sidebar  │  │ ┌──────────────────────────┐ │  │  Context  │  │
│  │  .tsx    │  │ │ StatusBar.tsx             │ │  │  Panel    │  │
│  │          │  │ ├──────────────────────────┤ │  │  .tsx     │  │
│  │ 프로젝트 │  │ │ Header (제목 + 뷰 토글)   │ │  │           │  │
│  │ 대화목록 │  │ ├──────────────────────────┤ │  │ Branch    │  │
│  │ 생성/삭제│  │ │ Messages scroll area     │ │  │ Artifacts │  │
│  │ 검색     │  │ │  MessageItem.tsx (반복)   │ │  │ Memos     │  │
│  │          │  │ │  RoundtableView.tsx (RT)  │ │  │ Skills    │  │
│  │          │  │ ├──────────────────────────┤ │  │ Cross-ses │  │
│  │  w=224px │  │ │ NewMessageInput.tsx       │ │  │  w=256px  │  │
│  │          │  │ └──────────────────────────┘ │  │           │  │
│  └──────────┘  └──────────────────────────────┘  └────────────┘  │
└──────────────────────────────────────────────────────────────────┘
```

근거: `src/components/tunaflow/AppShell.tsx`

---

## 3. 상태 관리

**단일 Store**: `src/stores/chatStore.ts` (Zustand v5)

### 3.1 Store 상태

| 필드 | 타입 | 설명 |
|------|------|------|
| `projects` | `Project[]` | 전체 프로젝트 목록 |
| `selectedProjectKey` | `string \| null` | 선택된 프로젝트 |
| `conversations` | `Conversation[]` | 선택된 프로젝트의 대화 목록 |
| `selectedConversationId` | `string \| null` | 선택된 대화 |
| `messages` | `Message[]` | 선택된 대화의 메시지 |
| `branches` | `Branch[]` | 선택된 대화의 브랜치 |
| `isRunning` | `boolean` | 에이전트 실행 중 여부 |
| `error` | `string \| null` | 마지막 에러 메시지 |
| `activeBranchId` | `string \| null` | 브랜치 스트림 모드 시 활성 브랜치 |
| `parentConversationId` | `string \| null` | 브랜치 스트림 복귀용 부모 대화 |
| `memos` | `Memo[]` | 선택된 대화의 메모 |
| `artifacts` | `Artifact[]` | 선택된 대화의 아티팩트 |
| `skills` | `SkillDef[]` | 전체 스킬 목록 |
| `activeSkills` | `string[]` | 활성 스킬 이름 |
| `crossSessionIds` | `string[]` | 크로스세션 포함 대화 ID |

### 3.2 주요 액션

| 액션 | Tauri invoke 대상 | 설명 |
|------|-------------------|------|
| `sendMessage(prompt, model?, systemPrompt?)` | `stream_with_claude` | Claude 스트리밍 전송 |
| `sendWithCodex(prompt, model?)` | `send_with_codex` | Codex one-shot |
| `sendWithGemini(prompt, model?)` | `send_with_gemini` | Gemini one-shot |
| `sendWithOpencode(prompt, model?)` | `send_with_opencode` | OpenCode one-shot |
| `sendRoundtable(prompt, rounds?)` | `roundtable_run` | RT 실행 |
| `sendRoundtableFollowup(prompt)` | `roundtable_followup` | RT follow-up |
| `openBranchStream(branchId)` | `open_branch_stream` | 브랜치 대화로 전환 |
| `closeBranchStream()` | - | 부모 대화로 복귀 |
| `selectConversation(id)` | `list_messages` + `list_branches` + `list_memos` + `list_artifacts` | 대화 선택 (4개 동시 로드) |

근거: `src/stores/chatStore.ts`

---

## 4. 컴포넌트 상세

### 4.1 AppShell (`src/components/tunaflow/AppShell.tsx`)
- 초기화: `useEffect`에서 프로젝트 로드, 없으면 default 생성
- 레이아웃: `flex h-screen` → Sidebar + main(ChatPanel) + ContextPanel

### 4.2 Sidebar (`src/components/tunaflow/Sidebar.tsx`)
- 로고 + 검색 + 프로젝트 트리 + 대화 목록
- 대화 생성 (Chat / Roundtable)
- 대화 삭제 (hover 시 Trash2 아이콘 → confirm dialog)
- shadow conv (`branch:*`) 필터링

### 4.3 ChatPanel (`src/components/tunaflow/ChatPanel.tsx`)
- StatusBar → Header → Messages scroll → (Branches panel) → NewMessageInput
- RT 대화 시 Stream/Table 뷰 토글
- `activeBranchId` 있을 때 브랜치 모드 표시
- 메시지마다 `onBranch` / `onMemo` 콜백 전달

### 4.4 MessageItem (`src/components/tunaflow/MessageItem.tsx`)
- User: 보라색 원형 아바타 + "You"
- Assistant: 엔진별 색상 배지 (persona 또는 engine 이름 표시)
- 스트리밍: typing-dot 애니메이션 + "streaming..." 텍스트
- hover 시: Branch / Memo / Copy 액션 버튼

### 4.5 RoundtableView (`src/components/tunaflow/RoundtableView.tsx`)
- 참가자 행 (상단 배지)
- 카드 레이아웃: 커넥터 라인 + 에이전트 아바타 + 메시지 카드
- hover 시 Copy 버튼

**라운드 감지 구조와 한계:**
- 현재 `groupIntoRounds()` 함수가 **persona 필드 반복을 감지**하여 라운드를 추론함
- Message 테이블에 `round` 필드가 없으므로 DB 기반 구분 불가
- 백엔드(`roundtable.rs:persist_round`)는 다중 라운드 시 `"--- Round N/M ---"` 형식의 헤더를 `engine='system'` 메시지로 삽입하지만, 프론트의 `groupIntoRounds()`는 이 헤더를 인식하지 않음
- 참가자 중 하나가 error로 응답하면 persona 반복 패턴이 깨질 수 있어 불안정
- **개선 방안**: (1) Message에 round 필드 추가 (V3 마이그레이션) 또는 (2) system 헤더 메시지를 파싱하여 라운드 구분

### 4.6 NewMessageInput (`src/components/tunaflow/NewMessageInput.tsx`)
- **Chat 모드**: 엔진 드롭다운 (Claude/Codex/Gemini/OpenCode)
- **RT 모드**: 참가자 표시 + Rounds 셀렉터 (1-3) + Follow-up 버튼
- Textarea auto-resize
- ⌘↵ 전송

### 4.7 ContextPanel (`src/components/tunaflow/ContextPanel.tsx`)
- 5개 탭: Branch / Artifacts / Memos / Skills / Cross-session
- 각 탭은 내부 함수 컴포넌트로 구현 (BranchPanel, ArtifactsPanel, ...)
- Skills: `loadSkills()` on mount
- Artifacts: 인라인 생성 폼 (type/title/content)
- Cross-session: 다른 대화 토글 (shadow conv 제외)

---

## 5. 레거시 컴포넌트 — 삭제 완료

아래 6개 파일은 2025-03-25에 삭제됨. `src/pages/` 디렉토리도 제거됨.

| 삭제된 파일 | 대체 컴포넌트 |
|-----------|------------|
| `ConversationList.tsx` | `Sidebar.tsx` |
| `MessageInput.tsx` | `NewMessageInput.tsx` |
| `MessageList.tsx` | `ChatPanel.tsx` + `MessageItem.tsx` |
| `SidePanel.tsx` | `ContextPanel.tsx` |
| `ProjectList.tsx` | `Sidebar.tsx` |
| `MainPage.tsx` | `AppShell.tsx` |

삭제 후 `tsc --noEmit` + `vite build` 모두 정상 확인됨.

---

## 6. 스타일 시스템

### CSS 토큰 (`src/index.css`)
- oklch 기반 색상
- 에이전트 커스텀 색상: `--agent-claude`, `--agent-codex`, `--agent-gemini`, `--agent-opencode`
- 상태 색상: `--status-draft`, `--status-approved`, `--status-rejected`
- 애니메이션: `typing-dot` (스트리밍), `stream-cursor` (커서 깜빡임), `slide-in-from-right` (패널)

### 유틸리티 (`src/lib/utils.ts`)
- `cn()`: clsx + tailwind-merge
- `AGENT_COLORS`, `AGENT_DOT_COLORS`: 엔진별 Tailwind 클래스
- `formatTimestamp()`: epoch ms → "HH:MM AM/PM"
- `isKnownEngine()`: type guard

---

## 7. Branch/Thread UI 현황 및 개선 지점

### 현재 구조
브랜치 "Open" 시 `selectedConversationId`를 `branch:{id}` shadow conv로 교체.
메시지 목록이 완전히 브랜치 메시지로 바뀜. StatusBar에 branch 배지 표시.
"← Back to main" 버튼으로 복귀.

### 문제
- 부모 대화 메시지를 동시에 볼 수 없음
- thread 느낌이 아니라 "대화 교체" 느낌

### 권장 개선 지점
v0의 `BranchThreadPanel.tsx`를 참고하여 **오른쪽 슬라이딩 오버레이 패널** 구현 가능:
- `AppShell.tsx`의 `<main>` 내부에 조건부 렌더링
- `animate-in slide-in-from-right-96` 애니메이션 (index.css에 이미 정의됨)
- ContextPanel 위에 겹치거나, ChatPanel 오른쪽에 추가 패널로

### v0 참고 파일
- `v0-export/components/tunaflow/BranchThreadPanel.tsx` — 슬라이딩 패널 레이아웃
- 이 파일의 디자인을 가져와 `activeBranchId` 상태에 연결하면 됨

### 구체적 삽입 지점 (코드 기준)
1. `AppShell.tsx`의 `<main>` 블록 내부, `<ChatPanel />` 다음에 조건부 렌더링:
   ```tsx
   <main className="flex-1 flex min-w-0 h-full">
     <ChatPanel />
     {activeBranchId && <BranchThreadPanel />}  ← 여기
   </main>
   ```
2. `chatStore.ts`의 `activeBranchId`, `parentConversationId` 상태를 그대로 활용
3. `closeBranchStream()` 액션으로 패널 닫기
4. 현재 `ChatPanel.tsx`의 branch 관련 로직 (branch header, adopt 버튼)을 새 패널로 이동

---

## 8. v0 병합 현황

v0 UI 병합은 완료됨 (2025-03-25). `v0-export/`는 참고용으로만 유지.

### 병합 완료된 항목
- Tailwind CSS v4 다크 테마 → `src/index.css`
- `cn()` 유틸리티 → `src/lib/utils.ts`
- 3패널 레이아웃 → `src/components/tunaflow/AppShell.tsx`
- 사이드바 → `Sidebar.tsx`
- 메시지 카드 → `MessageItem.tsx`
- RT 카드 뷰 → `RoundtableView.tsx`
- 컨텍스트 패널 → `ContextPanel.tsx`
- 상태 바 → `StatusBar.tsx`
- 메시지 입력 → `NewMessageInput.tsx`

### v0에서 가져오지 않은 항목 (향후 활용 가능)
- `v0-export/components/tunaflow/BranchThreadPanel.tsx` — 브랜치 슬라이딩 패널 (P0 개선 과제)
- `v0-export/components/ui/*.tsx` — shadcn/ui 컴포넌트 (필요 시 개별 도입)
- `v0-export/lib/tunaflow-data.ts` — mock 데이터 (개발/테스트용)
