//! Native macOS notification bridge — direct UNUserNotificationCenter path.
//!
//! `tauri-plugin-notification` v2.3.3 의 macOS path 는 내부적으로 `notify_rust` 의
//! osascript fallback 을 호출해, 알림 클릭 시 macOS 가 Script Editor (또는
//! osascript) 를 frontmost 로 띄운다 (외부 사용자 batmania52 보고 #6, 2026-04-29).
//! Codesigned/Notarized release 이전까지는 plugin 내부 분기 (`is_dev()`) 만으로는
//! 회귀가 풀리지 않아 정공법 직접 ObjC bridge 로 전환한다.
//!
//! - macOS: `objc2-user-notifications` 로 `UNUserNotificationCenter` 직접 호출.
//! - Windows / Linux: 이 모듈은 컴파일되지 않으며, frontend 가 기존
//!   `tauri-plugin-notification` path 를 그대로 사용한다.
//!
//! SSOT: `docs/plans/nativeNotificationPlan_2026-04-29.md` (Path B, 권한 UX 옵션 D).

#![cfg(target_os = "macos")]

use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc, Mutex,
};
use std::time::Duration;

use block2::RcBlock;
use objc2::rc::Retained;
use objc2::runtime::Bool;
use objc2_foundation::{NSError, NSString};
use objc2_user_notifications::{
    UNAuthorizationOptions, UNAuthorizationStatus, UNMutableNotificationContent,
    UNNotificationRequest, UNNotificationSettings, UNUserNotificationCenter,
};
use serde::Serialize;

/// Tauri command 응답에 그대로 직렬화되는 권한 상태.
///
/// macOS 의 `UNAuthorizationStatus` 는 5종 (notDetermined / denied / authorized /
/// provisional / ephemeral) 이지만, frontend UX (옵션 D) 는 3-state 로 충분하다.
/// `provisional` 과 `ephemeral` 는 사실상 알림이 동작하므로 `authorized` 로 묶는다.
#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub enum NotificationAuthStatus {
    NotDetermined,
    Denied,
    Authorized,
}

impl From<UNAuthorizationStatus> for NotificationAuthStatus {
    fn from(s: UNAuthorizationStatus) -> Self {
        match s {
            UNAuthorizationStatus::NotDetermined => Self::NotDetermined,
            UNAuthorizationStatus::Denied => Self::Denied,
            // Authorized / Provisional / Ephemeral 모두 알림 발송 가능.
            _ => Self::Authorized,
        }
    }
}

/// completion handler 가 비동기로 호출되므로, 각 호출마다 짧은 대기 (≤ 5s) 로
/// 결과를 polling 한다. UI hot path 가 아니라 권한 요청 (사용자 dialog) /
/// 알림 발송 ack 수신 — 둘 다 사용자가 수 초 내 응답할 영역.
const COMPLETION_TIMEOUT: Duration = Duration::from_secs(5);
const COMPLETION_POLL_INTERVAL: Duration = Duration::from_millis(50);

/// 현재 알림 권한 상태를 조회한다 (`getNotificationSettingsWithCompletionHandler:`).
pub fn notification_authorization_status() -> Result<NotificationAuthStatus, String> {
    let result: Arc<Mutex<Option<NotificationAuthStatus>>> = Arc::new(Mutex::new(None));
    let done = Arc::new(AtomicBool::new(false));

    let result_for_block = Arc::clone(&result);
    let done_for_block = Arc::clone(&done);

    // SAFETY: completion handler 는 background queue 에서 호출되며 `settings`
    // 는 valid pointer 임이 Apple API 계약상 보장된다.
    let block = RcBlock::new(move |settings: std::ptr::NonNull<UNNotificationSettings>| {
        let status = unsafe { settings.as_ref() }.authorizationStatus();
        if let Ok(mut guard) = result_for_block.lock() {
            *guard = Some(NotificationAuthStatus::from(status));
        }
        done_for_block.store(true, Ordering::SeqCst);
    });

    let center = UNUserNotificationCenter::currentNotificationCenter();
    center.getNotificationSettingsWithCompletionHandler(&block);

    wait_for_done(&done)?;

    let guard = result
        .lock()
        .map_err(|e| format!("notification auth status mutex poisoned: {e}"))?;
    guard
        .ok_or_else(|| "notification auth status: handler did not set value".into())
}

/// 알림 권한을 요청한다. `notDetermined` 일 때 macOS native dialog 가 표시되고,
/// 사용자가 응답하면 granted bool 이 돌아온다. 이미 결정된 (denied/authorized) 경우
/// dialog 없이 즉시 현재 상태에 맞는 bool 만 돌아온다.
pub fn request_notification_authorization() -> Result<bool, String> {
    let result: Arc<Mutex<Option<bool>>> = Arc::new(Mutex::new(None));
    let done = Arc::new(AtomicBool::new(false));

    let result_for_block = Arc::clone(&result);
    let done_for_block = Arc::clone(&done);

    let block = RcBlock::new(move |granted: Bool, _err: *mut NSError| {
        if let Ok(mut guard) = result_for_block.lock() {
            *guard = Some(granted.as_bool());
        }
        done_for_block.store(true, Ordering::SeqCst);
    });

    let options = UNAuthorizationOptions::Alert | UNAuthorizationOptions::Sound;
    let center = UNUserNotificationCenter::currentNotificationCenter();
    center.requestAuthorizationWithOptions_completionHandler(options, &block);

    wait_for_done(&done)?;

    let guard = result
        .lock()
        .map_err(|e| format!("notification auth request mutex poisoned: {e}"))?;
    guard
        .ok_or_else(|| "notification auth request: handler did not set value".into())
}

