//! Main window restoration logic — logs what `tauri-plugin-window-state`
//! actually restored and centres the window when no saved position exists.

use tauri::Manager;

/// Log the window position/size that the window-state plugin restored, then
/// centre on the primary monitor when no saved position was found.
pub fn restore_window_state(app: &tauri::App) -> Result<(), Box<dyn std::error::Error>> {
    let Some(window) = app.get_webview_window("main") else {
        return Ok(());
    };

    // 2026-05-07 — issue #264 hotfix: PR #237 에서 도입했던 Windows 전용
    // `set_decorations(false)` 를 제거. devbug 외부 사용자 환경에서 native
    // chrome 사라진 후 `WindowControls` 가 마운트되지 않거나 click 이 가로채
    // 져서 *닫기/최소화/최대화/드래그 모두 부재* 차단 회귀. Native frame 으로
    // 회복하고 `WindowControls` 는 그대로 둬 "1 라인 통합" UX 는 후속 PR 에서
    // platform detection / drag region 격리가 검증된 후 다시 도입.
    //
    // mac/Linux 영향 0 (이 호출 자체가 cfg(windows) 분기였음).

    // window-state plugin restores position/size BEFORE setup runs.
    // Log actual state to diagnose restoration issues.
    let pos = window.outer_position().unwrap_or_default();
    let size = window.outer_size().unwrap_or_default();
    let scale = window.scale_factor().unwrap_or(1.0);
    eprintln!(
        "[bootstrap/window] restored: pos=({},{}) size={}x{} scale={:.1}",
        pos.x, pos.y, size.width, size.height, scale
    );

    // Only center if position is clearly unset (0,0).
    if pos.x == 0 && pos.y == 0 {
        eprintln!("[bootstrap/window] no saved position — centering on primary monitor");
        if let Some(monitor) = window.primary_monitor().ok().flatten() {
            let screen = monitor.size();
            let mon_pos = monitor.position();
            let win_w = 1200.0;
            let win_h = 800.0;
            let x = mon_pos.x as f64 + (screen.width as f64 / scale - win_w) / 2.0;
            let y = mon_pos.y as f64 + (screen.height as f64 / scale - win_h) / 2.0;
            let _ = window.set_position(tauri::PhysicalPosition::new(
                (x * scale) as i32,
                (y * scale) as i32,
            ));
        }
    }
    let _ = window.show();

    Ok(())
}
