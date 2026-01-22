// Student app - no server management needed
use std::process::Command;
use std::sync::Arc;
use std::net::UdpSocket;
use std::thread;
use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
struct ScreenSize {
    width: u32,
    height: u32,
}

#[derive(Debug, Deserialize, Serialize)]
struct MouseEvent {
    action: String, // "move", "click", "rightClick", "doubleClick", "scroll", "mouseDown", "mouseUp", "middleClick"
    x: Option<f64>,
    y: Option<f64>,
    button: Option<String>, // "left", "right", "middle"
    delta_x: Option<f64>,
    delta_y: Option<f64>,
}

#[derive(Debug, Deserialize, Serialize)]
struct KeyboardEvent {
    action: String, // "key", "keyDown", "keyUp", "text"
    key: Option<String>,
    text: Option<String>,
    modifiers: Option<Vec<String>>, // ["Control", "Alt", "Shift", "Meta"]
    code: Option<String>, // Physical key code
}

#[tauri::command]
fn control_computer(action: String) -> Result<String, String> {
    println!("[control_computer] Command called with action: {}", action);
    
    match action.as_str() {
        "shutdown" => {
            #[cfg(target_os = "windows")]
            {
                Command::new("shutdown")
                    .args(["/s", "/t", "0"])
                    .spawn()
                    .map_err(|e| format!("Failed to shutdown: {}", e))?;
                Ok("Shutdown command executed".to_string())
            }
            #[cfg(target_os = "macos")]
            {
                // Try osascript first
                if Command::new("osascript")
                    .args(["-e", "tell application \"System Events\" to shut down"])
                    .spawn()
                    .is_ok() {
                    return Ok("Shutdown command executed (osascript)".to_string());
                }
                // Fallback
                Command::new("shutdown")
                    .args(["-h", "now"])
                    .spawn()
                    .map_err(|e| format!("Failed to shutdown: {}", e))?;
                Ok("Shutdown command executed".to_string())
            }
            #[cfg(target_os = "linux")]
            {
                Command::new("shutdown")
                    .args(["-h", "now"])
                    .spawn()
                    .map_err(|e| format!("Failed to shutdown: {}", e))?;
                Ok("Shutdown command executed".to_string())
            }
            #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
            {
                Err("Shutdown not supported on this platform".to_string())
            }
        }
        
        "restart" => {
            #[cfg(target_os = "windows")]
            {
                Command::new("shutdown")
                    .args(["/r", "/t", "0"])
                    .spawn()
                    .map_err(|e| format!("Failed to restart: {}", e))?;
                Ok("Restart command executed".to_string())
            }
            #[cfg(target_os = "macos")]
            {
                if Command::new("osascript")
                    .args(["-e", "tell application \"System Events\" to restart"])
                    .spawn()
                    .is_ok() {
                    return Ok("Restart command executed (osascript)".to_string());
                }
                Command::new("shutdown")
                    .args(["-r", "now"])
                    .spawn()
                    .map_err(|e| format!("Failed to restart: {}", e))?;
                Ok("Restart command executed".to_string())
            }
            #[cfg(target_os = "linux")]
            {
                Command::new("shutdown")
                    .args(["-r", "now"])
                    .spawn()
                    .map_err(|e| format!("Failed to restart: {}", e))?;
                Ok("Restart command executed".to_string())
            }
            #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
            {
                Err("Restart not supported on this platform".to_string())
            }
        }
        
        "lock" => {
            #[cfg(target_os = "windows")]
            {
                Command::new("rundll32")
                    .args(["user32.dll,LockWorkStation"])
                    .spawn()
                    .map_err(|e| format!("Failed to lock: {}", e))?;
                Ok("Lock screen command executed".to_string())
            }
            #[cfg(target_os = "macos")]
            {
                Command::new("pmset")
                    .args(["displaysleepnow"])
                    .spawn()
                    .map_err(|e| format!("Failed to lock: {}", e))?;
                // Also lock the screen
                let _ = Command::new("/System/Library/CoreServices/Menu Extras/User.menu/Contents/Resources/CGSession")
                    .args(["-suspend"])
                    .spawn();
                Ok("Lock screen command executed".to_string())
            }
            #[cfg(target_os = "linux")]
            {
                // Try different lock commands
                if Command::new("gnome-screensaver-command")
                    .args(["-l"])
                    .spawn()
                    .is_ok() {
                    return Ok("Lock screen command executed (gnome)".to_string());
                }
                if Command::new("xdg-screensaver")
                    .args(["lock"])
                    .spawn()
                    .is_ok() {
                    return Ok("Lock screen command executed (xdg)".to_string());
                }
                Err("Lock screen not available on this system".to_string())
            }
            #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
            {
                Err("Lock screen not supported on this platform".to_string())
            }
        }
        
        "sleep" => {
            #[cfg(target_os = "windows")]
            {
                Command::new("rundll32")
                    .args(["powrprof.dll,SetSuspendState", "0,1,0"])
                    .spawn()
                    .map_err(|e| format!("Failed to sleep: {}", e))?;
                Ok("Sleep command executed".to_string())
            }
            #[cfg(target_os = "macos")]
            {
                Command::new("pmset")
                    .args(["sleepnow"])
                    .spawn()
                    .map_err(|e| format!("Failed to sleep: {}", e))?;
                Ok("Sleep command executed".to_string())
            }
            #[cfg(target_os = "linux")]
            {
                Command::new("systemctl")
                    .args(["suspend"])
                    .spawn()
                    .map_err(|e| format!("Failed to sleep: {}", e))?;
                Ok("Sleep command executed".to_string())
            }
            #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
            {
                Err("Sleep not supported on this platform".to_string())
            }
        }
        
        "logout" => {
            #[cfg(target_os = "windows")]
            {
                Command::new("shutdown")
                    .args(["/l"])
                    .spawn()
                    .map_err(|e| format!("Failed to logout: {}", e))?;
                Ok("Logout command executed".to_string())
            }
            #[cfg(target_os = "macos")]
            {
                Command::new("osascript")
                    .args(["-e", "tell application \"System Events\" to log out"])
                    .spawn()
                    .map_err(|e| format!("Failed to logout: {}", e))?;
                Ok("Logout command executed".to_string())
            }
            #[cfg(target_os = "linux")]
            {
                Command::new("logout")
                    .spawn()
                    .map_err(|e| format!("Failed to logout: {}", e))?;
                Ok("Logout command executed".to_string())
            }
            #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
            {
                Err("Logout not supported on this platform".to_string())
            }
        }
        
        _ => {
            Err(format!("Unknown action: {}", action))
        }
    }
}

