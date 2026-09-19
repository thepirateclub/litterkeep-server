# Goal ledger

Root Goal ID: `01a0ba65-3922-7d21-ac75-0963ef79819a`  
Root status: ACTIVE  
Current subgoal: G01

The actually available Goal interface was inspected. It supports creating and
updating one active root goal (`create_goal`, `get_goal`, `update_goal`), but
has no subgoal/dependency operation. The G00-G12 rows below are therefore the
authoritative local subgoal ledger, rather than invented Goal API calls.

| Goal | Depends on | Acceptance criterion | Status |
|---|---|---|---|
| G00 | none | build, help, lock, dependency report, saved goal | PASS |
| G01 | G00 | lifecycle/lease/wake tests and Linux poll integration | RUNNING |
| G02 | G01 | durable store, journal, blob, recovery | NOT RUN |
| G03 | G02 | TLS, AUTH, bounded admission | NOT RUN |
| G04 | G03 | durable SMTP submission and local delivery | NOT RUN |
| G05 | G04 | MIME + IMAP, independently checked attachment | NOT RUN |
| G06 | G05 | IMAP folders, flags, UID, IDLE | NOT RUN |
| G07 | G02 | POP3 backed by the shared store | NOT RUN |
| G08 | G02 | Sieve and ManageSieve delivery effect | NOT RUN |
| G09 | G04 | outbound SMTP, pool, retry/DSN | NOT RUN |
| G10 | G06,G07,G08,G09 | failure matrix, replay, resource plateaus | NOT RUN |
| G11 | G10 | offline check/backup and repeatable seed | NOT RUN |
| G12 | G11 | litterkeep and final acceptance report | NOT RUN |

## Checkpoints

### 2026-09-19: G00 PASS, G01 running

- Added a single Cargo package with no production dependencies. This preserves
  the allowlist while G00/G01 need neither TLS nor password hashing.
- Added the first real lifecycle core, not a telemetry registry: exclusive
  values leave the registry in a non-cloneable `ResourceLease`; release checks
  the current owner; cancellation does not return a live lease; parking uses
  event versions to prevent a lost wakeup.
- PASS: `cargo generate-lockfile`; `cargo fmt --check`; `cargo test --locked`
  (3 passed); `cargo build --locked --release --bin litterkeep-server`;
  `cargo clippy --locked --all-targets -- -D warnings`; `cargo run --locked -- help`;
  `cargo tree --locked -e normal` (only `litterkeep-server`).
- G01 is deliberately still RUNNING. The portable ownership and wake tests pass,
  but Linux `poll(2)` and an actual socket wakeup test are not written yet.
- G02 started: `store::Store` writes each blob to a new temporary path, calls
  `sync_all`, atomically publishes it, then appends and syncs mailbox metadata.
  The restart read test is PASS; writer locking, recovery and queue work remain.
- Repository convention: `mod.rs` is prohibited. The runtime module uses
  `src/runtime.rs` with an explicit path for its implementation file. The
  package now requires the installed Rust 1.97 toolchain and Edition 2024.
- Independent SMTP smoke: PASS on `127.0.0.1:2529` with standard Python
  `smtplib`. It stored `bob-1.eml` (37 bytes) and `bob.journal` (12 bytes).
  The test first exposed case-sensitive envelope prefixes and rejection of the
  standard `SIZE` parameter; both were fixed in our parser before the PASS.
- G01 now contains one Linux-only `serve` loop: it binds SMTP and IMAP in the
  same process, uses the owned `poll(2)` boundary, limits accepting and reading
  work per wake, and asks for writable events only while output is buffered.
  Session slots have generations. Storage request completions are routed by the
  pair, so completion after disconnect or slot reuse is ignored safely.
- Removed the former `serve-smtp` and `serve-imap` blocking commands and their
  independent `Store::open` calls. The binary now exposes one service command,
  so a Windows invocation fails explicitly instead of accidentally bypassing
  lifecycle ownership.
- The reactor does not call `Store`: a single bounded `StorageWorker` owns it.
  Requests are retried only after a completion wake, not by sleeps or a busy
  loop. A full wake socket is harmless because the response channel is the
  completion authority.
- PASS on the installed Windows toolchain: `cargo fmt --check`; `cargo test
  --locked` (15 passed); `cargo clippy --locked --all-targets -- -D warnings`;
  `cargo build --locked --release --bin litterkeep-server`.
- PASS: `cargo check --locked --target x86_64-unknown-linux-gnu` after adding
  the matching standard library target. This caught and fixed two Linux-only
  compile errors that were hidden by the Windows `cfg` path.
- NOT RUN: the Linux reactor itself and all G01 network acceptance cases. This
  host runs Windows, so cross-checking proves type checking only, not runtime
  socket behavior. `serve` reports that limitation on Windows instead of
  selecting the old blocking listeners.

### 2026-09-19: first Linux SMTP to IMAP path PASS

- A Linux/amd64 Docker execution image is available in
  `docker/Dockerfile.linux-test`. It is a verification environment, not a
  production dependency or part of the server topology.
- PASS in the running `litterkeep-g01` container, published as SMTP 2525 and
  IMAP 2143: `python tests/blackbox/smtp_imap_attachment.py 127.0.0.1 2525
  2143`. The independent standard-library `smtplib` client submitted a MIME
  message; the single store worker wrote `bob-1.eml` (749 bytes); independent
  `imaplib` fetched it; Python MIME decoding compared attachment bytes.
  Actual SHA-256: `3434bddb77229b18aab16f5a2894661d44505548389f84c11bce6d6b50efa4a4`.
- The client exposed two real IMAP interoperability omissions, `CAPABILITY`
  and read-only `EXAMINE`; both were implemented and retested. This proves the
  first local plaintext path only. It does not prove TLS, authentication
  security, account isolation, full IMAP, POP3, Sieve, outgoing SMTP, or the
  remaining G01 concurrency and cancellation matrix.

### 2026-09-19: G02 recovery frame checkpoint

- Store internals are now separated by responsibility: `model`, `journal`,
  `blobs`, `recovery`, and the owner-only facade. No `mod.rs` was introduced.
- New journal frames contain a version, UID, canonical blob name, and checksum.
  Recovery deletes only temporary blobs and trims only a non-terminated final
  journal tail. A complete corrupt frame remains on disk and produces an
  `InvalidData` error; an orphan blob is not delivered; a referenced missing
  blob is an error. Unit tests cover each case.
- PASS: `cargo fmt --check`; `cargo test --locked` (15 passed); `cargo clippy
  --locked --all-targets -- -D warnings`.
