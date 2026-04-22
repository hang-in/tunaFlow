# tunaChat v2 첫 구현 프롬프트

너는 지금부터 **Lead Engineer** 역할을 수행한다.  
목표는 `tunaChat v2`의 **새 프로젝트 초기 골격**을 만드는 것이다.

중요:
- 기존 tunachat/tunadish 코드를 포팅하지 말고 **새 프로젝트로 시작**한다.
- 단, 설계 기준은 반드시 `DATA_MODEL.md`를 따른다.
- 이번 작업 범위는 **DB + Rust command layer + Adapter 1개(Claude Code)** 까지만이다.
- UI는 최소 수준만 허용한다. 핵심은 데이터 모델과 실행 흐름을 먼저 고정하는 것이다.
- 추측하지 말고, 확정되지 않은 부분은 TODO로 남겨라.
- 최소 수정 원칙이 아니라 **새 프로젝트의 최소 골격을 정확히 만드는 것**이 목표다.

---

## 기준 문서
반드시 아래 문서를 먼저 읽고 그대로 따른다.

1. `docs/reference/DATA_MODEL.md`
2. `docs/reference/DATA_MODEL_FOUNDATION.md`

특히 아래 원칙을 반드시 따른다.
- Conversation은 컨테이너다.
- Branch는 Conversation 내부 독립 메시지 스트림이다.
- ContextPack은 runtime-only다.
- Roundtable은 `Conversation.mode='roundtable'`의 특수 케이스다.

---

## 기술 스택
새 프로젝트는 아래 스택으로 생성한다.

- **Tauri 2**
- **React + TypeScript + Vite**
- **Rust (Tauri command / DB / subprocess orchestration)**
- **SQLite**
- 프런트 상태관리는 우선 단순 store 구조만 준비한다. (Zustand 사용 가능)

---

## 이번 단계 목표
이번 작업에서는 아래 3가지만 구현한다.

### 1. SQLite 기반 로컬 SSOT 구축
구현 범위:
- DB 초기화
- migration 체계 기초
- 아래 테이블 생성
  - `projects`
  - `conversations`
  - `messages`
  - `branches`
  - `schema_version`
- `memos`, `artifacts`, `trace_log`, `messages_fts`는 이번 단계에서 **스키마만 넣어도 되고 CRUD는 생략 가능**

요구사항:
- Rust에서 SQLite를 직접 관리한다.
- 앱 시작 시 DB 파일 자동 생성
- migration version 관리
- 각 테이블 인덱스 추가
- 외래키 활성화
- 에러 메시지는 최대한 명확하게

---

### 2. Rust command layer 구축
아래 command를 구현한다.

#### Project commands
- `list_projects()`
- `create_project(input)`
- `get_project(key)`

#### Conversation commands
- `list_conversations(project_key)`
- `create_conversation(input)`
- `get_conversation(id)`

#### Message commands
- `list_messages(conversation_id)`
- `create_user_message(input)`
- `append_assistant_message(input)`
- `update_message_status(input)`

#### Branch commands
- `list_branches(conversation_id)`
- `create_branch(input)`
- `adopt_branch(input)`

요구사항:
- 각 command는 Rust struct input/output을 가진다.
- snake_case / camelCase 변환 일관성 유지
- 브랜치 생성 시 `checkpointId`, `parentBranchId`, `status='active'` 반영
- adopt_branch는 실제 요약 생성까지 하지 말고, 우선 아래까지만 구현:
  1. Branch.status를 `adopted`로 변경
  2. 부모 Conversation에 placeholder assistant message 삽입
     - content prefix: `<!-- branch-adopt-summary -->`
     - 본문: `Branch {label} adopted. Summary generation not implemented yet.`

주의:
- 지금은 UI보다 command layer가 우선이다.
- 모든 command는 테스트 가능한 순수 DB 동작 중심으로 작성한다.

---

### 3. Claude Code adapter 1개 구현
이번 단계에서 지원할 에이전트는 **Claude Code 하나만** 구현한다.
Gemini, Codex, OpenCode는 아직 추가하지 않는다.

#### Adapter 목표
- 로컬에 설치된 `claude` CLI를 subprocess로 실행
- 단발 요청을 비대화형으로 보냄
- stdout 결과를 받아 assistant message로 저장
- stderr/log는 추후 확장을 위해 캡처만 해둔다

