# CLI Reference

## Usage

```
rterm [OPTIONS] -- <COMMAND> [ARGS...]
```

All options must appear before `--`. Everything after `--` is the child command.

Management commands do not require a child command:

```text
rterm sessions [--json]
```

## Options

### `--bind <ADDR>`

Bind address for the embedded web server. Default: `127.0.0.1:7843`.

```bash
rterm --bind 0.0.0.0:9000 -- bash
rterm --bind 127.0.0.1:3000 -- codex
```

### `--lan`

Expose the web server on all network interfaces (`0.0.0.0`). Without this flag, binding is always to the address specified by `--bind` (defaults to localhost).

When `--lan` is set, rterm prints a LAN URL for phone access and a security warning. On WSL, it prints additional networking guidance.

```bash
rterm --lan -- bash
rterm --lan --bind 0.0.0.0:9000 -- codex
```

If `--lan` is set but `--bind` is loopback, the effective bind becomes `0.0.0.0:<port>`.

### `--write`

Allow browser clients to write input to the PTY. Without this flag, the browser is read-only.

```bash
rterm --lan --write -- codex
```

When a browser sends input without `--write`, the server responds with an error: `"browser input is disabled; restart with --write"`.

### `--max-clients <N>`

Maximum number of concurrent browser clients. Default: `1`.

```bash
rterm --max-clients 3 -- codex
```

When the limit is reached, new WebSocket connections receive HTTP `429 Too Many Requests`.

### `--once`

Stop accepting new browser clients after the first disconnect. Useful for single-session phone access.

```bash
rterm --lan --write --once -- codex
```

When the browser disconnects, `SessionState::closed_to_new_clients` is set to true and subsequent connections are rejected (even if `--max-clients` would allow them).

### `--headless`

Do not attach the local terminal to the PTY. The session is web-only.

```bash
rterm --headless --lan --write -- codex
```

In headless mode:
- No raw mode is entered on the local terminal
- No local stdin/stdout bridge is created
- Browser can resize the PTY (since no local terminal owns sizing)
- Terminal responses (e.g., cursor position queries) are synthesized

### `--token <TOKEN>`

Manually supply the browser URL token instead of auto-generating one.

```bash
rterm --token my-session -- codex
```

The URL becomes `http://127.0.0.1:7843/t/my-session`. Explicit tokens must use
URL-safe ASCII letters, digits, `-`, `_`, `.`, or `~`. Auto-generated tokens
contain five random words from EFF's long Diceware list. Explicit tokens are
limited to 256 bytes.

### `--allow-elevated`

Allow a terminal session to start as root or from an elevated Windows process.
rterm refuses this by default because the browser controls the child at the
same privilege level.

```powershell
rterm --allow-elevated -- pwsh
```

This guard applies to session startup. Read-only management commands such as
`rterm sessions` remain available.

### `--word-erase <SEQUENCE>`

Byte sequence sent when the browser user presses `Ctrl+Backspace`. Default: `\x17` (Ctrl+W).

Supports escape sequences:
- `\n` → newline
- `\r` → carriage return
- `\t` → tab
- `\\` → backslash
- `\xNN` → hex byte (e.g., `\x1b` for ESC)

```bash
# Standard Ctrl+W (default)
rterm -- bash

# ESC + DEL (common in some terminals)
rterm --word-erase '\x1b\x7f' -- bash
```

### `--backspace <SEQUENCE>`

Byte sequence sent when the browser user presses Backspace. The default is
`\x7f` (DEL), matching the documented VT input sequence used by ConPTY and Unix
terminals.

```bash
rterm --backspace '\x08' -- bash
```

This option supports the same escape syntax as `--word-erase`.
Malformed, empty, or sequences longer than 32 encoded bytes are rejected
instead of silently disabling the key.

## Session Modes

### Local Primary, Web Secondary (Default)

```bash
rterm --write -- codex
```

Local terminal active. Browser can observe (and write if `--write`). Local terminal owns PTY size.

### LAN Exposed

```bash
rterm --lan --write -- codex
```

Same as above but available on all network interfaces.

### Read-Only Web

```bash
rterm --lan -- codex
```

Browser can watch but cannot type. Default mode when `--write` is not supplied.

### Headless / Web-Only

```bash
rterm --headless --lan --write -- codex
```

No local terminal. Browser is the only controller.

### Write + Once

```bash
rterm --lan --write --once -- codex
```

Single browser client; web server stops accepting new clients after disconnect.

## Management Commands

### `rterm sessions`

List live sessions owned by the current OS user, including their browser URLs:

```text
4260  pid=4260  writable  codex
  Local URL: http://127.0.0.1:7843/t/harbor-lime-orbit-cabin-velvet
  LAN URL:   http://192.168.1.50:7843/t/harbor-lime-orbit-cabin-velvet
```

Use `--json` for automation:

```powershell
rterm sessions --json
```

Stale registry entries are discarded when sessions are listed.

## Exit Codes

rterm exits with the child process's exit code. If rterm itself fails (e.g., bind failure, PTY error), it exits with code `1`. If no exit code can be determined, it defaults to `1`.

## Environment Variables

| Variable | Purpose |
|----------|---------|
| `RTERM_EXIT_FILE` | Set internally for child helper; writes exit code to this file |
| `RUST_LOG` | Controls tracing verbosity (env-filter format). Default: `warn` |

Tracing levels: `error`, `warn`, `info`, `debug`, `trace`. Example: `RUST_LOG=rterm=debug cargo run -- bash`.

## Examples

```bash
# Basic: run bash with local web access
rterm -- bash

# Agent session with LAN phone access
rterm --lan --write -- codex

# Specify port and max clients
rterm --bind 0.0.0.0:9000 --max-clients 2 -- codex

# Headless daemon-like session
rterm --headless --lan --write --once -- bash

# Custom word-erase for terminal compatibility
rterm --word-erase '\x1b\x7f' -- bash

# PowerShell with custom config
rterm --lan --write -- pwsh -NoProfile
```