/// 알림 1건을 즉시 (no trigger) 발송한다.
///
/// `add(request:withCompletionHandler:)` 의 completion handler 는 OS 가 alert
/// 을 schedule 한 직후 호출되며, 사용자 클릭과는 무관하다. handler 의 `NSError`
/// 가 nil 이면 schedule 성공으로 간주한다.
pub fn send_native_notification(title: &str, body: &str) -> Result<(), String> {
    let title_ns = NSString::from_str(title);
    let body_ns = NSString::from_str(body);
    let identifier_ns = NSString::from_str(&format!("tunaflow.{}", uuid::Uuid::new_v4()));

    let content: Retained<UNMutableNotificationContent> = UNMutableNotificationContent::new();
    content.setTitle(&title_ns);
    content.setBody(&body_ns);
    // sound 는 default — `UNNotificationSound::default()` retain 비용 회피하고 nil
    // 로 두면 기본 알림음 사용 (macOS 표준).

    // trigger = nil → 즉시 발송.
    let request = UNNotificationRequest::requestWithIdentifier_content_trigger(
        &identifier_ns,
        &content,
        None,
    );

    let err_holder: Arc<Mutex<Option<String>>> = Arc::new(Mutex::new(None));
    let done = Arc::new(AtomicBool::new(false));

    let err_for_block = Arc::clone(&err_holder);
    let done_for_block = Arc::clone(&done);

    let block = RcBlock::new(move |err: *mut NSError| {
        if !err.is_null() {
            // SAFETY: Apple API 가 non-null err 를 valid NSError pointer 로
            // 호출하는 계약을 따른다. block 의 lifetime 동안 retain 됨.
            let err_ref = unsafe { &*err };
            let desc = err_ref.localizedDescription();
            if let Ok(mut g) = err_for_block.lock() {
                *g = Some(desc.to_string());
            }
        }
        done_for_block.store(true, Ordering::SeqCst);
    });

    let center = UNUserNotificationCenter::currentNotificationCenter();
    center.addNotificationRequest_withCompletionHandler(&request, Some(&block));

    wait_for_done(&done)?;

    let guard = err_holder
        .lock()
        .map_err(|e| format!("notification send mutex poisoned: {e}"))?;
    if let Some(msg) = guard.as_ref() {
        return Err(format!("UNUserNotificationCenter add failed: {msg}"));
    }
    Ok(())
}

/// completion handler 가 끝날 때까지 짧게 polling 한다. timeout 이면 Err.
fn wait_for_done(done: &Arc<AtomicBool>) -> Result<(), String> {
    let start = std::time::Instant::now();
    while !done.load(Ordering::SeqCst) {
        if start.elapsed() > COMPLETION_TIMEOUT {
            return Err(format!(
                "notification operation timed out after {:?}",
                COMPLETION_TIMEOUT
            ));
        }
        std::thread::sleep(COMPLETION_POLL_INTERVAL);
    }
    Ok(())
}

// ─── Tauri commands ──────────────────────────────────────────────────────────

#[tauri::command]
pub async fn notification_send_native(title: String, body: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || send_native_notification(&title, &body))
        .await
        .map_err(|e| format!("notification_send_native join error: {e}"))?
}

#[tauri::command]
pub async fn notification_request_permission() -> Result<bool, String> {
    tauri::async_runtime::spawn_blocking(request_notification_authorization)
        .await
        .map_err(|e| format!("notification_request_permission join error: {e}"))?
}

#[tauri::command]
pub async fn notification_get_status() -> Result<NotificationAuthStatus, String> {
    tauri::async_runtime::spawn_blocking(notification_authorization_status)
        .await
        .map_err(|e| format!("notification_get_status join error: {e}"))?
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_status_provisional_collapses_to_authorized() {
        // 옵션 D — frontend UX 는 3-state 로 충분하므로 provisional/ephemeral 도
        // authorized 로 묶는다.
        assert_eq!(
            NotificationAuthStatus::from(UNAuthorizationStatus::Provisional),
            NotificationAuthStatus::Authorized
        );
        assert_eq!(
            NotificationAuthStatus::from(UNAuthorizationStatus::Authorized),
            NotificationAuthStatus::Authorized
        );
    }

    #[test]
    fn auth_status_denied_and_not_determined_preserved() {
        assert_eq!(
            NotificationAuthStatus::from(UNAuthorizationStatus::Denied),
            NotificationAuthStatus::Denied
        );
        assert_eq!(
            NotificationAuthStatus::from(UNAuthorizationStatus::NotDetermined),
            NotificationAuthStatus::NotDetermined
        );
    }
}
