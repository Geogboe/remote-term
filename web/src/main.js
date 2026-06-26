import { Terminal } from "@xterm/xterm";
import { FitAddon } from "@xterm/addon-fit";

const token = window.RTERM_TOKEN || location.pathname.split("/").filter(Boolean).pop();
const terminalEl = document.getElementById("terminal");
const keys = document.getElementById("keys");
const encoder = new TextEncoder();
const decoder = new TextDecoder();
let writable = false;
let wordErase = new Uint8Array([0x17]);

const term = new Terminal({
  cursorBlink: true,
  convertEol: true,
  fontFamily: '"Cascadia Mono", "SFMono-Regular", Consolas, monospace',
  fontSize: 14,
  theme: {
    background: "#101214",
    foreground: "#f2f4f8",
    cursor: "#f2f4f8",
    selectionBackground: "#42526b"
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
  if (!writable || socket.readyState !== WebSocket.OPEN) return;
  socket.send(frame(0x02, bytes));
}

function resize() {
  fit.fit();
  const payload = encoder.encode(JSON.stringify({ type: "resize", cols: term.cols, rows: term.rows }));
  socket.send(frame(0x03, payload));
}

socket.addEventListener("open", resize);
socket.addEventListener("message", (event) => {
  const bytes = new Uint8Array(event.data);
  const kind = bytes[0];
  const payload = bytes.slice(1);
  if (kind === 0x01) {
    term.write(payload);
  } else if (kind === 0x03) {
    const control = JSON.parse(decoder.decode(payload));
    if (control.type === "status") {
      writable = control.writable;
      wordErase = new Uint8Array(control.word_erase || [0x17]);
    } else if (control.type === "error") {
      term.writeln(`\r\n[rterm] ${control.message}`);
    }
  }
});

term.onData((data) => sendBytes(encoder.encode(data)));
window.addEventListener("resize", resize);
window.addEventListener("keydown", (event) => {
  if (event.ctrlKey && event.key === "Backspace") {
    event.preventDefault();
    sendBytes(wordErase);
  }
});

const buttons = [
  ["Esc", "\x1b"], ["Tab", "\t"], ["Ctrl+C", "\x03"], ["Ctrl+D", "\x04"],
  ["Ctrl+⌫", () => wordErase], ["Enter", "\r"], ["↑", "\x1b[A"], ["↓", "\x1b[B"],
  ["←", "\x1b[D"], ["→", "\x1b[C"]
];

for (const [label, value] of buttons) {
  const button = document.createElement("button");
  button.type = "button";
  button.textContent = label;
  button.addEventListener("click", () => {
    const bytes = typeof value === "function" ? value() : encoder.encode(value);
    sendBytes(bytes);
    term.focus();
  });
  keys.appendChild(button);
}
