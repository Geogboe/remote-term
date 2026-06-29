import { spawn } from "node:child_process";
import path from "node:path";
import process from "node:process";

const port = 17845;
const token = "browser-input-smoke";
const binary = path.resolve(
  "target",
  "debug",
  process.platform === "win32" ? "rterm.exe" : "rterm"
);
const child = spawn(
  binary,
  [
    "--allow-elevated",
    "--headless",
    "--write",
    "--bind",
    `127.0.0.1:${port}`,
    "--token",
    token,
    "--",
    "pwsh",
    "-NoLogo",
    "-NoProfile"
  ],
  { stdio: ["ignore", "ignore", "pipe"] }
);

let startup = "";
child.stderr.setEncoding("utf8");
child.stderr.on("data", (chunk) => {
  startup += chunk;
});

function delay(milliseconds) {
  return new Promise((resolve) => setTimeout(resolve, milliseconds));
}

async function waitFor(predicate, description, timeout = 5000) {
  const deadline = Date.now() + timeout;
  while (Date.now() < deadline) {
    if (predicate()) return;
    if (child.exitCode !== null) {
      throw new Error(`rterm exited while waiting for ${description}:\n${startup}`);
    }
    await delay(25);
  }
  throw new Error(`timed out waiting for ${description}`);
}

function inputFrame(bytes) {
  const payload = bytes instanceof Uint8Array ? bytes : new TextEncoder().encode(bytes);
  const frame = new Uint8Array(payload.length + 1);
  frame[0] = 0x02;
  frame.set(payload, 1);
  return frame;
}

function controlFrame(value) {
  const payload = new TextEncoder().encode(JSON.stringify(value));
  const frame = new Uint8Array(payload.length + 1);
  frame[0] = 0x03;
  frame.set(payload, 1);
  return frame;
}

async function typeText(socket, text) {
  for (const character of text) {
    socket.send(inputFrame(character));
    await delay(5);
  }
}

function stripAnsi(value) {
  return value
    .replace(/\x1b\][^\x07]*(?:\x07|\x1b\\)/g, "")
    .replace(/\x1b\[[0-?]*[ -/]*[@-~]/g, "");
}

let socket;
let output = "";
let status;
try {
  await waitFor(() => startup.includes(`127.0.0.1:${port}`), "server startup");

  socket = new WebSocket(`ws://127.0.0.1:${port}/ws/${token}`);
  socket.binaryType = "arraybuffer";

  const decoder = new TextDecoder();
  socket.addEventListener("message", (event) => {
    const bytes = new Uint8Array(event.data);
    if (bytes[0] === 0x01) {
      output += decoder.decode(bytes.slice(1), { stream: true });
    } else if (bytes[0] === 0x03) {
      const control = JSON.parse(decoder.decode(bytes.slice(1)));
      if (control.type === "status") status = control;
    }
  });

  await waitFor(() => status?.writable === true, "writable status");
  await waitFor(() => stripAnsi(output).includes("PS "), "initial PowerShell prompt");

  output = "";
  socket.send(inputFrame("Write-Output (Get-Location).Path\r"));
  await waitFor(
    () => stripAnsi(output).toLowerCase().includes(process.cwd().toLowerCase()),
    "inherited working directory"
  );

  output = "";
  await typeText(socket, "Write-Output 'abx");
  socket.send(inputFrame(new Uint8Array(status.backspace)));
  await typeText(socket, "c'");
  socket.send(inputFrame("\r"));
  await waitFor(
    () => /(?:^|\r?\n)abc\r?\n/.test(stripAnsi(output)),
    "single-character browser Backspace"
  );

  output = "";
  await typeText(socket, "Write-Output 'alpha beta");
  socket.send(inputFrame(new Uint8Array(status.word_erase)));
  await typeText(socket, "gamma'");
  socket.send(inputFrame("\r"));
  await waitFor(
    () => /(?:^|\r?\n)alpha gamma\r?\n/.test(stripAnsi(output)),
    "browser Ctrl+Backspace word erase"
  );

  output = "";
  socket.send(inputFrame("[Microsoft.PowerShell.PSConsoleReadLine]::ClearHistory()\r"));
  await waitFor(() => stripAnsi(output).includes("PS "), "history reset");
  output = "";
  socket.send(inputFrame("Write-Output 'arrow-pass'\r"));
  await waitFor(() => stripAnsi(output).includes("arrow-pass"), "history seed command");
  await waitFor(() => stripAnsi(output).includes("PS "), "prompt after history seed");
  output = "";
  socket.send(inputFrame("\x1b[A\r"));
  await waitFor(
    () => stripAnsi(output).includes("arrow-pass"),
    "browser Up arrow history recall"
  );

  output = "";
  socket.send(inputFrame("Start-Sleep -Seconds 30\r"));
  await delay(300);
  socket.send(inputFrame(new Uint8Array([0x03])));
  await waitFor(() => stripAnsi(output).includes("PS "), "prompt after browser Ctrl+C");
  output = "";
  socket.send(inputFrame("Write-Output 'interrupt-pass'\r"));
  await waitFor(
    () => stripAnsi(output).includes("interrupt-pass"),
    "session usability after browser Ctrl+C"
  );

  output = "";
  socket.send(controlFrame({ type: "resize", cols: 100, rows: 30 }));
  await delay(200);
  socket.send(inputFrame("Write-Output $Host.UI.RawUI.WindowSize.Width\r"));
  await waitFor(
    () => /(?:^|\r?\n)100\r?\n/.test(stripAnsi(output)),
    "browser PTY resize"
  );

  output = "";
  const pasted = "x".repeat(2048);
  socket.send(inputFrame(`$value = '${pasted}'; Write-Output $value.Length\r`));
  await waitFor(
    () => /(?:^|\r?\n)2048\r?\n/.test(stripAnsi(output)),
    "large browser paste",
    10000
  );

  socket.send(inputFrame("exit\r"));
  await waitFor(() => child.exitCode !== null, "clean child exit");
  if (child.exitCode !== 0) {
    throw new Error(`rterm returned ${child.exitCode}:\n${startup}`);
  }

  console.log("browser input smoke passed");
} catch (error) {
  console.error("terminal output at failure:");
  console.error(JSON.stringify(stripAnsi(output)));
  console.error(`status: ${JSON.stringify(status)}`);
  throw error;
} finally {
  socket?.close();
  if (child.exitCode === null) child.kill();
}
