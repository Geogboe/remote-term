import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";
import { overrideBytesForKey } from "./input.js";

const token = window.RTERM_TOKEN || location.pathname.split("/").filter(Boolean).pop();
const terminalEl = document.getElementById("terminal");
const keys = document.getElementById("keys");
const connectionStatus = document.getElementById("connection-status");
const connectionDot = document.getElementById("connection-dot");
const writeMode = document.getElementById("write-mode");
const encoder = new TextEncoder();
const decoder = new TextDecoder();
let writable = false;
let backspace = new Uint8Array([0x7f]);
let wordErase = new Uint8Array([0x17]);
let connected = false;

const term = new Terminal({
  cursorBlink: false,
  cursorStyle: "bar",
  cursorInactiveStyle: "outline",
  convertEol: true,
  fontFamily: '"Cascadia Mono", "SFMono-Regular", Consolas, monospace',
  fontSize: 14,
  lineHeight: 1.16,
  theme: {
    background: "#0b0e12",
    foreground: "#dce5ef",
    cursor: "#72a7ff",
    cursorAccent: "#0b0e12",
    selectionBackground: "#304665"
  }
});
const fit = new FitAddon();
term.loadAddon(fit);
term.open(terminalEl);
fit.fit();

const socket = new WebSocket(`${location.protocol === "https:" ? "wss" : "ws"}://${location.host}/ws/${token}`);
socket.binaryType = "arraybuffer";

function frame(kind, bytes) {
  const payload = bytes instanceof Uint8Array ? bytes : encoder.encode(bytes);
  const out = new Uint8Array(payload.length + 1);
  out[0] = kind;
  out.set(payload, 1);
  return out;
}

function sendBytes(bytes) {
  if (!writable || !connected || socket.readyState !== WebSocket.OPEN) return;
  socket.send(frame(0x02, bytes));
}

function resize() {
  fit.fit();
  if (socket.readyState !== WebSocket.OPEN) return;
  const payload = encoder.encode(JSON.stringify({ type: "resize", cols: term.cols, rows: term.rows }));
  socket.send(frame(0x03, payload));
}

function renderState(message, state) {
  connectionStatus.textContent = message;
  connectionDot.dataset.state = state;
  writeMode.textContent = writable ? "Writable" : "Read-only";
  for (const button of keys.querySelectorAll("button")) {
    button.disabled = !connected || !writable;
  }
}

socket.addEventListener("open", () => {
  connectionStatus.textContent = "Authorizing";
  connectionDot.dataset.state = "connecting";
  resize();
});
socket.addEventListener("message", (event) => {
  const bytes = new Uint8Array(event.data);
  if (bytes.length === 0) return;
  const kind = bytes[0];
  const payload = bytes.slice(1);
  if (kind === 0x01) {
    term.write(payload);
  } else if (kind === 0x03) {
    const control = JSON.parse(decoder.decode(payload));
    if (control.type === "status") {
      writable = control.writable;
      backspace = new Uint8Array(control.backspace || [0x7f]);
      wordErase = new Uint8Array(control.word_erase || [0x17]);
      connected = true;
      term.options.disableStdin = !writable;
      renderState("Connected", "connected");
      if (writable) term.focus();
    } else if (control.type === "error") {
      term.writeln(`\r\n[rterm] ${control.message}`);
    }
  }
});
socket.addEventListener("close", () => {
  connected = false;
  renderState("Disconnected", "disconnected");
});
socket.addEventListener("error", () => {
  connected = false;
  renderState("Connection error", "error");
});

term.onData((data) => sendBytes(encoder.encode(data)));
term.attachCustomKeyEventHandler((event) => {
  const bytes = overrideBytesForKey(event, backspace, wordErase);
  if (bytes === undefined) return true;
  if (bytes.length > 0) sendBytes(bytes);
  return false;
});
window.addEventListener("resize", resize);

const buttons = [
  ["Esc", "\x1b"], ["Tab", "\t"], ["⌫", () => backspace],
  ["Ctrl+C", "\x03"], ["Ctrl+D", "\x04"], ["Ctrl+⌫", () => wordErase],
  ["Enter", "\r"], ["↑", "\x1b[A"], ["↓", "\x1b[B"],
  ["←", "\x1b[D"], ["→", "\x1b[C"]
];

for (const [label, value] of buttons) {
  const button = document.createElement("button");
  button.type = "button";
  button.textContent = label;
  button.disabled = true;
  button.addEventListener("click", () => {
    const bytes = typeof value === "function"
      ? value()
      : value instanceof Uint8Array
        ? value
        : encoder.encode(value);
    sendBytes(bytes);
    term.focus();
  });
  keys.appendChild(button);
}

renderState("Connecting", "connecting");
