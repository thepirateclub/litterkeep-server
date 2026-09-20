# litterkeep-server

Self-hosted Rust mail server for the local `litterkeep` integration profile.
It owns its protocol parsers, filesystem store and task/resource lifecycle.

Current checkpoint: TLS and STARTTLS SMTP submission authenticate accounts with
SASL PLAIN and deliver into the worker-owned filesystem store. IMAP supports
STARTTLS LOGIN and AUTHENTICATE PLAIN with SASL-IR, durable APPEND to INBOX,
SELECT/EXAMINE metadata,
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
Sieve language and wider failure coverage remain unfinished. Linux also has a
single-flight long-lived outbound scheduler in the shared `server_loop`; the
older bounded `queue-drain` remains available for diagnostics and recovery.

## Developer commands

```powershell
cargo build --locked --release --bin litterkeep-server
cargo test --locked
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo run --locked -- help
cargo tree --locked -e normal
```

## Linux outbound queue drain

```sh
litterkeep-server queue-drain ./data/g09-service-blackbox 127.0.0.1:3555 1
```

The command owns one bounded Store worker and one bounded outbound SMTP worker.
It persists `InFlight` before SMTP, waits for worker wake sockets, writes the
terminal durable state, and publishes a local DSN after a permanent failure.
The source server must be stopped first because the Store intentionally permits
only one writer. This is an explicit bounded drain. Normal service delivery
uses `serve-all-mail-outbound-starttls`, whose scheduler shares the Store owner
and reactor wakeup routing with the incoming protocols. Every Store request,
transport job and idle timer is represented by a real versioned wait in the
shared lifecycle orchestrator and is visible through `runtime tasks`/`stats`.

```sh
litterkeep-server serve-all-mail-outbound-starttls \
  0.0.0.0:1587 0.0.0.0:1143 0.0.0.0:1110 0.0.0.0:4190 \
  127.0.0.1:2526 ./data/g09-live server.der server.key.der
```

Outbound route принимает только явный числовой `IP:port`. Это исключает
скрытый blocking DNS из transport worker-а. TCP connect ограничен пятью
секундами, а каждая операция чтения и записи SMTP/TLS ограничена десятью
секундами. I/O timeout уничтожает грязное соединение, durable job переводится
в retry policy и такой channel никогда не возвращается в pool.

Первый фиксированный route-профиль разрешает Sieve redirect только в typed
домен `peer.test`. Адрес проверяется при построении DeliveryPlan и повторно в
transport worker до открытия socket-а. Запрещённый route применяет implicit
keep и пишет Sieve diagnostic; повреждённый queue job получает terminal
`5.7.1`, а не бессмысленные retry. После восьми `Received` trace fields новый
redirect также заменяется implicit keep, что ограничивает межсерверную петлю.

Для постоянного implicit-TLS маршрута используется отдельная typed команда:

```sh
litterkeep-server serve-all-mail-outbound-tls-starttls \
  0.0.0.0:1587 0.0.0.0:1143 0.0.0.0:1110 0.0.0.0:4190 \
  127.0.0.1:2465 peer.test peer-ca.der ./data/g09-live \
  server.der server.key.der
```

TLS route key не смешивается с plaintext: address, проверяемое имя и CA
закрепляются за отдельным pool. Reuse разрешён после successful DATA или
подтверждённого RSET; EOF, timeout, TLS error и отказ RSET уничтожают channel.
Linux test target дополнительно запускает эту route через постоянный scheduler:
trusted SAN завершает durable delivery, а неверный SAN оставляет restart-safe
`Deferred` job вместо потери письма или повторного использования TLS channel.

## Linux runtime control socket

```sh
litterkeep-server runtime stats --data-dir /data
litterkeep-server runtime tasks --data-dir /data
litterkeep-server runtime resources --data-dir /data
litterkeep-server runtime inspect 2:1 --data-dir /data
litterkeep-server runtime cancel 2:1 --data-dir /data
```

Offline-проверка берёт эксклюзивную блокировку Store, выполняет штатное
восстановление незавершённых записей и читает каждый blob, на который ссылается
mailbox или ожидающая исходящая доставка. Orphan blobs только подсчитываются и
не удаляются:

```text
litterkeep-server check --data-dir ./data
```

The server owns a local Unix-domain control socket in its poll loop and returns
only lifecycle metadata: task state, generation-protected IDs and held resource
IDs. It never returns mail body bytes, passwords or TLS keys. The socket lives
under the process-local temporary directory; an owner-only
`/data/runtime.control` locator names it. This avoids an unbounded Unix
`bind(2)` stall on Docker Desktop shared folders. The CLI validates that the
locator remains inside the temporary directory before connecting.
`cancel` переводит task в cooperative cancellation. Для клиентской сессии
reactor закрывает фактически принадлежащий ей socket lease через единый путь
cleanup. Outbound scheduler в idle завершается сразу, а при активной доставке
сначала фиксирует её terminal state, затем закрывает канал worker-а, выполняет
`join` и удаляет lifecycle metadata. Команда не освобождает чужой или ещё
используемый ресурс авансом.

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

The full SMTP DATA and IMAP response limit is 25 MiB, so a binary 10 MiB
attachment still fits after MIME base64 encoding. The current immutable-message
blob budget is also 25 MiB per data directory. The single storage writer
reconstructs actual usage at startup and rejects a new message before creating
its temporary blob when the budget would be exceeded.
The current on-disk marker is `litterkeep-store-v2`. There is no automatic
migration from earlier development formats; an unknown or unversioned data
directory with mail data is rejected.

The repository includes `tests/blackbox/tls_smtp_auth_attachment.py`, which
uses only Python's standard SMTP, IMAP and MIME clients. It was run successfully
against the Linux Docker execution image on 2026-09-19 and compared the decoded
attachment bytes. Docker is a test environment only, not a required part of
litterkeep-server.

Linux checks keep stage artifacts under `./data`; when Docker Desktop bind I/O
is unavailable, the same stage uses container-local `/data` and records that
limitation explicitly. The shared test PKI is kept in `./data/pki`. These local
artifacts are ignored by Git and
excluded from the Docker build context.

IMAP IDLE output is bounded per connection. If an individual subscriber stops
reading and exceeds that bound, only its session lease is retired; the reactor
and other mailbox subscribers continue from their own coherent projections.
After reconnecting, that client must issue SELECT again to obtain a fresh
sequence-number view.

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

Если bounded Sieve executor отклоняет уже активную версию во время доставки,
сервер применяет implicit keep и не публикует частично накопленные redirect или
fileinto actions. Committed ошибка доступна владельцу в стабильном TSV-профиле:

```sh
litterkeep-server sieve-errors ./data/g08-sieve-diagnostics bob
```

Команда offline: она открывает Store как единственный writer и потому не должна
запускаться параллельно с сервером. Каждая строка содержит delivery transaction,
account, фактически исполненную версию скрипта и типизированный код ошибки.
Diagnostic journal ограничен 1 MiB; исчерпание квоты отклоняет transaction до
SMTP success, а не принимает письмо без обещанной диагностики.

The only runtime binary is `target/release/litterkeep-server`. Current Docker
test state is placed under ignored `data/` stage directories.

See `GOAL.md` for actual stage status and `litterkeep-rust-terra-goal.md` for
the requested project contract.

Architecture and ownership boundaries are documented in
[`docs/architecture.md`](docs/architecture.md).
