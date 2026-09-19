# Goal ledger

Root Goal ID: `01a0ba65-3922-7d21-ac75-0963ef79819a`  
Root status: ACTIVE  
Current subgoal: G04

The actually available Goal interface was inspected. It supports creating and
updating one active root goal (`create_goal`, `get_goal`, `update_goal`), but
has no subgoal/dependency operation. The G00-G12 rows below are therefore the
authoritative local subgoal ledger, rather than invented Goal API calls.

| Goal | Depends on | Acceptance criterion | Status |
|---|---|---|---|
| G00 | none | build, help, lock, dependency report, saved goal | PASS |
| G01 | G00 | lifecycle/lease/wake tests and Linux poll integration | NOT RUN |
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

### 2026-09-19: source-boundary enforcement

- Enforced the repository rule that a Rust source file has one responsibility
  and stays within 300 lines. `reactor.rs` now owns the poll loop and session
  resources; `reactor/protocol.rs` owns SMTP/IMAP command state transitions.
  The lease type and orchestrator tests are likewise separate files.
- PASS after this move: format, 15 unit tests, Clippy, Linux target check, and
  the Docker SMTP-to-IMAP attachment checksum test.

### 2026-09-19: G03 crypto dependency and credential checkpoint

- Added only the allowlisted direct dependencies: `rustls 0.23.45` with
  `default-features = false` and the single `ring` provider, plus `argon2
  0.6.0` for Argon2id PHC password records and OS CSPRNG salts. `cargo tree`
  was inspected and the complete graph is locked.
- `auth::Credential` creates and verifies Argon2id PHC records. Its unit test
  proves the correct password is accepted and a different password rejected.
  It is now connected to bounded worker authentication and IMAP session
  completions, but TLS and full admission controls remain incomplete.

### 2026-09-19: IMAP password verification PASS

- `init /runtime` creates the documented fixture account `bob` with password
  `test-password`. Linux Docker `litterkeep-g03` was initialized on the named
  test volume and used for the independent Python test.
- PASS: `imaplib` LOGIN for Bob succeeded during the SMTP-to-IMAP attachment
  SHA-256 test. A second independent `imaplib` call with password `wrong`
  received `NO authentication failed`.
- This checkpoint does not prove TLS, SMTP AUTH, account isolation or bounded
  admission, so G03 remains RUNNING.

### 2026-09-19: Linux writer-lock crash recovery PASS

- Linux storage now keeps a real advisory exclusive `flock(2)` on
  `writer.lock` for the lifetime of the sole `Store` owner. The file itself is
  deliberately retained: the kernel releases the lock when the process exits,
  including an ungraceful termination, while a second live writer is refused.
- PASS in Docker on the named `litterkeep-lock-test` volume: a running server
  was terminated with `docker kill`; `writer.lock` was confirmed to remain;
  another server using the same volume became healthy. The independent Python
  SMTP to IMAP attachment test then passed with SHA-256
  `3434bddb77229b18aab16f5a2894661d44505548389f84c11bce6d6b50efa4a4`.
- Windows retains the portable create-new sentinel fallback. It has not been
  validated as crash-safe and is not claimed to have the Linux guarantee.
- G02 remains RUNNING: durable directory metadata syncing, cross-platform
  exclusive locking, quota and queue recovery acceptance are still missing.

### 2026-09-19: IMAP authorization boundary PASS

- `SELECT`, `EXAMINE` and `FETCH` now obtain the mailbox only from the account
  recorded after a successful worker completion. Before `LOGIN` the reactor
  returns tagged `NO authenticate first` and does not enqueue a storage read.
- PASS in the current Linux Docker server with
  `tests/blackbox/imap_requires_login.py`. This is a raw standard-library TCP
  client, not `imaplib`: the latter refuses to send SELECT while it is in its
  own NONAUTH state and therefore cannot be evidence of the server response.
- The independent SMTP to IMAP MIME attachment checksum regression also passed.
  G03 remains RUNNING because the service is still plaintext and does not yet
  have SMTP AUTH, TLS or admission controls.

### 2026-09-19: TLS material boundary PASS

- `tls::ServerMaterial` читает только DER leaf certificate и PKCS#8 private
  key, затем передаёт их непосредственно разрешённому `rustls 0.23.45` для
  валидации пары. Нет самописной криптографии и нет дополнительного PEM crate.
- PASS: unit-тесты подтверждают, что отсутствующие файлы не маскируются, а
  пустой DER не создаёт `ServerConfig`; полный `cargo test --locked` дал 19
  успешных тестов, Clippy с `-D warnings` прошёл.
