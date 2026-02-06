//! Student System Tray
//!
//! This module manages the system tray icon and menu for the student app.
//! The student app runs in the background and can be controlled via the tray icon.

use tauri::{
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    AppHandle, Manager, Runtime,
};
use std::sync::Arc;
use crate::student_agent::AgentState;

/// Setup the system tray for student app
/// Note: No quit option — students cannot exit the app (like Veyon/NetSupport)
pub fn setup_tray<R: Runtime>(app: &AppHandle<R>) -> Result<(), Box<dyn std::error::Error>> {
    // Create tray menu — no quit button, students cannot exit
    let show_item = MenuItem::with_id(app, "show", "Hiện cửa sổ", true, None::<&str>)?;
    let status_item = MenuItem::with_id(app, "status", "Trạng thái: Đang khởi động...", false, None::<&str>)?;
    let separator1 = PredefinedMenuItem::separator(app)?;
    let info_item = MenuItem::with_id(app, "info", "Smartlab Student đang chạy", false, None::<&str>)?;

    let menu = Menu::with_items(
        app,
        &[
            &status_item,
            &separator1,
            &show_item,
            &info_item,
        ],
    )?;

    // Build tray icon
    let _tray = TrayIconBuilder::with_id("main-tray")
        .tooltip("Smartlab Student - Đang chạy")
        .icon(app.default_window_icon().unwrap().clone())
        .menu(&menu)
        .on_menu_event(move |app: &AppHandle<R>, event| {
            match event.id.as_ref() {
                "show" => {
                    if let Some(window) = app.get_webview_window("main") {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
                _ => {}
            }
        })
        .on_tray_icon_event(|tray: &tauri::tray::TrayIcon<R>, event| {
            if let TrayIconEvent::Click {
                button: MouseButton::Left,
                button_state: MouseButtonState::Up,
                ..
            } = event
            {
                let app = tray.app_handle();
                if let Some(window) = app.get_webview_window("main") {
                    if window.is_visible().unwrap_or(false) {
                        let _ = window.hide();
                    } else {
                        let _ = window.show();
                        let _ = window.set_focus();
                    }
                }
            }
        })
        .build(app)?;

    // Setup window close handler to prevent closing
    if let Some(window) = app.get_webview_window("main") {
        let window_clone = window.clone();
        window.on_window_event(move |event| {
            if let tauri::WindowEvent::CloseRequested { api, .. } = event {
                // Prevent window from closing, just hide it
                let _ = window_clone.hide();
                api.prevent_close();
                log::info!("[StudentTray] Close requested, hiding window instead");
            }
        });
    }

    log::info!("[StudentTray] System tray initialized");
    Ok(())
}

/// Update tray status text
pub fn update_tray_status<R: Runtime>(
    app: &AppHandle<R>,
    status_text: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    // In Tauri 2, we need to rebuild the menu to update text
    // This is a limitation of the current tray API
    if let Some(tray) = app.tray_by_id("main-tray") {
        let show_item = MenuItem::with_id(app, "show", "Hiện cửa sổ", true, None::<&str>)?;
        let status_item = MenuItem::with_id(app, "status", format!("Trạng thái: {}", status_text), false, None::<&str>)?;
        let separator1 = PredefinedMenuItem::separator(app)?;
        let info_item = MenuItem::with_id(app, "info", "Smartlab Student đang chạy", false, None::<&str>)?;

        let menu = Menu::with_items(
            app,
            &[
                &status_item,
                &separator1,
                &show_item,
                &info_item,
            ],
        )?;
        
        tray.set_menu(Some(menu))?;
    }
    Ok(())
}

/// Update tray tooltip
pub fn update_tray_tooltip<R: Runtime>(
    app: &AppHandle<R>,
    tooltip: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if let Some(tray) = app.tray_by_id("main-tray") {
        tray.set_tooltip(Some(tooltip))?;
    }
    Ok(())
}

/// Show notification from tray
pub fn show_tray_notification<R: Runtime>(
    app: &AppHandle<R>,
    title: &str,
    body: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    use tauri_plugin_notification::NotificationExt;
    
    app.notification()
        .builder()
        .title(title)
        .body(body)
        .show()?;
    
    Ok(())
}

/// Monitor agent status and update tray
pub async fn monitor_agent_status<R: Runtime>(
    app: AppHandle<R>,
    agent_state: Arc<AgentState>,
) {
    use tokio::time::{sleep, Duration};
    use crate::student_agent::AgentStatus;

    let mut last_status = String::new();
    
    loop {
        let status = agent_state.get_status();
        let status_text = match &status {
            AgentStatus::Stopped => "Đã dừng",
            AgentStatus::Starting => "Đang khởi động...",
            AgentStatus::WaitingForTeacher => "Đang tìm giáo viên...",
            AgentStatus::Authenticating => "Đang xác thực...",
            AgentStatus::Connected { teacher_name, .. } => {
                &format!("Đã kết nối: {}", teacher_name)
            }
            AgentStatus::UpdateRequired { .. } => "Cần cập nhật",
            AgentStatus::Updating { progress, .. } => {
                &format!("Đang cập nhật: {:.0}%", progress * 100.0)
            }
            AgentStatus::Error { message } => {
                &format!("Lỗi: {}", message)
            }
        };

        // Only update if status changed
        if status_text != last_status {
            let _ = update_tray_status(&app, status_text);
            let _ = update_tray_tooltip(&app, &format!("Smartlab Student - {}", status_text));
            
            // Show notification on important status changes
            match &status {
                AgentStatus::Connected { teacher_name, .. } => {
                    let _ = show_tray_notification(
                        &app,
                        "Đã kết nối",
                        &format!("Đã kết nối với giáo viên: {}", teacher_name),
                    );
                }
                AgentStatus::Error { message } => {
                    let _ = show_tray_notification(
                        &app,
                        "Lỗi kết nối",
                        message,
                    );
                }
                _ => {}
            }
            
            last_status = status_text.to_string();
        }

        sleep(Duration::from_secs(1)).await;
    }
}
