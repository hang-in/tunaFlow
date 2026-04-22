# Subtask 04 — `search_messages` morphological 분기 + app-level `extract_snippet`

> 상위 plan: [searchPipelineFromSecallPlan-part2.md](./searchPipelineFromSecallPlan-part2.md)

## Changed files

- `src-tauri/src/commands/search/snippet.rs` — 신규. `extract_snippet()` 유틸.
- `src-tauri/src/commands/search/mod.rs` — `pub mod snippet;` + re-export.
- `src-tauri/src/commands/messages.rs` — `search_messages` (`:381`) 에 morphological 분기 + snippet SQL → app-level 호출로 교체.
- `src-tauri/src/commands/search/unified.rs` — `fts_conversation_search` (`:96`) 의 `snippet(messages_fts, ...)` SQL 제거 + `extract_snippet` 호출로 교체. (morphological 분기는 이미 존재 — 변경 없음.)

## Change description

### 1. `extract_snippet`

secall `bm25.rs:191` 포팅:

```rust
// src-tauri/src/commands/search/snippet.rs
/// 쿼리의 첫 non-whitespace term 이 처음 등장하는 지점 주변 ±half 만큼 char window 잘라 반환.
/// - byte boundary 가 아니라 **char boundary** 기준 (한글 다바이트 안전).
/// - 매칭 없으면 앞 `max_chars` char.
/// - 양 끝에 ellipsis `…` 추가.
/// - 매칭 term 을 `**…**` 로 하이라이트 (기존 UI 관습 유지).
pub fn extract_snippet(content: &str, query: &str, max_chars: usize) -> String {
    let chars: Vec<char> = content.chars().collect();
    if chars.is_empty() { return String::new(); }

    // 첫 term 추출 — whitespace split, 빈 제거, 첫 값
    let term = query.split_whitespace().next().unwrap_or("");
    if term.is_empty() || chars.len() <= max_chars {
        // 매칭 불가 or 전체 보여줄 수 있으면 그대로
        return chars.iter().collect();
    }

    // 대소문자 무시 매칭 (char vec 기준)
    let lower: String = content.to_lowercase();
    let term_lower = term.to_lowercase();
    let idx_byte = lower.find(&term_lower);

    let (start_char, end_char) = match idx_byte {
        None => (0usize, max_chars.min(chars.len())),
        Some(byte_idx) => {
            // byte→char 인덱스 복원
            let prefix = &content[..byte_idx];
            let term_start_char = prefix.chars().count();
            let half = max_chars / 2;
            let s = term_start_char.saturating_sub(half);
            let e = (term_start_char + max_chars - (term_start_char - s)).min(chars.len());
            (s, e)
        }
    };

    let mut out = String::new();
    if start_char > 0 { out.push('…'); }
    // term 하이라이트: term_lower 등장 구간을 찾아 `**..**` 삽입
    // 단순 구현: window 를 String 화 후 replace_first (case-insensitive 는 한 번만 수동)
    let window: String = chars[start_char..end_char].iter().collect();
    let hi = highlight_first(&window, term);
    out.push_str(&hi);
    if end_char < chars.len() { out.push('…'); }
    out
}

fn highlight_first(text: &str, term: &str) -> String {
    if term.is_empty() { return text.to_string(); }
    let lower_text = text.to_lowercase();
    let lower_term = term.to_lowercase();
    if let Some(pos) = lower_text.find(&lower_term) {
        // pos 는 byte — 원본 text 에서 해당 char 범위 추출
        let prefix = &text[..pos];
        let matched_bytes = lower_term.len();
        let end = pos + matched_bytes;
        if end <= text.len() && text.is_char_boundary(pos) && text.is_char_boundary(end) {
            let matched = &text[pos..end];
            let after = &text[end..];
            return format!("{}**{}**{}", prefix, matched, after);
        }
    }
    text.to_string()
}
```

테스트 필수:
```rust
#[test] fn extract_handles_korean_multibyte() { ... }
#[test] fn extract_returns_whole_text_if_short() { ... }
#[test] fn extract_adds_leading_ellipsis() { ... }
#[test] fn highlight_preserves_case_from_original() { ... }
```

### 2. `search_messages` (messages.rs:381)

