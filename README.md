# litterkeep-server

Self-hosted Rust mail server for the local `litterkeep` integration profile.
It owns its protocol parsers, filesystem store and task/resource lifecycle.

Current checkpoint: TLS and STARTTLS SMTP submission authenticate accounts with
SASL PLAIN and deliver into the worker-owned filesystem store. IMAP supports
STARTTLS authentication, durable APPEND to INBOX, SELECT/EXAMINE metadata,
sequence-set, UID FETCH, durable STORE/FETCH FLAGS, ENVELOPE, BODYSTRUCTURE and
MIME section/partial reads. APPEND atomically preserves initial flags and
INTERNALDATE with its UID. Event-driven IDLE wakes selected sessions only after
durable APPEND completion and returns to normal commands through DONE. A
durable folder catalog exposes default SPECIAL-USE folders plus CREATE/LIST;
named folders are usable through APPEND, SELECT, FETCH and STORE. RENAME keeps
stable internal IDs for the whole subtree; DELETE retires an ID permanently and
refuses INBOX or a parent that still has children. SUBSCRIBE/UNSUBSCRIBE persist
independently of folder existence, LSUB exposes missing subscriptions as
`\Noselect`, and STATUS reads mailbox counters without selecting it.
STORE keeps `\Deleted` separate from durable EXPUNGE tombstones; an EXPUNGE
rebuilds sequence numbers without reusing UIDs and is denied after EXAMINE.
This is not yet a complete mail server: POP3/STLS has a verified SMTP-to-INBOX
read/delete checkpoint with maildrop leases and implicit TLS. ManageSieve has
a verified STARTTLS/SASL literal checkpoint that activates a `fileinto` rule
before SMTP-to-IMAP delivery. Broader external interoperability, the remaining
Sieve language, durable outbound transport integration and failure coverage
remain unfinished.

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

Linux checks reuse bind-mounted stage directories under `./data`; for example,
the subscriptions checkpoint uses `./data/g06-subscriptions-status`. The shared
test PKI is kept in `./data/pki`. These local artifacts are ignored by Git and
excluded from the Docker build context.

## POP3 STARTTLS checkpoint

```sh
cargo run --locked -- init ./runtime alice alice-password
cargo run --locked -- init ./runtime bob bob-password
cargo run --locked -- serve-pop3-starttls 0.0.0.0:1110 ./runtime ./server.der ./server.key.der
cargo run --locked -- serve-pop3-tls 0.0.0.0:1995 ./runtime ./server.der ./server.key.der
cargo run --locked -- serve-all-starttls 0.0.0.0:1587 0.0.0.0:1143 0.0.0.0:1110 ./runtime ./server.der ./server.key.der
```

The POP3 test account is `bob/bob-password`; clients must use STLS before
`PASS` on port 1110, or implicit TLS on port 1995. The verified Linux test is split into SMTP delivery and POP3 reading so
the same bind-mounted `./data/g07-pop3` store is reused across server restart:

```sh
python tests/blackbox/pop3_starttls_delivery.py --phase smtp --port 11587 --ca ./data/pki/ca.pem
python tests/blackbox/pop3_starttls_delivery.py --phase pop3 --port 11110 --ca ./data/pki/ca.pem
python tests/blackbox/pop3_starttls_delivery.py --phase empty --port 11110 --ca ./data/pki/ca.pem
```

`serve-all-starttls` is the one-process acceptance mode: SMTP, IMAP and POP3
share one storage owner and poll reactor. The destructive POP3 fixture uses
`./data/g07-pop-destructive` and the dedicated account `pop/pop-password`.

## ManageSieve literal checkpoint

```sh
cargo run --locked -- init ./runtime alice alice-password
cargo run --locked -- init ./runtime bob bob-password
cargo run --locked -- serve-all-mail-starttls 0.0.0.0:1587 0.0.0.0:1143 0.0.0.0:1110 0.0.0.0:4190 ./runtime ./server.der ./server.key.der
python tests/blackbox/managesieve_literal_delivery.py --smtp-port 1587 --imap-port 1143 --managesieve-port 4190 --ca ./data/pki/ca.pem
```

All four listeners run in the same process, so the ManageSieve script catalog,
SMTP delivery and IMAP view share exactly one filesystem Store owner. The
black-box client performs STARTTLS and SASL PLAIN, checks a synchronous
`CHECKSCRIPT {n}` literal and a fragmented `PUTSCRIPT {n+}` literal, activates
the script, sends an SMTP message as Alice and reads it from Bob's `Invoices`
folder through IMAP. The test accounts are `alice/alice-password` and
`bob/bob-password`.

The only runtime binary is `target/release/litterkeep-server`. Current Docker
test state is placed under ignored `data/` stage directories.

See `GOAL.md` for actual stage status and `litterkeep-rust-terra-goal.md` for
the requested project contract.

Architecture and ownership boundaries are documented in
[`docs/architecture.md`](docs/architecture.md).
