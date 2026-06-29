# Starship session module

## Goal

Allow a shell running inside rterm to show concise session information in a
Starship prompt.

## Behavior

- rterm injects non-secret session metadata into the child environment:
  - session ID
  - local or LAN exposure
  - read-only or writable browser access
  - shared local/browser control or web-only control
- `rterm starship` prints a compact prompt segment when invoked inside an rterm
  child session.
- `rterm starship` prints nothing outside an rterm session so the Starship
  custom module remains hidden.
- Browser credentials and tokenized URLs are never exposed through the prompt
  integration environment.
- The integration works with Starship's cross-platform custom-command module
  and does not require shell-specific environment syntax.

## Prompt format

```text
rterm:<session-id> <local|lan>/<ro|rw>/<shared|web>
```

Example:

```text
rterm:4260 lan/rw/shared
```

## Configuration

Users opt in through `starship.toml`:

```toml
[custom.rterm]
command = 'rterm starship'
when = true
style = 'bold blue'
format = '([$output]($style) )'
description = 'Current rterm session'
```

The default Starship `$all` format includes custom modules. Users with an
explicit top-level format can place `${custom.rterm}` where desired.
