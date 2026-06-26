# Session Lifecycle

## Startup Sequence

1. **CLI Parsing**: `clap` parses arguments. Validates required command and flag combinations.
2. **Token Generation**: Random 32-char alphanumeric token, or manual via `--token`.
3. **Bind Address Resolution**: If `--lan` and loopback bind, promotes to `0.0.0.0`.
4. **Startup Print**: Outputs local URL, LAN URL (if `--lan`), mode, max clients, bind address, and WSL guidance.
5. **Session Initialization**:
   - Creates `mpsc::unbounded_channel` for PTY input
   - Creates `SessionState` with shared config and channels
   - Binds TCP listener for the web server
6. **Raw Terminal Mode**: If not headless, enters crossterm raw mode via RAII guard.
7. **Local Bridge**: If not headless and on an interactive terminal:
   - Spawns stdin reader thread (reads bytes, sends to PTY input channel)
   - Spawns stdout writer task (async, receives from broadcast and writes to stdout)
8. **PTY Spawn**:
   - Allocates PTY via `portable-pty`
   - Spawns child process by re-executing rterm itself with `__rterm-child <marker> -- <command>`
   - Starts four background threads: reader, writer, child-waiter, exit-file-watcher
9. **Resize Watcher**: If not headless, polls terminal size every 250ms.
10. **Web Server**: Spawns axum server on the TCP listener.

## Running State

During normal operation, data flows continuously:

```
local keyboard → stdin thread → input channel → PTY writer thread → PTY → child
PTY → reader thread → output broadcast → local stdout task + WebSocket clients
```

### Input Handling

All input sources (local stdin, WebSocket clients, resize watcher) send `PtyCommand` messages through the same `mpsc::UnboundedSender`. The single PTY writer thread consumes these sequentially, avoiding write conflicts.

### Output Handling

The PTY reader thread pushes all output bytes to:
1. The scrollback ring buffer (`Scrollback`)
2. The broadcast channel (`broadcast::Sender`)

Consumers (local stdout, WebSocket clients) receive bytes independently. A lagging consumer has its messages silently dropped (`RecvError::Lagged`).

### Exit Code Detection

Exit code detection is triple-redundant:

1. **OSC Escape Sequence**: The child helper (`__rterm-child`) writes `\x1b]6973;rterm-exit:<marker>:<code>\x07` to stdout just before exiting. The `ExitMarkerFilter` in the PTY reader detects this, removes it from output, and sends the exit code via the oneshot channel.

2. **Exit File**: The child helper writes the exit code to `$TMP/rterm-exit-<marker>-<pid>.txt`. A dedicated thread polls this file every 50ms.

3. **Child Wait**: A dedicated thread polls `child.try_wait()` every 50ms.

Whichever mechanism fires first wins. All three share a single `Arc<Mutex<Option<oneshot::Sender>>>` — the first to take the sender sends the exit code.

### Exit Code Range

The child helper clamps the exit code to `0..=255` using `status.code().unwrap_or(1).clamp(0, 255) as u8`. A spawn failure returns code `127`.

## Shutdown Sequence

When the exit code is received via the oneshot channel:

1. **Drop Input Sender**: Dropping the `input_tx` sender closes the channel, causing the PTY writer thread to exit its receive loop.
2. **Join PTY Writer**: `PtyHandle::join_writer()` drops the handle, joining the writer thread.
3. **Abort Local Handles**: All local bridge tasks (stdin reader, stdout writer) are aborted.
4. **Abort Resize Watcher**: The resize polling task is aborted.
5. **Abort Web Server**: The axum server task is aborted.
6. **Return Exit Code**: The session function returns the exit code.
7. **Raw Mode Restore**: When `RawTerminalGuard` drops (on function return or panic), crossterm raw mode is disabled, restoring the terminal to its previous state.

## Client Connection Lifecycle

```
Client connects
  ├── Token validated? No → 401 Unauthorized
  ├── ClientPermit available? No → 429 Too Many Requests
  │
  ├── Send status frame (writable mode, word-erase sequence)
  ├── Send scrollback replay
  ├── Enter select loop:
  │     select! {
  │       WebSocket message → handle input/control/ping
  │       Broadcast output → forward to WebSocket
  │     }
  │
  └── Client disconnects (socket close, error, or loop break)
       └── ClientPermit drops → active_clients -= 1
       └── If --once → closed_to_new_clients = true
```

### `--once` Behavior

When `--once` is set, after the first client disconnects, `closed_to_new_clients` is set to `true`. Any subsequent connection attempts receive `None` from `try_acquire_client()` — even if the active client count is zero.

### Scrollback Replay

When a new WebSocket client connects, the server sends the entire current scrollback buffer (up to 1 MB) as consecutive `0x01` output frames before streaming live output. This gives late-connecting clients the full session history.

## Error Recovery

- **Bind failure**: Returns error immediately (port in use, permission denied)
- **PTY open failure**: Returns error immediately
- **Child spawn failure**: Returns error immediately
- **Reader/writer thread failure**: Thread exits silently; the exit code fallback mechanisms ensure the session still terminates
- **Local bridge failure**: Input/output errors on the local terminal cause the bridge tasks to exit; the session continues until the child exits
- **WebSocket disconnect**: Client is cleaned up; session continues