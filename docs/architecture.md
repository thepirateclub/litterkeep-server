# litterkeep-server architecture

> **About this document.** How the process is built: resource ownership, write durability, protocols and identity, platform limits.
>
> Documentation navigation — [docs/README.md](README.md), project status — [STATUS.md](../STATUS.md).

## Process and ownership

The server is a single process. Incoming protocols are served by a Linux reactor
on `poll(2)`, which also owns the TCP sockets and the session state. A session
has a slot and a generation, so a completion of a closed or reused session can
never reach a new client.

The reactor does not open the Store and performs no file input/output. It places
bounded `Request` values on the worker queue and waits on the wake socket. Write
interest is requested only when there is output. A single worker owns the Store
and performs commits and reads sequentially, so the single writer is a
structural requirement rather than a convention.

Four protocols are brought up in one process — SMTP submission, IMAP, POP3 and
ManageSieve — and all of them share one Store owner. This means the Sieve
script catalog, SMTP delivery and IMAP visibility of a message are always
consistent: there is no window in which a script has been applied to a delivery
but the message is not yet visible in a mailbox.

Outgoing delivery is split into a separate bounded worker with its own
connection pool. Pools are not mixed across route types: plaintext and TLS
routes are pinned to different pools so that a TLS key can never leak into a
plaintext channel.

A protocol session is a resource of the orchestrator: `ResourceLease<Session>`
holds the real socket and state, while the registry stores only verifiable
metadata. A resource value is never copied on transfer and never appears in
control diagnostics.

For details on tasks, waiting and cancellation, see
[runtime-lifecycle.md](runtime-lifecycle.md).

## Protocols and identity

Wire commands are decoded into typed enums at the parser boundary. TLS is
implemented on `rustls`: the certificate and key are given as DER files, and the
server configuration is assembled and validated before the listener opens, so a
wrong key or certificate is rejected before the socket accepts its first
connection.

Both implicit TLS and STARTTLS are supported. In implicit TLS mode the
handshake completes before any protocol greeting is sent, so credentials never
travel in clear text. STARTTLS and STLS are not advertised inside an already
protected channel.

`Accounts` stores Argon2id PHC records. The worker performs the password
check, and the reactor changes the session state only after the corresponding
completion. Mailbox read operations are allowed only after that state change:
before authentication the reactor does not place any request on the Store.

The owner of a mailbox, a Sieve script and any message operation is taken from
the authenticated session, never from an arbitrary owner ID supplied by the
client. A full external-domain address never becomes a credential path or a
Store owner.

Protocol-specific profiles: [protocol-coverage.md](protocol-coverage.md),
[imap-profile.md](imap-profile.md), [sieve-coverage.md](sieve-coverage.md).

## Durability

A blob is first written to a temporary file and synced. It is then published
atomically, after which the checksummed journal frame is synced. An incomplete
journal tail is discarded during recovery. A fully corrupted record and a
missing referenced blob are errors. An orphan blob is never delivered.

Mailbox links, flags, folders, subscriptions, Recent, Sieve, diagnostics, the
queue and global transaction commits use framed append-only journals. A mailbox
link becomes visible only after the global commit, so a reader never sees half
of a transaction.

A UID is monotonic within a mailbox and is never reused after EXPUNGE. COPY
shares an immutable blob, MOVE publishes the destination and the source tombstone
in one transaction. A pending outbound delivery stays the owner of the blob until
the terminal transition, so a restart loses neither the message nor its blob.

The root of the data directory is marked `litterkeep-store-v2`. An unknown or
unmarked directory with mail data is rejected: there is no automatic guessing of
an older format.

For the format details, see [storage-format.md](storage-format.md).

## The owner lock

On Linux the single owner additionally holds an advisory `flock(2)` on
`writer.lock`. The file itself survives a restart, and the kernel lock is
released when the process ends, so a leftover lock file after a crash does not
block a new owner.

