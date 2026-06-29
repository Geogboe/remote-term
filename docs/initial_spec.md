# Project: `rterm`

`rterm` is a small Rust CLI for running a command inside a managed pseudo-terminal and optionally exposing that same terminal session to a browser on the local network. It is intended for long-running coding-agent sessions where the user starts work locally, walks away, and later checks or controls the session from a phone.

It is not a general remote desktop tool. It streams terminal bytes, not pixels.

## Problem statement

The core problem is continuity for interactive terminal work.

Coding agents such as Codex or Claude often run as long-lived interactive terminal programs. The user starts the session at a real keyboard because that is the best place to review context, type prompts, interrupt mistakes, and inspect output. Later, the user may leave the desk while the agent is still running. At that point they want to check progress, copy or read recent output, answer a prompt, send `Ctrl+C`, or type a short follow-up from a phone.

Existing options are awkward for this workflow:

```text
tmux/screen       powerful, but must be adopted as the session model
SSH from phone    workable, but mobile terminal input is unpleasant and setup-heavy
remote desktop    streams pixels and is poor for terminal text/control
cloud relay       adds infrastructure and trust concerns
ad hoc web UIs    do not control the real terminal process
```

`rterm` should make the session host nearly invisible. The user should be able to prefix a command:

```bash
rterm --lan --write -- codex
```

Then keep using the local terminal normally, while also having a secure-enough, tokenized browser view for the same underlying PTY. The phone is not a second shell and not a remote desktop. It is another controller/viewer for the one terminal session that `rterm` owns from the start.

The tool cannot solve retroactive attachment to a process that was started outside `rterm`. That limitation is acceptable because the intended habit is simple: use `rterm -- <command>` for sessions that might need later phone access.

The core design is:

```text
local terminal
     ⇅
rterm process
     ⇅
PTY / ConPTY
     ⇅
child command: bash / pwsh / codex / claude / etc.

browser + xterm.js
     ⇅ WebSocket
rterm embedded web server
```

`portable-pty` is the likely backend for PTY handling because it provides a cross-platform Rust API for system pseudo-terminals, including Unix PTYs and Windows behavior through the native platform layer. ([Docs.rs][1]) For the browser, xterm.js is the obvious front-end because it is a terminal component for the web and is used by projects such as VS Code, Tabby, Hyper, and ttyd. ([GitHub][2]) For the server, `axum` is a good fit because it is a Rust HTTP routing/request library built around the Tokio/Hyper ecosystem, and its `ws` feature supports WebSocket upgrade handling and split read/write tasks. ([Docs.rs][3])

## Review notes

Reviewed on 2026-06-26 against upstream docs for `portable-pty`, `axum`, `crossterm`, xterm.js, and Microsoft WSL networking guidance.

Implementation should start with a small runtime viability spike before broad feature work:

```text
1. cargo init and dependency resolution on the target dev machine
2. native Windows ConPTY smoke test on arm64 and x64
3. Linux PTY smoke test on arm64 and x64
4. local raw-mode passthrough smoke test for Ctrl+C, Ctrl+D, Ctrl+Backspace/word-erase, arrows, resize, and alternate screen
5. minimal axum WebSocket echo/output path
6. browser xterm.js rendering from bundled assets
```

The biggest correctness risks are not the HTTP server. They are terminal ownership, Windows ConPTY behavior, raw terminal restoration, resize policy, and avoiding PTY read/write deadlocks.

Decisions made in this spec:

```text
v0 default bind: 127.0.0.1
v0 LAN exposure: explicit --lan only, read-only by default
v0 browser write access: explicit --write only
v0 max browser clients: 1
v0 resize authority: local terminal wins while attached
v0 WSL behavior: detect and print guidance; do not mutate Windows firewall or portproxy state
v0 session persistence: no attach to already-running commands; rterm must own the PTY from start
v0 session discovery: publish live credentials in a per-user registry for `rterm sessions`
v0 generated credential: five random EFF long-list words separated by hyphens
v0 elevated execution: refuse session startup unless --allow-elevated is explicit
v0 frontend assets: source in repo, production assets embedded into the Rust binary
v0 target matrix: Windows arm64/x64 and Linux arm64/x64
v0 WebSocket protocol: one WebSocket endpoint with typed binary terminal frames and JSON control frames
```

No open product decisions are blocking the first implementation pass. Revisit target support after smoke tests prove the PTY behavior on each target.

## Goals

The primary goal is this workflow:

```bash
cd ~/repo
rterm -- codex
```

The local terminal behaves normally. The user sees the agent output, types locally, copies text, interrupts with `Ctrl+C`, uses word-editing chords such as `Ctrl+Backspace`, etc.

At startup, `rterm` prints:

```text
rterm session started
Local:  http://127.0.0.1:7843/t/<token>
LAN:    http://192.168.1.50:7843/t/<token>
Mode:   local + web, write enabled for one client
```

From Android, the user opens the LAN URL. The browser shows the same terminal session and can send keys.

Important behavior: the local terminal and the browser are two views/controllers of one underlying PTY. The child process has no idea whether input came from the local keyboard or the phone.

## Non-goals

`rterm` should not try to attach to arbitrary existing terminal sessions.

It should not capture Windows Terminal as pixels.

It should not expose an always-running remote shell daemon by default.

It should not require tmux, screen, dtach, SSH, RDP, VNC, or a cloud relay.

It should not become a general collaborative terminal product at first.

## CLI

Basic usage:

```bash
rterm -- codex
```

Run an explicit shell:

```bash
rterm -- bash
rterm -- pwsh
rterm -- wsl.exe
```

Bind only to localhost:

```bash
rterm --bind 127.0.0.1:7843 -- codex
```

Expose on LAN:

```bash
rterm --lan -- codex
```

Read-only phone view is the default LAN mode:

```bash
rterm --lan -- codex
```

Allow phone control:

```bash
rterm --lan --write -- codex
```

Single-client, exit web server after phone disconnects:

```bash
rterm --write --once -- codex
```

Suggested default:

```text
bind: 127.0.0.1
web write: disabled unless --write
token: generated as five random EFF long-list words every run
max web clients: 1
session lifetime: tied to child process
```

Suggested convenient write mode:

```bash
rterm --lan --write --once -- codex
```

That is probably the mode you’d use most often.

## User experience

Startup:

```text
$ rterm --lan --write -- codex

rterm
  Session: brave-river-4382
  Local URL: http://127.0.0.1:7843/t/harbor-lime-orbit-cabin-velvet
  LAN URL:   http://192.168.1.50:7843/t/harbor-lime-orbit-cabin-velvet
  Web mode:  writable, single client
  Kill key:  Ctrl+Shift+]
```

Then the child command takes over the terminal:

```text
codex>
```

The local terminal should feel like the command was started directly. That means raw-mode handling, terminal resize propagation, `Ctrl+C`, `Ctrl+D`, `Ctrl+Backspace`/word-erase, arrow keys, colors, alternate screen, and line wrapping all need to pass through cleanly.

On phone, the browser UI should be extremely simple:

```text
┌──────────────────────────────┐
│ terminal viewport             │
│                               │
├──────────────────────────────┤
│ Esc Tab Ctrl Alt ↑ ↓ ← →      │
│ Ctrl+C Ctrl+D Ctrl+⌫ Enter    │
│ Paste                         │
└──────────────────────────────┘
```

The on-screen key strip matters because mobile keyboards are poor terminal keyboards.

## Architecture

The process has five main parts.

First, the PTY manager creates a PTY and launches the child command inside it.

Second, the local bridge connects the user’s local terminal to the PTY. It reads stdin and writes to the PTY, then reads PTY output and writes to local stdout.

Third, the web server serves a static HTML/JS app and a WebSocket endpoint.

Fourth, the web bridge connects the WebSocket to the same PTY. It broadcasts PTY output to the browser and writes browser input back to the PTY.

Fifth, the session coordinator handles lifecycle, auth token validation, single-client rules, resize events, and shutdown.

Conceptually:

```text
                 ┌──────────────┐
local stdin ───▶ │              │ ───▶ child PTY stdin
local stdout ◀── │ PTY router   │ ◀─── child PTY stdout/stderr
websocket  ───▶  │              │
websocket  ◀───  └──────────────┘
```

Internally, avoid letting multiple tasks write directly to the PTY unsafely. Use channels:

```text
LocalInputTask  ─┐
WebInputTask    ─┼─▶ input_tx ─▶ PtyWriterTask
ResizeTask      ─┘

PtyReaderTask ─▶ broadcast_tx ─▶ LocalOutputTask
                          └────▶ WebOutputTask
```

This keeps the data flow easier to reason about.

## Backend crates

Likely crate set:

