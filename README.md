# litterkeep-server

Self-hosted Rust mail server for the local `litterkeep` integration profile.
It owns its protocol parsers, filesystem store and task/resource lifecycle.

Current checkpoint: the first Linux SMTP to IMAP path, including worker-owned
Argon2id account verification and durable local storage. This is not yet a
complete mail server: TLS, SMTP authentication, account isolation, full IMAP,
POP3, Sieve and outbound SMTP remain unfinished.

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
cargo run --locked -- init ./runtime
cargo run --locked -- serve 127.0.0.1:2525 127.0.0.1:2143 ./runtime
```

This is one process with both listeners and one filesystem-store owner. The
`init` creates the local test account `bob` with password `test-password`.
It is a development fixture, not a production account-provisioning mechanism.
The current host runs Windows, so `serve` refuses to run here rather than
selecting old blocking listeners. The Linux Docker acceptance result is
documented below.

The repository includes `tests/blackbox/smtp_imap_attachment.py`, which uses
only Python's standard SMTP, IMAP and MIME clients. It was run successfully
against the Linux Docker execution image on 2026-09-19. Docker is a test
environment only, not a required part of litterkeep-server.

The only runtime binary is `target/release/litterkeep-server`. Runtime data,
TLS files and test artifacts will be placed under `runtime/` and are ignored.

See `GOAL.md` for actual stage status and `litterkeep-rust-terra-goal.md` for
the requested project contract.

Architecture and ownership boundaries are documented in
[`docs/architecture.md`](docs/architecture.md).
