# rterm v0 validation

Use this file to record implementation validation against `docs/initial_spec.md`.

## Automated checks

```powershell
task test
task fmt
task lint
```

## Frontend bundle

```powershell
task web:install
task web:build
```

The generated assets are embedded from `src/web/static/`.

## Smoke checks

Child process output and exit code:

```powershell
task smoke:exit-code
```

Expected:

```text
rterm-smoke
```

The process should exit with code `7`.

Browser route/auth check:

```powershell
cargo run -- --write -- pwsh
```

Open the printed `Local URL`. A wrong token should return `404`; the WebSocket
should reject wrong tokens and reject browser input unless `--write` is set.

## Target matrix

Record smoke evidence for:

```text
Windows x64
Windows arm64
Linux x64
Linux arm64
```

## Current evidence

Recorded 2026-06-26 from the local Windows ARM64 development environment:

```text
task fmt        passed
task test       passed, 15 tests
task lint       passed
task web:build  passed
task smoke      passed, printed rterm-smoke and exited 0
task smoke:web  passed, valid token 200, asset 200, wrong token 404
```

`task smoke:exit-code` printed `rterm-smoke` and returned exit status `7`,
matching the child process exit code. The task itself reports failure because
the smoke intentionally exits nonzero.

Still needs validation on real interactive terminals:

```text
Ctrl+C
Ctrl+D
Ctrl+Backspace / word erase
arrows
large paste
resize
alternate screen
browser write input from a real browser
```

Still needs target-machine smoke evidence:

```text
Windows x64
Linux x64
Linux arm64
```
