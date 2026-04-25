---
title: Branch session policy — brand 는 main interactive session 을 공유한다
status: active
canonical: true
priority: P1 (사용자 의도 SSOT, 토큰 낭비 방지)
created_at: 2026-04-25
updated_at: 2026-04-25
supersedes:
  - docs/ideas/ptySessionPolicy.md  # PTY 시대 정책. WS 전환 후 일반화한 본 문서가 SSOT
related:
  - docs/plans/branchInheritsMainSessionPlan_2026-04-25.md  # 본 정책의 코드 회복 작업
  - docs/plans/sessionContinuityFixPlan.md
  - docs/plans/longTermMemoryRoadmapPlan_2026-03-30.md  # SSOT-first memory 철학
  - src-tauri/src/agents/claude_sdk_session.rs
  - src-tauri/src/agents/codex_app_server.rs
  - src-tauri/src/commands/agents_helpers/send_common/context_loading.rs
---

# 핵심 원칙

> **branch 는 main 의 interactive session 을 그대로 이어받는다.**

분기→adopt/폐기 흐름에서 brand 는 "같은 사고의 연장" 이지 "별 세션" 이 아니다.
이 결정은 PTY 시대 (`docs/ideas/ptySessionPolicy.md:164`) 부터 사용자가 박아둔
의도이며, sdk-url WS 모드 (s36, 2026-04-15) 와 codex app-server 모드에서도 동일
하게 유지된다. SSOT 는 본 문서.

## 사용자 직접 인용 (raw conversation log)

> 브랜치는 ws모드로 입장하는데 엔진이 바뀌면 어떻게 맥락을 알려주지? 그리고
> 가능하면 **컨텍스트팩에 모든걸 넣지말고 중요한 컨텍스트+필요시 검색할 수 있는
> 대규모의 컨텍스트 저장소(첫대화부터 직전대화까지 모두 원본으로 저장되어있는)**
> 가 있잖아? — 2026-04-17 (PTY → WS 전환 시기, jsonl `037bb82f`)

> RT는 컨텍스트팩으로도 충분했고, **브랜치는 메인에서 바로 이어지는 건데
> 컨텍스트팩 올리면 낭비**라고 생각했었었고 — 2026-04-25

# Invariants (코드 검증 가능한 형태)

- **[INV-1]** brand:* shadow conv 의 send 가 사용하는 SESSIONS / RESUME_IDS /
  CONV_THREADS lookup 키는 **root main `conversation_id`**.
  검증: `src-tauri/src/agents/claude_sdk_session.rs::session_key_for`,
  `src-tauri/src/agents/codex_app_server.rs::current_thread_key/get_or_create_thread`.

- **[INV-2]** brand send 시 **(same engine as root)** ContextPack 의
  `recent_context` / `parent_messages` / `compressed_memory` /
  `retrieval_chunks` / `cross_session_data` / `thread_inheritance` /
  `document_chunks` 빌드 skip. 정적 레이어 (identity / persona / project /
  agent-role) 만 포함.
  검증: `src-tauri/src/commands/agents_helpers/send_common/context_loading.rs::apply_branch_session_inheritance`.

- **[INV-3]** **engine 변경 시** (예: Claude → Codex) brand 는 별 session 으로
  fallback. ContextPack 정상 빌드 (현재 동작 유지). brand same-engine 로직은
  no-op.
  검증: 위 helper 의 `is_engine_continuity` 분기.

- **[INV-4]** **DB raw 메시지가 SSOT.** retrieval / compressed memory /
  vector search 는 SSOT 위 helper layer 이며 정책 위배가 아니다. brand 가
  필요하면 `tool-request:recent_turns:N` 마커로 명시 조회한다.

- **[INV-5]** 본 정책은 brand session 통합 + ContextPack 낭비 제거 범위.
  shadow conv DB 모델, adopt summary placeholder, branches 테이블, UI 드로어
  등은 그대로 유지.

# 데이터 모델

shadow conversation 모델은 손대지 않는다 (UI 격리 + 메시지 보존). 변경 지점은
**runtime 레지스트리 키** 와 **ContextPack 조립 로직** 두 곳뿐.

