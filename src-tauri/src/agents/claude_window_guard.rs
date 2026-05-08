//! Claude SDK 세션 누적 window guard — `[1m]` variant detection + cap 분기
//!
//! SSOT: `docs/plans/claudeSdkSessionWindowGuardPlan_2026-05-09.md` Task 04.
//!
//! Plan §0.3: `MAX_TOTAL_PROMPT = 60,000` chars 가 *system 영역 outgoing* 만
//! 가드하고 claude SDK 세션 누적 history 미가드. v0.1.7-beta 이후 사용자 본인
//! 환경에서 Reviewer 단계 *"Prompt is too long"* 회귀 표면화. Root cause = SDK
//! 세션의 누적 input_tokens 가 200K (default) 또는 1M (`[1m]` variant) 한계
//! 초과.
//!
//! 본 모듈은 *분기 인프라* — 모델별 임계값을 결정하는 helper 만 제공하고,
//! 실제 fresh-rotate trigger 는 `claude_sdk_session.rs` 의 stream_run_sdk
//! 진입 path 가 본 모듈의 `current_window_guard_threshold(model_id)` 를 호출.
//!
//! INV-CSW-5 (Plan §1): `[1m]` variant 사용자 (claude-opus-4-7-1m 등) 영향 0 —
//! 1M 모드는 단일 turn 자체가 1M 까지 받으므로 임계 900K (90% 안전마진) 적용,
//! default 모드는 200K limit 의 90% 인 180K 적용.

/// default 모드 (200K context window) 의 임계값. Anthropic 공식 200,000 의 90%
/// 안전마진. Reviewer 등 누적 turn 직전 에 cumulative input_tokens 가 본 값
/// 도달하면 fresh-rotate 발동.
pub const SDK_WINDOW_GUARD_TOKENS_DEFAULT: u64 = 180_000;

/// `[1m]` variant 모드 (1M context window) 의 임계값. 1,000,000 의 90% 안전마진.
/// 본 모드는 사용자가 명시적으로 1M variant 모델 (claude-opus-4-7-1m 등) 을
/// 선택했을 때만 적용 — 일반 default 사용자는 영향 0 (INV-CSW-5).
pub const SDK_WINDOW_GUARD_TOKENS_1M: u64 = 900_000;

/// 모델 ID 가 1M context window variant 인지 판정.
///
/// 매칭 규칙 (둘 중 하나 true 면 variant):
/// 1. `model_id.ends_with("-1m")` — Anthropic 공식 suffix 컨벤션
/// 2. known list — 명시적으로 알려진 1M variant ID (suffix 컨벤션 외)
///
/// 신규 1M variant 추가 시 known list 갱신 별 PR (Architect 결정, Plan §3
/// Task 04 위험 항목).
///
/// 안전한 판정 (false negative 우선) — 매칭 실패 시 default cap (180K) 적용.
/// 1M variant 가 default 로 분류되면 사용자 UX 마찰만 (불필요한 fresh-rotate),
/// 회귀는 0.
pub fn is_1m_variant(model_id: &str) -> bool {
    if model_id.ends_with("-1m") {
        return true;
    }
    // Known 1M variant list — 미래 신규 variant 등록 시 갱신.
    const KNOWN_1M_VARIANTS: &[&str] = &[
        "claude-opus-4-7-1m",
        "claude-haiku-4-5-1m",
        "claude-sonnet-4-6-1m",
    ];
    KNOWN_1M_VARIANTS.contains(&model_id)
}

/// 모델 ID 에 맞는 SDK window guard 임계값을 반환.
///
/// `[1m]` variant 면 `SDK_WINDOW_GUARD_TOKENS_1M` (900K), 그 외엔
/// `SDK_WINDOW_GUARD_TOKENS_DEFAULT` (180K).
///
/// `model_id` 가 None 또는 빈 문자열이면 default cap — 보수적 기본값.
pub fn current_window_guard_threshold(model_id: Option<&str>) -> u64 {
    match model_id {
        Some(id) if !id.is_empty() && is_1m_variant(id) => SDK_WINDOW_GUARD_TOKENS_1M,
        _ => SDK_WINDOW_GUARD_TOKENS_DEFAULT,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn is_1m_variant_detects_known_variants() {
        // -1m suffix 컨벤션
        assert!(is_1m_variant("claude-opus-4-7-1m"));
        assert!(is_1m_variant("claude-haiku-4-5-1m"));
        assert!(is_1m_variant("claude-sonnet-4-6-1m"));

        // 미래 신규 variant — suffix 컨벤션 따른다고 가정 시 자동 인식
        assert!(is_1m_variant("claude-opus-5-0-1m"));

        // default 변종은 false
        assert!(!is_1m_variant("claude-opus-4-7"));
        assert!(!is_1m_variant("claude-sonnet-4-6"));
        assert!(!is_1m_variant("claude-haiku-4-5"));
        assert!(!is_1m_variant("claude-opus-4-7-20260417"));

        // edge cases
        assert!(!is_1m_variant(""));
        assert!(!is_1m_variant("1m"));
        assert!(!is_1m_variant("claude"));
    }

    #[test]
    fn current_window_guard_threshold_returns_correct_value_per_variant() {
        // 1M variant
        assert_eq!(
            current_window_guard_threshold(Some("claude-opus-4-7-1m")),
            SDK_WINDOW_GUARD_TOKENS_1M,
            "1M variant 는 900K 임계"
        );
        assert_eq!(
            current_window_guard_threshold(Some("claude-haiku-4-5-1m")),
            SDK_WINDOW_GUARD_TOKENS_1M,
        );

        // default
        assert_eq!(
            current_window_guard_threshold(Some("claude-opus-4-7")),
            SDK_WINDOW_GUARD_TOKENS_DEFAULT,
            "default 는 180K 임계"
        );
        assert_eq!(
            current_window_guard_threshold(Some("claude-sonnet-4-6")),
            SDK_WINDOW_GUARD_TOKENS_DEFAULT,
        );

        // None / 빈 문자열 → default (보수적)
        assert_eq!(
            current_window_guard_threshold(None),
            SDK_WINDOW_GUARD_TOKENS_DEFAULT,
        );
        assert_eq!(
            current_window_guard_threshold(Some("")),
            SDK_WINDOW_GUARD_TOKENS_DEFAULT,
        );
    }

    #[test]
    fn threshold_constants_are_safe_margins() {
        // Anthropic 공식 한도의 90% 이하
        assert!(SDK_WINDOW_GUARD_TOKENS_DEFAULT <= 200_000 * 9 / 10);
        assert!(SDK_WINDOW_GUARD_TOKENS_1M <= 1_000_000 * 9 / 10);
    }
}
