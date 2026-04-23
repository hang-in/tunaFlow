use serde::ser::SerializeStruct;
use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("Database error: {0}")]
    Database(#[from] rusqlite::Error),

    #[error("Not found: {0}")]
    NotFound(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("Agent error: {0}")]
    Agent(String),

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Lock poisoned")]
    Lock,
}

impl AppError {
    /// Returns a snake_case code identifier for this error variant.
    pub fn code(&self) -> &'static str {
        match self {
            AppError::Database(_) => "db_error",
            AppError::NotFound(_) => "not_found",
            AppError::Io(_) => "io_error",
            AppError::Json(_) => "json_error",
            AppError::Agent(_) => "agent_error",
            AppError::BadRequest(_) => "bad_request",
            AppError::Lock => "lock_error",
        }
    }

    /// Payload string (without the variant prefix). FE 는 `t('error.${code}', { context })`
    /// 패턴으로 사용자 메시지 렌더. i18nPlan Phase 4A-1 / INV-2.
    pub fn context(&self) -> String {
        match self {
            AppError::Database(e) => e.to_string(),
            AppError::NotFound(s) => s.clone(),
            AppError::Io(e) => e.to_string(),
            AppError::Json(e) => e.to_string(),
            AppError::Agent(s) => s.clone(),
            AppError::BadRequest(s) => s.clone(),
            AppError::Lock => String::new(),
        }
    }
}

/// Serializes as `{ "code": "...", "context": "...", "message": "..." }`.
/// `context` 가 plan §1.1 / Phase 4A-1 의 신규 계약. `message` 는 backward-compat
/// 용으로 유지 (FE 의 기존 catch 블록이 점진적으로 `t('error.${code}')` 로 이관
/// 될 때까지). 메시지는 영어 (thiserror Display) — locale 변환은 FE 담당.
impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let mut s = serializer.serialize_struct("AppError", 3)?;
        s.serialize_field("code", self.code())?;
        s.serialize_field("context", &self.context())?;
        s.serialize_field("message", &self.to_string())?;
        s.end()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serializes_with_code_context_and_message() {
        let err = AppError::NotFound("plan-42".into());
        let json = serde_json::to_value(&err).unwrap();
        assert_eq!(json["code"], "not_found");
        assert_eq!(json["context"], "plan-42");
        assert!(json["message"].as_str().unwrap().contains("plan-42"));
    }

    #[test]
    fn lock_error_has_empty_context() {
        let err = AppError::Lock;
        let json = serde_json::to_value(&err).unwrap();
        assert_eq!(json["code"], "lock_error");
        assert_eq!(json["context"], "");
    }

    #[test]
    fn all_variants_produce_stable_codes() {
        let pairs: &[(AppError, &str)] = &[
            (AppError::NotFound("x".into()), "not_found"),
            (AppError::Agent("y".into()), "agent_error"),
            (AppError::BadRequest("z".into()), "bad_request"),
            (AppError::Lock, "lock_error"),
        ];
        for (err, expected_code) in pairs {
            assert_eq!(err.code(), *expected_code);
        }
    }
}
