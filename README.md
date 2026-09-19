# litterkeep-server

Self-hosted Rust mail server for the local `litterkeep` integration profile.
It owns its protocol parsers, filesystem store and task/resource lifecycle.

Current checkpoint: TLS and STARTTLS SMTP submission authenticate accounts with
SASL PLAIN and deliver into the worker-owned filesystem store. IMAP supports
STARTTLS authentication, durable APPEND to INBOX, SELECT/EXAMINE metadata,
sequence-set, UID FETCH, durable STORE/FETCH FLAGS, ENVELOPE, BODYSTRUCTURE and
MIME section/partial reads. Event-driven IDLE wakes selected sessions only
after durable APPEND completion and returns to normal commands through DONE.
This is not yet a complete mail server: folders, full POP3, Sieve, durable
outbound transport integration and the remaining acceptance matrix are
unfinished.

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

## Implicit TLS submission command

```sh
cargo run --locked -- init ./runtime alice alice-password
cargo run --locked -- init ./runtime bob bob-password
cargo run --locked -- serve-tls 0.0.0.0:1465 0.0.0.0:1993 ./runtime ./server.der ./server.key.der
```

Connection parameters for this checkpoint are SMTP submission on port 1465
with implicit TLS and `AUTH PLAIN`, plus IMAP on port 1993 with implicit TLS.
The acceptance accounts are `alice/alice-password` and `bob/bob-password`.
The certificate must be trusted by the client and contain the listener host in
its SAN. Test-only material can be generated with
`cargo run --locked --example make_test_pki -- ./test-pki`.

The current immutable-message blob budget is 25 MiB per data directory. The
single storage writer reconstructs actual usage at startup and rejects a new
message before creating its temporary blob when the budget would be exceeded.
The current on-disk marker is `litterkeep-store-v2`. There is no automatic
migration from earlier development formats; an unknown or unversioned data
directory with mail data is rejected.

The repository includes `tests/blackbox/tls_smtp_auth_attachment.py`, which
uses only Python's standard SMTP, IMAP and MIME clients. It was run successfully
against the Linux Docker execution image on 2026-09-19 and compared the decoded
attachment bytes. Docker is a test environment only, not a required part of
litterkeep-server.

The only runtime binary is `target/release/litterkeep-server`. Runtime data,
TLS files and test artifacts will be placed under `runtime/` and are ignored.

See `GOAL.md` for actual stage status and `litterkeep-rust-terra-goal.md` for
the requested project contract.

Architecture and ownership boundaries are documented in
[`docs/architecture.md`](docs/architecture.md).
