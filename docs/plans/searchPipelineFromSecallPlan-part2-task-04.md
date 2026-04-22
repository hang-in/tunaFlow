# Subtask 04 — `search_messages` morphological 분기 + app-level `extract_snippet`

> 상위 plan: [searchPipelineFromSecallPlan-part2.md](./searchPipelineFromSecallPlan-part2.md)

## Changed files

- `src-tauri/src/commands/search/snippet.rs` — 신규. `extract_snippet()` 유틸.
- `src-tauri/src/commands/search/mod.rs` — `pub mod snippet;` + re-export.
- `src-tauri/src/commands/messages.rs` — `search_messages` (`:381`) 에 morphological 분기 + snippet SQL → app-level 호출로 교체.
- `src-tauri/src/commands/search/unified.rs` — `fts_conversation_search` (`:96`) 의 `snippet(messages_fts, ...)` SQL 제거 + `extract_snippet` 호출로 교체. (morphological 분기는 이미 존재 — 변경 없음.)

## Change description

### 1. `extract_snippet` — char-window 추출 (하이라이트 없음)

> **Codex review 2026-04-22 반영**: 초안은 `content.to_lowercase()` 의 byte offset 을 원본 `content` 에 역매핑하는 구조였는데, Turkish `İ`→`i̇` / German `ß`→`ss` 등 Unicode case expansion 에서 byte length 가 변해 boundary panic. 하이라이트 마커 `**…**` 를 이 단계에서 완전 제거하고 **순수 char-window 추출** 만 수행한다. 하이라이트는 UI 측 클라이언트 렌더 (React) 또는 후속 별도 plan 으로 분리.

```rust
// src-tauri/src/commands/search/snippet.rs
/// Case-insensitive first-term 매칭 지점 주변 ±half 만큼의 **char window** 추출.
/// - char boundary 보존 (multi-byte 안전).
/// - byte offset 이 아니라 **char index 만** 사용 — Unicode case expansion 에서도 panic 없음.
/// - 매칭 없으면 앞 `max_chars` char.
/// - 양 끝 축약 표시 `…` 추가.
/// - **하이라이트 마커는 삽입하지 않는다**. 호출자가 UI 단에서 DOM 하이라이트.
pub fn extract_snippet(content: &str, query: &str, max_chars: usize) -> String {
    let chars: Vec<char> = content.chars().collect();
    if chars.is_empty() { return String::new(); }
    if chars.len() <= max_chars { return content.to_string(); }

    let term = query.split_whitespace().next().unwrap_or("");
    let match_at = if term.is_empty() { None } else { find_term_char_index(&chars, term) };

    let (s, e) = match match_at {
        None => (0usize, max_chars.min(chars.len())),
        Some(idx) => {
            let half = max_chars / 2;
            let s0 = idx.saturating_sub(half);
            (s0, (s0 + max_chars).min(chars.len()))
        }
    };

    let mut out = String::new();
    if s > 0 { out.push('…'); }
    out.extend(chars[s..e].iter());
    if e < chars.len() { out.push('…'); }
    out
}

/// Char-level case-insensitive substring search. No byte-offset translation.
///
/// 한계: term 쪽이 Unicode case expansion 을 일으키는 글자 (`ß`) 를 포함하면
/// 완전 매칭이 어렵다 — 해당 케이스는 매칭 실패로 처리 (false negative 수용).
/// 기본 사용처 (한글/영문 일반 쿼리) 는 안전. 완전한 Unicode case folding 은
/// 후속 plan (ICU 의존성 도입 또는 별도 기능).
fn find_term_char_index(haystack: &[char], term: &str) -> Option<usize> {
    // term 을 char 단위로 to_lowercase — multi-char case expansion 시 첫 char 만 채용.
    // 즉 "ß" (= "ss") 의 경우 첫 char "s" 로 비교해 false negative 수용.
    let t_lower: Vec<char> = term.chars()
        .filter_map(|c| c.to_lowercase().next())
        .collect();
    if t_lower.is_empty() || t_lower.len() > haystack.len() { return None; }
    'outer: for start in 0..=(haystack.len() - t_lower.len()) {
        for (i, &tc) in t_lower.iter().enumerate() {
            let hc_first = haystack[start + i].to_lowercase().next();
            if hc_first != Some(tc) { continue 'outer; }
        }
        return Some(start);
    }
    None
}
```

