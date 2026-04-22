# ContextPack DB/Assembly 분리 리팩토링

프로젝트: `/Users/d9ng/privateProject/tunaFlow`
모든 응답과 보고는 한국어로 작성하라.

---

## 사전 읽기 (필수)

아래 문서와 파일을 **반드시** 먼저 읽어라. 읽지 않고 작업하면 기존 동작이 깨진다.

1. `CLAUDE.md` — 프로젝트 전체 구조, 코딩 컨벤션, 안전 규칙
2. `src-tauri/src/commands/agents_helpers/send_common.rs` — 리팩토링 대상 (전체 읽기)
3. `src-tauri/src/commands/agents_helpers/context_pack.rs` — 섹션 빌더 함수들
4. `src-tauri/src/commands/context_queries.rs` — DB 쿼리 함수들
5. `src-tauri/src/commands/agents.rs` — 호출부 (prepare_engine_run 사용처)
6. `src-tauri/src/guardrail.rs` — 예산 상수 + truncation 함수
7. `src-tauri/src/commands/conversation_memory.rs` — load_compressed_memory 함수

---

## 배경

`build_normalized_prompt_with_budget()` 함수 (send_common.rs, line 155-553)가 현재 540줄이며, 내부에서 DB 쿼리 + 프롬프트 조립 + 예산 적용을 **한 함수 안에서** 전부 수행한다.

이 구조의 문제:
- **단위 테스트 불가능**: DB `Connection`이 필수이므로 순수 조립 로직을 테스트할 수 없음
- **DB lock 범위 과대**: `prepare_engine_run()`이 이 함수를 호출하는 동안 write lock을 보유
- **섹션 빌더가 conn을 직접 받음**: build_plan_section(conn), build_findings_section(conn) 등

---

## 목표

`build_normalized_prompt_with_budget(conn, ...)`를 두 단계로 분리:

```
Phase A: load_context_data(conn, ...) → ContextData   (DB 의존, 빠르게 실행)
Phase B: assemble_prompt(data, ...)   → (String, Option<String>, ContextPackMeta)  (순수 함수, DB 무관)
```

기존 동작과 결과가 **완전히 동일**해야 한다. 프론트엔드 호출 시그니처 변경 없음.

---

## 현재 함수 내 DB 쿼리 목록 (Phase A로 추출 대상)

`build_normalized_prompt_with_budget()` 안에서 발생하는 DB 호출:

| # | 위치 (현재 line) | 쿼리 | 용도 |
|---|---|---|---|
| 1 | ~197 | `SELECT COUNT(*) > 0 FROM plans WHERE conversation_id=? AND status='active'` | auto mode: has_plan 확인 |
| 2 | ~299 | `load_recent_messages_with_author(conn, conversation_id, 6)` | recent context (현재 대화) |
| 3 | ~301-308 | `SELECT parent_id FROM conversations WHERE id=?` + `load_recent_messages_with_author(conn, parent_id, 4)` | parent context (branch) |
| 4 | ~332 | `resolve_plan_conversation_id(conn, conversation_id)` | plan lookup용 conv ID |
| 5 | ~334 | `build_plan_section(conn, plan_conv_id)` | plan 섹션 빌드 |
| 6 | ~341 | `build_findings_section(conn, plan_conv_id)` | findings 섹션 빌드 |
| 7 | ~348 | `build_artifact_handoff_section(conn, plan_conv_id)` | artifacts 섹션 빌드 |
| 8 | ~363-367 | `SELECT project_key FROM conversations WHERE id=?` | retrieval용 project_key |
| 9 | ~371-376 | `SELECT id FROM messages WHERE conversation_id=? ORDER BY timestamp DESC LIMIT 12` + `retrieve_relevant_chunks_with_overlap(...)` | retrieval chunks |
| 10 | ~409 | `load_compressed_memory(conn, conversation_id)` | compressed memory |
| 11 | ~449-451 | `conversation_label(conn, id)` + `load_recent_messages(conn, id, 3)` (per cross_session_id) | cross-session context |
| 12 | ~466 | `build_thread_inheritance_section(conn, conversation_id)` | branch thread inheritance |

---

## 작업 단계

### Step 1: ContextData 구조체 정의

`send_common.rs`에 (또는 새 파일 `context_data.rs`에) 구조체를 만든다:

