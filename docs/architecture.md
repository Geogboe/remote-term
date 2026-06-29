# rterm Architecture

`rterm` runs a command inside a managed PTY and exposes the same terminal session to a tokenized browser view. The local terminal remains the primary controller; the browser observes by default and writes only when `--write` is supplied.

## High-Level Data Flow

```
                 ┌────────────┐
local stdin  ──▶ │            │ ──▶ child PTY stdin
local stdout ◀── │ PTY Router │ ◀── child PTY stdout/stderr
websocket   ──▶  │            │
websocket   ◀──  └────────────┘
```

Input flows through channels to a single writer task. Output is broadcast to all consumers.

```
LocalInputTask  ─┐
WebInputTask    ─┼─▶ input_tx (mpsc) ─▶ PtyWriterTask
ResizeTask      ─┘

PtyReaderTask ─▶ output_tx (broadcast) ─▶ LocalOutputTask
                                    └────▶ WebOutputTask
```

## Crate Structure

```
src/
├── main.rs              Entry point, child helper, tracing, startup
├── lib.rs               Module declarations
├── cli.rs               CLI argument parsing (clap derive)
├── platform/
│   ├── mod.rs           Re-exports
│   ├── command.rs       Cross-platform command resolution
│   ├── ctrl_c.rs        Windows child-helper Ctrl+C protection
│   ├── elevation.rs     Windows token / Unix effective-UID guard
│   ├── lan_ip.rs        LAN IP detection
│   └── wsl.rs           WSL detection and guidance
├── security/
│   ├── mod.rs           Re-exports
│   └── token.rs         Random token generation and validation
├── session/
│   ├── mod.rs           Session coordinator, RunConfig, SessionState, ClientPermit
│   ├── pty.rs           PTY spawn, reader/writer threads, resize watcher, local bridge
│   ├── raw_terminal.rs  Crossterm raw mode guard (RAII)
│   ├── registry.rs      Per-user active-session discovery records
│   └── scrollback.rs    Ring buffer for PTY output replay
└── web/
    ├── mod.rs           Re-exports
    ├── assets.rs        compile-time embedded static assets (include_str!)
    ├── protocol.rs      WebSocket binary/JSON frame protocol
    ├── server.rs        Axum HTTP router, WebSocket handler, client management
    └── static/          Generated frontend assets (committed build output)
```

## Module Details

### `main.rs`

The binary entry point. Handles two execution paths:

1. **`__rterm-child`**: Internal re-execution marker. When rterm spawns a PTY child, it re-executes itself with `__rterm-child <marker> -- <command>`. This child helper runs the real user command, writes the exit code to both a temp file and an OSC escape sequence on stdout, then exits. On Windows, an application-defined console handler consumes Ctrl+C in the helper process so the wrapped shell can interrupt its active command without terminating the wrapper.

2. **Management mode**: `rterm sessions` probes and lists live per-user registry records.

3. **Normal mode**: Initializes tracing, refuses unintended elevated execution,
generates or validates the token, prints startup information, and calls
`session::run_session`.

### `cli.rs`

Uses `clap` derive macros for argument parsing. Fields:

| Flag | Type | Default | Description |
|------|------|---------|-------------|
| `--bind` | `SocketAddr` | `127.0.0.1:7843` | Bind address for embedded web server |
| `--lan` | `bool` | `false` | Expose on all interfaces (`0.0.0.0`) |
| `--write` | `bool` | `false` | Allow browser clients to write to the PTY |
| `--max-clients` | `usize` | `1` | Maximum concurrent browser clients |
| `--once` | `bool` | `false` | Stop accepting new clients after first disconnect |
| `--headless` | `bool` | `false` | Do not attach local terminal to the PTY |
| `--token` | `Option<String>` | `None` (auto-generated) | Manual URL token |
| `--allow-elevated` | `bool` | `false` | Permit root/elevated session startup |
| `--backspace` | `Option<String>` | `\x7f` (DEL) | Browser Backspace byte sequence |
| `--word-erase` | `String` | `\x17` (Ctrl+W) | Browser Ctrl+Backspace byte sequence |
| `[command]` | `Vec<String>` | (required) | Command after `--` separator |