**필수 테스트** (Codex 가 반례로 지목한 케이스 포함):

```rust
#[test]
fn snippet_handles_turkish_dotted_i_without_panic() {
    // "İ" -> "i̇" (i + combining dot). 초안 코드는 여기서 byte boundary panic.
    let s = extract_snippet("Start AİB middle padding text that is long enough", "b", 20);
    assert!(!s.is_empty(), "panic 없이 반환되어야");
    // 매칭 여부는 보장하지 않음 — panic-free 가 primary 기대.
}

#[test]
fn snippet_handles_german_sharp_s() {
    // "ß" -> "ss". term="strasse" 로 "Straße" 매칭 시도하면 false negative 수용.
    let s = extract_snippet("In der Straße steht ein Haus mit vielen Fenstern heute", "straße", 30);
    assert!(!s.is_empty());
}

#[test]
fn snippet_korean_multibyte_safe() {
    let s = extract_snippet("오늘 아키텍처 설계 회의를 했다. 아키텍처 문서도 정리했다.", "아키텍처", 20);
    assert!(s.contains("아키텍처"));
    assert!(!s.contains("**"), "하이라이트 마커 삽입 금지");
}

#[test]
fn snippet_returns_whole_when_short() {
    let s = extract_snippet("짧은 글", "짧은", 120);
    assert_eq!(s, "짧은 글");
}

#[test]
fn snippet_no_match_returns_prefix() {
    let s = extract_snippet("아무것도 매칭되지 않는 긴 문장입니다. 매우 긴 문장입니다. 정말 긴 문장", "플랜", 20);
    assert!(s.chars().count() <= 21);  // +1 for optional leading ellipsis
    assert!(!s.contains("**"));
}

#[test]
fn snippet_empty_input() {
    assert_eq!(extract_snippet("", "anything", 100), "");
}
```

UI 측 하이라이트는 `query` 를 받아 React 에서 `<mark>` 로 처리 (별도 PR 또는 Subtask 05 범위 조정). 현재 검색 UI 가 `**...**` 마크다운을 렌더하는 경로를 사용 중이면 그 경로를 **임시로 query string 기반 클라이언트 사이드 highlight** 로 교체해야 UX regression 최소.

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
    // snippet 은 **원본 effective_query** 로 생성 — tokenized 가 아닌 사용자 의도 단어로 window 추출.
    // 하이라이트 마커는 포함되지 않음 (UI 측에서 query 기반 client-side highlight 수행).
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

> **Codex review 2026-04-22 (2차) 반영**: 이전 verification 은 `**하이라이트**` 를 검사하는 구 계약. 본 verification 은 **char-window + 하이라이트 없음** 계약으로 재작성. 하이라이트 기능 자체는 UI 단에서 담당.