#### 1차 범위
- `claude -p` 기반 단발 실행만 구현
- `--output-format json` 사용
- Conversation 단위 ResumeToken은 **이번 단계에서 구조만 준비하고 실제 저장/재사용은 TODO 처리 가능**
- streaming은 아직 구현하지 말고, 요청 완료 후 결과를 한 번에 assistant message로 저장

#### Rust command
- `send_with_claude(input)`

입력 예시:
- projectKey
- conversationId
- userMessageId
- prompt
- model(optional)
- systemPrompt(optional)

동작 순서:
1. user message 저장 또는 기존 user message 참조
2. subprocess로 `claude` 실행
3. stdout 파싱
4. assistant message 저장 (`status='done'`)
5. 실패 시 assistant message를 `status='error'`로 저장하거나 명시적 오류 반환

주의:
- ContextPack 전체 구현은 아직 하지 않는다.
- 단, systemPrompt를 붙일 수 있는 함수 시그니처와 TODO는 남긴다.
- resume token은 필드/인터페이스만 열어두고 TODO로 남긴다.

---

## 파일 구조 요구사항
새 프로젝트의 파일 구조는 아래 원칙을 따른다.

```text
src/
  app/
  pages/
  components/
  stores/
  types/
  lib/

src-tauri/
  src/
    main.rs
    commands/
      projects.rs
      conversations.rs
      messages.rs
      branches.rs
      agents.rs
    db/
      mod.rs
      schema.rs
      migrations.rs
      models.rs
    agents/
      mod.rs
      claude.rs
    errors.rs
```

요구사항:
- DB 관련 로직과 agent subprocess 로직을 분리
- command 파일도 도메인별로 분리
- 타입 이름은 `DATA_MODEL.md`의 용어를 우선 사용

---

## 프런트엔드 최소 요구사항
UI는 최소한 아래만 구현한다.

- 프로젝트 목록
- 대화 목록
- 메시지 목록
- 입력창 1개
- “Claude로 보내기” 버튼 1개

중요:
- UI 완성도보다 command 연결 확인이 목적이다.
- Branch UI는 이번 단계에서 목록 표시만 가능하면 충분하다.
- adopt 버튼은 있으면 좋지만 필수는 아니다.

---

## 구현 순서
반드시 아래 순서로 작업한다.

1. 새 Tauri 프로젝트 생성
2. SQLite 초기화 및 migration 체계 추가
3. DB models/commands 구현
4. 최소 프런트엔드 연결
5. Claude adapter 구현
6. message send → assistant save 흐름 연결
7. 기본 수동 테스트

---

## 산출물
이번 작업 완료 시 반드시 아래를 제공하라.

### 1. 수정/생성 파일 목록
각 파일별 역할 설명 포함

### 2. DB 스키마 요약
테이블/컬럼/인덱스 요약

### 3. 구현된 command 목록
입력/출력 포함

### 4. Claude adapter 동작 설명
명령행 인자, stdout 파싱 방식, 실패 처리 방식 설명

### 5. 남은 TODO
- ResumeToken 실제 저장/재사용
- ContextPack 구성
- streaming 지원
- Roundtable
- rawq 연동

---

## 품질 기준
- 빌드 가능해야 한다.
- 타입 에러 없어야 한다.
- command 간 책임이 섞이지 않아야 한다.
- 추후 Codex/Gemini/OpenCode adapter 추가가 쉬운 구조여야 한다.
- 임시 구현은 TODO 주석으로 명확히 표시한다.

---

## 금지사항
- 기존 tunadish/tunachat 구조를 무비판적으로 복붙하지 말 것
- 한 파일에 DB/command/UI/agent logic을 다 넣지 말 것
- Branch를 Conversation과 같은 레벨 엔티티처럼 구현하지 말 것
- ContextPack을 DB 테이블로 만들지 말 것
- Roundtable을 별도 top-level session system으로 분리하지 말 것

---

## 최종 요청
작업 후 아래 형식으로 답하라.

1. 구현 개요
2. 파일별 변경 사항
3. 핵심 코드 설명
4. 빌드/실행 방법
5. 남은 TODO
