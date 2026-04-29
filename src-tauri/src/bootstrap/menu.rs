//! Application menu — minimal native menu so Settings is accessible from the
//! macOS menu bar (Cmd+,) and Windows/Linux menu before any project is
//! selected.
//!
//! Scope (`docs/plans/globalSettingsAndRecentProjectsPlan_2026-04-29.md`,
//! Task 01):
//! - Add a single "Settings..." item under the app submenu (macOS — first
//!   submenu) and under a top-level `tunaFlow` submenu on Win/Linux.
//! - Wire its activation to a frontend event (`tunaflow:open-settings`) so
//!   the menu and the global keyboard listener share the same code path.
//! - Frontend already listens to `tunaflow:open-settings` at AppShell root.
//!
//! We deliberately keep the menu small: full edit/window menus add UX noise
//! on Windows/Linux where they are not standard. macOS gets the standard
//! app submenu (About, Hide, Quit) via `PredefinedMenuItem` so the system
//! menu bar isn't empty.
//!
//! ### Cmd+, accelerator
//!
//! macOS shows the accelerator hint on the menu item (assigned via the
//! second arg of `MenuItemBuilder::accelerator`). The actual key handling
//! is done in the frontend (`AppShell.tsx` keydown listener) so that
//! ProjectStartup screen (where no menu may yet be visible) also responds
//! to the shortcut. Both paths emit the same `tunaflow:open-settings`
//! event and converge at the AppShell-level mount.

use tauri::menu::{
    AboutMetadata, MenuBuilder, MenuItemBuilder, PredefinedMenuItem, SubmenuBuilder,
};
use tauri::{App, AppHandle, Emitter};

const SETTINGS_MENU_ID: &str = "tf_settings_open";

/// Build and attach the app-level menu, and register the menu-event handler.
///
/// Called from `lib.rs` `.setup(...)`. Errors are propagated as
/// `Box<dyn Error>` consistent with other bootstrap functions.
pub fn install(app: &App) -> Result<(), Box<dyn std::error::Error>> {
    let handle = app.handle();

    // Settings... item — Cmd+, on macOS, Ctrl+, elsewhere.
    let settings_item = MenuItemBuilder::with_id(SETTINGS_MENU_ID, "Settings...")
        .accelerator("CmdOrCtrl+,")
        .build(handle)?;

    // App submenu (first submenu — becomes the macOS app menu when packaged
    // under the app bundle name; on Win/Linux it shows as a top-level
    // "tunaFlow" menu).
    let app_submenu = SubmenuBuilder::new(handle, "tunaFlow")
        .item(&PredefinedMenuItem::about(
            handle,
            Some("About tunaFlow"),
            Some(AboutMetadata::default()),
        )?)
        .separator()
        .item(&settings_item)
        .separator()
        .item(&PredefinedMenuItem::hide(handle, None)?)
        .item(&PredefinedMenuItem::hide_others(handle, None)?)
        .item(&PredefinedMenuItem::show_all(handle, None)?)
        .separator()
        .item(&PredefinedMenuItem::quit(handle, None)?)
        .build()?;

    let menu = MenuBuilder::new(handle).item(&app_submenu).build()?;

    // Tauri 2.x: app-level set_menu so it applies to all windows + macOS bar.
    app.set_menu(menu)?;

    // Wire menu events — only one item at the moment, but match on id so
    // adding more items later is mechanical.
    app.on_menu_event(move |app_handle: &AppHandle, event| {
        if event.id() == SETTINGS_MENU_ID {
            if let Err(e) = app_handle.emit("tunaflow:menu-open-settings", ()) {
                eprintln!("[menu] emit open-settings failed: {e}");
            }
        }
    });

    Ok(())
}
