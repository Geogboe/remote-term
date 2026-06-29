# Security Model

## Threat Model

rterm exposes a terminal session over HTTP/WebSocket. The primary threats are:

1. Unauthorized access to the terminal session
2. Network eavesdropping on terminal output and input
3. Token leakage through URLs, logs, or browser history
4. Unintended LAN exposure

## Default Security Posture

rterm is conservative by default:

| Control | Default | Rationale |
|---------|---------|-----------|
| Bind address | `127.0.0.1` | Local only; no network exposure |
| WebSocket write | Disabled | Read-only observation by default |
| Token | Five random EFF long-list words | Human-readable, per-session |
| Max clients | 1 | Single observer |
| LAN exposure | Explicit opt-in (`--lan`) | Never accidentally exposed |
| TLS | Not implemented | v0 scope limitation |

## Token Authentication

### Generation

Tokens contain five independently selected words from EFF's 7,776-word long
Diceware list. This provides approximately 64.6 bits of entropy. Selection uses
the application's cryptographic random-number generator. See
`docs/adr/0001-human-readable-session-credentials.md`.

### Validation

Tokens are validated using exact string comparison (`candidate == expected`). Empty strings are rejected. The comparison is not constant-time, but timing side-channels are not a practical concern for local network use.

### Usage

The token appears in:
- The terminal page URL: `/t/{token}`
- The WebSocket URL: `/ws/{token}`
- The injected `window.RTERM_TOKEN` in the HTML page
- URLs printed to stderr at startup

### Lifecycle

Tokens are:
- Generated per-run (never reused)
- Printed to stderr at startup
- Stored in the current user's active-session registry while the session lives
- Removed from the registry on normal shutdown
- Pruned when stale or corrupt registry entries are encountered

On Unix, rterm creates the registry directory with mode `0700` and registry
files with mode `0600`. On Windows, the registry inherits the current user's
Local AppData ACL. Administrators and processes already running as the same
user remain inside the trust boundary.

## Elevated Execution

rterm refuses to start a session as Unix root or from an elevated Windows
process unless `--allow-elevated` is supplied. The Windows check reads the
current process token's `TokenElevation` value; Unix checks the effective user
ID.

The guard limits accidental exposure of an elevated shell. It does not reduce
the privileges of the child when bypassed.

## Prompt Integration

The optional Starship integration exports only a numeric session ID and
enumerated mode values. Browser tokens and tokenized URLs are not placed in the
prompt environment. `rterm starship` validates inherited values before printing
them to prevent prompt control-sequence injection.

## LAN Exposure

`--lan` changes the bind address from loopback to all interfaces. This is an explicit opt-in with warnings:

```
LAN mode exposes this terminal URL to devices on your local network.
Use only on trusted networks.
```

### WSL Considerations

When running in WSL with `--lan`:
- WSL2 typically uses NAT networking; the service may not be reachable from LAN
- rterm detects WSL and prints guidance (Windows localhost access and portproxy instructions)
- rterm **never** modifies Windows firewall or portproxy rules automatically

### Network Trust

LAN mode assumes the local network is trusted. On untrusted networks (public WiFi, hotel, coffee shop), anyone on the same network with the URL token can access the terminal. Use `--write` only on trusted networks.

## Input Validation

### WebSocket Frames

- Unknown frame types cause the connection to be dropped
- Malformed JSON in control frames causes the connection to be dropped
- Text WebSocket messages are rejected with an error
- Input frames are rejected when `--write` is not enabled

### URL Paths

- Token paths are validated against the session token
- Invalid tokens return `404` (terminal page) or `401` (WebSocket)
- Static asset paths (`/assets/*`) are fixed and do not traverse the filesystem

### PTY Input

- All input bytes (local and browser) are sent to the PTY as-is
- No filtering, sanitization, or escaping is applied to terminal input
- The PTY's behavior depends entirely on the child process running inside it

## Limited Persistent State

rterm persists only live discovery metadata:
- Per-user active-session records contain the browser URLs and are removed on
  normal shutdown
- Crashes can leave stale records, which `rterm sessions` probes and removes
- Temporary exit code files remain under the system temporary directory
- No auth database or config
- No log files by default (tracing goes to stderr)
- No browser local storage or cookies

## Known Limitations (v0)

| Limitation | Risk | Mitigation |
|------------|------|------------|
| No TLS | Eavesdropping on LAN | Use on trusted networks only |
| No password auth | Token-only access | Token is high-entropy; not in URL bar after initial load (SPA) |
| No rate limiting | Brute-force token guessing | ~64.6-bit session credential remains impractical to guess online |
| No CORS headers | Cross-origin access | Single-origin SPA; no CORS needed |
| WebSocket origin not checked | Cross-origin WebSocket | Token in path provides equivalent protection |
| No input sanitization | Terminal escape injection | Inherent to terminal use; child process must be trusted |

## Future Security (v0.3+)

Planned security enhancements:
- `--tls-cert` / `--tls-key` for HTTPS/WSS
- `--basic-auth user:pass` for additional authentication
- Rate limiting on token validation endpoints
- Config file for persistent preferences (no secrets in config)