#[tauri::command]
fn control_mouse(event: MouseEvent) -> Result<String, String> {
    println!("[control_mouse] Event: {:?}", event);
    
    match event.action.as_str() {
        "move" => {
            if let (Some(x), Some(y)) = (event.x, event.y) {
                #[cfg(target_os = "macos")]
                {
                    // Try enigo first, fallback to osascript
                    use enigo::*;
                    let mut enigo = Enigo::new();
                    match enigo.mouse_move_to(x as i32, y as i32) {
                        Ok(_) => Ok("Mouse moved (enigo)".to_string()),
                        Err(_) => {
                            // Fallback to osascript
                            let script = format!(
                                "tell application \"System Events\"\n\
                                 set mouse location to {{{}, {}}}\n\
                                 end tell",
                                x as i32, y as i32
                            );
                            Command::new("osascript")
                                .args(["-e", &script])
                                .spawn()
                                .map_err(|e| format!("Failed to move mouse: {}", e))?;
                            Ok("Mouse moved (osascript)".to_string())
                        }
                    }
                }
                #[cfg(target_os = "windows")]
                {
                    use enigo::*;
                    let mut enigo = Enigo::new();
                    enigo.mouse_move_to(x as i32, y as i32)
                        .map_err(|e| format!("Failed to move mouse: {}", e))?;
                    Ok("Mouse moved".to_string())
                }
                #[cfg(target_os = "linux")]
                {
                    use enigo::*;
                    let mut enigo = Enigo::new();
                    enigo.mouse_move_to(x as i32, y as i32)
                        .map_err(|e| format!("Failed to move mouse: {}", e))?;
                    Ok("Mouse moved".to_string())
                }
                #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
                {
                    Err("Mouse control not supported on this platform".to_string())
                }
            } else {
                Err("Missing x or y coordinates".to_string())
            }
        }
        "click" | "leftClick" => {
            #[cfg(target_os = "macos")]
            {
                use enigo::*;
                let mut enigo = Enigo::new();
                match enigo.mouse_click(MouseButton::Left) {
                    Ok(_) => Ok("Mouse clicked (enigo)".to_string()),
                    Err(_) => {
                        // Fallback to osascript
                        let script = "tell application \"System Events\"\n\
                                      click at {0, 0}\n\
                                      end tell";
                        Command::new("osascript")
                            .args(["-e", script])
                            .spawn()
                            .map_err(|e| format!("Failed to click: {}", e))?;
                        Ok("Mouse clicked (osascript)".to_string())
                    }
                }
            }
            #[cfg(target_os = "windows")]
            {
                use enigo::*;
                let mut enigo = Enigo::new();
                enigo.mouse_click(MouseButton::Left)
                    .map_err(|e| format!("Failed to click: {}", e))?;
                Ok("Mouse clicked".to_string())
            }
            #[cfg(target_os = "linux")]
            {
                use enigo::*;
                let mut enigo = Enigo::new();
                enigo.mouse_click(MouseButton::Left)
                    .map_err(|e| format!("Failed to click: {}", e))?;
                Ok("Mouse clicked".to_string())
            }
            #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
            {
                Err("Mouse click not supported on this platform".to_string())
            }
        }
        "rightClick" => {
            #[cfg(target_os = "macos")]
            {
                use enigo::*;
                let mut enigo = Enigo::new();
                match enigo.mouse_click(MouseButton::Right) {
                    Ok(_) => Ok("Mouse right clicked (enigo)".to_string()),
                    Err(_) => {
                        // Fallback to osascript
                        let script = "tell application \"System Events\"\n\
                                      right click at {0, 0}\n\
                                      end tell";
                        Command::new("osascript")
                            .args(["-e", script])
                            .spawn()
                            .map_err(|e| format!("Failed to right click: {}", e))?;
                        Ok("Mouse right clicked (osascript)".to_string())
                    }
                }
            }
            #[cfg(target_os = "windows")]
            {
                use enigo::*;
                let mut enigo = Enigo::new();
                enigo.mouse_click(MouseButton::Right)
                    .map_err(|e| format!("Failed to right click: {}", e))?;
                Ok("Mouse right clicked".to_string())
            }
            #[cfg(target_os = "linux")]
            {
                use enigo::*;
                let mut enigo = Enigo::new();
                enigo.mouse_click(MouseButton::Right)
                    .map_err(|e| format!("Failed to right click: {}", e))?;
                Ok("Mouse right clicked".to_string())
            }
            #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
            {
                Err("Mouse right click not supported on this platform".to_string())
            }
        }
        "doubleClick" => {
            #[cfg(target_os = "macos")]
            {
                use enigo::*;
                let mut enigo = Enigo::new();
                // Double click = two clicks quickly
                match enigo.mouse_click(MouseButton::Left) {
                    Ok(_) => {
                        std::thread::sleep(std::time::Duration::from_millis(50));
                        match enigo.mouse_click(MouseButton::Left) {
                            Ok(_) => Ok("Mouse double clicked (enigo)".to_string()),
                            Err(_) => Ok("Mouse double clicked (partial)".to_string()),
                        }
                    },
                    Err(_) => {
                        // Fallback to osascript
                        let script = "tell application \"System Events\"\n\
                                      double click at {0, 0}\n\
                                      end tell";
                        Command::new("osascript")
                            .args(["-e", script])
                            .spawn()
                            .map_err(|e| format!("Failed to double click: {}", e))?;
                        Ok("Mouse double clicked (osascript)".to_string())
                    }
                }
            }
            #[cfg(target_os = "windows")]
            {
                use enigo::*;
                let mut enigo = Enigo::new();
                enigo.mouse_click(MouseButton::Left)
                    .map_err(|e| format!("Failed to double click: {}", e))?;
                std::thread::sleep(std::time::Duration::from_millis(50));
                enigo.mouse_click(MouseButton::Left)
                    .map_err(|e| format!("Failed to double click: {}", e))?;
                Ok("Mouse double clicked".to_string())
            }
            #[cfg(target_os = "linux")]
            {
                use enigo::*;
                let mut enigo = Enigo::new();
                enigo.mouse_click(MouseButton::Left)
                    .map_err(|e| format!("Failed to double click: {}", e))?;
                std::thread::sleep(std::time::Duration::from_millis(50));
                enigo.mouse_click(MouseButton::Left)
                    .map_err(|e| format!("Failed to double click: {}", e))?;
                Ok("Mouse double clicked".to_string())
            }
            #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
            {
                Err("Mouse double click not supported on this platform".to_string())
            }
        }
        "middleClick" => {
            #[cfg(target_os = "macos")]
            {
                use enigo::*;
                let mut enigo = Enigo::new();
                match enigo.mouse_click(MouseButton::Middle) {
                    Ok(_) => Ok("Mouse middle clicked (enigo)".to_string()),
                    Err(_) => {
                        // Fallback to osascript
                        let script = "tell application \"System Events\"\n\
                                      middle click at {0, 0}\n\
                                      end tell";
                        Command::new("osascript")
                            .args(["-e", script])
                            .spawn()
                            .map_err(|e| format!("Failed to middle click: {}", e))?;
                        Ok("Mouse middle clicked (osascript)".to_string())
                    }
                }
            }
            #[cfg(target_os = "windows")]
            {
                use enigo::*;
                let mut enigo = Enigo::new();
                enigo.mouse_click(MouseButton::Middle)
                    .map_err(|e| format!("Failed to middle click: {}", e))?;
                Ok("Mouse middle clicked".to_string())
            }
            #[cfg(target_os = "linux")]
            {
                use enigo::*;
                let mut enigo = Enigo::new();
                enigo.mouse_click(MouseButton::Middle)
                    .map_err(|e| format!("Failed to middle click: {}", e))?;
                Ok("Mouse middle clicked".to_string())
            }
            #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
            {
                Err("Mouse middle click not supported on this platform".to_string())
            }
        }
        "mouseDown" => {
            let button = event.button.as_deref().unwrap_or("left");
            #[cfg(target_os = "macos")]
            {
                use enigo::*;
                let mut enigo = Enigo::new();
                let mouse_button = match button {
                    "right" => MouseButton::Right,
                    "middle" => MouseButton::Middle,
                    _ => MouseButton::Left,
                };
                match enigo.mouse_down(mouse_button) {
                    Ok(_) => Ok(format!("Mouse down ({})", button)),
                    Err(e) => Err(format!("Failed to mouse down: {:?}", e)),
                }
            }
            #[cfg(target_os = "windows")]
            {
                use enigo::*;
                let mut enigo = Enigo::new();
                let mouse_button = match button {
                    "right" => MouseButton::Right,
                    "middle" => MouseButton::Middle,
                    _ => MouseButton::Left,
                };
                enigo.mouse_down(mouse_button)
                    .map_err(|e| format!("Failed to mouse down: {:?}", e))?;
                Ok(format!("Mouse down ({})", button))
            }
            #[cfg(target_os = "linux")]
            {
                use enigo::*;
                let mut enigo = Enigo::new();
                let mouse_button = match button {
                    "right" => MouseButton::Right,
                    "middle" => MouseButton::Middle,
                    _ => MouseButton::Left,
                };
                enigo.mouse_down(mouse_button)
                    .map_err(|e| format!("Failed to mouse down: {:?}", e))?;
                Ok(format!("Mouse down ({})", button))
            }
            #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
            {
                Err("Mouse down not supported on this platform".to_string())
            }
        }
        "mouseUp" => {
            let button = event.button.as_deref().unwrap_or("left");
            #[cfg(target_os = "macos")]
            {
                use enigo::*;
                let mut enigo = Enigo::new();
                let mouse_button = match button {
                    "right" => MouseButton::Right,
                    "middle" => MouseButton::Middle,
                    _ => MouseButton::Left,
                };
                match enigo.mouse_up(mouse_button) {
                    Ok(_) => Ok(format!("Mouse up ({})", button)),
                    Err(e) => Err(format!("Failed to mouse up: {:?}", e)),
                }
            }
            #[cfg(target_os = "windows")]
            {
                use enigo::*;
                let mut enigo = Enigo::new();
                let mouse_button = match button {
                    "right" => MouseButton::Right,
                    "middle" => MouseButton::Middle,
                    _ => MouseButton::Left,
                };
                enigo.mouse_up(mouse_button)
                    .map_err(|e| format!("Failed to mouse up: {:?}", e))?;
                Ok(format!("Mouse up ({})", button))
            }
            #[cfg(target_os = "linux")]
            {
                use enigo::*;
                let mut enigo = Enigo::new();
                let mouse_button = match button {
                    "right" => MouseButton::Right,
                    "middle" => MouseButton::Middle,
                    _ => MouseButton::Left,
                };
                enigo.mouse_up(mouse_button)
                    .map_err(|e| format!("Failed to mouse up: {:?}", e))?;
                Ok(format!("Mouse up ({})", button))
            }
            #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
            {
                Err("Mouse up not supported on this platform".to_string())
            }
        }
        "scroll" => {
            if let (Some(dx), Some(dy)) = (event.delta_x, event.delta_y) {
                #[cfg(target_os = "macos")]
                {
                    use enigo::*;
                    let mut enigo = Enigo::new();
                    // Support both horizontal and vertical scrolling
                    if dy.abs() > 0.0 {
                        match enigo.mouse_scroll_y(dy as i32) {
                            Ok(_) => {},
                            Err(_) => {
                                // Fallback to osascript
                                let script = format!(
                                    "tell application \"System Events\"\n\
                                     scroll at {0, 0} by {{{}, {}}}\n\
                                     end tell",
                                    dy as i32, dx as i32
                                );
                                Command::new("osascript")
                                    .args(["-e", &script])
                                    .spawn()
                                    .map_err(|e| format!("Failed to scroll: {}", e))?;
                                return Ok("Mouse scrolled (osascript)".to_string());
                            }
                        }
                    }
                    if dx.abs() > 0.0 {
                        match enigo.mouse_scroll_x(dx as i32) {
                            Ok(_) => Ok("Mouse scrolled (enigo)".to_string()),
                            Err(_) => Ok("Mouse scrolled (vertical only)".to_string()),
                        }
                    } else {
                        Ok("Mouse scrolled (enigo)".to_string())
                    }
                }
                #[cfg(target_os = "windows")]
                {
                    use enigo::*;
                    let mut enigo = Enigo::new();
                    if dy.abs() > 0.0 {
                        enigo.mouse_scroll_y(dy as i32)
                            .map_err(|e| format!("Failed to scroll: {}", e))?;
                    }
                    if dx.abs() > 0.0 {
                        enigo.mouse_scroll_x(dx as i32)
                            .map_err(|e| format!("Failed to scroll: {}", e))?;
                    }
                    Ok("Mouse scrolled".to_string())
                }
                #[cfg(target_os = "linux")]
                {
                    use enigo::*;
                    let mut enigo = Enigo::new();
                    if dy.abs() > 0.0 {
                        enigo.mouse_scroll_y(dy as i32)
                            .map_err(|e| format!("Failed to scroll: {}", e))?;
                    }
                    if dx.abs() > 0.0 {
                        enigo.mouse_scroll_x(dx as i32)
                            .map_err(|e| format!("Failed to scroll: {}", e))?;
                    }
                    Ok("Mouse scrolled".to_string())
                }
                #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
                {
                    Err("Mouse scroll not supported on this platform".to_string())
                }
            } else {
                Err("Missing delta_x or delta_y".to_string())
            }
        }
        _ => {
            Err(format!("Unknown mouse action: {}", event.action))
        }
    }
}

