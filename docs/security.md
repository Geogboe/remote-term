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
| Token | Random 32-char alphanumeric | High-entropy, per-session |
| Max clients | 1 | Single observer |
| LAN exposure | Explicit opt-in (`--lan`) | Never accidentally exposed |
| TLS | Not implemented | v0 scope limitation |

## Token Authentication

### Generation

Tokens are generated using `rand::distr::Alphanumeric` which produces 32 characters from `[A-Za-z0-9]`. At ~5.95 bits per character, this gives approximately 190 bits of entropy.

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
- Never persisted to disk
- Never logged (use caution with `RUST_LOG`)
- Printed to stderr at startup

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

## No Persistent State

rterm maintains no persistent state:
- No session files on disk (except temporary exit code files in `$TMP`)
- No auth database or config
- No log files by default (tracing goes to stderr)
- No browser local storage or cookies

## Known Limitations (v0)

| Limitation | Risk | Mitigation |
|------------|------|------------|
| No TLS | Eavesdropping on LAN | Use on trusted networks only |
| No password auth | Token-only access | Token is high-entropy; not in URL bar after initial load (SPA) |
| No rate limiting | Brute-force token guessing | 190-bit entropy makes brute-force impractical |
| No CORS headers | Cross-origin access | Single-origin SPA; no CORS needed |
| WebSocket origin not checked | Cross-origin WebSocket | Token in path provides equivalent protection |
| No input sanitization | Terminal escape injection | Inherent to terminal use; child process must be trusted |

## Future Security (v0.3+)

Planned security enhancements:
- `--tls-cert` / `--tls-key` for HTTPS/WSS
- `--basic-auth user:pass` for additional authentication
- Rate limiting on token validation endpoints
- Config file for persistent preferences (no secrets in config)