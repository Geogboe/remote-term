# remote-term

remote-term runs a command inside a managed PTY and exposes the same terminal
session to a tokenized browser view. The CLI executable is `rterm`.

Primary workflow:

```powershell
rterm --lan --write -- codex
```

The local terminal remains the primary controller. The browser can observe by
default and can write only when `--write` is supplied.

Generated browser credentials use five random words so the URL can be
transcribed:

```text
http://192.168.1.50:7843/t/harbor-lime-orbit-cabin-velvet
```

Recover URLs for sessions started in another terminal:

```powershell
rterm sessions
```

Show safe rterm session metadata in a
[Starship custom module](https://starship.rs/config/#custom-commands):

```toml
[custom.rterm]
command = 'rterm starship'
when = true
style = 'bold blue'
format = '([$output]($style) )'
description = 'Current rterm session'
```

The module displays values such as `rterm:4260 lan/rw/shared` without exposing
the browser credential. A ready-to-copy configuration is available at
[`docs/examples/starship.toml`](docs/examples/starship.toml).

rterm refuses to start terminal sessions as root or from an elevated Windows
process. Use `--allow-elevated` only when the elevated child session is
intentional.

## Installation

Linux/macOS:

```sh
curl -fsSL https://raw.githubusercontent.com/Geogboe/remote-term/main/install.sh | sh
```

Windows PowerShell:

```powershell
irm https://raw.githubusercontent.com/Geogboe/remote-term/main/install.ps1 | iex
```

Manual downloads are available from the latest GitHub release:

```text
https://github.com/Geogboe/remote-term/releases
```

## Development

```powershell
task check
task ci
```

Run a command:

```powershell
task run -- pwsh
task run:lan -- codex
```

Run a local smoke command:

```powershell
cargo run -- -- pwsh -NoProfile -Command "Write-Output rterm-smoke; exit 7"
```

Expected result: `rterm-smoke` is printed and the wrapper exits with the child
exit code.

## Browser URLs

Each run generates a five-word token:

```text
http://127.0.0.1:7843/t/harbor-lime-orbit-cabin-velvet
```

Use `--lan` to bind on all interfaces and print a LAN URL. Use `--write` only on
trusted networks.