#[tauri::command]
fn control_keyboard(event: KeyboardEvent) -> Result<String, String> {
    println!("[control_keyboard] Event: {:?}", event);
    
    match event.action.as_str() {
        "text" => {
            if let Some(text) = event.text {
                #[cfg(target_os = "macos")]
                {
                    use enigo::*;
                    let mut enigo = Enigo::new();
                    match enigo.key_sequence(&text) {
                        Ok(_) => Ok("Text typed (enigo)".to_string()),
                        Err(_) => {
                            // Fallback to osascript
                            let script = format!(
                                "tell application \"System Events\"\n\
                                 keystroke \"{}\"\n\
                                 end tell",
                                text.replace("\"", "\\\"").replace("\\", "\\\\")
                            );
                            Command::new("osascript")
                                .args(["-e", &script])
                                .spawn()
                                .map_err(|e| format!("Failed to type text: {}", e))?;
                            Ok("Text typed (osascript)".to_string())
                        }
                    }
                }
                #[cfg(target_os = "windows")]
                {
                    use enigo::*;
                    let mut enigo = Enigo::new();
                    enigo.key_sequence(&text)
                        .map_err(|e| format!("Failed to type text: {}", e))?;
                    Ok("Text typed".to_string())
                }
                #[cfg(target_os = "linux")]
                {
                    use enigo::*;
                    let mut enigo = Enigo::new();
                    enigo.key_sequence(&text)
                        .map_err(|e| format!("Failed to type text: {}", e))?;
                    Ok("Text typed".to_string())
                }
                #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
                {
                    Err("Keyboard input not supported on this platform".to_string())
                }
            } else {
                Err("Missing text".to_string())
            }
        }
        "key" | "keyDown" => {
            if let Some(key) = event.key {
                // Helper function to map key string to enigo::Key
                fn map_key(key_str: &str) -> Option<enigo::Key> {
                    match key_str {
                        "Enter" | "Return" => Some(enigo::Key::Return),
                        "Backspace" => Some(enigo::Key::Backspace),
                        "Tab" => Some(enigo::Key::Tab),
                        "Escape" | "Esc" => Some(enigo::Key::Escape),
                        "Space" => Some(enigo::Key::Space),
                        "Delete" => Some(enigo::Key::Delete),
                        "Up" | "ArrowUp" => Some(enigo::Key::UpArrow),
                        "Down" | "ArrowDown" => Some(enigo::Key::DownArrow),
                        "Left" | "ArrowLeft" => Some(enigo::Key::LeftArrow),
                        "Right" | "ArrowRight" => Some(enigo::Key::RightArrow),
                        "Home" => Some(enigo::Key::Home),
                        "End" => Some(enigo::Key::End),
                        "PageUp" => Some(enigo::Key::PageUp),
                        "PageDown" => Some(enigo::Key::PageDown),
                        "F1" => Some(enigo::Key::F1),
                        "F2" => Some(enigo::Key::F2),
                        "F3" => Some(enigo::Key::F3),
                        "F4" => Some(enigo::Key::F4),
                        "F5" => Some(enigo::Key::F5),
                        "F6" => Some(enigo::Key::F6),
                        "F7" => Some(enigo::Key::F7),
                        "F8" => Some(enigo::Key::F8),
                        "F9" => Some(enigo::Key::F9),
                        "F10" => Some(enigo::Key::F10),
                        "F11" => Some(enigo::Key::F11),
                        "F12" => Some(enigo::Key::F12),
                        "Control" | "Ctrl" => Some(enigo::Key::Control),
                        "Alt" => Some(enigo::Key::Alt),
                        "Shift" => Some(enigo::Key::Shift),
                        "Meta" | "Cmd" | "Command" => Some(enigo::Key::Meta),
                        _ => None,
                    }
                }
                
                // Handle modifiers
                let modifiers = event.modifiers.as_ref().unwrap_or(&vec![]);
                let has_ctrl = modifiers.contains(&"Control".to_string()) || modifiers.contains(&"Ctrl".to_string());
                let has_alt = modifiers.contains(&"Alt".to_string());
                let has_shift = modifiers.contains(&"Shift".to_string());
                let has_meta = modifiers.contains(&"Meta".to_string()) || modifiers.contains(&"Cmd".to_string()) || modifiers.contains(&"Command".to_string());
                
                #[cfg(target_os = "macos")]
                {
                    use enigo::*;
                    let mut enigo = Enigo::new();
                    
                    // Press modifiers first
                    if has_ctrl {
                        let _ = enigo.key(Key::Control, Direction::Press);
                    }
                    if has_alt {
                        let _ = enigo.key(Key::Alt, Direction::Press);
                    }
                    if has_shift {
                        let _ = enigo.key(Key::Shift, Direction::Press);
                    }
                    if has_meta {
                        let _ = enigo.key(Key::Meta, Direction::Press);
                    }
                    
                    // Press the main key
                    if let Some(enigo_key) = map_key(&key) {
                        let result = enigo.key(enigo_key, Direction::Press);
                        
                        // Release modifiers
                        if has_meta {
                            let _ = enigo.key(Key::Meta, Direction::Release);
                        }
                        if has_shift {
                            let _ = enigo.key(Key::Shift, Direction::Release);
                        }
                        if has_alt {
                            let _ = enigo.key(Key::Alt, Direction::Release);
                        }
                        if has_ctrl {
                            let _ = enigo.key(Key::Control, Direction::Release);
                        }
                        
                        match result {
                            Ok(_) => Ok(format!("Key {} pressed with modifiers (enigo)", key)),
                            Err(_) => {
                                // Fallback to osascript
                                let mods = vec![
                                    if has_ctrl { "control down" } else { "" },
                                    if has_alt { "option down" } else { "" },
                                    if has_shift { "shift down" } else { "" },
                                    if has_meta { "command down" } else { "" },
                                ].into_iter().filter(|s| !s.is_empty()).collect::<Vec<_>>().join(", ");
                                
                                let script = if !mods.is_empty() {
                                    format!(
                                        "tell application \"System Events\"\n\
                                         key code {} using {{{}}}\n\
                                         end tell",
                                        key, mods
                                    )
                                } else {
                                    format!(
                                        "tell application \"System Events\"\n\
                                         key code {}\n\
                                         end tell",
                                        key
                                    )
                                };
                                
                                Command::new("osascript")
                                    .args(["-e", &script])
                                    .spawn()
                                    .map_err(|e| format!("Failed to press key: {}", e))?;
                                Ok(format!("Key {} pressed with modifiers (osascript)", key))
                            }
                        }
                    } else {
                        // Release modifiers if key not found
                        if has_meta {
                            let _ = enigo.key(Key::Meta, Direction::Release);
                        }
                        if has_shift {
                            let _ = enigo.key(Key::Shift, Direction::Release);
                        }
                        if has_alt {
                            let _ = enigo.key(Key::Alt, Direction::Release);
                        }
                        if has_ctrl {
                            let _ = enigo.key(Key::Control, Direction::Release);
                        }
                        Err(format!("Unsupported key: {}", key))
                    }
                }
                #[cfg(target_os = "windows")]
                {
                    use enigo::*;
                    let mut enigo = Enigo::new();
                    
                    // Press modifiers first
                    if has_ctrl {
                        let _ = enigo.key(Key::Control, Direction::Press);
                    }
                    if has_alt {
                        let _ = enigo.key(Key::Alt, Direction::Press);
                    }
                    if has_shift {
                        let _ = enigo.key(Key::Shift, Direction::Press);
                    }
                    
                    // Press the main key
                    if let Some(enigo_key) = map_key(&key) {
                        enigo.key(enigo_key, Direction::Press)
                            .map_err(|e| format!("Failed to press key {}: {}", key, e))?;
                        
                        // Release modifiers
                        if has_shift {
                            let _ = enigo.key(Key::Shift, Direction::Release);
                        }
                        if has_alt {
                            let _ = enigo.key(Key::Alt, Direction::Release);
                        }
                        if has_ctrl {
                            let _ = enigo.key(Key::Control, Direction::Release);
                        }
                        
                        Ok(format!("Key {} pressed with modifiers", key))
                    } else {
                        // Release modifiers if key not found
                        if has_shift {
                            let _ = enigo.key(Key::Shift, Direction::Release);
                        }
                        if has_alt {
                            let _ = enigo.key(Key::Alt, Direction::Release);
                        }
                        if has_ctrl {
                            let _ = enigo.key(Key::Control, Direction::Release);
                        }
                        Err(format!("Unsupported key: {}", key))
                    }
                }
                #[cfg(target_os = "linux")]
                {
                    use enigo::*;
                    let mut enigo = Enigo::new();
                    
                    // Press modifiers first
                    if has_ctrl {
                        let _ = enigo.key(Key::Control, Direction::Press);
                    }
                    if has_alt {
                        let _ = enigo.key(Key::Alt, Direction::Press);
                    }
                    if has_shift {
                        let _ = enigo.key(Key::Shift, Direction::Press);
                    }
                    
                    // Press the main key
                    if let Some(enigo_key) = map_key(&key) {
                        enigo.key(enigo_key, Direction::Press)
                            .map_err(|e| format!("Failed to press key {}: {}", key, e))?;
                        
                        // Release modifiers
                        if has_shift {
                            let _ = enigo.key(Key::Shift, Direction::Release);
                        }
                        if has_alt {
                            let _ = enigo.key(Key::Alt, Direction::Release);
                        }
                        if has_ctrl {
                            let _ = enigo.key(Key::Control, Direction::Release);
                        }
                        
                        Ok(format!("Key {} pressed with modifiers", key))
                    } else {
                        // Release modifiers if key not found
                        if has_shift {
                            let _ = enigo.key(Key::Shift, Direction::Release);
                        }
                        if has_alt {
                            let _ = enigo.key(Key::Alt, Direction::Release);
                        }
                        if has_ctrl {
                            let _ = enigo.key(Key::Control, Direction::Release);
                        }
                        Err(format!("Unsupported key: {}", key))
                    }
                }
                #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
                {
                    Err("Keyboard input not supported on this platform".to_string())
                }
            } else {
                Err("Missing key".to_string())
            }
        }
        "keyUp" => {
            if let Some(key) = event.key {
                fn map_key(key_str: &str) -> Option<enigo::Key> {
                    match key_str {
                        "Control" | "Ctrl" => Some(enigo::Key::Control),
                        "Alt" => Some(enigo::Key::Alt),
                        "Shift" => Some(enigo::Key::Shift),
                        "Meta" | "Cmd" | "Command" => Some(enigo::Key::Meta),
                        _ => None,
                    }
                }
                
                #[cfg(any(target_os = "windows", target_os = "macos", target_os = "linux"))]
                {
                    use enigo::*;
                    let mut enigo = Enigo::new();
                    if let Some(enigo_key) = map_key(&key) {
                        enigo.key(enigo_key, Direction::Release)
                            .map_err(|e| format!("Failed to release key {}: {}", key, e))?;
                        Ok(format!("Key {} released", key))
                    } else {
                        Err(format!("Unsupported key for release: {}", key))
                    }
                }
                #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
                {
                    Err("Keyboard input not supported on this platform".to_string())
                }
            } else {
                Err("Missing key".to_string())
            }
        }
        _ => {
            Err(format!("Unknown keyboard action: {}", event.action))
        }
    }
}

