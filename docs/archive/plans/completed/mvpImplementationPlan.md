# tunaChat v2 MVP 구현 순서 + 파일 단위 작업 리스트

## 0. 전제
이 작업은 **새 프로젝트**에서 진행한다.  
기존 tunachat/tunadish 포팅이 아니라, `DATA_MODEL.md`를 기준으로 한 재구현이다.

핵심 원칙:
- 데이터 모델 먼저
- Rust command layer 먼저
- Adapter 1개 먼저
- UI는 최소 수준
- rawq / 하네스 / 고급 문서화는 뒤로 미룬다

---

## 1단계 — 프로젝트 생성 / 뼈대 확정

### 목표
- 새 Tauri 2 프로젝트 생성
- 기본 폴더 구조 확정
- 참조 문서 배치

### 생성/정리 파일

#### 루트
- `README.md` — 프로젝트 개요 및 실행법
- `package.json` — 프런트엔드 의존성
- `tsconfig.json` — TS 설정
- `vite.config.ts` — Vite 설정

#### 문서
- `docs/reference/DATA_MODEL.md`
- `docs/reference/DATA_MODEL_FOUNDATION.md`
- `docs/reference/MVP_SCOPE.md` — MVP 포함/제외 범위
- `docs/reference/IMPLEMENTATION_NOTES.md` — 구현 메모/TODO

#### 프런트
- `src/main.tsx`
- `src/App.tsx`
- `src/app/router.tsx` 또는 단일 엔트리

#### Tauri / Rust
- `src-tauri/src/main.rs`
- `src-tauri/Cargo.toml`
- `src-tauri/tauri.conf.json`

### 완료 조건
- 앱이 빈 화면으로라도 실행됨
- Tauri invoke 기본 호출 가능

---

## 2단계 — DB 계층 구축

### 목표
- SQLite 초기화
- migration 체계 구축
- 핵심 테이블 생성

### 생성 파일

#### DB 모듈
- `src-tauri/src/db/mod.rs` — DB 연결 진입점
- `src-tauri/src/db/schema.rs` — CREATE TABLE / INDEX 정의
- `src-tauri/src/db/migrations.rs` — schema_version 및 migration 실행
- `src-tauri/src/db/models.rs` — Rust DB 모델 struct
- `src-tauri/src/db/seed.rs` — 선택, 초기 데이터/샘플용

#### 공통
- `src-tauri/src/errors.rs` — DB/command 공통 에러

### 이번 단계 테이블
- `projects`
- `conversations`
- `messages`
- `branches`
- `schema_version`

### 함께 스키마만 넣을 수 있는 테이블
- `memos`
- `artifacts`
- `trace_log`
- `messages_fts`

### 완료 조건
- 앱 시작 시 DB 자동 생성
- migration version 기록됨
- 외래키 활성화
- 최소 CRUD 테스트 통과

---

## 3단계 — Rust 도메인 command layer

### 목표
- Project / Conversation / Message / Branch command 구현
- 프런트에서 호출 가능하게 expose

### 생성 파일

#### Commands
- `src-tauri/src/commands/mod.rs`
- `src-tauri/src/commands/projects.rs`
- `src-tauri/src/commands/conversations.rs`
- `src-tauri/src/commands/messages.rs`
- `src-tauri/src/commands/branches.rs`
- `src-tauri/src/commands/agents.rs` — 초기는 placeholder 가능

#### 타입/입출력
- `src-tauri/src/commands/dto.rs` — command input/output struct 모음

### 구현 command

#### Projects
- `list_projects`
- `create_project`
- `get_project`

#### Conversations
- `list_conversations`
- `create_conversation`
- `get_conversation`

#### Messages
- `list_messages`
- `create_user_message`
- `append_assistant_message`
- `update_message_status`

#### Branches
- `list_branches`
- `create_branch`
- `adopt_branch`

### adopt_branch 임시 처리
- `branches.status='adopted'`
- 부모 conversation에 placeholder assistant message 삽입

### 완료 조건
- Tauri invoke로 모든 command 호출 가능
- DB 반영 확인 가능

---

## 4단계 — 프런트 최소 골격

### 목표
- 최소 UI에서 Project / Conversation / Message 흐름 확인

### 생성 파일

#### 페이지
- `src/pages/projects/ProjectsPage.tsx`
- `src/pages/conversations/ConversationsPage.tsx`
- `src/pages/chat/ChatPage.tsx`