Methods:
- `effective_bind()`: When `--lan` is set and bind is loopback, returns `0.0.0.0:<port>`.
- `decoded_backspace()`: Uses the VT DEL default or parses an explicit sequence.
- `decoded_word_erase()`: Parses escape sequences like `\x1b\x7f` into bytes.

### `platform::command`

Cross-platform command resolution. On Unix, uses the command as-is. On Windows:
- Detects file extensions (`.ps1`, `.cmd`, `.bat`, `.exe`, `.com`)
- Wraps `.ps1` scripts through `pwsh -NoProfile -ExecutionPolicy Bypass -File`
- Wraps `.cmd`/`.bat` scripts through `cmd /C`
- Searches PATH and PATHEXT for executables

### `platform::lan_ip`

Uses `local-ip-address` crate to detect the primary LAN IP. Filters out loopback and link-local addresses.

### `platform::wsl`

Detects WSL by reading `/proc/sys/kernel/osrelease` (or `/proc/version` as fallback) and checking for `microsoft` in the content. Provides human-readable LAN guidance for WSL networking.

### `security::token`

Generates five-word tokens from EFF's 7,776-word long list using `rand`.
Explicit tokens are restricted to URL-path-safe ASCII. Request validation uses
exact string comparison.

### `session::mod`

Core session coordinator. Contains:

- **`RunConfig`**: Configuration struct passed from CLI to session runner.
- **`PtyCommand`**: Enum for commands sent to the PTY writer (`Input(Vec<u8>)`, `Resize { cols, rows }`).
- **`SessionState`**: Shared state between all session components. Contains token, write mode, client limits, channels (input/output), scrollback buffer, and atomic counters for active clients and closed-to-new-clients flag.
- **`ClientPermit`**: RAII guard. Acquired when a browser connects, released on drop. Enforces `max_clients` and `--once` behavior.
- **`run_session()`**: Main orchestrator. Sets up channels, spawns the PTY, starts local bridge (if not headless), starts resize watcher (if not headless), starts the web server, waits for exit, and tears down all tasks.

### `session::registry`

Publishes one JSON record per live process under the current user's local data
directory. Records contain the PID, executable name, mode, and full browser
URLs. Writes are atomic; an RAII guard removes the record on normal shutdown.
`rterm sessions` probes the authenticated local route and removes stale or
malformed records, with a short grace period for sessions still starting.

### `session::pty`

The PTY subsystem. Spawns four background threads:

1. **PTY Reader Thread** (`rterm-pty-reader`): Reads from the PTY master, filters for exit marker OSC sequences, pushes to scrollback, broadcasts to output channel, and optionally synthesizes terminal responses (for headless/non-interactive mode).
2. **Child Wait Thread** (`rterm-child-wait`): Polls `child.try_wait()` every 50ms and sends exit code via oneshot channel.
3. **Exit File Watch Thread** (`rterm-exit-file-watch`): Polls a temp file for the child's exit code (backup mechanism to the OSC sequence).
4. **PTY Writer Thread** (`rterm-pty-writer`): Receives `PtyCommand` from the input channel, writes input bytes to PTY or resizes the PTY.

Additional components:
- **`ExitMarkerFilter`**: State machine that scans PTY output for `ESC]6973;rterm-exit:<marker>:<code>\x07` sequences, removes them from output, and emits `ReaderEvent::Exit(code)`.
- **`TerminalResponder`**: In headless/non-interactive mode, watches for cursor position queries (`\x1b[6n`) and responds with `\x1b[1;1R`.

The PTY command builder explicitly inherits rterm's current working directory.
This overrides `portable-pty`'s Windows home-directory default and preserves
the expected `cd repo; rterm -- <command>` workflow.
- **`start_local_bridge()`**: Spawns stdin-reader and stdout-writer tasks to bridge the local terminal to the PTY.
- **`start_resize_watcher()`**: Polls terminal size every 250ms and sends resize commands when dimensions change.

### `session::raw_terminal`

RAII wrapper around crossterm raw mode. Enables raw mode on the local terminal when rterm starts, and restores the previous mode on drop (normal exit, panic, or Ctrl+C).

### `session::scrollback`