```toml
[dependencies]
anyhow = "1"
clap = { version = "4", features = ["derive"] }
tokio = { version = "1", features = ["full"] }
axum = { version = "0.8", features = ["ws"] }
tower-http = { version = "0.7", features = ["fs", "trace"] }
portable-pty = "0.9"
crossterm = "0.29"
tracing = "0.1"
tracing-subscriber = "0.3"
rand = "0.10"
base64 = "0.22"
local-ip-address = "0.6"
```

Confirm these with `cargo update`/`cargo tree` during the first implementation pass. In particular, keep `axum`, `tower-http`, and the Tokio stack on mutually compatible versions rather than blindly taking latest for every crate.

I’d keep the UI assets bundled into the binary with `include_str!` / `include_bytes!` at first, rather than requiring a separate install directory. If the frontend uses a bundler, commit the source assets and make the Rust build embed the generated output deterministically.

## Frontend

Use xterm.js. It already handles terminal rendering, ANSI escapes, cursor movement, selection, scrollback, and resize. ([GitHub][2])

The current package names are under the `@xterm/*` scope. The frontend should use `@xterm/xterm` and `@xterm/addon-fit`, not the older unscoped `xterm` package name. The xterm.js docs recommend importing the CSS stylesheet explicitly and loading addons through `Terminal.loadAddon`. ([xterm.js docs][4])

The frontend needs:

```text
index.html
main.ts / main.js
@xterm/xterm
@xterm/xterm/css/xterm.css
@xterm/addon-fit
optional @xterm/addon-web-links
```

The browser opens:

```text
GET /t/{token}
```

Then connects:

```text
WS /ws/{token}
```

WebSocket messages should be binary or small JSON envelopes.

Simple protocol option, if prioritizing easiest debugging over throughput:

From server to browser:

```json
{ "type": "output", "data": "<base64 terminal bytes>" }
```

From browser to server:

```json
{ "type": "input", "data": "<base64 terminal bytes>" }
{ "type": "resize", "cols": 120, "rows": 32 }
```

Use JSON only for control/status messages and binary frames for terminal bytes from the start. Terminal I/O is already bytes, and this avoids base64 overhead in the hot path. A minimal binary protocol can still be small:

```text
0x01 + raw bytes        PTY output server -> browser
0x02 + raw bytes        input browser -> server
0x03 + JSON bytes       resize/status/control
```

## Local terminal behavior

This is the part that makes it feel “normal.”

When `rterm` starts, it puts the local terminal into raw mode. Then it forwards stdin bytes directly to the PTY. It forwards PTY bytes directly back to stdout. `crossterm` provides raw-mode helpers and terminal size/event APIs, but this area needs careful smoke testing because a failed restore leaves the user’s terminal in a bad state. ([Docs.rs][5])

It should restore the local terminal mode on exit, panic, Ctrl+C, or child termination.

It should watch local terminal resize events and resize the PTY accordingly.

When the browser attaches and resizes, there is a policy decision. I’d use:

```text
Local terminal owns PTY size while local terminal is attached.
Browser receives fit-to-current-size by default.
Browser can request resize only when --web-resize is enabled or local side is detached.
```

Otherwise, opening a phone browser could resize your local terminal to a tiny Android viewport, which would be irritating.

## Session modes

Mode 1: local primary, web secondary.

This is the main mode.

```bash
rterm --lan --write -- codex
```

Local terminal remains active. Phone can observe and optionally send keys. Local terminal size wins.

Mode 2: web primary.

```bash
rterm --headless --lan --write -- codex
```

No local terminal bridge. Useful if launched from a script or Windows shortcut. Browser controls the session.

Mode 3: view-only.

```bash
rterm --lan -- codex
```

Phone can watch but cannot type. This should be the default for LAN unless `--write` is explicitly supplied.

Mode 4: local-only with resumable browser.

```bash
rterm -- codex
```

Starts local command and binds web server only to localhost. This is safe default behavior. Later you could add a command to open LAN exposure, but that requires a control socket and is a v2 feature.

## Security model

Default security should be conservative.

Defaults:

```text
Bind 127.0.0.1 only.
Generate a random five-word token every run.
Write access disabled for browser unless --write.
Only one browser client allowed.
No cloud service.
No tunneling.
Persist the auth token only in the per-user active-session registry while the
session is alive.
Refuse elevated/root execution unless --allow-elevated is supplied.
```

`--lan` should be an explicit opt-in.

When `--lan` is used, `rterm` should print a warning like:

```text
LAN mode exposes this terminal session to devices on your local network.
Use only on trusted networks.
URL token grants access.
```

For stronger security, add:

```bash
rterm --lan --write --password -- codex
```

But for a toy, a random high-entropy URL token plus LAN-only bind is probably enough for convenience. Password auth can come after the basic flow works.

Useful flags:

```text
--bind 127.0.0.1:7843
--lan
--token <manual-token>
--write
--max-clients 1
--backspace \x7f
--once
--idle-timeout 15m
--exit-on-disconnect
--no-clipboard
```

Later:

```text
--tls-cert
--tls-key
--basic-auth user:pass
```

I would not make TLS mandatory for v0 because cert handling is annoying on phones. But make it possible later.

## WSL and Windows LAN exposure

If running inside WSL, `--lan` may not be enough because WSL2 usually uses NAT by default. Newer WSL mirrored networking can allow LAN access more directly, but it depends on Windows/WSL version and firewall configuration. The tool should detect WSL and print guidance rather than attempting privileged Windows changes automatically. ([Microsoft Learn][6])

Example:

```text
Detected WSL2.
This service is listening inside WSL at 127.0.0.1:7843.
Windows can likely reach it at http://localhost:7843/t/<token>.
For phone access, use WSL mirrored networking when available, or expose the WSL port from Windows using portproxy/firewall rules.
```

Optional helper command:

```bash
rterm windows-portproxy --port 7843
```

But I would be careful here. Creating Windows firewall rules from a WSL tool is messy and sensitive. Better v0 behavior: print the commands.

Example generated output:

```powershell
netsh interface portproxy add v4tov4 `
  listenaddress=0.0.0.0 `
  listenport=7843 `
  connectaddress=<WSL_IP_FROM_wsl_hostname_-I> `
  connectport=7843

New-NetFirewallRule `
  -DisplayName "rterm 7843" `
  -Direction Inbound `
  -Action Allow `
  -Protocol TCP `
  -LocalPort 7843 `
  -Profile Private
```

## Input behavior

Browser input should not try to synthesize OS-level keyboard events. It should send terminal bytes to the PTY.

That is a major advantage over window capture tools. When the user presses `Ctrl+C`, the browser sends the control byte. When the user types text, it sends UTF-8. When the user presses arrows, xterm.js sends terminal escape sequences.

`Ctrl+Backspace` should work as word erase because that is common Windows muscle memory. Locally, `rterm` should preserve whatever bytes the user’s terminal sends rather than trying to reinterpret the key. In the browser, add a `Ctrl+Backspace` key strip button and keyboard handling that sends the configured word-erase sequence. Default to `\x17` (`Ctrl+W`) because it is widely understood by shells/readline-style prompts as backward-kill-word; allow this to become configurable if Windows shells or specific agents need a different sequence.

For the mobile key strip, buttons can write known byte sequences:

```text
Esc      \x1b
Tab      \t
Enter    \r
Ctrl+C   \x03
Ctrl+D   \x04
Ctrl+⌫   \x17
Up       \x1b[A
Down     \x1b[B
Right    \x1b[C
Left     \x1b[D
```

The browser should support paste by sending pasted text as input bytes. It should probably ask for confirmation before pasting large blocks.

## Output behavior

PTY output should be broadcast to all viewers. Since the default max clients is one, this is simple.

The local terminal should always receive PTY output unless running in headless mode.

The browser should receive output from the point it connects. For v1, add a scrollback ring buffer so late connections see the recent session.

Suggested scrollback design:

```text
Keep last N bytes of PTY output, default 1–5 MB.
On browser connect, replay scrollback to xterm.js.
Then stream live output.
```

This makes phone pickup much better. You don’t just see a blank terminal waiting for new output.

## Process lifecycle

When the child exits, `rterm` exits with the child’s exit code.

When local user presses `Ctrl+C`, it should pass through to the child, not kill `rterm`.

Use a separate emergency kill chord, for example:

```text
Ctrl+Shift+]
```

Or simpler for v0: let normal shell/job control happen and rely on closing the terminal.

When browser disconnects:

```text
default: session continues
--once: web server stops accepting new clients
--exit-on-disconnect: terminate child process
```

For your use case, I’d default to session continuing. Phones sleep and disconnect constantly.

## Example commands

Start local Codex with phone watch/control available only on localhost:

```bash
rterm --write -- codex
```

Start local Codex and expose to LAN:

```bash
rterm --lan --write -- codex
```

Start read-only LAN watch:

```bash
rterm --lan -- codex
```

Start shell instead of direct agent:

```bash
rterm --lan --write -- bash
```