| 레지스트리 | 위치 | 키 정책 |
|---|---|---|
| `SESSIONS` | `claude_sdk_session.rs` | brand:* → root main conv_id |
| `RESUME_IDS` | `claude_sdk_session.rs` | brand:* → root main conv_id |
| `CONV_THREADS` | `codex_app_server.rs` | brand:* → root main conv_id |
| `BRANCH_ROOT_CACHE` | `claude_sdk_session.rs` | brand:* → root main conv_id (helper 1회 lookup 용) |
| `LAST_DELIVERED_KEY` | `session_freshness.rs` | conv_id 그대로 (freshness 비교는 brand 와 main 모두 root key 의 sdk session 식별자를 보므로 동일) |

# brand → main session 연결의 수명주기

| 이벤트 | 동작 |
|---|---|
| brand 첫 send (Claude) | `cache_branch_root_from_db` 가 BRANCH_ROOT_CACHE 채움 → SESSIONS / RESUME_IDS lookup 이 root key 로 정규화 → main 의 sdk-url WS 세션 재사용 |
| brand 첫 send (Codex) | `session_key_for` → CONV_THREADS lookup 이 root key 로 정규화 → main 의 codex thread 재사용 |
| brand send 시 engine 동일 | ContextPack dynamic 섹션 비움 (sdk-url WS / codex thread 가 prior history 보유) |
| brand send 시 engine 다름 | 별 session 으로 fallback. ContextPack 정상 빌드 (정적 + dynamic). main 세션은 그대로 둔다 |
| brand adopt | shadow conv 의 메시지를 summary 로 main 에 insert. session 은 그대로 살아 있음 (다음 main send 가 같은 sdk session 재사용) |
| brand archive/delete | 단순히 shadow conv 만 hidden. session 은 main 이 계속 사용 |

# 엔진별 세션 backbone (현재)

| 엔진 | Backbone | brand 공유 매커니즘 |
|---|---|---|
| claude (claude-code) | `claude --sdk-url` WS + HTTP POST events | SESSIONS/RESUME_IDS root key |
| codex | `codex app-server --listen ws` (JSON-RPC 2.0) | CONV_THREADS root key |
| gemini | one-shot CLI (현재) | brand 공유 미적용 (engine 변경 시 fallback 과 동일 경로) |
| ollama / lmstudio | one-shot HTTP (openai-compat) | 동일 — 매 send 별 session |

# 구현 가이드 — 새 backbone 추가 시

새 엔진을 stateful interactive session 으로 도입할 때 본 정책을 따르려면:

1. session/thread registry 의 모든 lookup 에 `claude_sdk_session::session_key_for(conv_id)` 통과
2. ContextPack continuation 판정 (`session_freshness::current_session_key`) 에
   해당 엔진 분기 추가 — `claude` / `codex` 와 동일한 패턴
3. brand 첫 send 진입점에서 `cache_branch_root_from_db` 가 캐시를 채우도록 보장
4. INV-3 (engine 변경 시 fallback) 보존 검증 — 다른 엔진의 brand 는 normalize
   하지 않는다 (각 엔진의 session 은 분리 유지)

# 사용자 의도 SSOT 보존 (메타 노트)

본 정책의 invariant 들은 **사용자가 raw conversation 에 박아둔 의도** 를 코드
가능한 형태로 옮겨 둔 것이다. s36 → s40 사이에 의도가 task 파이프라인으로
surface 되지 못해 6 세션 간 divergence 가 누적됐고, 그 사이 (s38) 에는 반대
방향 (brand 도 ContextPack 으로 채우는 방향) 의 변경이 들어가기까지 했다.

이 클래스의 의도 누락을 막는 메타 작업은 별 plan 으로 추적된다:
`docs/plans/userIntentSsotSurfacingPlan_2026-04-25.md`.

본 문서를 옮기거나 갱신할 때는 **사용자 의도 인용** 섹션을 절대 약화시키지 말
것 — 의도의 raw evidence 를 잃으면 같은 divergence 가 재발한다.
