# WebSocket Protocol

rterm uses a single WebSocket endpoint at `/ws/{token}` for terminal I/O. Messages are binary frames with a one-byte type prefix followed by a payload.

## Frame Types

| Type | Byte | Payload | Direction | Description |
|------|------|---------|-----------|-------------|
| Output | `0x01` | Raw bytes | Server → Client | Terminal output from the PTY |
| Input | `0x02` | Raw bytes | Client → Server | Keyboard/input to the PTY |
| Control | `0x03` | JSON bytes | Bidirectional | Status, resize, errors, ping/pong |

## Wire Format

Each binary WebSocket message has this structure:

```
[1 byte: type] [N bytes: payload]
```

- Type `0x01`: Payload is raw terminal output bytes. Written directly to the xterm.js terminal.
- Type `0x02`: Payload is raw input bytes. Forwarded to the PTY writer (if write mode is enabled).
- Type `0x03`: Payload is a JSON-encoded control message (see below).

## Control Messages

Control messages use JSON with a `type` discriminator field (`serde` tagged enum, `rename_all = "snake_case"`).

### Server → Client

#### Status

Sent on WebSocket connect. Informs the client of its capabilities.

```json
{
  "type": "status",
  "writable": true,
  "backspace": [127],
  "word_erase": [23]
}
```

| Field | Type | Description |
|-------|------|-------------|
| `writable` | `bool` | Whether this client can write input to the PTY |
| `backspace` | `[u8]` | Byte sequence for Backspace (default `[127]` = DEL) |
| `word_erase` | `[u8]` | Byte sequence for Ctrl+Backspace (default `[23]` = `\x17`) |

#### Error

Sent when the client performs an invalid action.

```json
{
  "type": "error",
  "message": "browser input is disabled; restart with --write"
}
```

| Field | Type | Description |
|-------|------|-------------|
| `message` | `string` | Human-readable error description |

#### Pong

Response to a client Ping.

```json
{
  "type": "pong"
}
```

### Client → Server

#### Resize

Requests a PTY resize. Only honored when `browser_resize` is enabled (headless mode or non-interactive terminal).

```json
{
  "type": "resize",
  "cols": 80,
  "rows": 24
}
```

| Field | Type | Description |
|-------|------|-------------|
| `cols` | `u16` | Terminal width in columns |
| `rows` | `u16` | Terminal height in rows |

#### Ping

Keepalive / latency check.

```json
{
  "type": "ping"
}
```

## Connection Lifecycle

```
Client                          Server
  │                                │
  ├── GET /ws/{token} ────────────►│
  │                                ├── Validate token
  │                                ├── Acquire ClientPermit
  │                                │
  │◄── 101 Switching Protocols ────┤
  │◄── Control: status ────────────┤  (writable mode, Backspace, word-erase)
  │◄── Output: scrollback replay ──┤  (recent terminal output)
  │                                │
  │◄══ Output frames (streaming) ══┤  (live PTY output)
  │══► Input frames ──────────────►│  (keyboard input, if writable)
  │══► Control: resize ───────────►│  (if browser_resize enabled)
  │══► Control: ping ─────────────►│
  │◄══ Control: pong ──────────────┤
  │                                │
  │──► Close / disconnect ────────►│
  │                                ├── Release ClientPermit
  │                                ├── If --once: close to new clients
  │                                │
```

## Error Handling

- Invalid frame type: Connection is dropped (server logs the error)
- Malformed JSON in control frames: Connection is dropped
- Text WebSocket messages: Server sends error control frame `"text websocket messages are not supported"`
- Input when `--write` is disabled: Server sends error control frame

## Binary vs Text

All terminal I/O uses binary frames for efficiency (no base64 overhead). Only control messages use JSON text (encoded as binary frame payload). Text WebSocket messages (`Message::Text`) are explicitly rejected.

## Implementation Notes

- PTY output is broadcast to all connected clients via `tokio::sync::broadcast`
- If a consumer lags, `RecvError::Lagged` is silently ignored (frames are skipped)
- The scrollback buffer replays recent output to new connections so they see session history
- Scrollback capacity: 1 MB (1,048,576 bytes) by default
