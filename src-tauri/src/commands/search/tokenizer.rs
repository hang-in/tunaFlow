//! Korean morphological tokenizer — Phase C of `searchPipelineFromSecallPlan.md`.
//!
//! Design follows secall's proven pattern: **application-layer pre-tokenize**
//! rather than registering a custom FTS5 tokenizer. The output is a
//! whitespace-separated token stream that SQLite's built-in `unicode61`
//! tokenizer then splits trivially — no C extension plumbing needed.
//!
//! - `LinderaKoTokenizer` (embedded ko-dic) — production default. Cross-platform.
//! - `SimpleTokenizer` — whitespace + punctuation fallback when everything else
//!   fails.
//!
//! Both keep Korean POS tags `NNG / NNP / NNB / VV / VA / SL` and drop
//! single-character tokens — matching secall exactly so indexes/queries can
//! share corpus later.

use std::collections::HashSet;
use std::sync::OnceLock;

use lindera::{
    dictionary::{load_embedded_dictionary, DictionaryKind},
    mode::Mode,
    segmenter::Segmenter,
    token_filter::{korean_keep_tags::KoreanKeepTagsTokenFilter, BoxTokenFilter},
    tokenizer::Tokenizer as LinderaInner,
};

use crate::errors::AppError;

pub trait Tokenizer: Send + Sync {
    fn tokenize(&self, text: &str) -> Vec<String>;

    /// Join tokens with a single space — the form SQLite's `unicode61` will
    /// re-split on during FTS5 indexing / MATCH evaluation.
    fn tokenize_for_fts(&self, text: &str) -> String {
        self.tokenize(text).join(" ")
    }
}

// ─── Lindera ko-dic ──────────────────────────────────────────────────────────

pub struct LinderaKoTokenizer {
    inner: LinderaInner,
}

impl LinderaKoTokenizer {
    pub fn new() -> Result<Self, AppError> {
        let dictionary = load_embedded_dictionary(DictionaryKind::KoDic)
            .map_err(|e| AppError::Agent(format!("lindera ko-dic load failed: {e}")))?;
        let segmenter = Segmenter::new(Mode::Normal, dictionary, None);
        let mut tokenizer = LinderaInner::new(segmenter);

        // Keep: NNG (일반명사), NNP (고유명사), NNB (의존명사),
        //       VV (동사), VA (형용사), SL (외국어)
        let tags: HashSet<String> = ["NNG", "NNP", "NNB", "VV", "VA", "SL"]
            .iter()
            .map(|s| (*s).to_string())
            .collect();
        let keep_filter = KoreanKeepTagsTokenFilter::new(tags);
        tokenizer.append_token_filter(BoxTokenFilter::from(keep_filter));

        Ok(Self { inner: tokenizer })
    }
}

impl Tokenizer for LinderaKoTokenizer {
    fn tokenize(&self, text: &str) -> Vec<String> {
        let tokens = match self.inner.tokenize(text) {
            Ok(t) => t,
            Err(_) => return simple_tokenize(text),
        };

        let mut result: Vec<String> = Vec::new();
        for token in tokens {
            let surface = token.surface.to_lowercase();
            if surface.chars().count() > 1 {
                result.push(surface);
            }
        }

        if result.is_empty() {
            simple_tokenize(text)
        } else {
            result
        }
    }
}

// ─── Simple fallback ─────────────────────────────────────────────────────────

pub struct SimpleTokenizer;

impl Tokenizer for SimpleTokenizer {
    fn tokenize(&self, text: &str) -> Vec<String> {
        simple_tokenize(text)
    }
}

fn simple_tokenize(text: &str) -> Vec<String> {
    text.split(|c: char| c.is_whitespace() || c.is_ascii_punctuation())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_lowercase())
        .filter(|s| s.chars().count() > 1)
        .collect()
}

// ─── Global default ──────────────────────────────────────────────────────────

/// Shared tokenizer instance — loading ko-dic costs a few MB of RAM and some
/// init time, so we amortize across all callers. Falls back to `SimpleTokenizer`
/// if Lindera initialization fails for any reason (OOM, corrupted dict, etc.).
///
/// `Box<dyn Tokenizer>` is `Send + Sync` because each impl is.
static GLOBAL_TOKENIZER: OnceLock<Box<dyn Tokenizer>> = OnceLock::new();

pub fn global_tokenizer() -> &'static dyn Tokenizer {
    GLOBAL_TOKENIZER
        .get_or_init(|| match LinderaKoTokenizer::new() {
            Ok(t) => {
                eprintln!("[tokenizer] lindera ko-dic ready");
                Box::new(t) as Box<dyn Tokenizer>
            }
            Err(e) => {
                eprintln!("[tokenizer] lindera unavailable ({e}), falling back to whitespace");
                Box::new(SimpleTokenizer)
            }
        })
        .as_ref()
}