```rust
/// All data needed to assemble a ContextPack, pre-loaded from DB.
pub struct ContextData {
    pub conversation_id: String,
    pub project_path: Option<String>,
    pub prompt: String,
    pub is_branch: bool,

    // Auto mode signals
    pub has_active_plan: bool,

    // Recent context
    pub current_messages: Vec<(String, String, Option<String>, Option<String>)>, // (role, content, engine, persona)
    pub parent_messages: Vec<(String, String, Option<String>, Option<String>)>,

    // Structured memory
    pub plan_section: Option<String>,       // pre-built by build_plan_section
    pub findings_section: Option<String>,   // pre-built by build_findings_section
    pub artifacts_section: Option<String>,  // pre-built by build_artifact_handoff_section

    // Retrieval
    pub project_key: Option<String>,
    pub recent_message_ids: Vec<String>,
    pub retrieval_chunks: Vec<crate::commands::context_queries::RetrievedChunk>,

    // Compressed memory
    pub compressed_memory: Option<String>,

    // Cross-session
    pub cross_session_data: Vec<(String, Vec<(String, String)>)>, // (label, messages)

    // Thread inheritance
    pub thread_inheritance: Option<String>,

    // Pass-through (no DB needed)
    pub active_skills: Vec<String>,
    pub cross_session_ids: Vec<String>,
    pub persona_fragment: Option<String>,
    pub context_mode_override: Option<String>,
    pub context_budget_cap: Option<usize>,
}
```

### Step 2: load_context_data() 함수 작성

DB 쿼리 12개를 이 함수에 모은다. 시그니처:

```rust
pub fn load_context_data(
    conn: &Connection,
    conversation_id: &str,
    prompt: &str,
    project_path: Option<&str>,
    active_skills: &[String],
    cross_session_ids: &[String],
    persona_fragment: Option<&str>,
    context_mode_override: Option<&str>,
    context_budget_cap: Option<usize>,
) -> ContextData
```

**주의**: retrieval chunks 로딩 시 `existing_context` (이미 조립된 섹션 텍스트)가 필요한데, 이건 아직 조립 전이므로 Phase A에서는 None으로 전달하거나, retrieval을 Phase B에서 lazy하게 처리해야 한다.

**해결 방법**: retrieval의 overlap penalty용 `existing_context`는 retrieval 품질에만 영향. Phase A에서 `existing_context = None`으로 chunks를 가져오고, overlap penalty는 Phase B의 assemble 단계에서 점수 재조정하거나, 단순히 None으로 전달해도 품질 차이가 미미함. 기존 동작과의 차이를 최소화하려면 `retrieve_relevant_chunks_with_overlap(..., None)`으로 호출.

### Step 3: assemble_prompt() 함수 작성

DB 의존 없는 순수 함수:

```rust
pub fn assemble_prompt(
    data: &ContextData,
    identity_fragment: Option<&str>,
) -> (String, Option<String>, ContextPackMeta)
```

현재 `build_normalized_prompt_with_budget()`의 line 170-553에 해당하는 조립 로직을 여기로 이동. 단, DB 쿼리 호출 부분은 모두 `data.필드`로 대체.

### Step 4: build_normalized_prompt_with_budget() 리와이어

기존 함수를 두 단계 호출로 변경:

```rust
pub fn build_normalized_prompt_with_budget(
    conn: &Connection,
    conversation_id: &str,
    prompt: &str,
    project_path: Option<&str>,
    active_skills: &[String],
    cross_session_ids: &[String],
    persona_fragment: Option<&str>,
    context_mode_override: Option<&str>,
    context_budget_cap: Option<usize>,
) -> (String, Option<String>, ContextPackMeta) {
    let data = load_context_data(conn, conversation_id, prompt, project_path, active_skills, cross_session_ids, persona_fragment, context_mode_override, context_budget_cap);
    assemble_prompt(&data, persona_fragment)
}
```

**반환 타입과 시그니처는 변경하지 않는다.** 호출부 수정 없음.

### Step 5: prepare_engine_run() DB lock 범위 축소

현재 (send_common.rs ~577-607):
```rust
let conn = state.write.lock()?;
persist_user_message(...);
let (ep, sys_ctx, meta) = build_normalized_prompt_with_budget(&conn, ...); // 여기서 DB 쿼리 12개
// ... pre-create msg, etc.
// lock released
```

변경 후:
```rust
let data = {
    let conn = state.write.lock()?;
    persist_user_message(...);
    let data = load_context_data(&conn, ...);
    // pre-create streaming msg + create job도 여기서
    data
    // lock released
};
// DB lock 없이 순수 조립
let (ep, sys_ctx, meta) = assemble_prompt(&data, identity_frag);
```

### Step 6: 단위 테스트 추가

