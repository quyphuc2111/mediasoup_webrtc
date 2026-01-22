use std::sync::Mutex;

#[cfg(target_os = "macos")]
use core_graphics::event::{
    CGEvent, CGEventTap, CGEventTapLocation, CGEventTapOptions,
    CGEventTapPlacement, CGEventType,
};
#[cfg(target_os = "macos")]
use core_foundation::runloop::CFRunLoop;

#[cfg(target_os = "windows")]
use windows::{
    core::*,
    Win32::Foundation::*,
    Win32::UI::WindowsAndMessaging::*,
    Win32::System::Threading::*,
};

static INPUT_BLOCKED: Mutex<bool> = Mutex::new(false);

#[cfg(target_os = "macos")]
static EVENT_TAP: Mutex<Option<CGEventTap>> = Mutex::new(None);

#[cfg(target_os = "windows")]
struct WindowsHooks {
    keyboard: HHOOK,
    mouse: HHOOK,
}

#[cfg(target_os = "windows")]
static HOOK_HANDLES: Mutex<Option<WindowsHooks>> = Mutex::new(None);

pub fn set_input_blocked(block: bool) -> Result<(), String> {
    let mut blocked = INPUT_BLOCKED.lock().map_err(|e| e.to_string())?;
    
    if *blocked == block {
        return Ok(()); // Already in desired state
    }
    
    *blocked = block;
    
    if block {
        #[cfg(target_os = "macos")]
        start_event_tap()?;
        #[cfg(target_os = "windows")]
        start_windows_hook()?;
    } else {
        #[cfg(target_os = "macos")]
        stop_event_tap()?;
        #[cfg(target_os = "windows")]
        stop_windows_hook()?;
    }
    
    Ok(())
}

#[cfg(target_os = "macos")]
fn start_event_tap() -> Result<(), String> {
    let mut tap_guard = EVENT_TAP.lock().map_err(|e| e.to_string())?;
    
    if tap_guard.is_some() {
        return Ok(()); // Already running
    }
    
    // Create event tap for keyboard and mouse events
    let event_tap = CGEventTap::new(
        CGEventTapLocation::HID, // Monitor HID events
        CGEventTapPlacement::HeadInsertEventTap,
        CGEventTapOptions::Default,
        vec![
            CGEventType::KeyDown,
            CGEventType::KeyUp,
            CGEventType::FlagsChanged,
            CGEventType::LeftMouseDown,
            CGEventType::LeftMouseUp,
            CGEventType::RightMouseDown,
            CGEventType::RightMouseUp,
            CGEventType::OtherMouseDown,
            CGEventType::OtherMouseUp,
            CGEventType::MouseMoved,
            CGEventType::ScrollWheel,
        ],
        |_proxy, event_type, event| -> Option<CGEvent> {
            // Check if input is blocked
            let blocked = match INPUT_BLOCKED.lock() {
                Ok(guard) => *guard,
                Err(_) => return Some(event), // If lock fails, allow event
            };
            
            if !blocked {
                return Some(event); // Pass through if not blocked
            }
            
            // Block all keyboard and mouse events
            match event_type {
                CGEventType::KeyDown
                | CGEventType::KeyUp
                | CGEventType::FlagsChanged
                | CGEventType::LeftMouseDown
                | CGEventType::LeftMouseUp
                | CGEventType::RightMouseDown
                | CGEventType::RightMouseUp
                | CGEventType::OtherMouseDown
                | CGEventType::OtherMouseUp
                | CGEventType::MouseMoved
                | CGEventType::ScrollWheel => {
                    // Return None to block the event
                    None
                }
                _ => Some(event), // Allow other events
            }
        },
    )
    .map_err(|e| format!("Failed to create event tap: {:?}. Make sure the app has Accessibility permissions in System Settings > Privacy & Security > Accessibility.", e))?;
    
    // Enable the event tap
    event_tap.enable();
    
    *tap_guard = Some(event_tap);
    
    // Run event loop in a separate thread
    std::thread::spawn(|| {
        let run_loop = unsafe { CFRunLoop::get_current() };
        unsafe {
            CFRunLoop::run_current();
        }
    });
    
    Ok(())
}

#[cfg(target_os = "macos")]
fn stop_event_tap() -> Result<(), String> {
    let mut tap_guard = EVENT_TAP.lock().map_err(|e| e.to_string())?;
    
    if let Some(tap) = tap_guard.take() {
        tap.disable();
    }
    
    Ok(())
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn low_level_keyboard_proc(
    n_code: i32,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    if n_code >= 0 {
        let blocked = match INPUT_BLOCKED.lock() {
            Ok(guard) => *guard,
            Err(_) => return CallNextHookEx(None, n_code, w_param, l_param),
        };
        
        if blocked {
            // Block all keyboard events
            return LRESULT(1);
        }
    }
    
    CallNextHookEx(None, n_code, w_param, l_param)
}

#[cfg(target_os = "windows")]
unsafe extern "system" fn low_level_mouse_proc(
    n_code: i32,
    w_param: WPARAM,
    l_param: LPARAM,
) -> LRESULT {
    if n_code >= 0 {
        let blocked = match INPUT_BLOCKED.lock() {
            Ok(guard) => *guard,
            Err(_) => return CallNextHookEx(None, n_code, w_param, l_param),
        };
        
        if blocked {
            // Block all mouse events
            return LRESULT(1);
        }
    }
    
    CallNextHookEx(None, n_code, w_param, l_param)
}

#[cfg(target_os = "windows")]
fn start_windows_hook() -> Result<(), String> {
    let mut hook_guard = HOOK_HANDLES.lock().map_err(|e| e.to_string())?;
    
    if hook_guard.is_some() {
        return Ok(()); // Already running
    }
    
    unsafe {
        let module_handle = GetModuleHandleW(None)
            .map_err(|e| format!("Failed to get module handle: {:?}", e))?;
        
        // Install low-level keyboard hook
        let keyboard_hook = SetWindowsHookExW(
            WH_KEYBOARD_LL,
            Some(low_level_keyboard_proc),
            module_handle,
            0,
        )
        .map_err(|e| format!("Failed to install keyboard hook: {:?}. Make sure the app is running with appropriate permissions.", e))?;
        
        // Install low-level mouse hook
        let mouse_hook = SetWindowsHookExW(
            WH_MOUSE_LL,
            Some(low_level_mouse_proc),
            module_handle,
            0,
        )
        .map_err(|e| {
            // If mouse hook fails, unhook keyboard hook
            let _ = UnhookWindowsHookEx(keyboard_hook);
            format!("Failed to install mouse hook: {:?}. Make sure the app is running with appropriate permissions.", e)
        })?;
        
        *hook_guard = Some(WindowsHooks {
            keyboard: keyboard_hook,
            mouse: mouse_hook,
        });
    }
    
    Ok(())
}

#[cfg(target_os = "windows")]
fn stop_windows_hook() -> Result<(), String> {
    let mut hook_guard = HOOK_HANDLES.lock().map_err(|e| e.to_string())?;
    
    if let Some(hooks) = hook_guard.take() {
        unsafe {
            UnhookWindowsHookEx(hooks.keyboard)
                .ok()
                .map_err(|e| format!("Failed to unhook keyboard: {:?}", e))?;
            UnhookWindowsHookEx(hooks.mouse)
                .ok()
                .map_err(|e| format!("Failed to unhook mouse: {:?}", e))?;
        }
    }
    
    Ok(())
}
