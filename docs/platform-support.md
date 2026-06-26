# Platform Support

## Target Matrix

| Platform | Architecture | Status |
|----------|-------------|--------|
| Windows | x64 | Supported |
| Windows | arm64 | Supported |
| Linux | x64 | Supported |
| Linux | arm64 | Supported |

## Windows

### PTY Backend

Uses `portable-pty` which delegates to Windows ConPTY. No special configuration needed.

### Command Resolution

Windows has special command resolution logic in `platform::command`:

- **Executables**: `.exe`, `.com` — resolved via PATH and PATHEXT
- **PowerShell scripts**: `.ps1` — wrapped through `pwsh -NoProfile -ExecutionPolicy Bypass -File <script>`
- **Batch scripts**: `.cmd`, `.bat` — wrapped through `cmd /C <script>`
- **PATH search**: Uses `PATHEXT` environment variable for extension priority

### Terminal Behavior

- Raw terminal mode is enabled via `crossterm` on Windows
- Windows Terminal and ConPTY support the expected terminal escape sequences
- `Ctrl+Backspace` sends `\x17` (Ctrl+W) by default; may need customization for some Windows shells

### WSL Interop

rterm can be run inside WSL. See [WSL section](#wsl) below.

## Linux

### PTY Backend

Uses Unix PTY via `portable-pty`. Standard `/dev/ptmx` allocation.

### Command Resolution

On Unix, commands are resolved by the shell's PATH lookup. rterm passes the command name directly. No special handling for script extensions.

### Terminal Behavior

- Raw mode via `crossterm` (uses termios)
- Standard ANSI escape sequences
- SIGWINCH for resize events

## WSL (Windows Subsystem for Linux)

### Detection

rterm detects WSL by reading `/proc/sys/kernel/osrelease` (or `/proc/version` fallback) and checking for `microsoft` in the content string.

### LAN Networking

WSL2 uses NAT by default, which means the rterm web server is not directly reachable from LAN devices even with `--lan`.

When WSL is detected and `--lan` is set, rterm prints:

```
Detected WSL. Windows may reach this at http://localhost:7843/t/<token>.
For phone access, use WSL mirrored networking when available, or expose the port from Windows with portproxy/firewall rules.
```

### Solutions for Phone Access from WSL

**Option 1: Mirrored Networking (WSL2, newer versions)**

Enable in `.wslconfig`:
```ini
[wsl2]
networkingMode=mirrored
```

**Option 2: Windows Port Forwarding**

In an **admin** PowerShell:
```powershell
# Find WSL IP
wsl hostname -I

# Create portproxy rule
netsh interface portproxy add v4tov4 `
  listenaddress=0.0.0.0 `
  listenport=7843 `
  connectaddress=<WSL_IP> `
  connectport=7843

# Allow through firewall
New-NetFirewallRule `
  -DisplayName "rterm 7843" `
  -Direction Inbound `
  -Action Allow `
  -Protocol TCP `
  -LocalPort 7843 `
  -Profile Private
```

**Option 3: Access via Windows localhost**

If you only need access from the Windows host (not other LAN devices), use `http://localhost:7843/t/<token>` from a Windows browser.

## macOS

Not currently tested or targeted. The code should work in theory (Unix PTY + crossterm) but is not validated.

## Dependencies by Platform

| Crate | Windows | Linux | Purpose |
|-------|---------|-------|---------|
| `portable-pty` | ConPTY | Unix PTY | PTY allocation and management |
| `crossterm` | Windows API | termios | Raw terminal mode, resize detection |
| `local-ip-address` | Windows | Linux | LAN IP detection |

## Testing Per Platform

For each target, validate:

```text
✓ cargo build
✓ cargo test
✓ Interactive terminal: Ctrl+C, Ctrl+D, Ctrl+Backspace, arrows, resize, alternate screen
✓ Exit code propagation
✓ Web server binds and serves terminal page
✓ WebSocket connects and streams output
✓ --lan binds to all interfaces
```

See `docs/v0_validation.md` for the full smoke test checklist.