`assemble_prompt()`는 `ContextData` fixture만으로 테스트 가능:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    fn empty_context_data() -> ContextData {
        ContextData {
            conversation_id: "test-conv".into(),
            project_path: Some("/tmp/test".into()),
            prompt: "hello".into(),
            is_branch: false,
            has_active_plan: false,
            current_messages: vec![],
            parent_messages: vec![],
            plan_section: None,
            findings_section: None,
            artifacts_section: None,
            project_key: None,
            recent_message_ids: vec![],
            retrieval_chunks: vec![],
            compressed_memory: None,
            cross_session_data: vec![],
            thread_inheritance: None,
            active_skills: vec![],
            cross_session_ids: vec![],
            persona_fragment: None,
            context_mode_override: None,
            context_budget_cap: None,
        }
    }

    #[test]
    fn assemble_empty_data_returns_prompt_only() {
        let data = empty_context_data();
        let (assembled, sys_ctx, meta) = assemble_prompt(&data, None);
        assert!(assembled.contains("hello"));
        assert_eq!(meta.sections.len(), 1); // project only
    }

    #[test]
    fn assemble_with_plan_includes_plan_section() {
        let mut data = empty_context_data();
        data.plan_section = Some("## Plan\n- Step 1".into());
        data.context_mode_override = Some("standard".into());
        let (_, _, meta) = assemble_prompt(&data, None);
        assert!(meta.sections.contains(&"plan".to_string()));
    }

    #[test]
    fn auto_mode_short_prompt_selects_lite() {
        let mut data = empty_context_data();
        data.prompt = "ㅇㅇ".into();
        let (_, _, meta) = assemble_prompt(&data, None);
        assert!(meta.mode.contains("Lite"));
    }

    #[test]
    fn auto_mode_with_skills_pushes_toward_full() {
        let mut data = empty_context_data();
        data.active_skills = vec!["a".into(), "b".into(), "c".into()];
        let (_, _, meta) = assemble_prompt(&data, None);
        assert!(meta.mode.contains("Full"));
    }
}
```

---

## 호출부 — 수정이 필요한 곳

| 파일 | 함수 | 변경 내용 |
|------|------|----------|
| `send_common.rs` | `build_normalized_prompt_with_budget()` | Phase A + B 호출로 리와이어 |
| `send_common.rs` | `build_normalized_prompt()` | 변경 없음 (위 함수를 호출) |
| `send_common.rs` | `prepare_engine_run()` | DB lock 범위 축소 (load_context_data → release → assemble_prompt) |
| `agents.rs` | `start_claude_stream` 등 4개 | **변경 없음** (prepare_engine_run을 호출) |

---

## 절대 하지 말 것

1. **반환 타입 변경 금지**: `build_normalized_prompt_with_budget()`의 `(String, Option<String>, ContextPackMeta)` 반환 타입 유지
2. **호출 시그니처 변경 금지**: 기존 함수의 파라미터 목록 변경 금지. 새 함수를 추가하고 기존 함수가 위임하는 구조
3. **프론트엔드 수정 금지**: Tauri command 시그니처 불변
4. **섹션 빌더 함수 수정 금지**: `build_plan_section()`, `build_findings_section()` 등은 현재 `conn`을 받는 시그니처 유지. 이 함수들의 DB 분리는 별도 작업
5. **retrieval 로직 변경 금지**: `retrieve_relevant_chunks_with_overlap()`의 알고리즘/가중치 변경하지 말 것. 호출 위치만 Phase A로 이동
6. **한 번에 여러 파일을 동시에 큰 폭으로 바꾸지 말 것**: Step 1-6 순서대로 진행하고 각 단계마다 `cargo check` 확인

---

## 검증

각 단계 완료 후:
```bash
cd src-tauri && cargo check          # 컴파일
cd src-tauri && cargo test --lib     # 53+ 테스트 통과 (새 테스트 포함)
cd .. && npx tsc --noEmit            # TypeScript
cd .. && npx vitest run              # Frontend 55 테스트
```

최종 확인:
- `build_normalized_prompt_with_budget()`의 기존 호출부 3곳이 모두 동일하게 동작
- `prepare_engine_run()`의 DB lock이 `load_context_data()` 범위로 축소됨
- `assemble_prompt()`에 최소 4개 단위 테스트 추가됨
- 기존 53개 Rust 테스트 + 55개 Frontend 테스트 전부 통과

---

## 성공 기준

- `assemble_prompt()`가 `Connection`을 받지 않는 순수 함수
- `ContextData` fixture로 auto mode, section 포함/제외, budget 적용을 테스트 가능
- `prepare_engine_run()`의 DB lock 범위가 `load_context_data()` 호출까지로 축소
- 기존 동작 100% 보존 (프론트엔드 호출 시그니처 불변)
