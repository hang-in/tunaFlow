# DATA_MODEL.md — tunaChat v2 도메인 모델 명세서

> Single Source of Truth.  
> 근거: DATA_MODEL_FOUNDATION.md + CLAUDE.md + aoc-reference.md + harness-architecture.md

---

## 0. 모델 해석 원칙

### 0.1 Conversation과 Branch의 경계

**Conversation은 대화 컨테이너이고, Branch는 Conversation 내부의 독립 메시지 스트림이다.**  
즉 Branch는 독립된 Project나 최상위 Session이 아니라, 특정 Conversation의 일부 시점에서 파생된 하위 실험 경로이다.

- Conversation은 메시지 흐름의 루트 컨테이너다.
- Branch는 Conversation 내부에서 `checkpointId`를 기준으로 포크된 별도 메시지 스트림이다.
- Branch는 부모 Conversation의 맥락을 읽기 전용으로 참조하지만, 이후 메시지는 독립적으로 축적된다.
- Branch의 adopt는 부모 Conversation에 **브랜치 요약을 삽입**하는 것이며, 브랜치 메시지를 부모에 병합하는 것이 아니다.

### 0.2 ContextPack의 성격

**ContextPack은 영속 엔티티가 아니라 실행 시점에만 조합되는 runtime-only 구조다.**

- SQLite에 저장하지 않는다.
- 매 요청마다 Agent, Skill, rawq 결과, Cross-session summary, ResumeToken을 조합해서 구성한다.
- 이 중 ResumeToken만 별도로 영속화된다.

### 0.3 Roundtable의 정체성

**Roundtable은 별도 최상위 엔티티가 아니라 `Conversation.mode='roundtable'`인 특수 케이스다.**

- 기본 저장/조회/목록 구조는 Conversation을 따른다.
- 다만 내부 실행 상태로 `RoundtableState`를 추가로 가진다.
- 따라서 UI, DB, 실행 흐름은 Conversation 공통 구조를 재사용한다.

---

## 1. Core Entities

### 1.1 Workspace

**정의**: 사용자의 로컬 파일시스템 루트. 하위 프로젝트를 자동 발견한다.

**책임**: 1단계 하위 폴더를 스캔하여 Project로 분류.

| 필드 | 타입 | 설명 |
|------|------|------|
| rootPath | string | 스캔 대상 디렉토리 절대 경로 |
| lastScannedAt | integer (epoch) | 마지막 스캔 시각 |

**관계**: 1 Workspace → N Project

**저장**: 인메모리 (스캔 결과는 Project로 영속화)

**근거**: DATA_MODEL 설계 문서. **코드상 미구현** — `scan_workspace` 커맨드 없음. `src-tauri/src/lib.rs` 참조.

---

### 1.2 Project

**정의**: 작업 단위. git 저장소, 에이전트 채팅 세션, 또는 외부 채널(mattermost/slack).

**책임**: 대화, 에이전트, 스킬, 아티팩트의 소유자.

| 필드 | 타입 | 설명 |
|------|------|------|
| key | string (PK) | 고유 식별자 |
| name | string | 표시 이름 |
| path | string? | 파일시스템 경로 (없으면 채널) |
| type | enum | `'project'` \| `'discovered'` \| `'chat'` \| `'channel'` |
| defaultEngine | string? | 기본 AI 엔진 |
| workspaceRoot | string? | 소속 워크스페이스 루트 |
| source | enum | `'configured'` \| `'discovered'` |
| updatedAt | integer (epoch) | 마지막 갱신 시각 |

**관계**: 1 Project → N Conversation, N Agent, N Memo, N Artifact

**저장**: SQLite `projects` 테이블

**근거**: `src-tauri/src/db/models.rs:6-16`, `src-tauri/src/commands/projects.rs`, `src/types/index.ts:1-10`

---

### 1.3 Conversation

**정의**: 사용자와 에이전트 간의 대화 세션.

