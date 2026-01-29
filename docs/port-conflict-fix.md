# Port 3017 Conflict Resolution

## Issue
Occasionally, the Student Agent fails to start on the default port (3017) because a previous instance (or zombie process) is still holding the port.

## Solution
We implemented an **Auto-Kill** mechanism in `src-tauri/src/student_agent.rs`.

Before binding to the port, `start_agent` now calls `kill_port_holder(port)`, which executes:

### On Windows
```powershell
Get-Process ... | Where-Object { $_.Id -ne <CURRENT_PID> } | Stop-Process -Force
```
*Requires PowerShell (standard on modern Windows).*

### On macOS / Linux
```bash
lsof -t -i:3017 | grep -v ^<CURRENT_PID>$ | xargs kill -9
```
*Requires `lsof` tool.*

## Behavior
1. Agent starts.
2. Checks configured port (default 3017).
3. Forcefully terminates any process listening on that port **EXCEPT the current application itself**.
   - This prevents the app from killing itself if it already bound the port (e.g. for Discovery).
4. Waits 500ms.
5. Binds to the port.

This ensures the agent always reclaims its port upon restart.