#### 컴포넌트
- `src/components/layout/AppShell.tsx`
- `src/components/projects/ProjectList.tsx`
- `src/components/conversations/ConversationList.tsx`
- `src/components/chat/MessageList.tsx`
- `src/components/chat/MessageComposer.tsx`
- `src/components/branches/BranchList.tsx`

#### 상태관리
- `src/stores/projectStore.ts`
- `src/stores/conversationStore.ts`
- `src/stores/messageStore.ts`
- `src/stores/branchStore.ts`
- `src/stores/runStore.ts`

#### 호출 유틸
- `src/lib/tauri.ts` — invoke 래퍼
- `src/lib/api/projects.ts`
- `src/lib/api/conversations.ts`
- `src/lib/api/messages.ts`
- `src/lib/api/branches.ts`

#### 타입
- `src/types/domain.ts`
- `src/types/api.ts`

### 완료 조건
- 프로젝트 생성 가능
- 대화 생성 가능
- 메시지 입력/목록 표시 가능
- 브랜치 목록 표시 가능

---

## 5단계 — Claude adapter 1개 연결

### 목표
- 로컬 `claude` CLI subprocess 실행
- 단발 실행 후 assistant message 저장

### 생성 파일

#### Agents
- `src-tauri/src/agents/mod.rs`
- `src-tauri/src/agents/claude.rs`
- `src-tauri/src/agents/types.rs` — adapter 공통 타입

#### Command
- `src-tauri/src/commands/agents.rs`

### 구현 범위
- `send_with_claude`
- `claude -p ... --output-format json`
- stdout 파싱
- assistant message 저장
- 실패 시 에러 반환 또는 error 상태 저장

### TODO만 남길 것
- resume token 실제 재사용
- systemPrompt full injection
- streaming
- cancellation

### 완료 조건
- 사용자가 메시지 입력 후 Claude 응답이 DB와 UI에 저장됨

---

## 6단계 — Branch 기본 동작 확인

### 목표
- 특정 메시지 기준 브랜치 생성
- 브랜치 목록/상태 확인
- adopt placeholder 동작 확인

### 수정 파일
- `src/components/branches/BranchList.tsx`
- `src/components/chat/MessageList.tsx`
- `src/lib/api/branches.ts`
- `src/stores/branchStore.ts`
- `src-tauri/src/commands/branches.rs`

### 완료 조건
- 메시지에서 브랜치 생성 가능
- 브랜치 상태 변경 확인 가능
- adopt 후 placeholder summary 메시지 삽입 확인 가능

---

## 7단계 — 안정화 / 테스트 / 정리

### 목표
- 최소 수동 테스트
- 문서 정리
- 이후 adapter 확장 가능 상태 확보

### 생성 파일
- `docs/reference/TEST_SCENARIOS.md`
- `docs/reference/KNOWN_LIMITATIONS.md`
- `docs/reference/NEXT_STEPS.md`

### 테스트 항목
1. 프로젝트 생성
2. 대화 생성
3. 사용자 메시지 저장
4. Claude 응답 저장
5. 브랜치 생성
6. adopt 처리
7. 앱 재시작 후 데이터 복원

### 완료 조건
- MVP 1차 골격 완성
- 다음 단계(Codex adapter / ResumeToken / ContextPack)로 넘어갈 수 있음

---

## MVP 범위에서 제외할 것

### 이번에 하지 않음
- Gemini / Codex / OpenCode adapter
- rawq 연동
- Roundtable
- Artifact 생성 UI
- Memo 저장 UI
- Skill 시스템
- ResumeToken 실제 저장/복원
- Branch diff UI
- Archive 복원
- Git branch 자동 연동
- Budget governor
- 하네스 자동 실행

---

## 추천 작업 순서 요약

1. 새 프로젝트 생성
2. DATA_MODEL.md 배치
3. DB schema/migration
4. Rust CRUD command
5. 최소 프런트 연결
6. Claude adapter
7. Branch 기본 동작
8. 테스트/문서화

---

## 새 프로젝트로 가도 되는가?

**된다. 오히려 새 프로젝트로 가는 것이 맞다.**

이유:
- 기존 tunachat 포팅 문제를 끌고 가지 않음
- DATA_MODEL.md를 기준으로 구조를 처음부터 고정 가능
- tunadish의 도메인 개념은 유지하면서 구현 부채는 버릴 수 있음
- AI에게 파일 단위로 더 정확히 작업 지시 가능

단, 조건:
- 기존 코드를 무시하라는 뜻이 아니라 **참조만 하고 직접 포팅하지 말아야 한다**
- 기존 프로젝트는 레퍼런스 저장소로 두고, 새 프로젝트는 실행 저장소로 분리하는 것이 좋다
