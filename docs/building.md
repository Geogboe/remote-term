# Building & Development

## Prerequisites

- **Rust**: Latest stable (edition 2024 compatible)
- **Node.js**: For building frontend assets (npm)
- **PowerShell**: Required for Windows smoke test scripts (also available on Linux via `pwsh`)

## Quick Start

```bash
# Install web dependencies (first time)
task web:install

# Build everything and run tests
task test
task fmt
task lint

# Build and run
task run -- bash
```

## Build Tasks

rterm uses [Taskfile](https://taskfile.dev) for build orchestration.

Linux release archives include musl builds for both x64 and ARM64. The shell
installer prefers these portable archives and falls back to GNU/glibc builds.
Native Linux CI compiles both musl release targets before a release tag is used.

### Development

| Task | Description |
|------|-------------|
| `task test` | Run Rust tests |
| `task fmt` | Format Rust code (`cargo fmt --all`) |
| `task fmt:check` | Verify formatting without changing files |
| `task lint` | Run clippy with `-D warnings` |
| `task web:install` | Install npm dependencies |
| `task web:build` | Build frontend assets with esbuild |
| `task web:test` | Run browser input unit tests |
| `task web:verify` | Rebuild and verify committed browser assets |
| `task check` | Run all non-interactive quality checks |
| `task build` | Build web assets + debug binary |
| `task build:release` | Build web assets + release binary |
| `task run -- <cmd>` | Build and run rterm with `--write` (defaults to `pwsh`) |
| `task run:lan -- <cmd>` | Build and run with `--lan --write` |

### Smoke Tests

| Task | Description |
|------|-------------|
| `task smoke` | Run a quick child output smoke test |
| `task smoke:exit-code` | Verify child exit code propagation |
| `task smoke:web` | Probe tokenized web routes on localhost |
| `task smoke:browser-input` | Drive Backspace, word erase, arrows, Ctrl+C, resize, and paste through WebSocket and a real PTY |
| `task smoke:sessions` | Verify live-session lookup and registry cleanup |
| `task smoke:starship` | Verify Starship metadata visibility inside a wrapped child |
| `task smoke:final-output` | Stress immediately exiting children for final-output loss |
| `task ci` | Run all checks and native smoke paths |

### Manual Testing

```bash
# Interactive terminal test
cargo run -- -- bash

# Exit code test
cargo run -- -- pwsh -NoProfile -Command "Write-Output rterm-smoke; exit 7"

# Browser test
cargo run -- --write -- pwsh -NoProfile
# Open http://127.0.0.1:7843/t/<token>
```

## Frontend Development

The web frontend source lives in `web/src/`. After making changes:

```bash
# If you added/removed npm packages
task web:install

# Rebuild frontend assets
task web:build

# Verify the built files
ls src/web/static/
```

The build output in `src/web/static/` is committed to the repository. This means:
- Users building from source don't need Node.js if they don't modify the frontend
- The Rust build can embed assets without a JS build step
- Frontend changes require rebuilding before they take effect

### Frontend Architecture

- **Source**: `web/src/main.js` (ES module, imports xterm.js and FitAddon)
- **Bundler**: esbuild (fast, zero-config)
- **Output**: Single IIFE bundle at `src/web/static/main.js`
- **Styles**: xterm.css concatenated with app styles at `src/web/static/style.css`

## Project Structure

```
rterm/
├── Cargo.toml            Rust dependencies
├── Taskfile.yml          Build tasks
├── src/                  Rust source
│   ├── main.rs           Binary entry point
│   ├── lib.rs            Library root
│   ├── cli.rs            CLI parsing
│   ├── platform/         Platform-specific code
│   ├── security/         Token generation and validation
│   ├── session/          Session management, PTY, raw terminal, scrollback
│   └── web/              Web server, protocol, assets
├── web/                  Frontend source
│   ├── package.json      npm dependencies
│   ├── build.mjs         esbuild build script
│   └── src/              JavaScript/HTML/CSS sources
├── scripts/              Test/smoke scripts
├── docs/                 Documentation
│   ├── initial_spec.md   Original product specification
│   ├── v0_validation.md  v0 testing checklist
│   └── adr/              Architecture Decision Records
├── LICENSE               MIT
└── README.md             Project overview
```

## Code Quality

```bash
# Run all checks
task test && task fmt && task lint
```

- **No warnings allowed**: `clippy` runs with `-D warnings`
- **Formatting**: `cargo fmt --all` (standard Rust style)
- **Tests**: `cargo test` (unit tests in each module)

## CI Readiness

The project is set up for CI with:
- Deterministic frontend build (committed output)
- Standard Rust toolchain (no special features required)
- Self-contained binary (embedded assets, no runtime file dependencies)

To run CI-equivalent checks:

```bash
task web:build    # Ensure frontend is current
task test         # Run all tests
task fmt          # Check formatting
task lint         # Check for warnings and errors
```
