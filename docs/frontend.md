# Frontend Architecture

The rterm browser UI is a single-page application built with xterm.js, bundled with esbuild, and embedded into the Rust binary at compile time.

## Source Layout

```
web/
├── package.json          Node dependencies and build script
├── package-lock.json     Lock file
├── build.mjs             esbuild build script
└── src/
    ├── index.html        HTML shell (token placeholder)
    ├── main.js           Application entry point
    └── style.css         Custom app styles

src/web/static/           (build output — committed to repo)
├── index.html            Copied from web/src/index.html
├── main.js               Bundled, minified xterm.js app
└── style.css             Concatenated xterm.css + app styles
```

## Build Pipeline

The build script (`web/build.mjs`):

1. Bundles `web/src/main.js` with esbuild (IIFE format, minified, no sourcemaps)
2. Concatenates `@xterm/xterm/css/xterm.css` + `web/src/style.css` into `style.css`
3. Copies `web/src/index.html` to the output directory

Output goes to `src/web/static/`, which is embedded at compile time by `include_str!`.

## Dependencies

| Package | Version | Purpose |
|---------|---------|---------|
| `@xterm/xterm` | ^5.5.0 | Terminal emulator (rendering, ANSI, selection) |
| `@xterm/addon-fit` | ^0.11.0 | Auto-fit terminal to container |
| `esbuild` | ^0.25.0 | JavaScript bundling (dev dependency) |

## Application Structure

### HTML (`index.html`)

Minimal structure:

```html
<main id="app">
  <div id="terminal"></div>      <!-- xterm.js container -->
  <div id="keys"></div>          <!-- Mobile key strip -->
</main>
<script>window.RTERM_TOKEN = "__TOKEN__";</script>
<script src="/assets/main.js"></script>
```

The `__TOKEN__` placeholder is replaced server-side with the actual session token before the HTML is served.

### JavaScript (`main.js`)

The application performs these steps on load:

1. **Token extraction**: Reads `window.RTERM_TOKEN` (or falls back to path parsing)
2. **Terminal initialization**: Creates xterm.js `Terminal` with a stable nonblinking cursor, dark terminal theme, and `FitAddon`
3. **WebSocket connection**: Connects to `ws://host/ws/{token}` with binary mode
4. **Status handshake**: Receives initial `status` control frame (writable mode, word-erase sequence)
5. **Scrollback replay**: Receives buffered output frames to populate the terminal
6. **Live streaming**: Enters the message loop for real-time terminal I/O
7. **Status rail**: Shows connection and read-only/writable state
8. **Key deck**: Creates on-screen buttons for mobile terminal keys

#### Terminal Configuration

```javascript
const term = new Terminal({
  cursorBlink: false,
  cursorStyle: "bar",
  convertEol: true,
  fontFamily: '"Cascadia Mono", "SFMono-Regular", Consolas, monospace',
  fontSize: 14,
  theme: {
    background: "#0b0e12",
    foreground: "#dce5ef",
    cursor: "#72a7ff",
    selectionBackground: "#304665"
  }
});
```

#### WebSocket Protocol Handling

- `0x01` (Output): Writes raw bytes to the terminal via `term.write()`
- `0x02` (Input): Sends keyboard bytes to the server (if writable)
- `0x03` (Control): Parses JSON for status updates or error messages

#### Mobile Key Strip

Eleven buttons for mobile-friendly terminal control:

| Button | Bytes Sent |
|--------|------------|
| Esc | `\x1b` |
| Tab | `\t` |
| ⌫ | `\x7f` |
| Ctrl+C | `\x03` |
| Ctrl+D | `\x04` |
| Ctrl+⌫ | Configured word-erase (default `\x17`) |
| Enter | `\r` |
| ↑ | `\x1b[A` |
| ↓ | `\x1b[B` |
| ← | `\x1b[D` |
| → | `\x1b[C` |

The Ctrl+Backspace button sends the `word_erase` sequence from the server status frame, allowing customization via `--word-erase`.

#### Keyboard Handling

- General keyboard input is forwarded to the server via `term.onData()`
- Backspace is intercepted before xterm's default handler and sends one DEL byte
- `Ctrl+Backspace` sends exactly one configured word-erase sequence
- Keyup is consumed without sending a second erase sequence
- Alt/Meta-modified Backspace remains under xterm control
- Resize frames are sent only while the socket is open

### CSS (`style.css`)

Combined stylesheet:
- **xterm.css**: Standard xterm.js terminal styling (from `@xterm/xterm/css/xterm.css`)
- **App styles**: Terminal-native dark theme, connection rail, responsive key deck, safe-area padding, and visible keyboard focus

The page uses a three-row CSS grid:
```css
#app {
  height: 100svh;
  display: grid;
  grid-template-rows: auto minmax(0, 1fr) auto;
}
```

The status rail stays compact, the terminal fills available space, and the key
deck remains reachable at the bottom. Controls stay disabled until a writable
session is authorized.

## Server-Side Asset Serving

Assets are embedded at compile time and served from memory:

```rust
// src/web/assets.rs
pub const INDEX_HTML: &str = include_str!("static/index.html");
pub const MAIN_JS: &str = include_str!("static/main.js");
pub const STYLE_CSS: &str = include_str!("static/style.css");
```

The server routes:

| Route | Handler | Content-Type |
|-------|---------|-------------|
| `/t/{token}` | `terminal_page()` | `text/html` |
| `/assets/main.js` | `main_js()` | `text/javascript` |
| `/assets/style.css` | `style_css()` | `text/css` |

No filesystem access is needed at runtime — the binary is self-contained.
