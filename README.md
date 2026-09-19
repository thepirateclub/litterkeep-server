# litterkeep-server

Self-hosted Rust mail server for the local `litterkeep` integration profile.
It owns its protocol parsers, filesystem store and task/resource lifecycle.

Current checkpoint: G00 bootstrap plus a G01 Linux reactor implementation.
This is not yet a complete SMTP/IMAP server. TLS and account authentication are
not implemented, and the Linux runtime acceptance suite remains unrun.

## Developer commands

```powershell
cargo build --locked --release --bin litterkeep-server
cargo test --locked
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo run --locked -- help
cargo tree --locked -e normal
```

## Linux G01 reactor command

```sh
cargo run --locked -- serve 127.0.0.1:2525 127.0.0.1:2143 ./runtime
```

This is one process with both listeners and one filesystem-store owner. The
current host runs Windows, so the command refuses to run here rather than
selecting the old blocking listeners. Linux testing is still required before
this command is treated as an acceptance result.

The repository includes `tests/blackbox/smtp_imap_attachment.py`, which uses
only Python's standard SMTP, IMAP and MIME clients. It was run successfully
against the Linux Docker execution image on 2026-09-19. Docker is a test
environment only, not a required part of litterkeep-server.

The only runtime binary is `target/release/litterkeep-server`. Runtime data,
TLS files and test artifacts will be placed under `runtime/` and are ignored.

See `GOAL.md` for actual stage status and `litterkeep-rust-terra-goal.md` for
the requested project contract.