- **Unit (snippet)**:
  ```rust
  #[test]
  fn snippet_extracts_korean_window_without_markers() {
      let s = extract_snippet("오늘 아키텍처 설계 회의를 했다. 아키텍처 문서도 정리했다.", "아키텍처", 30);
      assert!(s.contains("아키텍처"), "매칭 term 포함");
      assert!(!s.contains("**"), "하이라이트 마커는 삽입 금지");
      assert!(s.chars().count() <= 32, "±ellipsis 여유 포함 상한");
  }

  #[test]
  fn snippet_returns_whole_when_short() {
      let s = extract_snippet("짧은 글", "짧은", 120);
      assert_eq!(s, "짧은 글");
      assert!(!s.contains("…"));
  }

  #[test]
  fn snippet_falls_back_on_no_match() {
      let s = extract_snippet("아무것도 매칭되지 않는 긴 문장입니다. 매우 긴 문장입니다. 정말 긴 문장.", "플랜", 20);
      assert!(!s.contains("**"));
      assert!(!s.is_empty());
  }

  #[test]
  fn snippet_handles_turkish_dotted_i_no_panic() {
      // content.to_lowercase() = "ai̇b" (i + combining dot) — byte offset 역매핑 시 panic 지점.
      // 신규 구현은 char-only 이므로 panic 없이 반환되어야.
      let s = extract_snippet("Start AİB middle padding text long enough for window", "b", 20);
      assert!(!s.is_empty(), "panic 없이 반환");
  }

  #[test]
  fn snippet_handles_german_sharp_s_gracefully() {
      // "ß" ↔ "ss" multi-char expansion. false negative 는 수용, panic 은 금지.
      let s = extract_snippet("In der Straße steht ein Haus mit vielen Fenstern heute", "straße", 30);
      assert!(!s.is_empty());
  }

  #[test]
  fn snippet_empty_input() {
      assert_eq!(extract_snippet("", "anything", 100), "");
  }
  ```
- **Integration (search_messages)**:
  ```rust
  #[tokio::test]
  async fn search_messages_uses_morph_when_flag_on() {
      // seed 1 message "아키텍처를 설계한다" with content_tokenized = "아키텍처 설계"
      std::env::set_var("TUNAFLOW_MORPH_QUERY", "1");
      let res = search_messages("아키텍처를".into(), "proj".into(), None, state).unwrap();
      assert_eq!(res.len(), 1, "morph 쿼리가 인덱스 매칭되어야");
      assert!(res[0].content_snippet.contains("아키텍처"), "snippet 원본에 term 포함");
      assert!(!res[0].content_snippet.contains("**"), "하이라이트 마커는 반환값에 포함되지 않음");
      std::env::remove_var("TUNAFLOW_MORPH_QUERY");
  }
  ```
- `cargo test --lib commands::search::snippet`
- `cargo test --lib commands::messages::tests::search_messages_*`
- `cargo check`
- `npx tsc --noEmit` — exit 0 (FE 는 하이라이트 client-side 렌더 전환이 Subtask 05 와 조율 범위. 본 subtask 에서는 snippet 반환 포맷 변경만 — FE 가 plain text 로 받는 것에는 호환).

## Risks

- **Multi-term 쿼리**: `extract_snippet` 은 첫 term 만 매칭. 사용자 쿼리 "플랜 검색" → 첫 term "플랜" 만. 기존 `snippet()` 은 FTS5 가 여러 term 을 처리. 기능 regression 이지만 secall 동일 수준. 후속 개선 별도.
- **대소문자 매칭 — 완전한 Unicode case folding 미지원**: `to_lowercase()` 가 multi-char case expansion (ß→ss, İ→i̇) 을 일으키는 경우 matching 은 first-char 비교로 false negative 수용 (panic 은 없음). 완전한 case folding 은 ICU 의존성 도입 필요 — 별도 plan.
- **Byte-vs-char boundary**: 본 구현은 **byte offset 을 전혀 사용하지 않음**. `chars().collect::<Vec<char>>()` 와 char index 만 사용. multi-byte 안전. Codex review 로 확정.
- **Snippet 에서 하이라이트 마커 (`**...**`) 를 삽입하지 않음**: 기존 UI 가 `**...**` 마크다운을 렌더하던 경로는 query 기반 client-side highlight (React `<mark>` 또는 유사) 로 교체 필요. 이 UX 부분은 Subtask 05 와 함께 조정.
- **unified.rs 시그니처 변경**: 내부 함수이므로 caller 1 곳만 수정. breaking 위험 낮음.