**책임**: 메시지 스트림 소유. 브랜치의 루트. 설정 오버라이드 보유. `mode='roundtable'`일 때 원탁회의 대화의 루트 역할도 수행.

| 필드 | 타입 | 설명 |
|------|------|------|
| id | string (PK) | UUID |
| projectKey | string (FK→Project) | 소속 프로젝트 |
| label | string | 표시 이름 |
| customLabel | string? | 사용자 지정 이름 (label보다 우선) |
| type | enum | `'main'` \| `'branch'` \| `'discussion'` |
| mode | enum | `'chat'` \| `'roundtable'` |
| parentId | string? (FK→Conversation) | 브랜치 부모 대화 |
| source | enum | `'tunadish'` \| `'mattermost'` \| `'slack'` |
| createdAt | integer (epoch) | 생성 시각 |
| updatedAt | integer (epoch) | 마지막 갱신 시각 |
| **settings** | | (인라인 — ConvSettings) |
| engine | string? | 엔진 오버라이드 |
| model | string? | 모델 오버라이드 |
| persona | string? | 에이전트 persona 오버라이드 |
| triggerMode | enum? | `'always'` \| `'mentions'` \| `'off'` |
| **usage** | | (토큰 추적) |
| totalInputTokens | integer | 누적 입력 토큰 |
| totalOutputTokens | integer | 누적 출력 토큰 |
| totalCostUsd | real | 누적 비용 (USD) |

**관계**:
- 1 Conversation → N Message (ordered by timestamp)
- 1 Conversation → N Branch
- 1 Conversation → 0..1 ResumeToken
- 1 Conversation → 0..1 RoundtableState (mode='roundtable'일 때)

**저장**: SQLite `conversations` 테이블

**근거**: `src-tauri/src/db/models.rs:19-42`, `src-tauri/src/commands/conversations.rs`, `src/types/index.ts:12-30`

---

### 1.4 Branch

**정의**: Conversation에서 특정 메시지(checkpoint)를 기준으로 포크한 독립 대화 경로.

**책임**: 부모 대화와 독립적 메시지 스트림 유지. adopt 시 요약을 부모에 삽입.

| 필드 | 타입 | 설명 |
|------|------|------|
| id | string (PK) | UUID |
| conversationId | string (FK→Conversation) | 루트 대화 |
| label | string | 자동 생성 이름 (b1, b1.1, b1.1.1) |
| customLabel | string? | 사용자 지정 이름 |
| status | enum | `'active'` \| `'adopted'` \| `'archived'` \| `'discarded'` |
| checkpointId | string? (FK→Message) | 포크 기준 메시지 |
| parentBranchId | string? (FK→Branch) | 부모 브랜치 (중첩 분기 시) |
| sessionId | string? | 연결된 대화 세션 ID |
| gitBranch | string? | 연동된 git 브랜치 이름 |
| createdAt | integer (epoch) | 생성 시각 |

**관계**:
- 1 Branch → N Message (키: `branch:{id}`)
- 1 Branch → 0..N Branch (중첩, via parentBranchId)
- N Branch → 1 Conversation (루트)

**저장**: SQLite `branches` 테이블

**근거**: `src-tauri/src/db/models.rs:92-105`, `src-tauri/src/commands/branches.rs`, `src/types/index.ts:45-56`

---

### 1.5 Message

**정의**: 대화 내 개별 메시지. 순서는 timestamp으로 보장.

**책임**: 대화 내용 단위. 메모/브랜치의 참조 대상.

| 필드 | 타입 | 설명 |
|------|------|------|
| id | string (PK) | UUID 또는 서버 발급 ID |
| conversationId | string (FK→Conversation) | 소속 대화 (브랜치는 `branch:{branchId}`) |
| role | enum | `'user'` \| `'assistant'` |
| content | string | 메시지 본문 (마크다운) |
| timestamp | integer (epoch ms) | 생성 시각 |
| status | enum | `'sending'` \| `'streaming'` \| `'done'` \| `'error'` |
| progressContent | string? | 스트리밍 중 마지막 진행 상태 텍스트 |
| engine | string? | 생성에 사용된 엔진 |
| model | string? | 생성에 사용된 모델 |
| persona | string? | 토론 시 페르소나/역할 |

