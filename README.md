# litterkeep-server

Self-hosted Rust mail server for the local `litterkeep` integration profile.
It owns its protocol parsers, filesystem store and task/resource lifecycle.

Current checkpoint: G00 bootstrap plus the G01 lifecycle core. This is not yet
a complete SMTP/IMAP server. A local plaintext SMTP acceptance smoke is
implemented; IMAP, TLS and account authentication are not yet network-ready.

## Developer commands

```powershell
cargo build --locked --release --bin litterkeep-server
cargo test --locked
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo run --locked -- help
cargo tree --locked -e normal
```

## Current local SMTP smoke

```powershell
.\target\release\litterkeep-server.exe serve-smtp 127.0.0.1:2525 .\runtime
```

The listener accepts an SMTP transaction and writes locally addressed test mail
to `runtime/blobs/` plus `runtime/mailboxes/bob.journal`. It is intentionally
not an open relay and not a production deployment profile.

The only runtime binary is `target/release/litterkeep-server`. Runtime data,
TLS files and test artifacts will be placed under `runtime/` and are ignored.

See `GOAL.md` for actual stage status and `litterkeep-rust-terra-goal.md` for
the requested project contract.