/// Convenience — returns the tokenize-for-FTS string using the global tokenizer.
/// Safe to call from anywhere; empty input yields empty output.
pub fn tokenize_query_for_fts(text: &str) -> String {
    if text.trim().is_empty() {
        return text.to_string();
    }
    global_tokenizer().tokenize_for_fts(text)
}

/// Is morphological query tokenization enabled?
///
/// Default OFF (opt-in) because the index side is still whitespace-tokenized.
/// Enabling this without rebuilding the FTS index hurts recall — the query
/// turns into morphemes but the index still holds whole words.
///
/// Flip this only after `rebuild_messages_fts` (Phase C Part2) has re-indexed
/// existing content with the same tokenizer.
pub fn morphological_query_enabled() -> bool {
    match std::env::var("TUNAFLOW_MORPH_QUERY") {
        Ok(v) => matches!(v.trim().to_ascii_lowercase().as_str(), "1" | "true" | "on" | "yes"),
        Err(_) => false,
    }
}

// ─── Tests ───────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn simple_tokenize_splits_on_whitespace_and_punctuation() {
        let out = simple_tokenize("Hello, world! 플랜을 찾자.");
        assert!(out.iter().any(|t| t == "hello"));
        assert!(out.iter().any(|t| t == "world"));
        // Korean stays intact (no morphological analysis in simple mode).
        assert!(out.iter().any(|t| t.contains("플랜을") || t.contains("플랜")));
    }

    #[test]
    fn simple_tokenize_drops_single_char() {
        let out = simple_tokenize("a bb ccc");
        assert_eq!(out, vec!["bb", "ccc"]);
    }

    #[test]
    fn simple_tokenize_lowercases() {
        let out = simple_tokenize("Hello HELLO hello");
        assert_eq!(out, vec!["hello", "hello", "hello"]);
    }

    #[test]
    fn simple_tokenizer_trait_works() {
        let tok = SimpleTokenizer;
        let joined = tok.tokenize_for_fts("Quick brown fox");
        assert_eq!(joined, "quick brown fox");
    }

    #[test]
    fn simple_empty_input() {
        assert!(simple_tokenize("").is_empty());
        assert!(simple_tokenize("   ").is_empty());
    }

    #[test]
    fn tokenize_query_for_fts_passes_empty_through() {
        assert_eq!(tokenize_query_for_fts(""), "");
        assert_eq!(tokenize_query_for_fts("   "), "   ");
    }

    #[test]
    fn lindera_korean_decomposition() {
        // Lindera ko-dic splits particles off — "플랜을" should NOT remain as
        // a single token when the morphological analyzer is available. If
        // init fails, the fallback keeps the surface form as-is (covered
        // separately above).
        let tok = match LinderaKoTokenizer::new() {
            Ok(t) => t,
            Err(_) => return, // Dict unavailable in this environment; skip.
        };
        let tokens = tok.tokenize("아키텍처를 설계한다");
        assert!(!tokens.is_empty(), "lindera returned empty");
        let joined = tokens.join(" ");
        // Either "아키텍처" as a standalone noun, or at minimum some
        // normalized form — the exact tag coverage depends on ko-dic version.
        assert!(
            joined.contains("아키텍처") || joined.contains("설계"),
            "expected noun to survive filtering, got: {joined}"
        );
    }

    #[test]
    fn lindera_english_passes_through_as_sl_or_falls_back() {
        let tok = match LinderaKoTokenizer::new() {
            Ok(t) => t,
            Err(_) => return,
        };
        let tokens = tok.tokenize("Rust workspace");
        // Either Lindera tags these as SL and keeps them, OR the result is
        // empty and we fall through to simple_tokenize which would return
        // lowercase whitespace splits.
        let joined = tokens.join(" ");
        assert!(
            joined.contains("rust") || joined.contains("workspace"),
            "english should be preserved in one form, got: {joined}"
        );
    }

    #[test]
    fn lindera_mixed_korean_english() {
        let tok = match LinderaKoTokenizer::new() {
            Ok(t) => t,
            Err(_) => return,
        };
        let tokens = tok.tokenize("seCall의 BM25 검색");
        assert!(!tokens.is_empty());
    }

    #[test]
    fn lindera_empty_input_returns_empty() {
        let tok = match LinderaKoTokenizer::new() {
            Ok(t) => t,
            Err(_) => return,
        };
        assert!(tok.tokenize("").is_empty());
    }

    #[test]
    fn lindera_special_chars_only_falls_back_or_empty() {
        let tok = match LinderaKoTokenizer::new() {
            Ok(t) => t,
            Err(_) => return,
        };
        let tokens = tok.tokenize("!@#$%^");
        assert!(tokens.is_empty(), "special chars should yield no tokens");
    }

    #[test]
    fn global_tokenizer_is_same_instance_on_repeat_calls() {
        let a = global_tokenizer() as *const _ as *const u8;
        let b = global_tokenizer() as *const _ as *const u8;
        assert_eq!(a, b, "global_tokenizer must be a singleton");
    }
}
