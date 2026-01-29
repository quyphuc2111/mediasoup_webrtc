# Port 3017 Conflict Resolution

## Issue
Occasionally, the Student Agent fails to start on the default port (3017) because a previous instance (or zombie process) is still holding the port.

## Solution
We implemented an **Auto-Kill** mechanism in `src-tauri/src/student_agent.rs`.

Before binding to the port, `start_agent` now calls `kill_port_holder(port)`, which executes:

### On Windows
```powershell
Get-Process -Id (Get-NetTCPConnection -LocalPort 3017).OwningProcess | Stop-Process -Force
```
*Requires PowerShell (standard on modern Windows).*

### On macOS / Linux
```bash
lsof -t -i:3017 | xargs kill -9
```
*Requires `lsof` tool.*

## Behavior
1. Agent starts.
2. Checks configured port (default 3017).
3. Forcefully terminates any process listening on that port.
4. Waits 500ms.
5. Binds to the port.

This ensures the agent always reclaims its port upon restart.