- Это не TLS-сетевой результат. `ServerConfig` еще не подключён к
  неблокирующему reactor, поэтому TLS handshake, частичные records, таймауты и
  проверка независимым клиентом остаются NOT RUN.

### 2026-09-19: TLS record boundary PASS

- Добавлена `tls_transport::ServerTls`, владеющая только `rustls`
  `ServerConnection`, а не TCP. Она ограничивает encrypted input 64 KiB,
  накапливает неполный TLS record и вызывает `process_new_packets` только для
  полного record. `WouldBlock` при отсутствии plaintext во время handshake не
  закрывает живую сессию.
- PASS: `partial_client_hello_is_buffered` передаёт ClientHello двумя частями,
  подтверждая, что первая часть не попадает в обработку как повреждённый record.
  Полный набор `cargo test --locked` содержит 20 успешных тестов.
- Reactor пока использует plaintext `TcpStream`; соединение `ServerTls` с его
  lifecycle, poll interest, handshake timeout и внешним TLS-клиентом остается
  NOT RUN.

### 2026-09-19: outbound SMTP dialogue checkpoint

- CLI `deliver` читает RFC822-файл и вызывает собственный исходящий SMTP
  transport. PASS: Windows-клиент `cargo run --locked -- deliver` передал
  `tests/fixtures/outbound.eml` работающему Linux Docker серверу на порту 2525;
  после этого независимый SMTP to IMAP MIME-тест снова прошел.
- Неудачная попытка запустить клиент вторым Docker-контейнером через Docker
  Desktop network namespace зависла и не считается приемкой Linux-клиента.
  TLS, durable queue integration, retry и pool для G09 остаются NOT RUN.
- PASS дополнительно: `tests/blackbox/imap_outbound_fixture.py` через
  независимый Python `imaplib` нашёл Subject fixture, переданного `deliver`, и
  сравнил полученные RFC822 bytes с `tests/fixtures/outbound.eml`.

### 2026-09-19: G04 local-recipient routing checkpoint

- SMTP больше не записывает каждое принятое сообщение в жёстко заданный
  mailbox `bob`. После `RCPT` reactor передаёт local-part в ограниченный
  `StorageWorker`; только его проверка существующего account добавляет
  получателя в envelope. Неизвестный или неразбираемый локальный адрес
  получает `550` на RCPT, до DATA и до публикации blob.
- После терминатора DATA worker повторно проверяет весь список получателей,
  затем фиксирует письмо в каждом mailbox и только его completion разрешает
  `250 queued`. Unit-тест доказывает, что неизвестный второй получатель не
  даёт частичной записи. Транзакция забирает список RCPT ровно один раз.
- PASS в отдельном чистом Linux Docker volume
  `litterkeep-recipient-routing-test`: собран образ
  `litterkeep-server:recipient-routing`; созданы `bob/test-password` и
  `alice/alice-password`; `python tests/blackbox/smtp_recipient_routing.py
  127.0.0.1 2527 2145` подтвердил доставку только Alice и `550` неизвестному
  получателю. Затем `python tests/blackbox/smtp_imap_attachment.py 127.0.0.1
  2527 2145` снова подтвердил attachment SHA-256
  `3434bddb77229b18aab16f5a2894661d44505548389f84c11bce6d6b50efa4a4`.
- PASS на Windows: `cargo fmt --check`; `cargo test --locked` (24 passed);
  `cargo clippy --locked --all-targets -- -D warnings`. Кросс-проверка target
  `x86_64-unknown-linux-gnu` не выполнена: на host отсутствует
  `x86_64-linux-gnu-gcc`, нужный transitive `ring`; это не заменяет Linux
  Docker build и run, которые прошли.
- G04 остаётся NOT RUN: защищённый submission listener, cleanup оборванной
  DATA с отдельной проверкой, лимит получателей, Received/Return-Path и
  гарантии повторного локального delivery ещё не завершены. G03 также NOT RUN,
  поскольку настоящий TLS и SMTP AUTH не подключены к reactor.

### 2026-09-19: G04 abandoned DATA checkpoint

- Добавлен независимый raw-TCP тест `tests/blackbox/smtp_abandoned_data.py`.
  Он получает ответы на EHLO, MAIL, RCPT и DATA, отправляет незавершённое тело,
  закрывает TCP без терминатора и читает Bob mailbox отдельным `imaplib`.
- PASS в новом чистом Linux Docker volume: тест подтвердил zero messages после
  разрыва, затем маршрут Alice и SMTP-to-IMAP attachment regression снова
  прошли. Разрыв DATA очищает только session buffer и не публикует blob.

