# Multi-Monitor Mouse Input Fix

## Issue
On student machines with multiple monitors, the screen capture was correctly showing the **Primary Monitor**, but mouse inputs (clicks, movements) were landing on the wrong screen (e.g., the secondary monitor).

## Root Cause
The previous implementation calculated mouse coordinates assuming the Primary Monitor always starts at `(0, 0)` of the virtual desktop.
```rust
// Old incorrect logic
let x = (event.x * screen_width) as i32; // Assumes x_offset = 0
```

In multi-monitor setups (especially where the secondary is to the left or top of the primary), the Primary Monitor's origin is NOT `(0, 0)`.
For example, if Monitor 2 (1920x1080) is to the left of Main Monitor (1920x1080):
- Monitor 2 range: `[0, 1920]`
- Main Monitor range: `[1920, 3840]`

Sending `x=100` would move the mouse to Monitor 2, while the user intended to click on the Main Monitor.

## Solution
Updated `src-tauri/src/student_agent.rs` -> `handle_mouse_input`:
1.  **Fetch Monitor Info**: Calls `xcap::Monitor::all()` to get the current layout.
2.  **Identify Primary**: Selects the monitor with `is_primary() == true`, matching the screen capture logic.
3.  **Apply Offsets**: Adds the monitor's `x` and `y` coordinates to the calculated position.

```rust
// New correct logic
let x = monitor.x() + (event.x * monitor.width()) as i32;
let y = monitor.y() + (event.y * monitor.height()) as i32;
```

This ensures mouse inputs strictly map to the coordinate space of the monitor being shared.