#[tauri::command]
fn get_screen_size() -> Result<ScreenSize, String> {
    println!("[get_screen_size] Getting screen size");
    
    #[cfg(target_os = "macos")]
    {
        // Use osascript to get screen size
        let script = "tell application \"Finder\"\n\
                      set screenSize to bounds of window of desktop\n\
                      set screenWidth to item 3 of screenSize\n\
                      set screenHeight to item 4 of screenSize\n\
                      return screenWidth & \",\" & screenHeight\n\
                      end tell";
        
        let output = Command::new("osascript")
            .args(["-e", script])
            .output()
            .map_err(|e| format!("Failed to get screen size: {}", e))?;
        
        let output_str = String::from_utf8_lossy(&output.stdout);
        let parts: Vec<&str> = output_str.trim().split(',').collect();
        
        if parts.len() >= 2 {
            let width = parts[0].trim().parse::<u32>()
                .map_err(|_| "Failed to parse width".to_string())?;
            let height = parts[1].trim().parse::<u32>()
                .map_err(|_| "Failed to parse height".to_string())?;
            Ok(ScreenSize { width, height })
        } else {
            // Fallback: use system_profiler
            let output = Command::new("system_profiler")
                .args(["SPDisplaysDataType"])
                .output()
                .map_err(|e| format!("Failed to get screen size: {}", e))?;
            
            let output_str = String::from_utf8_lossy(&output.stdout);
            // Parse resolution from output (e.g., "Resolution: 1920 x 1080")
            // For now, return default
            Ok(ScreenSize { width: 1920, height: 1080 })
        }
    }
    
    #[cfg(target_os = "windows")]
    {
        // Use PowerShell to get screen size
        let script = "Add-Type -AssemblyName System.Windows.Forms; [System.Windows.Forms.Screen]::PrimaryScreen.Bounds.Width; [System.Windows.Forms.Screen]::PrimaryScreen.Bounds.Height";
        let output = Command::new("powershell")
            .args(["-Command", &script])
            .output()
            .map_err(|e| format!("Failed to get screen size: {}", e))?;
        
        let output_str = String::from_utf8_lossy(&output.stdout);
        let lines: Vec<&str> = output_str.trim().lines().collect();
        
        if lines.len() >= 2 {
            let width = lines[0].trim().parse::<u32>()
                .map_err(|_| "Failed to parse width".to_string())?;
            let height = lines[1].trim().parse::<u32>()
                .map_err(|_| "Failed to parse height".to_string())?;
            Ok(ScreenSize { width, height })
        } else {
            Ok(ScreenSize { width: 1920, height: 1080 })
        }
    }
    
    #[cfg(target_os = "linux")]
    {
        // Use xrandr to get screen size
        let output = Command::new("xrandr")
            .args(["--current"])
            .output()
            .map_err(|e| format!("Failed to get screen size: {}", e))?;
        
        let output_str = String::from_utf8_lossy(&output.stdout);
        // Parse output (e.g., "Screen 0: minimum 320 x 200, current 1920 x 1080")
        for line in output_str.lines() {
            if line.contains("current") {
                if let Some(current_part) = line.split("current").nth(1) {
                    let parts: Vec<&str> = current_part.split('x').collect();
                    if parts.len() >= 2 {
                        let width = parts[0].trim().parse::<u32>()
                            .map_err(|_| "Failed to parse width".to_string())?;
                        let height = parts[1].split(',').next()
                            .and_then(|h| h.trim().parse::<u32>().ok())
                            .ok_or_else(|| "Failed to parse height".to_string())?;
                        return Ok(ScreenSize { width, height });
                    }
                }
            }
        }
        Ok(ScreenSize { width: 1920, height: 1080 })
    }
    
    #[cfg(not(any(target_os = "windows", target_os = "macos", target_os = "linux")))]
    {
        Err("Screen size detection not supported on this platform".to_string())
    }
}