**관계**:
- N Message → 1 Conversation (또는 Branch)
- 1 Message → 0..N Memo (via Memo.messageId)
- 1 Message → 0..N Branch (via Branch.checkpointId)

**저장**: SQLite `messages` 테이블 + FTS5 `messages_fts` 가상 테이블

**근거**: `src-tauri/src/db/models.rs:45-58`, `src-tauri/src/commands/messages.rs`, `src/types/index.ts:32-43`

---

### 1.6 Agent

**정의**: AI 에이전트의 선언적 정의. 마크다운 파일로 관리.

**책임**: persona(역할), 모델 선택, RBAC(도구 권한), 시스템 프롬프트 정의.

| 필드 | 타입 | 설명 |
|------|------|------|
| name | string | 파일명 (확장자 제외) |
| description | string | 역할 설명 (frontmatter) |
| mode | enum | `'primary'` \| `'subagent'` |
| model | string | `{provider}/{model-id}` 형식 |
| temperature | number? | 0.0–1.0 |
| tools | Record<string, boolean> | RBAC 도구 권한 (bash, write, edit, read 등) |
| systemPrompt | string | 마크다운 body 전체 |

**파일 형식**:
```yaml
---
description: 설계/판단
mode: primary
model: anthropic/claude-opus-4-6
temperature: 0.1
tools:
  write: false
  edit: false
  bash: false
---
시스템 프롬프트 본문 (마크다운)
```

**관계**: N Agent → 1 Project (docs/agents/ 디렉토리)

**저장**: 파일시스템 `docs/agents/*.md`

**RBAC 변환 규칙** (agentLoader.ts):
- 기본 허용: `["Read", "Grep", "Glob"]`
- `tools.bash: true` → `"Bash"` 추가
- `tools.write: true` → `"Write"` 추가
- `tools.edit: true` → `"Edit"` 추가
- CLI 전달: `--allowedTools Read,Grep,Glob,Bash,...`

**근거**: `src-tauri/src/agents/loader.rs` (Rust 로더). 관리 UI 미구현.

---

### 1.7 ContextPack

**정의**: 에이전트 실행 시 프롬프트에 주입되는 보조 컨텍스트 묶음.

**성격**: **runtime-only 구조**. 영속화하지 않으며, 실행 시점에만 생성되고 폐기된다.

**책임**: resume token, rawq 검색 결과, 스킬 본문, 크로스 세션 요약 등을 실행 시점에 조합.

| 구성 요소 | 출처 | 주입 방식 | 라이프사이클 |
|-----------|------|----------|------------|
| **ResumeToken** | 이전 실행의 CompletedEvent | CLI `--resume TOKEN` 인자 | Conversation 단위. 엔진 변경 시 폐기 |
| **Agent SystemPrompt** | docs/agents/*.md body | CLI `--append-system-prompt` | Agent 파일 변경 시 갱신 |
| **Skill Content** | ~/.tunachat/skills/*/SKILL.md body | 시스템 프롬프트 prefix로 주입 | 프로젝트/브랜치의 activeSkills에 따라 선택 |
| **rawq SearchResult** | rawq search 실행 결과 | 프롬프트 prefix로 주입 | 메시지 단위 (매 요청마다 재실행) |
| **CodeMap** | rawq map 실행 결과 | 새 세션 시작 시 1회 주입 | 세션 단위 |
| **CrossSessionSummary** | 같은 프로젝트의 다른 대화 요약 | 프롬프트 prefix로 주입 | 메시지 단위 (선택적) |

**저장**: 인메모리 조합 (실행 시점에 동적 생성). ResumeToken만 영속화.

