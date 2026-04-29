//! Non-macOS stub for native notification commands.
//!
//! macOS 외 OS 에서는 frontend 의 platform 분기 (notificationStore.ts) 가
//! 기존 `tauri-plugin-notification` path 를 사용하므로 이 stub 은 호출되지
//! 않는다. 그러나 Tauri command registration 은 OS 와 무관하게 컴파일 단계에서
//! resolve 되므로, lib.rs 가 unconditionally `notification::*` 를 참조하도록
//! 같은 이름의 stub 을 제공한다.
//!
//! stub 호출 = macOS 가 아닌데 frontend 가 잘못 invoke 한 경우 → Err 반환으로
//! 표면화. silent ok 금지 (코드베이스 error visibility 정책).

#![cfg(not(target_os = "macos"))]

use serde::Serialize;

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum NotificationAuthStatus {
    NotDetermined,
    Denied,
    Authorized,
}

const NOT_MACOS_MSG: &str =
    "native notification commands are macOS-only; use tauri-plugin-notification on this OS";

#[tauri::command]
pub async fn notification_send_native(_title: String, _body: String) -> Result<(), String> {
    Err(NOT_MACOS_MSG.into())
}

#[tauri::command]
pub async fn notification_request_permission() -> Result<bool, String> {
    Err(NOT_MACOS_MSG.into())
}

#[tauri::command]
pub async fn notification_get_status() -> Result<NotificationAuthStatus, String> {
    Err(NOT_MACOS_MSG.into())
}