// UDP Control Server
fn start_udp_control_server(port: u16) -> Result<(), String> {
    let socket = UdpSocket::bind(format!("0.0.0.0:{}", port))
        .map_err(|e| format!("Failed to bind UDP socket: {}", e))?;
    
    println!("[UDP Server] Listening on port {}", port);
    
    let mut buf = [0u8; 4096];
    
    loop {
        match socket.recv_from(&mut buf) {
            Ok((size, addr)) => {
                let data = &buf[..size];
                
                // Parse JSON message
                match serde_json::from_slice::<serde_json::Value>(data) {
                    Ok(json) => {
                        println!("[UDP Server] Received from {}: {:?}", addr, json);
                        
                        // Handle mouse control
                        if let Some(action) = json.get("type").and_then(|t| t.as_str()) {
                            match action {
                                "mouse" => {
                                    if let Ok(event) = serde_json::from_value::<MouseEvent>(json.clone()) {
                                        let _ = control_mouse(event);
                                    }
                                }
                                "keyboard" => {
                                    if let Ok(event) = serde_json::from_value::<KeyboardEvent>(json.clone()) {
                                        let _ = control_keyboard(event);
                                    }
                                }
                                "control" => {
                                    if let Some(action_str) = json.get("action").and_then(|a| a.as_str()) {
                                        let _ = control_computer(action_str.to_string());
                                    }
                                }
                                _ => {
                                    println!("[UDP Server] Unknown action: {}", action);
                                }
                            }
                        }
                    }
                    Err(e) => {
                        println!("[UDP Server] Failed to parse JSON: {}", e);
                    }
                }
            }
            Err(e) => {
                println!("[UDP Server] Error receiving: {}", e);
            }
        }
    }
}