**근거**: `src-tauri/src/commands/agents.rs:190-348` (assemble_system_prompt, build_skills_section, build_rawq_section, build_cross_session_section, build_context_summary)

---

### 1.8 ResumeToken

**정의**: CLI 에이전트의 세션 연속성 토큰. `--resume TOKEN` 인자로 전달.

**책임**: 이전 대화 상태 복원. 에이전트 내부 상태(메모리, 컨텍스트 윈도우)를 유지.

| 필드 | 타입 | 설명 |
|------|------|------|
| engine | string | 토큰을 발급한 엔진 ID |
| value | string | 토큰 문자열 |
| conversationId | string (FK→Conversation) | 소속 대화 |

**라이프사이클**:
1. 첫 실행: 토큰 없음 → 새 세션
2. CompletedEvent 수신: `resume_token` 저장
3. 다음 실행: `--resume {value}` 전달 → 세션 연속
4. 엔진 변경: 토큰 폐기 → 새 세션

**저장**: SQLite (tunaChat) 또는 JSON `~/.tunapi/tunadish_conv_sessions.json` (tunapi)

**근거**: `src-tauri/src/db/schema.rs:9-15` (V2_SCHEMA), `src-tauri/src/commands/agents.rs:298-307` (저장/복원 로직)

---

### 1.9 Artifact

**정의**: 대화/브랜치에서 생성된 문서 산출물.

**책임**: 계획, Task Brief, diff 요약, 테스트 보고서 등의 구조화된 출력물 관리.

| 필드 | 타입 | 설명 |
|------|------|------|
| id | string (PK) | UUID |
| conversationId | string? (FK→Conversation) | 출처 대화 |
| branchId | string? (FK→Branch) | 출처 브랜치 |
| type | enum | `'plan'` \| `'task_brief'` \| `'diff'` \| `'test_report'` |
| title | string | 제목 |
| content | string | 본문 (마크다운) |
| status | enum | `'draft'` \| `'approved'` \| `'rejected'` |
| createdAt | integer (epoch) | 생성 시각 |
| updatedAt | integer (epoch) | 마지막 갱신 시각 |

**저장**: SQLite `artifacts` 테이블 (V1 스키마에 포함)

**근거**: `src-tauri/src/db/schema.rs:110-124`, `src-tauri/src/db/models.rs:76-89`, `src-tauri/src/commands/artifacts.rs`

**TODO**: Artifact 생성 UI 및 상태 전이 트리거 미확정 (MVP-2)

---

### 1.10 Memo

**정의**: 메시지의 영구 스냅샷. 사용자가 중요하다고 판단한 내용.

**책임**: 프로젝트 단위 지식 저장. 메시지 역참조.

| 필드 | 타입 | 설명 |
|------|------|------|
| id | string (PK) | UUID |
| messageId | string (FK→Message) | 원본 메시지 |
| conversationId | string (FK→Conversation) | 출처 대화 |
| projectKey | string (FK→Project) | 소속 프로젝트 |
| content | string | 메시지 내용 스냅샷 |
| type | enum | `'decision'` \| `'review'` \| `'idea'` \| `'context'` |
| tags | string[] (JSON) | 사용자 태그 |
| createdAt | integer (epoch) | 저장 시각 |

**저장**: SQLite `memos` 테이블 (V1 스키마에 포함)

**근거**: `src-tauri/src/db/schema.rs:94-108`, `src-tauri/src/db/models.rs:61-73`, `src-tauri/src/commands/memos.rs`

---

### 1.11 RoundtableConsensus

**정의**: Roundtable (RT) 라운드에서 도달한 axis 별 합의 항목. 라운드 간 누적 영구화.

