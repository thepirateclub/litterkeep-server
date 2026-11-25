# Project status

This page answers three questions: how ready the server is, what already works
and has been verified, and what is still only planned. The detailed protocol
profiles stay in [`docs/`](docs/README.md), and the machine-readable acceptance
matrix is in [`docs/acceptance.tsv`](docs/acceptance.tsv).

> **In short:** this is a self-contained Rust mail server with its own protocol
> parsers, its own MIME and Sieve implementation, and its own filesystem store.
> SMTP submission, IMAP4rev1, POP3 and ManageSieve are implemented and verified
> at wire level. The project does not claim full RFC conformance and does not
> claim interoperability with real mail clients and servers.

**The Russian version of this page — [docs/ru/STATUS.md](docs/ru/STATUS.md).**

## How to read this document

The designations come from the project convention and are used the same way in
every document:

| Designation | Meaning |
|---|---|
| **PASS** | There is a typed parser, a reactor or execution state transition and a profile test. The behaviour is confirmed by an independent wire check. |
| **PARTIAL** | Deliberately limited semantics. `CAPABILITY` does not present it as a broader extension. The boundary is named explicitly. |
| **NOT STARTED** | Not implemented and not advertised. |

Review date: **2026-11-25**. Sources: `README.md`, `docs/imap-profile.md`,
`docs/protocol-coverage.md`, `docs/sieve-extension-audit.md`,
`docs/sieve-coverage.md`, `docs/acceptance.md`.

## Readiness summary