Then inside it:

```bash
cd ~/repo
codex
```

Headless mode from a script:

```bash
rterm --headless --lan --write --once -- codex
```

## Suggested module layout

```text
src/
  main.rs
  cli.rs

  session/
    mod.rs
    manager.rs
    pty.rs
    router.rs
    scrollback.rs

  local/
    mod.rs
    raw_terminal.rs
    stdin.rs
    stdout.rs
    resize.rs

  web/
    mod.rs
    server.rs
    routes.rs
    websocket.rs
    protocol.rs
    assets.rs

  security/
    mod.rs
    token.rs
    auth.rs
    bind.rs

  platform/
    mod.rs
    wsl.rs
    lan_ip.rs
```

Core structs:

```rust
struct SessionConfig {
    command: Vec<String>,
    bind_addr: SocketAddr,
    web_write: bool,
    max_clients: usize,
    once: bool,
    headless: bool,
    token: String,
}

struct TerminalSession {
    id: SessionId,
    input_tx: mpsc::Sender<PtyInput>,
    output_tx: broadcast::Sender<Vec<u8>>,
    resize_tx: mpsc::Sender<PtyResize>,
    scrollback: Scrollback,
}
```

Protocol:

```rust
enum ClientMessage {
    Input(Vec<u8>),
    Resize { cols: u16, rows: u16 },
    Ping,
}

enum ServerMessage {
    Output(Vec<u8>),
    Status(SessionStatus),
    Error(String),
}
```

## MVP implementation plan

Version 0.1 should avoid trying to be perfect.

Implement:

```text
rterm -- command...
local stdin/stdout bridge
PTY child process
raw terminal mode
terminal resize from local terminal
axum HTTP server
xterm.js static page
WebSocket terminal output
browser input when --write is set
random five-word URL token
single browser client
active-session credential lookup
elevated/root startup refusal with explicit bypass
Windows arm64/x64 release validation
Linux arm64/x64 release validation
```

v0 acceptance criteria:

```text
rterm -- <command> runs the child command with normal local terminal behavior.
Ctrl+C, Ctrl+D, Ctrl+Backspace/word-erase, arrows, paste, resize, colors, and alternate screen pass through.
Local terminal mode is restored after child exit, wrapper shutdown, and error paths covered by tests.
--lan exposes a tokenized read-only browser view by default.
--lan --write allows browser input to the same PTY.
Browser input is rejected unless write mode is enabled.
Only one browser client is accepted by default.
Invalid or missing tokens cannot access the terminal page or WebSocket.
Child exit code becomes the rterm exit code.
`rterm sessions` lists credentials for live sessions owned by the current user.
Elevated/root session startup fails unless `--allow-elevated` is supplied.
Smoke validation passes on Windows arm64/x64 and Linux arm64/x64.
```

Skip initially:

```text
TLS
password auth
multi-client collaboration
persistent sessions
Windows service mode
fancy config files
file browser
clipboard sync
screen recording
window capture
```

Version 0.2:

```text
scrollback replay
mobile key strip
large paste confirmation
better WSL detection/help
--once
--idle-timeout
LAN IP display
```

Version 0.3:

```text
basic auth
TLS
config file
headless mode
session names
detach local side while child continues
```

Careful: that last one starts creeping toward terminal-multiplexer territory. You may want “detach local side” eventually, but I would not put it in v0.

## The main tradeoff

This solves your dislike of tmux by making the session host invisible:

```bash
rterm -- codex
```

But it cannot solve this case:

```text
I already started codex normally in Windows Terminal.
Now attach my phone to that exact process.
```

For that, something must have owned the PTY from the start. `rterm` becomes that owner. The win is that you do not have to learn a multiplexer; you just prefix the command.

That is a reasonable tool. It is small enough to build, useful enough to justify, and it fits your “single portable CLI” preference pretty well.

[1]: https://docs.rs/portable-pty/latest/portable_pty/ "portable_pty - Rust"
[2]: https://github.com/xtermjs/xterm.js/ "xtermjs/xterm.js: A terminal for the web"
[3]: https://docs.rs/axum/latest/axum/extract/ws/ "axum WebSocket extractor docs"
[4]: https://xtermjs.org/docs/guides/import/ "xterm.js importing guide"
[5]: https://docs.rs/crossterm/latest/crossterm/terminal/ "crossterm terminal docs"
[6]: https://learn.microsoft.com/en-us/windows/wsl/networking "Accessing network applications with WSL"