**책임**: 라운드 N+1 의 synthesizer + 참여자 prompt 에 *"이미 합의된 axis"* 명시 인계 + Architect dispatch 시 ContextPack 의 *"## Roundtable Consensus"* 섹션 인계 (devbug #263 회복).

| 필드 | 타입 | 설명 |
|------|------|------|
| id | string (PK) | UUID |
| conversationId | string (FK→Conversation) | RT 진행 conv (또는 brand shadow `branch:<id>`) |
| roundIndex | integer | 합의 도달 라운드 번호 (1..N) |
| axis | string | 합의 주제 키워드 (예: *"compression"*, *"budget"*) |
| decision | string | 1~3 문장 합의 요약 (synthesizer 추출) |
| participants | string[] (JSON) | 합의 참여자 이름 list (예: `["claude","codex"]`) |
| confidence | real (0.0-1.0) | synthesizer 의 합의 신뢰도 판단 |
| createdAt | integer (epoch ms) | persist 시각 |

**저장**: SQLite `roundtable_consensus` 테이블 (DB migration v50, 2026-05-07).
INDEX: `idx_roundtable_consensus_conv_round (conversation_id, round_index)`.

**관련 컬럼**: `messages.rt_round_index` (DB migration v51, nullable) — RT round
헤더 + 참여자 메시지 + synthesizer 헤더에 round_num 기록. ContextPack 의
`load_recent_messages_excluding_rt()` 가 `rt_round_index IS NULL` 만 collect →
single agent dispatch 가 RT round transcript 를 *주제별 컨텍스트* 로 prepend
하지 않음 (시나리오 A 회복).

**근거**: `src-tauri/src/db/migrations.rs:apply_v50/apply_v51`,
`src-tauri/src/commands/roundtable_helpers/persist.rs:save_consensus/load_consensus/extract_consensus_items`,
`src-tauri/src/commands/agents_helpers/context_pack/db_queries.rs:build_rt_consensus_section`,
Plan: `docs/plans/roundtableConsensusPersistencePlan_2026-05-07.md`,
시나리오: `docs/reference/roundtableReproductionScenarios_2026-05-07.md`

---

## 2. Relationship Model

```text
Workspace (스캔 결과 → Project 생성)
 └─ Project
      ├─ Conversation (type='main', mode='chat'|'roundtable')
      │   ├─ Message[]                    (ordered by timestamp)
      │   ├─ ConvSettings                 (engine/model/persona/triggerMode inline)
      │   ├─ ResumeToken                  (0..1, 엔진별 고유)
      │   ├─ Branch[]
      │   │   ├─ checkpointId ──ref──→ Message
      │   │   ├─ parentBranchId ──ref──→ Branch (트리)
      │   │   ├─ Message[]              (독립 스트림, 키: branch:{id})
      │   │   └─ gitBranch? ──sync──→ Git branch
      │   └─ RoundtableState            (mode='roundtable'일 때)
      │       ├─ engines[]
      │       └─ transcript[]
      │
      ├─ Agent[]                         (docs/agents/*.md)
      ├─ Skill[]                         (~/.tunachat/skills/ + 프로젝트 로컬)
      ├─ Memo[]                          (프로젝트 단위)
      │   └─ messageId ──ref──→ Message
      ├─ Artifact[]                      (계획/brief/diff)
      └─ TraceEntry[]                    (토큰/비용 추적)

RunState                                 (인메모리, per Conversation)
JournalEntry                             (JSONL 파일, per channel — 감사 추적)
ContextPack                              (인메모리 조합, 실행 시점에 동적 생성)
```

---

## 3. Branch Semantics

### 3.1 checkpoint 의미

- **checkpointId**: 부모 대화의 특정 Message.id
- 브랜치 생성 시 checkpoint 이전의 모든 메시지가 "부모 컨텍스트"로 표시됨 (읽기 전용, dimmed)
- checkpoint 이후의 메시지는 브랜치 독립 스트림

### 3.2 parentBranchId 의미

- **null**: 메인 대화에서 직접 분기한 1차 브랜치
- **non-null**: 다른 브랜치에서 분기한 중첩 브랜치
- 이름 규칙: `b1` (1차) → `b1.1` (2차) → `b1.1.1` (3차)
- 깊이 제한: 없음 (코드상 제한 없음)

### 3.3 Adopt 흐름

#### 현재 구현 (코드 기준)

```text
1. 사용자가 ContextPanel → Branch 탭에서 "Adopt" 클릭
2. Branch.status → 'adopted' (비가역)
3. 부모 대화에 placeholder 메시지 삽입:
   "<!-- branch-adopt-summary -->
    Branch {label} adopted. Summary generation not implemented yet."
4. 실제 요약 생성 로직 없음 — placeholder 텍스트만 삽입됨
```

근거: `src-tauri/src/commands/branches.rs:195-241`

#### 설계 의도 (향후 개선 가능성)

- 원래 의도: 에이전트가 브랜치 메시지를 요약하여 부모에 삽입
- `<!-- branch-adopt-summary -->` prefix는 향후 특수 렌더링(BranchAdoptCard)을 위한 마커
- 현재는 단순 텍스트로 렌더링됨

### 3.4 Merge 결과 정의

- **adopt는 Git merge와 다르다**: 브랜치 메시지가 부모에 "복사"되는 것이 아니라, 향후 요약이 삽입될 예정 (현재는 placeholder)
- 원본 브랜치 메시지는 보존됨 (shadow conversation `branch:{id}`에 유지)
- 충돌(conflict) 개념 없음
- adopted 브랜치는 수정 불가, 삭제만 가능

**근거**: `src-tauri/src/commands/branches.rs:195-241`

---

## 4. Context Model

### 4.1 ResumeToken vs ContextPack vs rawq

| 구분 | ResumeToken | ContextPack | rawq |
|------|------------|-------------|------|
| **역할** | CLI 에이전트의 내부 세션 상태 복원 | 프롬프트에 주입할 보조 정보 묶음 | 코드베이스 검색 결과 |
| **소유** | Conversation 단위, 엔진별 고유 | 실행 단위 (매 요청마다 조합) | 메시지 단위 |
| **영속성** | SQLite 또는 JSON 파일 | 인메모리 (비영속) | 인메모리 (비영속) |
| **전달** | CLI 인자 `--resume TOKEN` | 시스템 프롬프트 또는 프롬프트 prefix | 프롬프트 prefix |
| **폐기** | 엔진 변경 시 | 매 요청마다 재생성 | 매 요청마다 재실행 |

### 4.2 ContextPack Attach 방식

**Claude 전용**. Codex/Gemini/OpenCode는 prompt만 전달하며 아래 과정을 거치지 않음.

근거: `src-tauri/src/commands/agents.rs` — `send_with_codex/gemini/opencode`는 `system_prompt: None` 전달.

실행 시점에 다음 순서로 조합 (`commands/agents.rs:323-348`):
1. Agent.systemPrompt (loader.rs) + 사용자 system_prompt → `combine_prompt_parts()`
2. Skill.content → `build_skills_section()` → guardrail 8K 제한
3. rawq search result → `build_rawq_section()` → guardrail 4K 제한
4. CrossSessionSummary → `build_cross_session_section()` → guardrail 6K 제한
5. Conversation context → `build_context_summary()` → guardrail 8K 제한
6. 전체 → `enforce_total_limit()` → 60K 제한
7. ResumeToken → `--resume TOKEN` (별도 CLI 인자)

### 4.3 Lifecycle

```text
[새 대화]
  → ContextPack = {agent prompt + skill + code map}
  → ResumeToken = null → CLI 새 세션

[응답 완료]
  → CompletedEvent.resume_token → 저장

[다음 메시지]
  → ContextPack = {agent prompt + skill + rawq(query)}
  → ResumeToken = 저장된 값 → CLI 세션 계속

[엔진 변경]
  → ResumeToken 폐기 → 새 세션

[브랜치 전환]
  → 별도 ResumeToken (TODO: 브랜치별 독립 여부 미확정)
```

---

## 5. Conversation Modes

### 5.1 Chat (기본)

| 속성 | 값 |
|------|-----|
| mode | `'chat'` |
| 참여자 | 1 user + 1 agent (엔진/모델) |
| 메시지 흐름 | user → assistant → user → ... |
| 설정 변경 | 대화 중 엔진/모델/페르소나 전환 가능 |
| 브랜치 | 지원 |

### 5.2 Roundtable

| 속성 | 값 |
|------|-----|
| mode | `'roundtable'` |
| 참여자 | 1 user + N agents (순차 실행) |
| 메시지 흐름 | user → agent1 → agent2 → agent3 → user → ... |
| 라운드 | total_rounds, current_round로 추적 |
| 컨텍스트 누적 | 각 에이전트의 답변이 다음 에이전트의 시스템 프롬프트에 포함 |
| transcript | `[(engine, answer)][]` 형태로 별도 저장 |
| follow-up | `!rt follow` 커맨드로 추가 라운드 |
| 브랜치 | TODO: 지원 여부 미확정 |

**근거**: `src-tauri/src/commands/roundtable.rs` (roundtable_run, roundtable_followup)

---

## 6. Persistence Mapping

### 6.1 SQLite (로컬 SSOT)

| 테이블 | 엔티티 | 버전 | 핵심 인덱스 |
|--------|--------|------|-----------|
| `projects` | Project | V1 | PK: key |
| `conversations` | Conversation | V1 (+V2: resume_token) | (project_key), (updated_at DESC) |
| `messages` | Message | V1 (+V51: rt_round_index) | (conversation_id, timestamp), (conversation_id, rt_round_index) WHERE NOT NULL |
| `branches` | Branch | V1 | (conversation_id), (session_id) |
| `messages_fts` | Message (검색) | V1 (스키마만, 미사용) | FTS5 가상 테이블 |
| `memos` | Memo | V1 | (project_key), (message_id) |
| `roundtable_consensus` | RoundtableConsensus | V50 | (conversation_id, round_index) |
| `artifacts` | Artifact | V1 | (conversation_id) |
| `trace_log` | TraceEntry | V1 (스키마만, 미사용) | (conversation_id) |
| `schema_version` | 마이그레이션 추적 | V0 | PK: version |

### 6.2 파일시스템

| 대상 | 경로 | 형식 |
|------|------|------|
| Agent 정의 | `docs/agents/*.md` | YAML frontmatter + 마크다운 |
| Skill 정의 | `~/.tunaflow/skills/*/SKILL.md` | YAML frontmatter + 마크다운 |
| Journal | 미구현 | - |
| Roundtable 세션 | 미구현 (transcript는 memos에 아카이브) | - |

### 6.3 인메모리 (Zustand)

| Store | 엔티티 | 비고 |
|-------|--------|------|
| chatStore | 전체 상태 (단일 Store) | SQLite write-through (via Tauri invoke) |

> **참고**: contextStore, runStore, systemStore는 설계 문서에 정의되었으나 실제 구현에서는 `chatStore.ts` 하나로 통합됨. `isRunning` 상태로 RunState를 대체.

---

## 7. State Machines

### 7.1 Branch Status

```text
        ┌──────────┐
        │  active   │
        └────┬──┬───┘
    adopt    │  │   archive
             ▼  ▼
      ┌──────┐  ┌──────────┐
      │adopted│  │ archived │
      └──────┘  └──────────┘
           │         │
           ▼         ▼
      ┌──────────────────┐
      │    (삭제 가능)     │
      └──────────────────┘
```

**전이 규칙**:
- `active → adopted`: branch.adopt (commit). 비가역.
- `active → archived`: branch.archive (보관). 비가역.
- `adopted → 삭제`: SQLite DELETE (클라이언트 전용)
- `archived → 삭제`: SQLite DELETE (클라이언트 전용)
- `active → active`: 상태 유지 (대화 계속)

**근거**: `src-tauri/src/commands/branches.rs` (adopt_branch에서 status 전이), `src/types/index.ts:50` (status enum)

### 7.2 Message Status

```text
  ┌─────────┐    전송 시작
  │ sending │ ──────────────→ ┌───────────┐
  └─────────┘                 │ streaming │
                              └─────┬─────┘
                           완료     │   에러
                              ┌─────▼─────┐
                              │   done    │
                              └───────────┘
                                    │
                              ┌─────▼─────┐
                              │   error   │
                              └───────────┘
```

**전이 규칙**:
- `sending → streaming`: sidecar StartedEvent 수신
- `streaming → done`: sidecar CompletedEvent 수신 (ok=true)
- `streaming → error`: sidecar CompletedEvent 수신 (ok=false)
- `sending → error`: 전송 실패
- `done`: 최종 상태 (변경 불가, 삭제만 가능)

**근거**: `src-tauri/src/db/schema.rs:67` (status column), `src/types/index.ts:38` (status enum)

### 7.3 Run Status

```text
  ┌───────┐   chat_send
  │ idle  │ ──────────→ ┌─────────┐
  └───────┘              │ running │
      ▲                  └────┬────┘
      │            cancel     │
      │               ┌──────▼──────┐
      │               │ cancelling  │
      │               └──────┬──────┘
      │     완료/취소 확인    │
      └──────────────────────┘
```

**전이 규칙**:
- `idle → running`: chat_send 요청
- `running → cancelling`: run.cancel 요청
- `running → idle`: CompletedEvent 수신
- `cancelling → idle`: 취소 확인 또는 CompletedEvent 수신

**근거**: `src/stores/chatStore.ts:83` (`isRunning: boolean`). runStore는 미구현 — chatStore의 `isRunning` 플래그로 대체됨. cancelling 상태도 미구현.

### 7.4 Artifact Status

```text
  ┌───────┐   생성
  │ draft │ ──────→ ┌──────────┐
  └───────┘         │ approved │
      │             └──────────┘
      │    거부
      └──────────→ ┌──────────┐
                   │ rejected │
                   └──────────┘
```

**근거**: `src-tauri/src/db/schema.rs:118` (status column), `src-tauri/src/commands/artifacts.rs` (update_artifact_status)

**TODO**: 상태 전이 트리거(수동/자동) 미확정

---

## 8. Open TODO

| 항목 | 상태 | 설명 |
|------|------|------|
| **브랜치별 독립 ResumeToken** | 미확정 | 브랜치 전환 시 부모 토큰 유지 or 폐기? |
| **브랜치별 독립 ConvSettings** | 계획 | 브랜치 내 모델 변경이 부모에 영향 주지 않도록 |
| **브랜치 ↔ git branch 자동 연동** | MVP-2 계획 | 이름 규칙, 생성/삭제 동기화 범위 |
| **Roundtable에서 브랜치 지원** | 미확정 | roundtable 대화에서 분기가 가능한가? |
| **Skill 시스템** | MVP-2 계획 | 스코프(global/project/branch), 도구 등록 포함 여부 |
| **Task Brief 위임** | MVP-2 계획 | architect → developer 흐름, 승인 게이트 위치 |
| **Artifact 상태 전이 트리거** | 미확정 | 수동 승인 or 자동 전이? |
| **Budget Governor** | MVP-3 계획 | 단위(conv/project), 초과 동작(중단/경고) |
| **외부 세션 통합** | MVP-1.5/3 | 읽기 전용 표시 or DB 병합? |
| **아카이브 → active 복원** | 미구현 | 필요 여부 미확정 |
| **브랜치 간 diff 비교 UI** | 미구현 | 필요 여부 미확정 |
| **Cross-session summary 구조화** | 미구현 | 현재 임시 프롬프트 주입만, 영속화 필요? |
