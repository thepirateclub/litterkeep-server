# litterkeep-server

[![CI](https://github.com/thepirateclub/litterkeep-server/actions/workflows/ci.yml/badge.svg)](https://github.com/thepirateclub/litterkeep-server/actions/workflows/ci.yml)
[![License: MIT](https://img.shields.io/badge/License-MIT-yellow.svg)](LICENSE)

A self-contained mail server written in Rust for the local `litterkeep`
integration profile. The server implements its own protocol parsers, its own
MIME and Sieve processing, its own filesystem store, and its own orchestration
of tasks and resources. No off-the-shelf mail engine, protocol library or
async runtime is used.

**Project readiness summary — [STATUS.md](STATUS.md).**
**Documentation navigation — [docs/README.md](docs/README.md).**
**Russian version — [README.ru.md](README.ru.md).**

## Contents

- [Capabilities](#capabilities)
- [Requirements](#requirements)
- [Quick start](#quick-start)
- [Run modes](#run-modes)
- [Outgoing delivery](#outgoing-delivery)
- [Maintenance and diagnostics](#maintenance-and-diagnostics)
- [Connecting a mail client](#connecting-a-mail-client)
- [Resource limits](#resource-limits)
- [Development and verification](#development-and-verification)
- [Project layout](#project-layout)
- [Documentation](#documentation)
- [Licence](#licence)

## Capabilities

### Protocols

| Protocol | Modes | State |
|---|---|---|
| SMTP submission | implicit TLS, STARTTLS | The main delivery path: `AUTH PLAIN`, `DSN` over EHLO, `MAIL RET`/`ENVID`, `RCPT NOTIFY`/`ORCPT`, DATA and BDAT |
| IMAP4rev1 | implicit TLS, STARTTLS | `LOGIN`, `AUTHENTICATE PLAIN` with SASL-IR, `APPEND`, `SELECT`/`EXAMINE`, `FETCH`, `SEARCH`, `STORE`, `COPY`/`MOVE`, `EXPUNGE`, `IDLE`, folder catalog, `STATUS` |
| POP3 | implicit TLS, STLS | `USER`/`PASS`, `RETR`/`DELE`, `UIDL`, maildrop leases |
| ManageSieve | STARTTLS | `AUTHENTICATE` with PLAIN, SCRAM-SHA-1, SCRAM-SHA-256 and SCRAM-SHA-256-PLUS; the full set of script catalog commands; synchronous and fragmented literals |

### Sieve

The advertised extensions are strictly limited to:
`fileinto subaddress envelope copy vacation vacation-seconds`.

The RFC 5229 profile (`set`, the `string` test, expansion of variables into
condition and action strings, `fileinto`/`redirect` targets, vacation strings,
envelope and comparator selectors) and RFC 5228 `encoded-character` decoding
(`${hex:...}`, `${unicode:...}`) are implemented internally but **not
advertised**. String octets are preserved without loss through the lexer,
matching and script actions.

### Storage and delivery

- A filesystem Store in the `litterkeep-store-v2` format with durable journals
  and crash recovery. There is no automatic migration from earlier formats.
- One Store owner per process. Every incoming protocol and the outgoing
  delivery share it.
- An outgoing scheduler with single-flight delivery, a retry policy and a local
  notification to the sender after a permanent failure.
- A control socket, an offline integrity check and a consistent backup.

The full profile of every protocol and the verified boundaries are described in
[docs/protocol-coverage.md](docs/protocol-coverage.md) and
[docs/imap-profile.md](docs/imap-profile.md).

## Requirements

| | |
|---|---|
| Language | Rust **1.97.0** (pinned in `rust-toolchain.toml` and `Cargo.toml`) |
| Server OS | Linux — the reactor is built on `poll(2)` |
| Build OS | Any, including Windows: `fmt`, `clippy` and the tests run there |

On Windows the project builds and tests, but `serve` and the other reactor
commands do not run and exit with an explicit message.

## Quick start

### 1. Build

```sh
cargo build --locked --release --bin litterkeep-server
```

The only executable produced by the build is `target/release/litterkeep-server`.

### 2. Initialise the data directory

```sh
cargo run --locked -- init --data-dir ./runtime
```

The command creates the local acceptance accounts `alice`, `bob`, `qa` and
`pop`. Random passwords are written to `./runtime/test-accounts.tsv` in the
format `<account-name><TAB><password>` with no header; on Linux the file gets
mode `0600`. Secrets are never printed to standard output. A repeated run
validates the existing records and leaves both the password file and the PHC
hashes byte-for-byte unchanged.

This is a local acceptance bootstrap, not an account management mechanism.

A separate opt-in command creates a Unicode recipient fixture for SMTPUTF8
acceptance in its own `test-smtputf8-account.tsv` file, without touching the
original credentials:

```sh
litterkeep-server init-smtputf8-test --data-dir ./runtime
```

### 3. Generate a test PKI

The certificate and key are given as DER files. The test material is produced
by a one-off command:

```sh
cargo run --locked --example make_test_pki -- ./test-pki
```

The client must trust the issuing CA, and the certificate must carry a SAN with
the listener host name. For a client using the name `litterkeep.test`, the SAN
must contain `litterkeep.test`.

### 4. Run

STARTTLS mode for SMTP submission, IMAP and POP3:

```sh
cargo run --locked -- init --data-dir ./runtime
cargo run --locked -- serve-all-starttls \
  0.0.0.0:1587 0.0.0.0:1143 0.0.0.0:1110 \
  ./runtime ./server.der ./server.key.der
```

## Run modes

The full list of commands with their arguments is in `litterkeep-server help`.

### One process for every incoming protocol

SMTP submission, IMAP, POP3 and ManageSieve in one reactor with one Store owner.
The TLS handshake completes before any protocol greeting is sent, so
credentials never travel in clear text.

```sh
cargo run --locked -- serve-all-mail-starttls \
  0.0.0.0:1587 0.0.0.0:1143 0.0.0.0:1110 0.0.0.0:4190 \
  ./runtime ./server.der ./server.key.der
```

### Implicit TLS

One process with SMTP submission, IMAP, POP3 and ManageSieve over implicit TLS.
The default ports are bound to the loopback interface:

```sh
cargo run --locked -- serve-all-mail-tls \
  127.0.0.1:1465 127.0.0.1:1993 127.0.0.1:1995 127.0.0.1:14190 \
  ./data ./data/pki/server.der ./data/pki/server.key.der
```

[RFC 8314](https://www.rfc-editor.org/rfc/rfc8314.html) describes implicit TLS
for SMTP submission, IMAP and POP3. Implicit TLS for ManageSieve is an
additional run mode of the server here, not a claim about a normative port or
an RFC 8314 requirement. The command does not replace existing STARTTLS
listeners and does not by itself mean full protocol compatibility.

### SMTP and IMAP only

```sh
# plaintext greeting with a mandatory upgrade
cargo run --locked -- serve-starttls 0.0.0.0:1587 0.0.0.0:1143 ./runtime ./server.der ./server.key.der

# implicit TLS
cargo run --locked -- serve-tls 0.0.0.0:1465 0.0.0.0:1993 ./runtime ./server.der ./server.key.der

# basic mode without TLS
cargo run --locked -- serve 127.0.0.1:2525 127.0.0.1:2143 ./runtime
```

### POP3 only

```sh
cargo run --locked -- serve-pop3-starttls 0.0.0.0:1110 ./runtime ./server.der ./server.key.der
cargo run --locked -- serve-pop3-tls 0.0.0.0:1995 ./runtime ./server.der ./server.key.der
```

The client must complete STLS before `PASS` on port 1110, or use implicit TLS on
port 1995. The POP3 acceptance account is `pop`, and its password is in
`test-accounts.tsv`.

### ManageSieve only

```sh
cargo run --locked -- serve-managesieve-starttls 0.0.0.0:4190 ./runtime ./server.der ./server.key.der
```

## Outgoing delivery

### STARTTLS route

```sh
litterkeep-server serve-all-mail-outbound-starttls \
  0.0.0.0:1587 0.0.0.0:1143 0.0.0.0:1110 0.0.0.0:4190 \
  127.0.0.1:2526 ./data/g09-live server.der server.key.der
```

### Implicit TLS route

A permanent implicit-TLS route uses a separate command: the address, the
verified name and the CA are pinned to a dedicated pool and are never mixed
with plaintext.

```sh
litterkeep-server serve-all-mail-outbound-tls-starttls \
  0.0.0.0:1587 0.0.0.0:1143 0.0.0.0:1110 0.0.0.0:4190 \
  127.0.0.1:2465 peer.test peer-ca.der ./data/g09-live \
  server.der server.key.der
```

Reuse of the connection is allowed after a successful DATA or a confirmed RSET.
An end of connection, a timeout, a TLS error and a refused RSET destroy the
connection. The Linux test additionally drives this route through the permanent
scheduler: a trusted SAN completes the durable delivery, while an invalid SAN
leaves a restart-safe job in the `Deferred` state instead of losing the message
or reusing the TLS connection.

### RFC 5321 phase timeouts

Outgoing SMTP with TLS uses the RFC 5321 phase timeouts. They are configured
with positive integer seconds at process start:

| Variable | Phase | Default |
|---|---|---:|
| `LITTERKEEP_SMTP_GREETING_TIMEOUT_SECONDS` | greeting | 300 |
| `LITTERKEEP_SMTP_COMMAND_TIMEOUT_SECONDS` | ordinary command, MAIL, RCPT | 300 |
| `LITTERKEEP_SMTP_DATA_START_TIMEOUT_SECONDS` | DATA response | 120 |
| `LITTERKEEP_SMTP_DATA_BLOCK_TIMEOUT_SECONDS` | each system record of a DATA block | 180 |
| `LITTERKEEP_SMTP_DATA_FINAL_TIMEOUT_SECONDS` | final response after the terminator | 600 |

```sh
LITTERKEEP_SMTP_GREETING_TIMEOUT_SECONDS=300
LITTERKEEP_SMTP_COMMAND_TIMEOUT_SECONDS=300
LITTERKEEP_SMTP_DATA_START_TIMEOUT_SECONDS=120
LITTERKEEP_SMTP_DATA_BLOCK_TIMEOUT_SECONDS=180
LITTERKEEP_SMTP_DATA_FINAL_TIMEOUT_SECONDS=600
```

Unset values fall back to the RFC defaults. An invalid or zero timeout is
rejected when the outgoing worker starts. An I/O timeout destroys the dirty
connection, the durable job moves into the retry policy, and the connection is
not returned to the pool.

### Route restrictions

A route accepts only an explicit numeric `IP:port`. This excludes hidden
blocking name resolution from the delivery worker. Establishing the connection
is limited to five seconds.

The first fixed route profile allows a Sieve redirect only into the typed
domain `peer.test`. The address is checked when the delivery plan is built and
again in the delivery worker before the socket is opened. A forbidden route
applies an implicit keep and writes a Sieve diagnostic, and a corrupted queue
job receives the terminal code `5.7.1` instead of meaningless retries. After
eight `Received` fields in the trace, a new redirect is also replaced by an
implicit keep, which bounds the server-to-server loop.

### One bounded queue pass

```sh
litterkeep-server queue-drain ./data/g09-service-blackbox 127.0.0.1:3555 1
```

The command owns one bounded Store worker and one bounded outgoing SMTP
worker. It records `InFlight` before the SMTP attempt, waits on the worker wake
sockets, writes the terminal durable state and publishes a local DSN after a
permanent failure.

The source server must be stopped first: the Store intentionally permits only
one writer. This is an explicit bounded pass for diagnostics and recovery.
Normal service delivery uses `serve-all-mail-outbound-starttls`, whose
scheduler shares the Store owner and the wake-up routing with the incoming
protocols. Every Store request, delivery job and idle timer is represented by a
real versioned wait in the shared orchestrator and is visible through
`runtime tasks` and `runtime stats`.

## Maintenance and diagnostics

### The control socket

```sh
litterkeep-server runtime stats --data-dir /data
litterkeep-server runtime tasks --data-dir /data
litterkeep-server runtime resources --data-dir /data
litterkeep-server runtime inspect 2:1 --data-dir /data
litterkeep-server runtime cancel 2:1 --data-dir /data
```

The server owns a local Unix-domain socket in its poll loop and returns only
lifecycle metadata: task state, generation-protected identifiers and held
resources. Message bodies, passwords and TLS keys are never returned. The
socket lives in the process-local temporary directory, and an owner-only
locator `/data/runtime.control` names it. This avoids an unbounded `bind(2)`
stall on shared Docker Desktop folders. The command validates that the locator
still lives inside the temporary directory before it connects.

The `cancel` command moves a task into cooperative cancellation. For a client
session the reactor closes the socket that actually belongs to it through a
single cleanup path. An idle outgoing scheduler finishes immediately, while an
active delivery keeps ownership until the current SMTP operation and its
durable transition complete, then closes the worker channel, performs the
`join` and removes the lifecycle metadata. An active SMTP socket is not
interrupted by `cancel`: the wait is bounded by the timeout of the current
phase, which can be up to 600 seconds. The command never releases a foreign or
still-in-use resource in advance.

### Integrity check

```sh
litterkeep-server check --data-dir ./data
```

The offline check takes the exclusive Store lock, performs the standard
recovery of unfinished records and reads every message blob referenced by a
mailbox or by a pending outgoing delivery. Orphan blobs are only counted and
are never deleted.

### Backup

```sh
litterkeep-server backup --data-dir ./data --output ./data-backup
litterkeep-server check --data-dir ./data-backup
```

A consistent backup is only allowed when the Store owner is stopped. The
destination directory must not exist and must not be inside the source
directory. The command copies the persistent data, syncs the copy and runs the
checker itself.

### Sieve diagnostics

```sh
litterkeep-server sieve-errors ./data/g08-sieve-diagnostics bob
```

The command is offline: it opens the Store as the single writer and therefore
must not run in parallel with the server. Every line contains the delivery
transaction, the account, the script version that was actually executed and a
typed error code.

If the bounded Sieve executor rejects an already active version during
delivery, the server applies an implicit keep and does not publish partially
accumulated redirects or `fileinto` actions. The diagnostics journal is limited
to 1 MiB; exhausting the quota rejects the transaction before SMTP success
rather than accepting a message without the promised diagnostics.

### Filling the test mailboxes

```sh
litterkeep-server seed 127.0.0.1:1465 127.0.0.1:1993 litterkeep.test \
  ./ca.der alice ./alice.password ./bob.password ./qa.password
```

The first network seed checkpoint uses SMTP submission with implicit TLS and
IMAP `APPEND`. Passwords are read from files and never reach the process
arguments. A repeated run looks only for its own `X-Litterkeep-Fixture` headers
in the target folders, so it does not double the fixtures and does not remove
manual mail.

The expected composition and the SHA-256 of the decoded attachments are recorded
in `tests/fixtures/seed-manifest.tsv`. The full set contains 35 messages and
includes text, Unicode, emoji, replies, `message/rfc822`, an HTML alternative,
the Sent, Drafts and Archive folders, flags and eight attachments, including
an exact 10 MiB attachment.

## Connecting a mail client

For the local STARTTLS profile, give litterkeep the full account address, for
example `alice@litterkeep.test`, one generated password from
`test-accounts.tsv`, and the following endpoints:

```text
IMAP host: localhost, port: 1143, mode: STARTTLS
SMTP host: localhost, port: 1587, mode: STARTTLS
Trust anchor: ca.pem from the initialised test PKI
```

When connecting a physical Android device over USB, only the necessary
listeners are redirected:

```powershell
adb reverse tcp:1143 tcp:1143
adb reverse tcp:1587 tcp:1587
```

For Docker the host ports may differ. An unknown CA must be rejected first;
then the CA is chosen in the standard litterkeep dialog. The server normalises
a full local address into the account owner but does not accept an external
domain.

The IMAP IDLE output is bounded per connection. If an individual subscriber
stops reading and exceeds that bound, only its session lease is retired; the
reactor and the other mailbox subscribers continue from their own consistent
projections. After reconnecting, that client must issue SELECT again to obtain
a fresh sequence number view.

The actual device status and the interface limitations are recorded in
[docs/client-setup.md](docs/client-setup.md).

## Resource limits

| Limit | Value |
|---|---|
| SMTP DATA and the IMAP response | 25 MiB |
| Immutable blob budget per data directory | 25 MiB |
| Sieve diagnostics journal | 1 MiB |
| Store operation timeout | 5 minutes |

A binary attachment of 10 MiB still fits after MIME base64 encoding. The single
storage writer reconstructs the actual usage at start-up and rejects a new
message before creating its temporary blob when the budget would be exceeded.

The current on-disk marker is `litterkeep-store-v2`. There is no automatic
migration from earlier development formats: an unknown or unmarked data
directory containing mail is rejected.

## Development and verification

```sh
cargo build --locked --release --bin litterkeep-server
cargo test --locked
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo run --locked -- help
cargo tree --locked -e normal
```

The same set runs in CI on every push: `fmt` and `clippy` on ubuntu and
windows, the tests on ubuntu and windows, plus a release build.

The black-box checks live in `tests/blackbox/`. They use only the standard
Python clients and require a running server. Docker is exclusively a
verification environment, not a required part of `litterkeep-server`.

Examples of the checks:

```sh
# TLS submission with an attachment
python tests/blackbox/tls_smtp_auth_attachment.py

# POP3: delivery, reading and an empty maildrop, reusing one directory
python tests/blackbox/pop3_starttls_delivery.py --phase smtp --port 11587 --ca ./data/pki/ca.pem
python tests/blackbox/pop3_starttls_delivery.py --phase pop3 --port 11110 --ca ./data/pki/ca.pem
python tests/blackbox/pop3_starttls_delivery.py --phase empty --port 11110 --ca ./data/pki/ca.pem

# ManageSieve: activating fileinto and delivering through to IMAP
python tests/blackbox/managesieve_literal_delivery.py \
  --smtp-port 1587 --imap-port 1143 --managesieve-port 4190 --ca ./data/pki/ca.pem
```

The Linux checks keep stage artefacts under `./data`; when bind I/O on Docker
Desktop is unavailable, the same stage uses the container-local `/data` and
records that limitation explicitly. The shared test PKI lives in `./data/pki`.
All of these local artefacts are ignored by Git and excluded from the Docker
build context. They are regenerable: `init` recreates the acceptance accounts
and `make_test_pki` recreates the test PKI, so `data/`, `runtime/`,
`runtime-smoke-*/` and the verification PKI directories can be deleted between
runs.

## Project layout

```text
src/            server source, one module per profile
  platform/     the only unsafe boundaries: poll(2) and flock(2)
  reactor/      the Linux poll reactor, sessions, the outgoing delivery scheduler
  storage_worker/  the single Store owner
  store/        filesystem Store, journals, recovery
  smtp/ imap/ pop3/ managesieve/  protocol parsers and state machines
  sasl/         SCRAM, Argon2id, the project's own SHA-1/SHA-256/HMAC/PBKDF2
  mime/         MIME, charset decoding, message structure
  sieve/        the Sieve parser and the bounded executor
docs/           project documentation, start at docs/README.md
docs/ru/        the same documentation in Russian
tests/          unit, integration and black-box checks
tools/          generators for the Unicode and charset tables
docker/         the Linux verification image
examples/       make_test_pki and helper tools
```

## Documentation

| Document | Contents |
|---|---|
| [STATUS.md](STATUS.md) | Readiness summary: what is done, what is partial, what is planned |
| [docs/README.md](docs/README.md) | Documentation navigation and reading order |
| [docs/ru/README.md](docs/ru/README.md) | The same navigation in Russian |
| [docs/architecture.md](docs/architecture.md) | Process, resource ownership, durability, platform limits |
| [docs/decisions.md](docs/decisions.md) | Architectural decisions and their cost |
| [docs/runtime-lifecycle.md](docs/runtime-lifecycle.md) | Tasks, waiting, wake-up, cancellation, observability |
| [docs/storage-format.md](docs/storage-format.md) | Storage format and recovery |
| [docs/security.md](docs/security.md) | Security profile |
| [docs/dependencies.md](docs/dependencies.md) | Production dependencies, the allowlist, the project's own primitives |
| [docs/unicode-data.md](docs/unicode-data.md) | Pinned Unicode data version |
| [docs/client-setup.md](docs/client-setup.md) | Connecting a mail client |
| [docs/protocol-coverage.md](docs/protocol-coverage.md) | Detailed protocol coverage |
| [docs/imap-profile.md](docs/imap-profile.md) | IMAP4rev1 and IMAP4rev2 matrix with statuses |
| [docs/sieve-coverage.md](docs/sieve-coverage.md) | Sieve and ManageSieve profile |
| [docs/sieve-extension-audit.md](docs/sieve-extension-audit.md) | Inventory of Sieve extensions |
| [docs/acceptance.md](docs/acceptance.md) | Final acceptance per task |
| [docs/acceptance.tsv](docs/acceptance.tsv) | Machine-readable acceptance matrix |
| [GOAL.md](GOAL.md) | Chronological development journal per stage |

## Licence

[MIT](LICENSE). The limits of the current profile and the way to report
vulnerabilities are described in [SECURITY.md](SECURITY.md).