```rust
// before
let mut stmt = conn.prepare(
    "SELECT m.id, m.conversation_id, COALESCE(c.custom_label, c.label, ''), m.role,
            snippet(messages_fts, 0, '**', '**', '…', 40), m.timestamp, m.engine, m.persona
     FROM messages_fts fts
     JOIN messages m ON m.rowid = fts.rowid
     JOIN conversations c ON c.id = m.conversation_id
     WHERE messages_fts MATCH ?1 AND c.project_key = ?2
     ORDER BY rank LIMIT ?3"
)?;

// after — morph 분기 + 원본 content 셀렉션
use crate::commands::search::{morphological_query_enabled, tokenize_query_for_fts, extract_snippet};

let fts_query = if morphological_query_enabled() {
    tokenize_query_for_fts(&effective_query)
} else {
    effective_query.clone()
};

let mut stmt = conn.prepare(
    "SELECT m.id, m.conversation_id, COALESCE(c.custom_label, c.label, ''), m.role,
            m.content, m.timestamp, m.engine, m.persona
     FROM messages_fts fts
     JOIN messages m ON m.rowid = fts.rowid
     JOIN conversations c ON c.id = m.conversation_id
     WHERE messages_fts MATCH ?1 AND c.project_key = ?2
     ORDER BY rank LIMIT ?3"
)?;

let results = stmt.query_map(params![fts_query, project_key, max], |row| {
    let content: String = row.get(4)?;
    // snippet 은 **원본 effective_query** 로 생성 — tokenized 가 아닌 사용자 의도 단어로 하이라이트
    let snippet = extract_snippet(&content, &effective_query, 120);
    Ok(SearchResult {
        message_id: row.get(0)?,
        conversation_id: row.get(1)?,
        conversation_label: row.get(2)?,
        role: row.get(3)?,
        content_snippet: snippet,
        timestamp: row.get(5)?,
        engine: row.get(6)?,
        persona: row.get(7)?,
    })
})?.filter_map(|r| r.ok()).collect();
```

### 3. `fts_conversation_search` (unified.rs:96)

동일 패턴으로 `m.content` 셀렉션 후 `extract_snippet(&content, query, 120)` 호출. `query` 파라미터는 함수로 내려오는 effective_query (unified.rs:73) — tokenized 가 아닌 원본.

**주의**: 현재 unified.rs 는 `query: &str` 만 받는데, morphological 이 활성화되면 `query` 는 이미 tokenized. 원본을 snippet 생성용으로 유지하려면 함수 시그니처를 확장:

```rust
fn fts_conversation_search(
    state: &DbState,
    fts_query: &str,        // FTS MATCH 용 (tokenized or not)
    display_query: &str,    // snippet 하이라이트용 (원본)
    project_key: &str,
    limit: usize,
) -> Result<Vec<UnifiedResult>, AppError>
```

호출부 (unified.rs:82) 도 `display_query = effective_query.clone()` 명시적으로 전달.

## Dependencies

depends_on: [01] — FTS 스키마/트리거가 새 구조여야 함.
(`[02, 03]` 은 없어도 동작하지만 실제 검색 결과 얻으려면 필요.)

## Verification

- **Unit (snippet)**:
  ```rust
  #[test]
  fn snippet_highlights_korean_match() {
      let s = extract_snippet("오늘 아키텍처 설계 회의를 했다. 아키텍처 문서도 정리했다.", "아키텍처", 30);
      assert!(s.contains("**아키텍처**"));
      assert!(s.len() < 40 + 6);  // ellipsis + marker 여유
  }

  #[test]
  fn snippet_returns_whole_when_short() {
      let s = extract_snippet("짧은 글", "짧은", 120);
      assert!(!s.starts_with('…'));
  }

  #[test]
  fn snippet_falls_back_on_no_match() {
      let s = extract_snippet("아무것도 매칭되지 않는 긴 문장입니다. 매우 긴 문장입니다.", "플랜", 20);
      assert!(!s.contains("**"));
  }
  ```
- **Integration (search_messages)**:
  ```rust
  #[tokio::test]
  async fn search_messages_uses_morph_when_flag_on() {
      // seed 1 message "아키텍처를 설계한다" with content_tokenized = "아키텍처 설계"
      std::env::set_var("TUNAFLOW_MORPH_QUERY", "1");
      let res = search_messages("아키텍처를".into(), "proj".into(), None, state).unwrap();
      assert_eq!(res.len(), 1);
      assert!(res[0].content_snippet.contains("**아키텍처**"));
      std::env::remove_var("TUNAFLOW_MORPH_QUERY");
  }
  ```
- `cargo test --lib commands::search::snippet`
- `cargo test --lib commands::messages::tests::search_messages_*`
- `cargo check`
- `npx tsc --noEmit` — exit 0 (FE 는 snippet 포맷 동일하므로 변경 없음).

## Risks

- **Multi-term 쿼리**: `extract_snippet` 은 첫 term 만 하이라이트. 사용자 쿼리 "플랜 검색" → 첫 term "플랜" 만. 기존 `snippet()` 은 FTS5 가 여러 term 을 처리. 기능 regression 이지만 secall 동일 수준. 후속 개선 별도.
- **대소문자 매칭**: `to_lowercase()` 가 ASCII 외 문자에 대해 Unicode-aware. 한글은 변화 없음. 영어 대소문자 mix 쿼리에서도 정상.
- **Byte-vs-char boundary**: 한글은 UTF-8 multi-byte. `str::find` 는 byte idx 반환 → char idx 복원 필요 (위 코드 참조). 테스트로 명시 커버.
- **Snippet 에서 하이라이트 마커 (`**...**`) 가 content 원본에도 존재하는 경우**: 예를 들어 사용자가 markdown `**bold**` 를 입력한 메시지. highlight 가 추가적으로 주입되면 markdown 파서가 혼동. **수용**: 기존 `snippet()` 도 `'**'` 사용 → UI 가 이미 처리 중. 변경 없음.
- **unified.rs 시그니처 변경**: 내부 함수이므로 caller 1 곳만 수정. breaking 위험 낮음.