#[tauri::command]
fn start_udp_server(port: u16) -> Result<String, String> {
    println!("[start_udp_server] Starting UDP server on port {}", port);
    
    // Start UDP server in a separate thread
    thread::spawn(move || {
        if let Err(e) = start_udp_control_server(port) {
            eprintln!("[UDP Server] Error: {}", e);
        }
    });
    
    Ok(format!("UDP server started on port {}", port))
}

fn get_local_ip() -> String {
    // Try to get local IP by connecting to external address
    if let Ok(socket) = UdpSocket::bind("0.0.0.0:0") {
        if socket.connect("8.8.8.8:80").is_ok() {
            if let Ok(addr) = socket.local_addr() {
                return addr.ip().to_string();
            }
        }
    }
    "127.0.0.1".to_string()
}

#[tauri::command]
fn get_udp_port() -> Result<u16, String> {
    // Try to bind to a random port to find an available port
    let socket = UdpSocket::bind("0.0.0.0:0")
        .map_err(|e| format!("Failed to bind socket: {}", e))?;
    
    let port = socket.local_addr()
        .map_err(|e| format!("Failed to get local addr: {}", e))?
        .port();
    
    Ok(port)
}

#[tauri::command]
fn get_local_ip_address() -> Result<String, String> {
    Ok(get_local_ip())
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            control_computer, 
            control_mouse, 
            control_keyboard, 
            get_screen_size,
            start_udp_server,
            get_udp_port,
            get_local_ip_address
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