Thread-safe ring buffer (`VecDeque<u8>` behind `Arc<Mutex<>>`). Default capacity: 1 MB. When full, oldest bytes are discarded. New connections receive a snapshot replay before live streaming begins.

### `web::assets`

Compile-time embedding of frontend assets using `include_str!`:
- `INDEX_HTML`: The terminal page (with `__TOKEN__` placeholder)
- `MAIN_JS`: Bundled xterm.js application
- `STYLE_CSS`: Combined xterm.css + app styles

### `web::protocol`

Binary WebSocket frame protocol:

| Frame Type | Byte | Payload | Direction |
|------------|------|---------|-----------|
| Output | `0x01` | Raw terminal bytes | Server → Client |
| Input | `0x02` | Raw terminal bytes | Client → Server |
| Control | `0x03` | JSON | Bidirectional |

Control messages (JSON, tagged enum):

Client → Server:
- `{"type": "resize", "cols": 80, "rows": 24}`
- `{"type": "ping"}`

Server → Client:
- `{"type": "status", "writable": true, "backspace": [127], "word_erase": [23]}`
- `{"type": "error", "message": "browser input is disabled; restart with --write"}`
- `{"type": "pong"}`

### `web::server`

Axum-based HTTP server with routes:

| Route | Method | Auth | Response |
|-------|--------|------|----------|
| `/t/{token}` | GET | Token in path | HTML page with token injected |
| `/ws/{token}` | GET | Token in path | WebSocket upgrade |
| `/assets/main.js` | GET | None | JavaScript bundle |
| `/assets/style.css` | GET | None | CSS bundle |

Invalid tokens return `404` (terminal page) or `401` (WebSocket). Exceeding `max_clients` returns `429`.

WebSocket handler lifecycle:
1. Validate token → 401 if invalid
2. Acquire `ClientPermit` → 429 if no slots available
3. On connect: send `status` control frame (writable mode, Backspace and word-erase sequences)
4. Send scrollback replay (snapshot of recent output)
5. Enter select loop: read from both the client WebSocket and the PTY broadcast channel
6. On disconnect: `ClientPermit` drop releases slot; if `--once`, marks session as closed to new clients

## Dependency Graph

```
main
├── cli (clap)
├── platform::command (std::process, PATHEXT on Windows)
├── platform::lan_ip (local-ip-address)
├── platform::wsl (std::fs)
├── security::token (rand)
├── session::run_session
│   ├── web::server (axum, tower-http, futures-util)
│   ├── session::SessionState (tokio::sync)
│   ├── session::pty (portable-pty, tokio)
│   ├── session::raw_terminal (crossterm)
│   └── session::scrollback (std::collections)
└── tracing-subscriber
```

## Concurrency Model

rterm uses a mix of sync and async:

- **Async (tokio)**: Web server (axum), local output bridge, resize watcher, session orchestrator
- **Sync threads**: PTY reader, PTY writer, child wait, exit file watch, local stdin bridge
- **Channels**:
  - `mpsc::unbounded_channel()` for input → PTY writer (lossless, backpressure via channel)
  - `broadcast::channel(256)` for PTY output → local/browser consumers (allows lagged consumers to skip)
  - `oneshot::channel()` for exit code from PTY threads to session orchestrator

## Resize Authority

The local terminal owns PTY size while attached. The resize watcher polls `crossterm::terminal::size()` every 250ms and sends resize commands only when dimensions change.

Browser resize is disabled by default. It is enabled when:
- `--headless` is set (no local terminal attached), or
- The local side is not an interactive terminal (e.g., piped input)

The `SessionState::browser_resize` field controls this.

## Exit Code Propagation

Exit code propagation uses a multi-mechanism approach for reliability:

1. **OSC escape sequence**: The child helper writes `\x1b]6973;rterm-exit:<marker>:<code>\x07` to stdout. The PTY reader's `ExitMarkerFilter` detects and removes this from output.
2. **Exit file**: The child helper writes the exit code to `$TMP/rterm-exit-<marker>-<pid>.txt`. The exit file watch thread polls this file.
3. **Process wait**: The child wait thread polls `child.try_wait()` every 50ms.

Whichever mechanism fires first sends the exit code via the oneshot channel. The session then tears down all tasks and returns the exit code.
