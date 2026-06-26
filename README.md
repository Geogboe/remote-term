# remote-term

remote-term runs a command inside a managed PTY and exposes the same terminal
session to a tokenized browser view. The CLI executable is `rterm`.

Primary workflow:

```powershell
rterm --lan --write -- codex
```

The local terminal remains the primary controller. The browser can observe by
default and can write only when `--write` is supplied.

## Installation

Linux/macOS:

```sh
curl -fsSL https://raw.githubusercontent.com/Geogboe/rterm/main/install.sh | sh
```

Windows PowerShell:

```powershell
irm https://raw.githubusercontent.com/Geogboe/rterm/main/install.ps1 | iex
```

Manual downloads are available from the latest GitHub release:

```text
https://github.com/Geogboe/rterm/releases
```

## Current Commands

```powershell
cargo test
cargo fmt --all
cargo clippy --all-targets --all-features -- -D warnings
```

Build embedded browser assets:

```powershell
cd web
npm install
npm run build
```

Run a local smoke command:

```powershell
cargo run -- -- pwsh -NoProfile -Command "Write-Output rterm-smoke; exit 7"
```

Expected result: `rterm-smoke` is printed and the wrapper exits with the child
exit code.

## Browser URLs

Each run generates a token:

```text
http://127.0.0.1:7843/t/<token>
```

Use `--lan` to bind on all interfaces and print a LAN URL. Use `--write` only on
trusted networks.