| Subsystem | Status | What is confirmed | Details |
|---|---|---|---|
| SMTP submission | **PASS** | Implicit TLS and STARTTLS, `AUTH PLAIN`, `DSN` over EHLO, `MAIL RET`/`ENVID`, `RCPT NOTIFY`/`ORCPT`, DATA and BDAT, RFC 5321 phase timeouts. The full semantics of the resulting DSN notifications per route remain unconfirmed | [protocol-coverage](docs/protocol-coverage.md#smtp) |
| IMAP4rev1 | **PARTIAL** | 21 command and property groups; the full profile is not closed yet, exact statuses are in the [matrix](docs/imap-profile.md) | [imap-profile](docs/imap-profile.md) |
| IMAP4rev2 (RFC 9051) | **NOT STARTED** | The target baseline of the next stage. It cannot be advertised before the mandatory semantics and the wire acceptance are done | [imap-profile](docs/imap-profile.md) |
| POP3 | **PASS** | STARTTLS/STLS and implicit TLS, `USER`/`PASS`, `RETR`/`DELE`, maildrop leases, UIDL across a restart | [protocol-coverage](docs/protocol-coverage.md#pop3) |
| ManageSieve | **PASS** | STARTTLS, SASL PLAIN, SCRAM-SHA-1, SCRAM-SHA-256, SCRAM-SHA-256-PLUS, `CHECKSCRIPT`/`PUTSCRIPT`/`LISTSCRIPTS`/`GETSCRIPT`/`SETACTIVE`/`DELETESCRIPT`/`RENAMESCRIPT`, literals | [sieve-coverage](docs/sieve-coverage.md) |
| Sieve | **PARTIAL** | Basic RFC 5228 profile, `fileinto`, `redirect`, `keep`/`discard`, header/address/size tests, diagnostics. The internal RFC 5229 profile and RFC 5228 `encoded-character` decoding are implemented but **not advertised** | [sieve-coverage](docs/sieve-coverage.md), [audit](docs/sieve-extension-audit.md) |
| Filesystem storage | **PASS** | `litterkeep-store-v2` format, durable journals, atomic blob publication, recovery, `check`, `backup` | [storage-format](docs/storage-format.md) |
| Outgoing delivery | **PASS** | Durable queue, single-flight scheduler on Linux, retry policy, local DSN after a permanent failure, TLS route with a pinned SAN and CA. Verified against an explicit relay peer; public DNS/MX and internet delivery were not tested | [protocol-coverage](docs/protocol-coverage.md#outbound-smtp) |
| Operational tooling | **PASS** | `init`, `seed`, `runtime stats/tasks/resources/inspect/cancel`, `check`, `backup`, `sieve-errors` | [architecture](docs/architecture.md) |

## Acceptance in numbers

The matrix [`docs/acceptance.tsv`](docs/acceptance.tsv) contains 439 checks:

| Status | Count | Meaning |
|---|---:|---|
| `PASS` | 349 | The check was performed and confirmed by an independent observation |
| `PARTIAL` | 79 | Only part of the boundary is confirmed, the rest is named explicitly |
| `SUPERSEDED` | 7 | An earlier checkpoint replaced by a later check |
| `BLOCKED` | 1 | The check could not be performed for an external reason |
| `FAIL` | 1 | The check was performed and did not pass |
| `GAP` | 1 | A recorded gap without an attempt to check |
| `NOT RUN` | 1 | Prepared but never executed |

The column format and the meaning of the statuses are explained in
[`docs/acceptance.md`](docs/acceptance.md#the-matrix-format).

## What is done and verified

### SMTP submission

A single process accepts submission on 1465 (implicit TLS) and 1587 (STARTTLS).
The TLS handshake completes before any greeting is sent, so credentials never
travel in clear text. `AUTH PLAIN` only works after TLS.

The accepted typed envelope passes through DATA and BDAT to the single Store
owner, is stored in a commit-linked sidecar and is recovered after a reopen.
RFC 3461 `DSN` is advertised over EHLO; independent raw-socket peers confirmed
the capability, the acceptance of `MAIL RET=HDRS ENVID=...`, `RCPT
NOTIFY=FAILURE ORCPT=...`, the completion of DATA and the refusal of the same
parameters after HELO.

The delivery phase uses RFC 5321 timeouts that can be configured at process
start. An I/O error destroys the dirty connection, the durable job moves into the
retry policy and the channel is not returned to the pool.

### IMAP4rev1

`CAPABILITY`, `ID`, `NOOP`, `LOGOUT`, `STARTTLS`, `LOGIN`,
`AUTHENTICATE PLAIN` with SASL-IR, `NAMESPACE`, `ENABLE`, `CREATE`/`DELETE`/
`RENAME`, `SUBSCRIBE`/`UNSUBSCRIBE`, `SELECT`/`EXAMINE`, `APPEND`, `FETCH`,
`UID FETCH`, `SEARCH`, `UID SEARCH`, `STORE`, `UID STORE`, `COPY`/`MOVE`,
`EXPUNGE`/`UID EXPUNGE`, `CLOSE`, `UNSELECT`, `CHECK`, `IDLE`/`DONE`,
`LIST SPECIAL-USE`, `RETURN` and `CREATE USE` are working.

Key confirmed properties: `APPEND` atomically preserves the initial flags and
`INTERNALDATE` together with the UID; the event-driven IDLE wakes only selected
sessions and returns to normal commands through `DONE` after a durable APPEND;
the durable folder catalog exposes the default `SPECIAL-USE` folders; `RENAME`
keeps stable internal IDs for the whole subtree; `DELETE` retires an ID
permanently and refuses INBOX or a parent that still has children; `LSUB`
exposes missing subscriptions as `\Noselect`; `STATUS` reads mailbox counters
without selecting the folder; `EXPUNGE` rebuilds the sequence numbers without
reusing UIDs and is denied after `EXAMINE`.

The exact line-by-line matrix with the status and the verifiable boundary is in
[`docs/imap-profile.md`](docs/imap-profile.md).

### POP3

`serve-pop3-starttls` on 1110 and `serve-pop3-tls` on 1995. STLS is mandatory
before `PASS`, or implicit TLS is used. The command order, the refusal of
`NOOP` outside TRANSACTION per RFC 1939 §5, the 40-character argument limit per
RFC 1939 §3, `UIDL` surviving a restart and deletion through a maildrop lease
are all verified.

### ManageSieve

All four listeners run in the same process, so the script catalog, SMTP delivery
and IMAP visibility share exactly one Store owner. STARTTLS, SASL PLAIN with
initial-response and with challenge/response flow, SCRAM-SHA-1, SCRAM-SHA-256
and SCRAM-SHA-256-PLUS when a TLS 1.3 exporter is available, synchronous and
fragmented literals and the full set of script catalog commands are
implemented.

### Sieve

The advertised capability is strictly limited to:
`fileinto subaddress envelope copy vacation vacation-seconds`.

The RFC 5229 profile is implemented internally but **not advertised**: `set`,
the `string` test, modifiers, expansion of variables into condition and action
strings, `fileinto`/`redirect` targets, vacation strings and envelope and
comparator selectors. RFC 5228 `encoded-character` decoding (`${hex:...}`,
`${unicode:...}`) is implemented as well, and string octets are preserved
through the lexer, matching and actions. Advertising these extensions requires
a separate acceptance.

Delivery errors are available to the owner through `sieve-errors`: every line
contains the delivery transaction, the account, the script version that was
actually executed and a typed error code. The journal is limited to 1 MiB, and
exhausting the quota rejects the transaction before SMTP success rather than
accepting a message without the promised diagnostics.

### Filesystem storage

A blob is published through a temporary file, `fsync`, an atomic rename and a
directory sync. Mailbox links, flags, folders, subscriptions, Recent, Sieve,
diagnostics, the queue and global transaction commits use framed append-only
journals. A fully invalid frame counts as corruption, an incomplete tail is
safely truncated.

A UID is monotonic within a mailbox and is never reused after EXPUNGE. COPY
shares an immutable blob, MOVE publishes the destination and the source
tombstone in one transaction. There is no automatic migration from earlier
development formats: an unknown or unmarked directory with mail data is
rejected.

### Operational tooling

`init` creates the local acceptance accounts with Argon2id PHC and random
passwords in `test-accounts.tsv`. `seed` fills the test mailboxes according to
the manifest `tests/fixtures/seed-manifest.tsv` and does not double the
fixtures on a repeated run. `check` takes the exclusive Store lock, performs
the standard recovery and reads every blob referenced by a mailbox or a pending
delivery; orphan blobs are only counted. `backup` requires a stopped Store
owner.

The runtime control socket returns only lifecycle metadata: task state,
generation-protected IDs and held resources. Message bodies, passwords and TLS
keys are neither returned nor logged.

## Partially implemented

| Area | Status | What is limited |
|---|---|---|
| IMAP `CAPABILITY`, `ID`, `NOOP`, `LOGOUT` | PARTIAL | `ID` answers `* ID NIL` so that it does not expose an implementation fingerprint. Unlisted extensions are not supported |
| IMAP `LIST`, `LSUB`, `STATUS` | PARTIAL | Multi-namespace and breakout naming are not implemented |
| IMAP `ENABLE` | PARTIAL | `UTF8=ACCEPT` is supported; `UTF8=ONLY` is neither supported nor advertised |
| IMAP `APPEND` | PARTIAL | Internationalised headers are confirmed, but the full RFC 9755 profile gate is not closed |
| IMAP `FETCH`, `UID FETCH` | PARTIAL | A limited set of section and partial reads; the full RFC 3501 set is not closed |
| IMAP `SEARCH`, `UID SEARCH` | PARTIAL | The evaluator is implemented, but no claim on the full RFC 9051 key group is made |
| IMAP address groups | PARTIAL | `ENVELOPE` and the SEARCH address keys are implemented in part |
| Sieve `variables` (RFC 5229) | PARTIAL | The profile works internally, the capability is not advertised |
| Sieve `encoded-character` | PARTIAL | The decoding works, the capability is not advertised |
| POP3 authentication | PASS with a limit | A successful password is confirmed; other mechanisms are not |
| Linux-only reactor | Platform limit | On Windows the project builds and tests, but `serve` and the other reactor commands do not run |

## Planned but not started

| Extension or goal | RFC | What it would take |
|---|---|---|
| IMAP4rev2 baseline | RFC 9051 | The full key group and the mandatory semantics, plus wire acceptance |
| `UTF8=ONLY` | RFC 9755 | A separate profile: reject legacy modified UTF-7 and answer `NO [CANNOT]` for UTF-8-dependent commands before `ENABLE` |
| Sieve `body` | RFC 5173 | MIME-aware `:raw`, `:content`, `:text` without matches across part boundaries, with CPU and memory limits |
| Sieve `reject`, `ereject` | RFC 5429 | Sieve evaluation before SMTP accepts DATA; a post-accept DSN is not equivalent to `ereject` |
| Sieve `imap4flags` | RFC 5232 | `setflag`, `addflag`, `removeflag`, `hasflag` and `:flags` with durable flag mutation |
| Sieve IMAP events | RFC 6785 | IMAP flag support and a defined event context for the execution |
| Sieve `mailboxid` | RFC 9042 | Stable mailbox OBJECTIDs, `:mailboxid` on `fileinto` and `mailboxidexists` |
| Sieve mailbox metadata | RFC 5490 | `mailboxexists`, metadata tests and metadata-based processing |
| Sieve special-use delivery | RFC 8579 | `specialuse`, `:specialuse` and agreement with the typed special-use catalog |

The full inventory with the next necessary boundary for each extension is in
[`docs/sieve-extension-audit.md`](docs/sieve-extension-audit.md).

## Deliberate limitations

These are not defects and not tasks; they are recorded boundaries of the
project.

- **Not intended for the public internet.** It is considered a research-grade
  implementation. Before any external exposure it needs a front-end with real
  TLS and an anti-abuse policy.
- **One Store owner.** A second parallel writer is rejected: `check`, `backup`
  and `sieve-errors` open the Store as the single writer and therefore must not
  run in parallel with the server.
- **No automatic format migration.** Moving from earlier development formats to
  `litterkeep-store-v2` is not performed.
- **Windows does not run the server.** The reactor relies on the Linux
  `poll(2)`; a Windows host is used for building, `fmt`, `clippy` and tests.
- **Size limits.** DATA and the IMAP response are limited to 25 MiB, and the
  immutable-blob budget in the data directory is also 25 MiB. An excess is
  rejected before the temporary blob is created.
- **Outgoing mail routing** accepts only an explicit numeric `IP:port`, which
  excludes hidden blocking DNS from the transport worker.
- **One route profile for a Sieve redirect** — the typed domain `peer.test`. A
  forbidden route yields an implicit keep and a Sieve diagnostic, and a
  corrupted job gets the terminal code `5.7.1` instead of meaningless retries.

## How to reproduce the verification

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --locked --release --bin litterkeep-server
cargo run --locked -- help
```

The pinned toolchain is Rust **1.97.0** (`rust-toolchain.toml`), which is also
the version in `Cargo.toml` and in the `rust:1.97-bookworm` Docker image.

The same set runs in CI on every push: `fmt` and `clippy` on ubuntu and
windows, the tests on ubuntu and windows, plus a release build. The badge at the
top of the README shows the current state.

The black-box checks live in `tests/blackbox/`. They require a running server
and a Linux environment; the README describes the order to run them. The
machine-readable matrix of the checks that were actually performed is
`docs/acceptance.tsv`.

## Where to look for details

| Document | Subject |
|---|---|
| [`docs/README.md`](docs/README.md) | Documentation navigation and reading order |
| [`docs/ru/README.md`](docs/ru/README.md) | The same navigation in Russian |
| [`docs/architecture.md`](docs/architecture.md) | Process, resource ownership, durability |
| [`docs/runtime-lifecycle.md`](docs/runtime-lifecycle.md) | Tasks, waiting, wake-up, cancellation |
| [`docs/storage-format.md`](docs/storage-format.md) | Storage format and recovery |
| [`docs/decisions.md`](docs/decisions.md) | Architectural decisions taken and their cost |
| [`docs/dependencies.md`](docs/dependencies.md) | Production dependencies and the project's own primitives |
| [`docs/security.md`](docs/security.md) | Security profile |
| [`docs/client-setup.md`](docs/client-setup.md) | Connecting a mail client |
| [`docs/protocol-coverage.md`](docs/protocol-coverage.md) | Detailed coverage of SMTP, IMAP, POP3, Sieve and outbound |
| [`docs/imap-profile.md`](docs/imap-profile.md) | Line-by-line IMAP matrix with statuses |
| [`docs/sieve-coverage.md`](docs/sieve-coverage.md) | Sieve and ManageSieve profile |
| [`docs/sieve-extension-audit.md`](docs/sieve-extension-audit.md) | Inventory of Sieve extensions |
| [`docs/unicode-data.md`](docs/unicode-data.md) | Pinned Unicode data version |
| [`docs/acceptance.md`](docs/acceptance.md) | Final acceptance per task |
| [`docs/acceptance.tsv`](docs/acceptance.tsv) | Machine-readable acceptance matrix |
| [`GOAL.md`](GOAL.md) | Development journal per stage |