On platforms without `flock` an exclusive lock marker is used: the file is
created on acquisition and removed on release. This is a portable implementation
without a confirmed crash-safe guarantee: after a crash the marker may remain
and require manual removal.

## Outgoing delivery

An outgoing delivery job is published by the same durable Store transaction as
every other record and becomes visible in the queue only after the global
commit ID. Before an SMTP attempt the job must receive a durable transition to
`InFlight`; recovery moves an unfinished `InFlight` into `Deferred`.

On Linux a single-flight long-lived scheduler runs inside the shared
`server_loop`. It shares the Store owner and the wake-up routing with the
incoming protocols, so every wait is represented by a real versioned record in
the shared orchestrator and is visible through `runtime tasks` and
`runtime stats`.

The bounded `queue-drain` remains available for diagnostics and recovery. It
does not replace the scheduler and must not run in parallel with the server:
the Store intentionally permits only one writer.

A route accepts only an explicit numeric `IP:port`, which excludes hidden
blocking DNS from the transport worker. The TCP connect is limited to five
seconds, and the remaining phases use RFC 5321 timeouts.

## Platform limits

The reactor relies on the Linux `poll(2)`, so `serve` and the other reactor
commands do not run on Windows and exit with an explicit message. A Windows
host is nevertheless fully used for building, `fmt`, `clippy` and tests: the
whole verification suite runs there.

Production unsafe boundaries are limited to two Linux FFI modules: the `poll(2)`
call in `src/platform/linux_poll.rs` and the `flock(2)` call in
`src/platform/linux_lock.rs`. The rest of the crate forbids unsafe through
`#![deny(unsafe_code)]`. The rationale is in [decisions.md](decisions.md).

## Resource limits

| Limit | Value |
|---|---|
| SMTP DATA and IMAP response | 25 MiB |
| Immutable blob budget per data directory | 25 MiB |
| ManageSieve response size | bounded by the literal profile |
| Sieve diagnostics journal | 1 MiB |

Exceeding the blob budget is rejected before the temporary file is created, so
a partial blob never appears in the data directory. Exhausting the diagnostics
journal quota rejects the transaction before SMTP success.

## Entry points

| Command | Purpose |
|---|---|
| `init` | Create the local acceptance accounts |
| `init-smtputf8-test` | Create the Unicode fixture for SMTPUTF8 acceptance |
| `serve` | SMTP and IMAP in one reactor |
| `serve-tls` | SMTP and IMAP with implicit TLS |
| `serve-starttls` | SMTP and IMAP with a plaintext greeting and a mandatory upgrade |
| `serve-pop3-starttls` | POP3 with STLS |
| `serve-pop3-tls` | POP3 with implicit TLS |
| `serve-managesieve-starttls` | ManageSieve with STARTTLS |
| `serve-all-starttls` | SMTP, IMAP and POP3 with STARTTLS |
| `serve-all-mail-starttls` | SMTP, IMAP, POP3 and ManageSieve with STARTTLS |
| `serve-all-mail-tls` | SMTP, IMAP, POP3 and ManageSieve with implicit TLS |
| `serve-all-mail-outbound-starttls` | The same plus the outgoing scheduler |
| `serve-all-mail-outbound-tls-starttls` | The same plus an outgoing TLS route with a pinned SAN and CA |
| `queue-drain` | One bounded pass over the outgoing queue |
| `check` | Offline integrity check of the data directory |
| `backup` | Consistent copy with the owner stopped |
| `seed` | Fill the test mailboxes from the manifest |
| `sieve-errors` | Typed diagnostics for Sieve delivery failures |
| `runtime` | `stats`, `tasks`, `resources`, `inspect`, `cancel` |
| `deliver`, `deliver-tls` | Send one prepared RFC822 file through the own transport |

The full list with arguments is in `litterkeep-server help`.

---

[Documentation navigation](README.md) · [Project status](../STATUS.md) · [README](../README.md)