### 2026-09-19: G04 basic SMTP command checkpoint

- SMTP FSM различает HELO и EHLO, отвечает на NOOP и принимает RFC-совместимый
  пустой reverse-path `MAIL FROM:<>`; RCPT всё ещё требует непустой path.
- PASS: `cargo test --locked` (25 passed), Clippy и format; в Linux Docker
  независимый `tests/blackbox/smtp_basic_commands.py` подтвердил HELO, NOOP,
  пустой MAIL, RSET и QUIT. SMTP-to-IMAP attachment regression также PASS.

### 2026-09-19: reactor I/O boundary checkpoint

- Выделен `reactor/session_io.rs`: он владеет только неблокирующими TCP read и
  write шагами session. Poll-loop, protocol FSM и storage completion остались
  отдельными владельцами. Это подготавливает точную границу для TLS transport.
- PASS: `reactor.rs` содержит 229 строк, `session_io.rs` 48 строк; format,
  `cargo test --locked` (25 passed) и Clippy прошли. При чтении больше нет
  прямого индексирования массива для фактически принятой длины.

### 2026-09-19: G04 relay-policy and recipient-bound checkpoint

- Envelope ограничен 100 получателями. Лимит проверяется самой SMTP
  транзакцией и не обходится асинхронной проверкой account у storage worker.
- На SMTP boundary domain преобразуется в `LocalRoute`. Только `test` и
  `litterkeep.test` попадают в local delivery policy; внешний домен получает
  `550` на RCPT до DATA. Это не заменяет будущую queue-маршрутизацию peer.
- PASS: `cargo test --locked` (26 passed), format и Clippy. Linux Docker
  `smtp_recipient_routing.py` подтвердил адрес Alice, отказ неизвестного
  mailbox и отказ `alice@outside.test`. Первый запуск теста намеренно не был
  засчитан: старый volume содержал письмо Alice и строгая проверка `== 1`
  упала. После удаления только созданного тестового volume сценарий PASS.

### 2026-09-19: граница локальной delivery-policy

- `reactor/protocol.rs` снова укладывается в лимит 300 строк: обработка SMTP и
  IMAP команд осталась в нём, а единственная ответственность
  `reactor/local_delivery.rs` теперь состоит в преобразовании SMTP path в
  локальный mailbox по ограниченной policy. `reactor.rs` перестал назначать
  session через прямое индексирование `Vec` и получает переиспользуемый слот
  через `get_mut`.
- PASS: `cargo fmt --check`; `cargo test --locked` (26 passed); `cargo clippy
  --locked --all-targets -- -D warnings`; `git diff --check`.
- Это локальное соблюдение правил исходников, а не подтверждение нового
  протокольного рубежа. G03 и G04 сохраняют статус NOT RUN, так как TLS ещё не
  присоединён к reactor.

### 2026-09-19: implicit-TLS reactor checkpoint

- `serve-tls` запускает implicit-TLS listener-ы с документированными default
  портами 1465/1993. Каждая session владеет отдельным `ServerTls`, plaintext
  protocol output и TLS wire output разделены, а SMTP/IMAP greeting не попадает
  в сеть до завершения handshake. Poll interest учитывает `rustls` wants-read
  и wants-write, поэтому TLS не создаёт поток на соединение и не теряет
  partial I/O.
- `examples/make_test_pki.rs` использует разрешённый только в dev-dependency
  `rcgen 0.14.10`: выпуск создаёт CA, отдельный leaf с DNS/IP SAN и DER-ключом
  для `serve`; повторный запуск не заменяет существующий CA. Первый выпуск
  обнаружил одинаковый default DN у CA и leaf, затем исправлены разные subject
  DN и Authority Key Identifier. Эта исправленная цепочка принята строгим
  клиентом.
- PASS в Linux Docker: образ `litterkeep-server:tls-reactor`, volume
  `litterkeep-tls-reactor-test`, listener 2993. Независимый стандартный Python
  `tls_imap_login.py` доверил `ca.pem`, проверил IP SAN `127.0.0.1` и выполнил
  IMAP LOGIN Bob. `tls_imap_rejections.py` подтвердил отказ неизвестного CA и
  неверного пароля. PASS также: `cargo test --locked` (26 passed), Clippy и
  format.
- G03 остаётся NOT RUN: отсутствуют SMTP AUTH, ограничение времени handshake,
  admission control и STARTTLS; результат доказывает только implicit TLS IMAP
  transport и защищённую проверку действующих IMAP credentials.
