# 0007. GPL-3.0-or-later, and no ROMs in the repository

- Status: Proposed: needs the owner's confirmation before the first public release

## Context

Parts of this project are based on GPL-3.0 work: code, data files (ADR 0003) and help texts. That makes it a
derivative work.

## Decision

- License the project GPL-3.0-or-later before it is distributed in any form: public repo, binaries or releases.
- Never commit ROMs, ROM dumps or extracted game assets. Tests use synthetic fixtures. Tests on real ROMs read
  them from a local directory given in `PPR_TEST_ROMS`.

## Consequences

- Distributed binaries must come with the source, which a public GitHub repo covers.
- Dependencies must be GPL-compatible, which `cargo-deny` checks in CI.
