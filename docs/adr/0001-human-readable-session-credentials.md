# ADR 0001: Human-readable session credentials

- Status: Accepted
- Date: 2026-06-26

## Context

rterm prints a credential-bearing browser URL when a session starts. The
original credential was 32 random alphanumeric characters, which provided
approximately 190 bits of entropy but was difficult to transcribe onto a
phone.

The credential is an online, session-scoped secret. It is not a password hash
that an attacker can test offline, and it ceases to work when the owning rterm
process exits.

## Decision

Automatically generated credentials contain five independently and
cryptographically selected words from the 7,772 separator-safe entries in
EFF's long Diceware list,
joined with hyphens:

```text
harbor-lime-orbit-cabin-velvet
```

Five selections provide approximately 64.6 bits of entropy. Repeated words are
allowed because rejecting them would bias generation. Four EFF entries with
internal hyphens are excluded so generated credentials always have exactly
five unambiguous hyphen-separated parts.

The application depends on `diceware_wordlists` for the audited word data and
uses the existing `rand` dependency for selection. This keeps the wordlist out
of the application source tree and avoids a build-time network dependency.

Explicit `--token` values remain supported for automation and testing, but
rterm validates that they are non-empty URL-path-safe ASCII strings.

## Consequences

- Generated URLs are longer in characters but substantially easier to read and
  transcribe.
- Generated credentials have less entropy than the previous random string, but
  remain impractical to guess through the online HTTP endpoint during a single
  session.
- The executable contains the selected dependency's embedded wordlist.
- Dependency upgrades must retain 7,772 separator-safe entries and URL-safe
  word properties or deliberately replace this ADR.

## Alternatives considered

### Keep 32 random alphanumeric characters

Rejected because transcription was the reported usability problem.

### Short permanent character code

Rejected because it reduces entropy without improving memorability as much as
random words.

### Temporary pairing code plus a separate token

Rejected because it adds credential expiry, redirect, and retry state to solve
a problem that a sufficiently strong random passphrase solves directly.

### Download the EFF list during compilation

Rejected because network-dependent builds are not reproducible and fail
offline.

### Vendor the wordlist in this repository

Rejected to keep large third-party data out of the application source tree.
