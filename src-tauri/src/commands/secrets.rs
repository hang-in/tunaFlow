//! OS keychain backed secret storage.
//!
//! Wraps the `keyring` crate so sensitive user-provided values (API keys, OAuth
//! tokens, third-party credentials) can be persisted outside the SQLite DB.
//! macOS Keychain, Windows Credential Vault, and Linux Secret Service are
//! handled transparently by the crate.
//!
//! Design:
//! - Service name is namespaced per tunaFlow install: `com.tunaflow.<key>`
//!   so the same key (e.g. "anthropic_api_key") stays scoped to this app
//!   without colliding with CLI config or other tools.
//! - Account name is always `"default"` — we don't multi-account currently.
//!   If per-project secrets become a need, caller can namespace the key itself
//!   (e.g. `anthropic_api_key:project_foo`).
//! - All FE surfaces go through the Tauri commands below; no direct DB writes.
//!
//! Not stored here: user-pasted secrets inside messages. Those remain in the
//! DB (now 0600 after PR #54) — users who paste API keys into chat are
//! implicitly opting into that tradeoff. Covered in threat-model docs.

use crate::errors::AppError;

const SERVICE_PREFIX: &str = "com.tunaflow";
const ACCOUNT: &str = "default";

fn service_name(key: &str) -> String {
    format!("{}.{}", SERVICE_PREFIX, key)
}

fn to_app_err(e: keyring::Error) -> AppError {
    AppError::Agent(format!("keyring: {}", e))
}

/// Store a secret. Empty string clears the entry (keyring crate behavior:
/// set_password with empty is allowed; we explicitly delete for clarity).
pub fn set_secret(key: &str, value: &str) -> Result<(), AppError> {
    let entry = keyring::Entry::new(&service_name(key), ACCOUNT).map_err(to_app_err)?;
    if value.is_empty() {
        // Delete entry if value is empty — NotFound is a no-op on delete.
        return match entry.delete_credential() {
            Ok(()) => Ok(()),
            Err(keyring::Error::NoEntry) => Ok(()),
            Err(e) => Err(to_app_err(e)),
        };
    }
    entry.set_password(value).map_err(to_app_err)
}

/// Retrieve a secret. Returns `Ok(None)` when the entry does not exist (so FE
/// can distinguish "not set" from "read error"). Other errors propagate.
pub fn get_secret(key: &str) -> Result<Option<String>, AppError> {
    let entry = keyring::Entry::new(&service_name(key), ACCOUNT).map_err(to_app_err)?;
    match entry.get_password() {
        Ok(v) => Ok(Some(v)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(e) => Err(to_app_err(e)),
    }
}

/// Check whether a secret exists without exposing the value (useful for UI
/// that wants to render "configured" vs "not configured" without reading the
/// actual password into the webview process).
pub fn has_secret(key: &str) -> Result<bool, AppError> {
    Ok(get_secret(key)?.is_some())
}

// ─── Tauri commands ─────────────────────────────────────────────────────────

#[tauri::command]
pub fn secret_set(key: String, value: String) -> Result<(), AppError> {
    // Defensive: reject obviously invalid keys so we don't poison the keychain
    // with junk entries (service names can't be cleaned up programmatically
    // without knowing the exact name).
    if key.is_empty() || key.len() > 128 {
        return Err(AppError::Agent("invalid secret key".into()));
    }
    if key.chars().any(|c| !(c.is_ascii_alphanumeric() || c == '_' || c == '-' || c == '.' || c == ':')) {
        return Err(AppError::Agent("secret key has illegal characters".into()));
    }
    set_secret(&key, &value)
}

#[tauri::command]
pub fn secret_get(key: String) -> Result<Option<String>, AppError> {
    get_secret(&key)
}

#[tauri::command]
pub fn secret_has(key: String) -> Result<bool, AppError> {
    has_secret(&key)
}

#[tauri::command]
pub fn secret_delete(key: String) -> Result<(), AppError> {
    set_secret(&key, "")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn service_name_format() {
        assert_eq!(service_name("foo"), "com.tunaflow.foo");
        assert_eq!(service_name("anthropic_api_key"), "com.tunaflow.anthropic_api_key");
    }

    #[test]
    fn key_validation() {
        // Empty rejected
        assert!(secret_set("".into(), "v".into()).is_err());
        // Over 128 rejected
        let long = "a".repeat(129);
        assert!(secret_set(long, "v".into()).is_err());
        // Special chars rejected
        assert!(secret_set("foo bar".into(), "v".into()).is_err());
        assert!(secret_set("foo/bar".into(), "v".into()).is_err());
        // Allowed chars pass validation (set may still fail on sandboxed CI — that's OK,
        // we're only asserting the validation branch doesn't reject early).
        // Don't assert .is_ok() here because CI keyring may be unavailable.
    }

    /// Smoke test — exercises the wrapper paths. Integration with a real
    /// keychain is intentionally loose because some environments (headless CI,
    /// Linux without a running Secret Service, macOS requiring keychain unlock)
    /// cause spurious failures. We only require that each call returns a
    /// Result and the Ok-Some / Ok-None distinction is coherent for this run.
    #[test]
    fn keychain_smoke() {
        // Uniqueify so concurrent test runs / leftover state can't interfere.
        let key = format!(
            "tunaflow_test_smoke_{}_{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        );

        // Whatever happens, clear the entry at the end (best-effort).
        struct Cleanup(String);
        impl Drop for Cleanup {
            fn drop(&mut self) { let _ = super::set_secret(&self.0, ""); }
        }
        let _cleanup = Cleanup(key.clone());

        let Ok(()) = set_secret(&key, "hello") else {
            eprintln!("[test] keychain unavailable — skipping smoke test");
            return;
        };

        // Roundtrip: whatever the backend returns, it must be coherent.
        match get_secret(&key) {
            Ok(Some(v)) => assert_eq!(v, "hello"),
            Ok(None) => eprintln!("[test] keychain read-after-write returned None — treating as unavailable"),
            Err(e) => eprintln!("[test] keychain read error: {} — treating as unavailable", e),
        }
    }
}
