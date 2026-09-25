# Goal ledger

Root Goal ID: `01a0ba65-3922-7d21-ac75-0963ef79819a`
Root status: ACTIVE  
Current subgoal: G17  

### 2026-09-25: G17 outbound SMTP 421 channel closure

- RFC 5321 §4.2.2 `421` теперь сохраняет результат `Deferred`, но отдельно
  помечает соединение как закрываемое. Обычный и implicit-TLS pool не посылают
  `RSET` в закрывающийся канал и не возвращают его в idle pool; следующая задача
  устанавливает новое соединение. Общее чтение SMTP reply и классификация кодов
  выделены в отдельный профильный модуль, чтобы исходный файл оставался в лимите
  300 строк.
- PASS: Windows Rust 1.97.1: 473 библиотечных + 1 бинарный тест, strict Clippy,
  fmt и diff check. Linux Docker Rust 1.97: 514 библиотечных + 1 бинарный тест,
  strict Clippy и doc-tests. Независимые plaintext/TLS SMTP test peers выдали
  421 и подтвердили отсутствие RSET и повторное подключение. Временный образ
  удалён; `litterkeep-g12-phone` оставлен. Данные этапа: `./data/g17-smtp-421-channel-close`.
  Litterkeep-клиент не проверялся; G17 остаётся RUNNING.

### 2026-09-25: G17 outbound SMTP reply phase semantics

- Reply success now follows SMTP command phase instead of treating every 2yz
  and 3yz as generic success. `354` is accepted only for DATA; greeting, QUIT
  and AUTH use their phase-specific success replies. MAIL/RCPT/final DATA/RSET
  accept positive completion codes while unexpected 3yz fails closed. Existing
  4yz deferred and 5yz permanent mapping remains. Multiline reply status codes
  must remain identical across all lines.
- PASS: Windows Rust 1.97.1 suite, 470 library + 1 binary test, strict Clippy,
  fmt, Python compile and diff check. Linux Docker Rust 1.97 test target, strict
  Clippy, 511 library + 1 binary test and doc-tests. Independent runtime peer
  returned 354 and 221 to MAIL and confirmed no RCPT followed; prior outbound
  SIZE, 8BITMIME and reply-boundary cases passed in the same run. Fixtures:
  `./data/g17-smtp-reply-semantics`. Temporary images removed; only pre-existing
  `litterkeep-g12-phone` remains. Litterkeep client not tested; G17 remains
  RUNNING.

### 2026-09-25: G17 outbound SMTP SIZE declaration

- Outbound ESMTP now parses the RFC 1870 `SIZE` keyword, optional numeric
  maximum and zero/unlimited form. When advertised it sends exact message
  octets in `MAIL FROM SIZE=`, including the terminal CRLF added to DATA but
  excluding dot-stuffing and the SMTP terminator. An advertised fixed maximum
  smaller than the message yields permanent failure before MAIL. Plain and
  implicit-TLS paths share the command builder; TLS regression combines SIZE
  with 8BITMIME.
- PASS: Windows Rust 1.97.1 suite, 468 library + 1 binary test, strict Clippy,
  fmt, Python compile and diff check. Linux Docker Rust 1.97 test target, strict
  Clippy, 509 library + 1 binary test and doc-tests. Independent runtime peer
  verifies declaration, omitted SIZE when unavailable, maximum rejection and
  existing 8BITMIME/reply-limit cases. Fixture:
  `./data/g17-smtp-size-outbound`. Litterkeep client not tested; G17 remains
  RUNNING.

### 2026-09-25: G17 outbound SMTP reply-line bounds

- Outbound SMTP reads greeting, command replies and multiline EHLO lines through
  one bounded reader. It accepts at most 512 octets including CRLF and rejects
  incomplete CRLF or oversized lines without growing its line buffer beyond the
  RFC limit.
- Unit tests cover exact 512, 513 and malformed LF. The independent SMTP peer
  acceptance covers 512-octet EHLO acceptance and pre-MAIL rejection of
  oversized greeting and EHLO responses. Existing 8BITMIME cases reran too.
- PASS: Windows Rust 1.97.1, 461 library + 1 binary test, strict Clippy, fmt,
  Python compile and diff check. Linux Docker Rust 1.97 test target: strict
  Clippy, 502 library + 1 binary test and doc-tests. Independent SMTP acceptance
  passed. Files remain under 300 lines; no numeric direct-index matches in the
  changed protocol/test files. Temporary images and test containers removed;
  only pre-existing `litterkeep-g12-phone` remains. Fixtures are in
  `./data/g17-smtp-reply-limit`. Litterkeep client not tested; G17 remains
  RUNNING.

### 2026-09-25: G17 outbound SMTP 8BITMIME negotiation

- Outbound transport now reads the complete multiline EHLO response into typed
  extension capabilities. A message with high-bit MIME body octets uses
  `MAIL ... BODY=8BITMIME` only when the peer advertised the extension; otherwise
  delivery is a permanent failure before MAIL. High-bit headers are rejected
  because they require SMTPUTF8, which remains unsupported.
- Common ordinary and implicit-TLS paths share this profile. Added unit checks
  for case-insensitive multiline EHLO parsing and body/header octet distinction.
  Independent `tests/blackbox/smtp_outbound_8bitmime.py` verifies exact UTF-8
  MIME 8bit payload bytes and refusal without an advertised extension.
- PASS: Windows Rust 1.97.1 suite, 458 library + 1 binary test, strict Clippy,
  fmt and diff check. Linux Docker Rust 1.97 test target: strict Clippy, 499
  library + 1 binary test and doc-tests. Independent 8BITMIME, DATA line-limit
  and dot-transparency peer acceptance passed. Python compile and line-count
  audit passed. Temporary images removed; only pre-existing
  `litterkeep-g12-phone` remains. Stage fixtures are in
  `./data/g17-smtp-8bitmime-outbound`. Litterkeep client not tested; G17 remains
  RUNNING.

### 2026-09-25: G17 outbound SMTP line profile checkpoint

- RFC 5321 §4.5.3.1.6 requires message text lines at most 1000 octets including
  CRLF, excluding the extra transparency dot. Outbound ordinary and implicit-TLS
  SMTP now validate CRLF framing and line bounds before MAIL; invalid message
  data returns permanent failure without starting an envelope.
- Unit regression covers 1000/1001 octet boundaries and bare CR/LF. New
  `tests/blackbox/smtp_outbound_line_profile.py` independently verifies a valid
  1000-octet line reaches the peer unchanged and a 1001-octet line is rejected
  before MAIL. The separate dot-transparency peer acceptance was rerun too.
- PASS: Linux Docker Rust 1.97 test target, strict Clippy, 494 library + 1
  binary test and doc-tests; both independent SMTP wire tests pass. Windows
  Rust 1.97.1 suite passed with 453 library + 1 binary test, strict Clippy,
  fmt and diff check. Python compile and line-count audit passed. Temporary
  Docker images removed; no stage containers were created and
  `litterkeep-g12-phone` was preserved. Stage fixtures remain under
  `./data/g17-sieve-vacation-qp-encoder-final`. G17 remains RUNNING.

### 2026-09-25: G17 RFC 5321 outbound SMTP transparency checkpoint

- RFC 5321 §4.5.2 требует удваивать точку в начале каждой строки SMTP DATA,
  чтобы содержимое не могло завершить транзакцию раньше времени. Общий
  `outbound::write_data` теперь применяет transparency и передаёт DATA
  terminator отдельно; обычный и TLS transport используют этот путь.
- Добавлены unit regression и независимый
  `tests/blackbox/smtp_outbound_dot_transparency.py`: Linux CLI отправляет
  message fixture на отдельный SMTP peer со строками `.`, `..two` и `.end`;
  peer снимает dot-stuffing и подтверждает точное совпадение octets плюс
  корректный завершающий CRLF.
- PASS: Windows targeted unit test, `cargo fmt --check`, Python compile и diff
  check. Linux Docker Rust 1.97 test target пересобран: strict Clippy, 493
  library + 1 binary test и doc-tests; независимый SMTP peer acceptance PASS.
  Litterkeep-клиент не проверялся. Временные Docker images удалены, контейнеров
  этапа не создавалось; `litterkeep-g12-phone` сохранён. Fixture находится в
  `./data/g17-sieve-vacation-qp-encoder-final`.
- G17 остаётся RUNNING.

### 2026-09-25: G17 outbound quoted-printable vacation wire checkpoint

- Добавлен независимый `tests/blackbox/sieve_vacation_qp_encoder.py`. Через
  ManageSieve задаётся UTF-8 vacation reason с пробелами и TAB; Alice отправляет
  trigger через SMTP STARTTLS, после остановки владельца Store durable queue
  дренируется отдельным Linux процессом на независимый SMTP peer. Peer-пayload
  независимо декодируется и сравнивается byte-for-byte; проверяются `=20`,
  `=09` и строки не длиннее 76 октетов. Litterkeep-клиент не проверялся.
- Сквозной тест обнаружил лишний пустой CRLF на исходящем SMTP DATA. Исправлен
  общий `outbound::write_data`, используемый обычным и implicit-TLS транспортом;
  добавлен regression test на единственный завершающий CRLF.
- PASS: Windows Rust 1.97.1 `cargo test --locked` (450 library + 1 binary),
  strict Clippy, fmt, diff check и Python compile до добавления последнего
  unit regression; затем Docker Linux Rust 1.97 test target пересобран без
  Cargo cache и прошёл Clippy, 492 library + 1 binary и doc-tests. Полный
  independent ManageSieve -> SMTP -> queue -> peer acceptance прошёл на
  runtime image. Stage data сохранена в
  `./data/g17-sieve-vacation-qp-encoder-final`; предыдущий stage после
  неудачных повторов тоже сохранён. Временные Docker containers и оба image
  tags удалены, сохранён ранее существовавший `litterkeep-g12-phone`.
  G17 остаётся RUNNING.

### 2026-09-25: G17 RFC 2045 Content-Transfer-Encoding comments over wire

- Добавлен независимый `tests/blackbox/mime_cte_comments.py`. SMTP STARTTLS
  доставляет Base64 MIME part с вложенным RFC 822 comment перед CTE, escaped
  right parenthesis и trailing comment. IMAP STARTTLS подтверждает
  BODYSTRUCTURE `BASE64`, точные decoded bytes через `BINARY.PEEK[1]` и
  сохранение исходного RFC822 сообщения.
- PASS: Python compile и Linux Docker acceptance на release binary test image
  с bind mount `./data/g17-mime-cte-comments-wire`. Rust source не менялся,
  Docker Cargo layers переиспользованы из cache, Windows suite не
  перезапускался. Проверен независимый клиент, не Litterkeep. Временные
  container/image удалены; остался ранее существовавший
  `litterkeep-g12-phone`. G17 остаётся RUNNING.

### 2026-09-25: G17 RFC 2045 7bit/8bit line-data wire profile

- Добавлен независимый `tests/blackbox/mime_line_data_profile.py`. Через
  IMAP STARTTLS APPEND и BINARY.PEEK проверены 998 против 999 октетов,
  высокие октеты в 8bit и отказ для 7bit, NUL в обоих профилях, bare CR и
  bare LF. Для двух malformed line-ending cases использован raw synchronizing
  literal: выяснилось, что стандартная Python `imaplib.append()` заранее
  канонизирует CR/LF и непригодна для таких fixtures.
- PASS: Python compile и независимый Linux acceptance на release binary из
  Docker test image с bind mount `./data/g17-mime-line-profile-wire`.
  Docker build переиспользовал cached Rust build/test layers; Rust source не
  менялся, Windows Cargo suite не перезапускался. Это IMAP storage/decode
  acceptance, не проверка SMTP DATA для malformed MIME. Временный контейнер и
  image tag удалены; `docker ps -a` оставил только ранее существовавший
  `litterkeep-g12-phone`. G17 остаётся RUNNING.

### 2026-09-25: G17 RFC 2045 quoted-printable padding over wire

- Добавлен независимый `tests/blackbox/mime_quoted_printable_padding.py`.
  SMTP STARTTLS отправляет QP text с буквальными пробелами и TAB на концах
  hard/soft lines, encoded `=20` перед padding и soft break. IMAP STARTTLS
  `BINARY.PEEK[1]` сравнивает все decoded octets с ожидаемым результатом.
  В сетевом тесте подтверждены RFC 2045 §§6.7 padding и soft-break правила;
  encoder whitespace и encoded `=09` остаются unit-only.
- PASS: Python compile и независимый acceptance на Linux release binary из
  Docker test image с bind mount `./data/g17-mime-qp-padding-wire`. Rust
  исходники не менялись, поэтому Rust suite не перезапускался; его последний
  результат относится к неизменённому source. Проверен независимый клиент,
  не Litterkeep. Временный контейнер и image tag удалены;
  `docker ps -a` оставил только ранее существовавший
  `litterkeep-g12-phone`. G17 остаётся RUNNING.

### 2026-09-25: G17 RFC 2045 malformed quoted-printable wire recovery

- Добавлен независимый `tests/blackbox/mime_quoted_printable_recovery.py`.
  SMTP STARTTLS передаёт text/plain QP body с некорректным `=G1` и неполным
  `=A`; IMAP STARTTLS `BINARY.PEEK[1]` подтверждает точные decoded octets,
  сохраняя escape-последовательности. Это проверяет tolerant incoming MIME
  decoder, не строгую Sieve `:mime` validation.
- PASS: Python compile и независимый acceptance на актуальном Linux release
  binary из Docker test image с отдельным bind mount
  `./data/g17-mime-qp-recovery-wire`. Rust исходники не менялись, поэтому
  Windows suite и Linux test-stage не перезапускались; предыдущий Rust suite
  остаётся текущим для неизменённого source. Litterkeep не проверялся.
  Временный контейнер и image tag удалены; `docker ps -a` оставил только
  ранее существовавший `litterkeep-g12-phone`. G17 остаётся RUNNING.

### 2026-09-25: G17 RFC 2045 default Content-Type wire checkpoint

- Добавлен независимый `tests/blackbox/mime_default_content_type.py`: через
  SMTP STARTTLS отправляет headerless RFC 822 сообщение и сообщение с
  синтаксически повреждённым Content-Type. IMAP STARTTLS проверяет
  BODYSTRUCTURE `TEXT/PLAIN`, неизменность исходных байтов и то, что сервер
  не вставил отсутствовавшее поле.
- PASS: Windows Rust 1.97.1 `cargo test --all-targets --locked` (450 library
  + 1 binary), strict Clippy, fmt, diff и Python compile. Linux test image
  собран из текущего checkout; независимый acceptance пройден на актуальном
  release binary с отдельным bind mount
  `./data/g17-mime-default-content-type-byte-proof`.
  Проверен именно локальный независимый клиент, не Litterkeep. Временный
  контейнер и image tag удалены; `docker ps -a` оставил только
  `litterkeep-g12-phone`. Stage data сохранена. Duplicate Content-Type
  rejection и US-ASCII body decoding остаются unit-only. G17 остаётся RUNNING.

### 2026-09-25: G17 unknown MIME CTE wire behavior

- Независимый SMTP STARTTLS -> IMAP STARTTLS сценарий теперь посылает часть с
  исходным `Content-Type: text/plain` и неизвестным CTE. На wire проверены
  effective `APPLICATION/OCTET-STREAM` в BODYSTRUCTURE, сохранение обоих raw
  MIME-заголовков, исключение уникального body token из `SEARCH BODY` и
  `NO [UNKNOWN-CTE]` для BINARY.
- PASS: `imap_binary_fetch.py` прошёл целиком вместе с существующими BINARY,
  Base64-noise, Seen и partial checks. Python compile, Windows Cargo suite
  (450 library + 1 binary), strict Clippy, fmt и diff checks прошли. Linux
  runtime image собран для независимой acceptance проверки; Rust source не
  менялся, поэтому Linux test-stage отдельно не заявляется. Stage data:
  `./data/g17-mime-unknown-cte-wire`. Временные контейнер/image удалены;
  `docker ps -a` оставил только `litterkeep-g12-phone`. G17 остаётся RUNNING.

### 2026-09-25: G17 RFC 2045 Base64 wire-compatibility checkpoint

- Независимый `imap_binary_fetch.py` расширен SMTP-доставкой multipart fixture,
  где `!#` встроены внутрь Base64 encoded attachment. Проверка сравнит точные
  decoded octets через IMAP `BINARY.PEEK[2]`; strict RFC 2047 decoder остаётся
  отдельным и не меняется.
- PASS: Windows Rust 1.97.1 suite 450 library + 1 binary, strict Clippy,
  format/diff и Python compile. Актуальный Linux runtime image собран; полный
  независимый `imap_binary_fetch.py` прошёл, включая SMTP -> IMAP Base64-noise
  round-trip и существующие BINARY acceptance checks. Rust source в этом
  checkpoint не менялся, поэтому новый Linux test-stage не заявляется.
  Временный контейнер и image tag удалены; `docker ps -a` оставил только
  `litterkeep-g12-phone`. Stage data сохранена в `./data/g17-mime-base64-wire`.
  G17 остаётся RUNNING.

### 2026-09-25: G17 strict MIME-Version comment handling

- Повторная RFC-сверка выявила, что предыдущий MIME-Version parser удалял все
  whitespace и мог ошибочно принять `1 . 0` как `1.0`. Обработка comments
  разделена: MIME-Version удаляет RFC 822 comments, сохраняя исходные
  пробельные символы; разрешён `1.(comment)0`, но не вставленный whitespace.
- Добавлены прямые тесты обоих comment-removal режимов и отрицательные
  MIME-Version cases. Независимый ManageSieve STARTTLS CHECKSCRIPT принимает
  `1.0`, `1.(comment)0` и комментарии вокруг версии; отвергает `1 . 0`, `2.0`
  и повторные поля.
- PASS: Windows Rust 1.97.1 `cargo test --all-targets --locked` (450 library
  + 1 binary), strict Clippy, `cargo fmt --all -- --check`, `git diff --check`
  и Python compile. Linux Docker Rust 1.97 `test` target: 491 library + 1
  binary, doc-tests, strict Clippy и release build. Независимый
  `tests/blackbox/managesieve_vacation_mime.py` прошёл на свежем release image.
  Временный runtime container и оба image tags удалены; `docker ps -a` оставил
  только `litterkeep-g12-phone`. Stage data сохранена в
  `./data/g17-vacation-mime-version-comments`. G17 остаётся RUNNING.

### 2026-09-25: G17 Sieve vacation MIME-Version checkpoint

- `vacation :mime` теперь допускает отсутствующий MIME-Version (сервер
  добавляет `1.0`) или единственное значение версии `1.0`. RFC 822 comments и
  whitespace в значении нормализуются; дубликаты, повреждённые и неизвестные
  версии отвергаются до сохранения script. Имя поля разбирается через
  `HeaderName::MimeVersion`, сгенерированный ответ не может объявить версию
  MIME, которую сервер не поддерживает.
- Unit проверки прошли (23 vacation-related); независимый ManageSieve CHECKSCRIPT,
  принял один MIME-Version 1.0 с вложенными comments, отверг 2.0 и duplicate.
- PASS: Windows Rust 1.97.1 `cargo test --all-targets --locked` (446 library
  + 1 binary), strict Clippy, `cargo fmt --all -- --check`, `git diff --check`
  и Python compile. Linux Docker Rust 1.97 `test` target: 487 library + 1
  binary, doc-tests, strict Clippy и release build. Независимый
  `tests/blackbox/managesieve_vacation_mime.py` прошёл через STARTTLS.
  Временный runtime container и test/runtime image tags удалены;
  `docker ps -a` оставил только `litterkeep-g12-phone`. Stage data сохранена в
  `./data/g17-vacation-mime-version`. G17 остаётся RUNNING.

### 2026-09-25: G17 RFC 2557 Content-Location URI-reference checkpoint

- `Content-Location` теперь проецируется только при единственном непустом
  значении с синтаксисом RFC 3986 URI-reference. Реализованы схема, relative
  reference, path/query/fragment, percent-encoding, authority, IPv6 и IPvFuture.
  Octets сохраняются: ссылки не разрешаются и не загружаются; IRI и policy
  разрешённых схем остаются вне профиля.
- Добавлены положительные RFC-примеры и отрицательные malformed URI случаи,
  включая malformed percent encoding, IPv6 authority и некорректную схему.
  Независимый IMAP STARTTLS `imaplib` acceptance подтвердил валидные absolute
  и relative locations в BODYSTRUCTURE, исключение `%ZZ` и отсутствие
  extension fields в basic BODY.
- PASS: Windows Rust 1.97.1 `cargo test --all-targets --locked` (445 library
  + 1 binary), strict Clippy, `cargo fmt --all -- --check`, `git diff --check`
  и Python compile. Linux Docker `test` target: 486 library + 1 binary,
  doc-tests, strict Clippy и release build. Временный контейнер и оба
  checkpoint image удалены; `docker ps -a` оставил только
  `litterkeep-g12-phone`. Stage data сохранена в `./data/g17-content-location-uri`.
  G17 остаётся RUNNING.

### 2026-09-25: G17 RFC 9051 Content-MD5 extension checkpoint

- MIME parser сохраняет только одно syntactically valid `Content-MD5` значение:
  RFC 822 comments и folding удаляются, strict Base64 должен декодироваться
  ровно в 16 октетов. Header digest передаётся как есть и не пересчитывается;
  соответствие digest телу не утверждается.
- RFC 9051 scalar/message BODYSTRUCTURE выводит digest в body-MD5 extension
  slot до disposition; basic BODY не получает extensions, multipart не
  использует scalar digest slot. Independent Python `imaplib` STARTTLS acceptance
  проверяет исходное значение на wire и отсутствие его в BODY.
- PASS: Windows Rust 1.97.1 `cargo test --all-targets --locked` (443 library +
  1 binary), strict Clippy, format/diff checks и Python compile. Linux Docker
  Rust 1.97 test target: release build, 484 library + 1 binary, doc-tests и
  strict Clippy. Независимый STARTTLS `imaplib` acceptance подтвердил значение
  body-MD5 перед disposition в BODYSTRUCTURE и отсутствие extensions в BODY.
  Временные image/container удалены; `litterkeep-g12-phone` сохранён. G17
  остаётся RUNNING.

### 2026-09-25: G17 RFC 5322/6532 Content-ID forms checkpoint

- Расширен parser единственного Content-ID: кроме dot-atom поддерживаются
  quoted id-left, domain-literal id-right и UTF-8 atext согласно RFC 6532.
  Проверяются единственный `@` вне кавычек/literal, баланс ограничителей и
  отсутствие управляющих символов. Obsolete RFC 5322 id forms остаются вне
  принятого подмножества; malformed и повторные поля не проецируются.
- Независимый Python `imaplib` STARTTLS acceptance проверяет quoted id-left и
  IPv6 domain literal в basic полях `BODY`/`BODYSTRUCTURE`. UTF-8 identifier
  формы подтверждаются unit-тестом.
- PASS: Windows Rust 1.97.1 `cargo test --all-targets --locked` (440 library +
  1 binary), strict Clippy, format/diff checks и Python compile. Linux Docker
  Rust 1.97 test target: release build, 481 library + 1 binary, doc-tests и
  strict Clippy; независимый STARTTLS `imaplib` acceptance прошёл. Временные
  image/container удалены; `litterkeep-g12-phone` сохранён. G17 остаётся
  RUNNING.

### 2026-09-25: G17 MIME BODY basic identity fields checkpoint

- MIME entity извлекает единственное корректное `Content-ID` из поддерживаемого
  подмножества `<dot-atom@dot-atom>` и единственное `Content-Description` с
  RFC 2047 decoding. Неоднозначные повторы и невалидные значения остаются в raw
  headers, но не проецируются в BODYSTRUCTURE. Quoted id-left и domain-literal
  Content-ID syntax пока не поддерживаются.
- `BODY` и `BODYSTRUCTURE` теперь сериализуют Content-ID/Description в базовых
  MIME-позициях. Расширения disposition/language/location остаются только в
  BODYSTRUCTURE. Независимый Python `imaplib` STARTTLS acceptance подтвердил
  оба basic поля в обоих профилях и отсутствие расширений в BODY.
- PASS: Windows Rust 1.97.1 `cargo test --all-targets --locked` (439 library +
  1 binary), strict Clippy, format и diff check. Linux Docker Rust 1.97 test
  target: release build, 480 library + 1 binary, doc-tests и strict Clippy;
  тот же независимый STARTTLS acceptance прошёл. Временные image/container
  удалены; `litterkeep-g12-phone` сохранён. Реальный клиент Litterkeep не
  проверялся. G17 остаётся RUNNING.

### 2026-09-25: G17 UTF-16 byte-order evidence checkpoint

- Generic `UTF-16` now rejects input shorter than the two initial octets needed
  to inspect a BOM, instead of silently assuming big-endian without checking.
  Explicit `UTF-16BE` and `UTF-16LE` retain their known order and accept empty
  payloads. Regression tests cover both policies. This follows RFC 2781 §4.3.
- Windows Rust 1.97.1: `cargo test --locked` (412 library + 1 binary), strict
  Clippy, `cargo fmt --check` и `git diff --check` прошли. Linux Docker test
  stage: release build, strict Clippy, 453 library + 1 binary tests и
  doc-tests прошли. Socket acceptance и stage data не создавались. Временный
  image удалён, `litterkeep-g12-phone` сохранён. G17 RUNNING.

### 2026-09-25: G17 UTF-16 MIME charset checkpoint

- Добавлено декодирование `UTF-16`, `UTF-16BE` и `UTF-16LE` в общем MIME
  charset-профиле: RFC 2047 encoded-word и RFC 2231 extended filename теперь
  преобразуются без внешней codec-зависимости. Проверены BOM/порядок байтов,
  supplementary scalar через корректную surrogate pair и отказ на неполных
  octets, противоположном BOM, некорректных surrogate. Charset зарегистрирован
  RFC 2781 как unsuitable для MIME `text/*`, поэтому текстовый SEARCH
  намеренно не декодирует body с этими метками.
- Windows Rust 1.97.1: `cargo test --locked` (412 library + 1 binary), strict
  Clippy, `cargo fmt --check` и `git diff --check` прошли. Linux Docker test
  stage: release build, strict Clippy, 453 library + 1 binary tests и
  doc-tests прошли. Socket acceptance не запускался; новые зависимости,
  постоянные контейнеры и stage data не создавались. Временный image удалён,
  `litterkeep-g12-phone` сохранён. G17 RUNNING.

### 2026-09-25: G17 RFC 2046 nested outer-boundary recovery checkpoint

- MIME parser propagates typed multipart close policy through nested entities.
  Within a parent multipart, its already-detected outer delimiter may terminate
  a truncated nested multipart at any depth, including through
  `message/rfc822`; the nested final body keeps exact source octets. The root
  multipart still rejects a missing close delimiter. Regression includes two
  unclosed nested levels and a following sibling part. Behavior checked against
  the explicit receiver requirement in RFC 2046 §5.1.2.
- Windows Rust 1.97.1: `cargo test --locked` (406 library + 1 binary), strict
  Clippy, `cargo fmt --check` и `git diff --check` прошли. Linux Docker test
  stage: release build, strict Clippy, 447 library + 1 binary tests и
  doc-tests прошли. Тесты unit-only; SMTP/IMAP sockets и Litterkeep client не
  проверялись. Временный image удалён, оставлен только `litterkeep-g12-phone`;
  отдельная stage data не создавалась. G17 RUNNING.

### 2026-09-25: G17 multipart closing delimiter bounds epilogue checkpoint

- MIME scanning now stops at the first closing delimiter. Boundary-shaped lines
  in the following epilogue remain epilogue text and cannot consume the
  multipart part budget or cause a valid message to be rejected. Regression
  uses a one-part budget and two delimiter-shaped epilogue lines. Behavior was
  checked against RFC 2046 §5.1.1 grammar (`[CRLF epilogue]` after the close).
- Windows Rust 1.97.1: `cargo test --locked` (404 library + 1 binary),
  `cargo clippy --all-targets --locked -- -D warnings`, `cargo fmt --check` и
  `git diff --check` прошли. Linux Docker test stage: release build, strict
  Clippy, 445 library + 1 binary tests и doc-tests прошли. Временный image
  удалён, в контейнерах остался только `litterkeep-g12-phone`; stage data не
  создавались. Проверка unit-only, SMTP/IMAP и клиентская интеграция не
  запускались. G17 RUNNING.

### 2026-09-25: G17 multipart boundary IMAP socket acceptance

- Независимый Python `imaplib` клиент через STARTTLS сделал IMAP APPEND MIME
  fixture с transport-padding на opening/closing delimiters; сервер вернул
  корректный BODYSTRUCTURE, точные octets первой части и исходные байты
  Base64-вложения через BINARY.PEEK. Linux release image собран, test data
  изолированы в `./data/g17-multipart-padding`.
- Windows Rust 1.97.1: `cargo test --locked` (403 library + 1 binary),
  `cargo clippy --all-targets --locked -- -D warnings`, `cargo fmt --check` и
  `git diff --check` прошли. Linux Docker test stage на Rust 1.97.1: release
  build, strict Clippy, 444 library tests, 1 binary test и doc-tests прошли.
  Socket acceptance использовал только опубликованный IMAP host-порт 52143;
  SMTP port mapping 52587 был доступен, но SMTP-сценарий не запускался.
  Временный контейнер и образ удалены; `litterkeep-g12-phone` сохранён.
  Независимая интеграция с клиентом Litterkeep не проверялась. G17 RUNNING.

### 2026-09-25: G17 multipart preamble boundary CRLF checkpoint

- Исправлен диапазон MIME preamble: CRLF непосредственно перед первым boundary
  delimiter больше не включается в preamble, поскольку RFC 2046 §5.1.1
  относит его к delimiter; собственный завершающий CRLF preamble сохраняется.
  Тест также подтверждает transport-padding на opening/closing delimiters и
  сохранение строки `--boundary-suffix` внутри part.
- Windows Rust 1.97.1: 402 library + 1 binary tests, fmt и strict Clippy PASS.
  Docker Linux Rust 1.97.1: release build, strict Clippy, 443 library + 1
  binary + doc-tests PASS. Проверено unit-тестом; socket acceptance не
  запускался. Временный verification image удалён; остался только ранее
  существовавший `litterkeep-g12-phone`. RFC 2046 §5.1.1 сверена с IETF.
  G17 RUNNING.

### 2026-09-25: G17 multipart/digest BODYSTRUCTURE checkpoint

- IMAP BODYSTRUCTURE regression фиксирует сериализацию headerless
  `multipart/digest`: дочерний `MESSAGE/RFC822`, ENVELOPE вложенного сообщения,
  его default `TEXT/PLAIN`, размеры в исходных октетах и тип `DIGEST` родителя.
  Это проверяет применение RFC MIME default на протокольной проекции, а не
  только на внутреннем MIME tree.
- Windows Rust 1.97.1: 400 library + 1 binary tests, fmt и strict Clippy PASS.
  Docker Linux Rust 1.97.1: release build, strict Clippy, 441 library + 1
  binary + doc-tests PASS. Новая регрессия unit-only, socket acceptance не
  запускался. Временный image удалён; в инвентаре контейнеров только ранее
  существовавший `litterkeep-g12-phone`. G17 RUNNING.

### 2026-09-25: G17 empty MIME header area checkpoint

- MIME header parser принимает пустую header area, ограниченную начальной CRLF;
  body начинается сразу после неё. Это позволяет multipart body-part без
  заголовков получить контекстный default Content-Type, включая
  `message/rfc822` в `multipart/digest`; обычный `multipart/mixed` сохраняет
  default `text/plain`.
- Тесты проверяют точный raw body offset, untyped digest message и untyped
  mixed text. Windows Rust 1.97.1: 399 library + 1 binary tests, fmt и strict
  Clippy PASS. Docker Linux Rust 1.97.1: release build, strict Clippy, 440
  library + 1 binary + doc-tests PASS. Unit-only, socket acceptance не
  запускался. Verification image удалён; в Docker оставлен только ранее
  существовавший `litterkeep-g12-phone`. RFC 2046 §5.1 и §5.1.5 сверены с IETF.
  G17 RUNNING.

### 2026-09-25: G17 multipart/digest default media type checkpoint

- RFC 2046 §5.1.5 default для дочерней части `multipart/digest` теперь
  типизированно выбирает `message/rfc822`; обычные multipart subtype сохраняют
  `text/plain`. Вложенное сообщение затем получает свой отдельный default
  `text/plain`. `multipart/digest` распознаётся отдельным вариантом enum.
- Регрессии проверяют digest child, вложенное сообщение и отсутствие изменения
  mixed default. Windows Rust 1.97.1: 398 library + 1 binary tests, fmt и strict
  Clippy PASS. Docker Linux Rust 1.97.1: release build, strict Clippy, 439
  library + 1 binary + doc-tests PASS. Проверка unit-only, socket acceptance не
  запускался. Временный verification image удалён; в Docker остался только
  ранее существовавший `litterkeep-g12-phone`. RFC 2046 §5.1.5 сверена с IETF.
  G17 RUNNING.

### 2026-09-25: G17 tolerant quoted-printable recovery checkpoint

- Входящий MIME quoted-printable decoder по рекомендации RFC 2045 §6.7
  сохраняет повреждённую `=` escape-последовательность буквально вместо потери
  или отказа от всего body. Строгий decoder оставлен отдельной функцией для
  Sieve vacation `:mime`, который не должен сохранять некорректный исходящий
  entity.
- Unit tests покрывают truncated/invalid escape recovery и сохранение strict
  rejection в Sieve. Windows Rust 1.97.1: 396 library + 1 binary tests, fmt и
  strict Clippy PASS. Docker Linux Rust 1.97.1: release build, strict Clippy,
  437 library + 1 binary + doc-tests PASS. Socket acceptance не запускался.
  `docker build --rm` использован; после теста сверяется inventory. RFC 2045
  §6.7 сверена с IETF Datatracker. G17 RUNNING.

### 2026-09-25: G17 MIME 7bit and 8bit line profile checkpoint

- MIME `7bit` и `8bit` transfer decoders теперь проверяют 998-октетную ширину
  строк, CRLF framing и запрет NUL. Только `8bit` разрешает high-bit octets;
  `binary` сохраняет любой поток без преобразования. Проверка линейная и не
  копирует данные до успешной валидации.
- Windows Rust 1.97.1: 396 library + 1 binary tests, fmt и strict Clippy PASS.
  Docker Linux Rust 1.97.1: release build, strict Clippy, 437 library + 1 binary
  + doc-tests PASS. Socket acceptance не запускался. `docker build --rm` stage
  завершён; inventory будет отражён после проверки. RFC 2045 §§2.7-2.8 сверены
  с IETF Datatracker. G17 RUNNING.

### 2026-09-25: G17 quoted-printable soft-break padding checkpoint

- Quoted-printable decoder распознаёт буквальные SP/TAB перед `=CRLF` как
  transport padding и удаляет их, при этом encoded `=20` перед soft break
  сохраняет. Это предотвращает потерю содержимого и ложные пробелы после
  транспортного line-wrap.
- Unit tests проверяют оба различающихся случая. Windows Rust 1.97.1: 395
  library + 1 binary tests, fmt и strict Clippy PASS. Docker Linux Rust 1.97.1:
  release build, strict Clippy, 436 library + 1 binary + doc-tests PASS. Socket
  acceptance не запускался. `docker build --rm` использован; после stage
  проверяется inventory. RFC 2045 §6.7 сверена с IETF Datatracker. G17 RUNNING.

### 2026-09-25: G17 MIME Content-Transfer-Encoding comments checkpoint

- `Content-Transfer-Encoding` теперь пропускает RFC 822 comments до
  типизированного разбора transfer-encoding token; вложенные комментарии и
  quoted-string обрабатываются общим MIME comment scanner. Тест покрывает
  comments перед `base64` и после него.
- Windows Rust 1.97.1: 394 library + 1 binary tests, fmt и strict Clippy PASS.
  Docker Linux Rust 1.97.1: release build, strict Clippy, 435 library + 1 binary
  + doc-tests PASS. Проверка unit-only, socket acceptance не запускался.
  `docker build --rm` тестировал Linux stage; cleanup inventory проверен после
  сборки. RFC 2045 §3 и §4 сверены с IETF Datatracker. G17 RUNNING.

### 2026-09-25: G17 quoted-printable trailing transport padding checkpoint

- MIME quoted-printable decoder удаляет буквальные SP/TAB в конце hard line
  и объекта, как предписывает RFC 2045 §6.7, но сохраняет encoded `=20`/`=09`.
  Собственный encoder кодирует SP/TAB как hex-octets, чтобы generated soft wrap
  и декодирование оставались обратимыми.
- Добавлены тесты line/end padding, encoded spaces и round-trip. Windows Rust
  1.97.1: 393 library + 1 binary tests, fmt и strict Clippy PASS. Docker Linux
  Rust 1.97.1: release build, strict Clippy, 434 library + 1 binary + doc-tests
  PASS. Socket acceptance не запускался. `docker build --rm` использован для
  тестового stage; контейнерная инвентаризация выполняется перед фиксацией.
  RFC 2045 §6.7 сверена с IETF Datatracker. G17 остаётся RUNNING.

### 2026-09-25: G17 MIME Base64 decoder profile checkpoint

- Декодирование разделено на строгий путь RFC 2047 encoded-word и MIME CTE
  путь RFC 2045 §6.8, который игнорирует октеты вне Base64-алфавита.
  Оба пути сохраняют проверку ширины, padding, нулевых pad bits и отсутствия
  данных после padding. Реализация однопроходная, временно хранит один quantum.
- Unit тесты проверяют допустимое игнорирование мусорных октетов и сохранение
  строгого encoded-word профиля. Windows Rust 1.97.1: 390 library + 1 binary
  tests, fmt и strict Clippy PASS. Docker Linux Rust 1.97.1: release build,
  strict Clippy, 431 library + 1 binary + doc-tests PASS. Независимый black-box
  fixture обновлён, но socket acceptance не запускался. После `docker build
  --rm` в инвентаре остался только `litterkeep-g12-phone`; временных контейнеров
  нет. RFC 2045 §6.8 сверена с IETF Datatracker. G17 остаётся RUNNING.

### 2026-09-25: G17 bounded multipart delimiter collection checkpoint

- MIME tree передаёт multipart parser-у остаток общего `MAX_PARTS` бюджета.
  Линейный проход прекращает накопление delimiter range при превышении числа
  допустимых дочерних частей, вместо построения неограниченного `Vec` до
  проверки рекурсивного дерева. Предел включает закрывающий delimiter.
- Unit тест строит entity с 256 дочерними частями при глобальном бюджете 256 и
  проверяет отказ на этапе bounded collection. Windows Rust 1.97.1: 387 library
  + 1 binary tests, fmt и strict Clippy PASS. Docker Linux Rust 1.97.1: release
  build, strict Clippy, 428 library + 1 binary + doc-tests PASS. Это unit-only,
  сетевой parser acceptance не запускался. `docker build --rm` оставил только
  `litterkeep-g12-phone`; временных контейнеров нет. G17 остаётся RUNNING.

### 2026-09-25: G17 multipart boundary grammar checkpoint

- Парсер RFC 2046 multipart delimiter теперь валидирует boundary по §5.1.1:
  от 1 до 70 ASCII-октетов, только `bchars`, конечный пробел запрещён.
  Проверки покрывают разрешённый punctuation-набор, пустое значение, 70/71
  октет, forbidden octet, non-ASCII и путь разбора multipart entity.
- Windows Rust 1.97.1: 386 library + 1 binary tests, fmt и strict Clippy PASS.
  Docker Linux Rust 1.97.1: release build, strict Clippy, 427 library + 1
  binary + doc-tests PASS. Unit-only, socket black-box не запускался.
  `docker build --rm` оставил только `litterkeep-g12-phone`; временных
  контейнеров нет. RFC 2046 §5.1.1 сверена с IETF Datatracker. G17 RUNNING.

### 2026-09-25: G17 malformed MIME Content-Type fallback checkpoint

- MIME parser applies RFC 2045 §5.2 recommended `text/plain` default when
  Content-Type is syntactically invalid; body SEARCH therefore treats the part
  using the US-ASCII default. The original malformed header remains untouched.
  Duplicate Content-Type fields are rejected instead of selecting the first.
- Unit tests cover malformed parameter syntax, preservation of header/body
  bytes, SEARCH fallback and duplicate rejection. Independent black-box script
  now includes the invalid-header case, but was not run over a socket. Windows
  Rust 1.97.1: 384 library + 1 binary tests, fmt and strict Clippy PASS. Docker
  Linux Rust 1.97.1: release build, strict Clippy, 425 library + 1 binary and
  doc-tests PASS. `docker build --rm` left only pre-existing
  `litterkeep-g12-phone`; no temporary containers remain. RFC 2045 §5.2 was
  checked against the IETF Datatracker. G17 remains RUNNING.

### 2026-09-25: G17 MIME composite transfer-encoding checkpoint

- MIME tree parser сверяет CTE каждого multipart/message entity с RFC 2045 §6.4:
  разрешены только 7bit/8bit/binary. Base64, quoted-printable и unknown CTE
  отклоняются до разборки вложенных byte ranges; повторный недопустимый CTE
  нельзя скрыть последующим допустимым полем. Валидный multipart/8bit остаётся
  читаемым.
- Unit tests покрывают multipart и message/partial с запрещёнными значениями,
  duplicate-CTE bypass и допустимый 8bit. Новые проверки unit-only; независимый
  сетевой MIME/FETCH клиент для этой ошибки не запускался. PASS: Windows Rust
  1.97.1 full suite (377 библиотечных тестов, 1 тест бинаря), strict Clippy,
  fmt и diff-check; Docker Linux Rust 1.97.1 release build, strict Clippy,
  418 библиотечных тестов, 1 тест бинаря и doc-tests. После проверки Docker
  оставлен только контейнер `litterkeep-g12-phone`; временных контейнеров нет.
- RFC 2045 §6.4 сверена с IETF Datatracker. G17 остаётся RUNNING.

### 2026-09-25: G17 unknown MIME transfer-encoding checkpoint

- MIME entity с неизвестным CTE теперь получает эффективный тип
  `application/octet-stream` и не сохраняет параметры объявленного типа в
  структурной проекции; исходные заголовки и encoded body остаются неизменными.
  BODYSTRUCTURE сообщает эффективный тип, SEARCH не просматривает такую часть
  как текст, а BINARY сохраняет обязательный RFC 9051 `NO [UNKNOWN-CTE]`.
- Unit tests проверяют тип/параметры/исходные байты, BODYSTRUCTURE и отсутствие
  текста в SEARCH projection. Black-box сценарий обновлён, но сетевой тест в
  этом checkpoint не запускался. Windows Rust 1.97.1: 379 library + 1 binary
  test, fmt и strict Clippy PASS. Docker Linux Rust 1.97.1: release build,
  strict Clippy, 420 library + 1 binary + doc-tests PASS. Test-stage Docker
  build использовал `--rm`; после него подтверждено отсутствие временных
  контейнеров, `litterkeep-g12-phone` сохранён. RFC 2045 §6.4 и RFC 9051
  сверены с IETF Datatracker. G17 остаётся RUNNING.

### 2026-09-25: G17 duplicate MIME transfer-encoding checkpoint

- Разбор CTE вынесен в отдельный `tree_transfer.rs`, сохранив разделение
  ответственности и лимит в 300 строк для `tree.rs`. Отсутствующий CTE
  трактуется как 7bit; единственный CTE разбирается обычным путём; второе
  Content-Transfer-Encoding поле отклоняется как неоднозначная entity, вместо
  выбора последнего значения и потери смысла предыдущего.
- Тест проверяет unknown + 8bit обход; unit покрытие включает допустимое
  отсутствие/одиночный CTE и ограничения composite. Windows Rust 1.97.1:
  381 library + 1 binary test, fmt и strict Clippy PASS. Docker Linux Rust
  1.97.1: release build, strict Clippy, 422 library + 1 binary + doc-tests
  PASS. После `docker build --rm` в inventory остался только ранее существующий
  `litterkeep-g12-phone`; временных контейнеров нет. Спецификации не задают
  алгоритм разрешения повторного CTE, поэтому повтор отклоняется как
  неоднозначный ввод, а не заявляется как буквальное MUST RFC. G17 RUNNING.

### 2026-09-25: G17 RFC 2047 header-token recognition checkpoint

- Unstructured-header projection распознаёт encoded-word только в начале
  field-body или после ASCII SP/HTAB и только как целый токен до конца значения
  либо LWS. Слитные формы `subject=?...?=` и `=?...?=suffix` остаются исходным
  текстом; соседние валидные encoded-word продолжают декодироваться, а
  разделяющий их LWS опускается.
- Новые границы покрыты unit tests. PASS: Windows Rust 1.97.1 полный suite
  (375 библиотечных тестов, 1 тест бинаря), strict Clippy, fmt и diff check;
  Docker Linux Rust 1.97.1 release build, strict Clippy, 416 библиотечных
  тестов, 1 тест бинаря и doc-tests.
- Независимый сетевой acceptance новых unstructured-header границ не выполнялся.
  RFC 2047 §6.1 сверена с IETF Datatracker. G17 остаётся RUNNING.

### 2026-09-25: G17 RFC 2047 phrase-Q profile checkpoint

- Phrase-context decoder применяет отдельный RFC 2047 §5 allowlist для Q:
  ASCII letters/digits и `!*+-/=_`; `=` требует двух hex digits. Например,
  raw точка принимается общим unstructured-header decoder-ом, но не phrase;
  её encoded form `=2E` допустима. B encoding этим Q-ограничением не затронут.
- PASS: Windows Rust 1.97.1 `cargo fmt --check`, `cargo test --all-targets
  --locked` (374 библиотечных теста, 1 тест бинаря), strict Clippy и
  `git diff --check`. Docker Linux Rust 1.97.1 release build, strict Clippy,
  415 библиотечных тестов, 1 тест бинаря и doc-tests. RFC 2047 §5 сверена с
  текстом RFC в IETF Datatracker.
- Litterkeep network client acceptance для новых Q punctuation границ не
  запускался. После Docker build временных контейнеров нет, сохранён только
  `litterkeep-g12-phone`. G17 остаётся RUNNING.

### 2026-09-25: G17 RFC 2047 encoded-text grammar checkpoint

- Encoded-word decoder проверяет RFC 2047 `encoded-text` до B/Q decoding:
  допускаются только непустые печатные ASCII octets, кроме SP и `?`. Пустой
  B/Q payload, raw whitespace/control и raw non-ASCII отклоняются; encoded Q
  octet вида `=01` остаётся валидным. Невалидное слово в header projection
  остаётся видимым исходным текстом.
- PASS: Windows Rust 1.97.1 `cargo fmt --check`, `cargo test --all-targets
  --locked` (373 библиотечных теста, 1 тест бинаря), strict Clippy и
  `git diff --check`. Docker Linux Rust 1.97.1 release build, strict Clippy,
  414 библиотечных тестов, 1 тест бинаря и doc-tests.
- Новые grammar проверки unit-only, без независимого сетевого клиента.
  `docker ps -a` сохранил только `litterkeep-g12-phone`; сборочный test-stage
  автоматически удаляет временный build container.
- RFC 2047 phrase-context restrictions для Q punctuation пока не проверяются
  общим decoder-ом, так как его API не получает синтаксический контекст.
  G17 остаётся RUNNING.

### 2026-09-25: G17 Windows-1252 MIME charset checkpoint

- Добавлен отдельный типизированный профиль IANA `windows-1252`: декодируются
  все значения октета, включая таблицу C1; RFC 2231 filename и MIME body
  поиска IMAP SEARCH проверены на байтах `80 96` -> `€–`. Принимается только
  каноническое имя `windows-1252`, браузерный псевдоним `cp1252` намеренно не
  считается зарегистрированной MIME-меткой.
- PASS: Windows Rust 1.97.1: `cargo fmt --check`, `cargo test --all-targets
  --locked` (371 библиотечных тестов, 1 тест бинаря), strict Clippy и
  `git diff --check`. Docker Linux Rust 1.97.1: 412 библиотечных тестов,
  1 тест бинаря и doc-tests; образ `litterkeep-linux-test:latest` собран.
- Новые charset, RFC 2231 и SEARCH проверки unit-only; независимое сетевое
  подключение и интеграция клиента Litterkeep для этого профиля не проверялись.
  `docker ps -a` оставил только сохранённый `litterkeep-g12-phone`; временный
  тестовый контейнер запущен с `--rm`. G17 остаётся RUNNING.

### 2026-09-25: G17 RFC 2231 extended payload checkpoint

- Extended parameter payload теперь принимает только RFC 2231 attribute-char
  либо percent-encoded octet; hex digits в escape соответствуют ABNF RFC
  (заглавные A-F). Raw tspecials, апострофы payload, повреждённые escapes и
  строчные hex digits отвергаются отдельным decoder-ом.
- Regressions сохраняют valid UTF-8 filename и смешанные quoted/unquoted
  continuation segments, а также проверяют language-delimited initial value.
  Новые проверки unit-only, независимый сетевой сценарий не выполнялся.
- PASS: Windows Rust 1.97.1 cargo test --all-targets --locked (368 тестов
  библиотеки, 1 тест бинаря), strict Clippy, fmt и diff check. Docker Linux
  Rust 1.97.1 release build, strict Clippy, 409 тестов библиотеки и 1 тест
  бинаря.
- Ограничения: MIME API не сохраняет и не валидирует language tag; charset
  декодируется только поддерживаемым явным allowlist, пустой charset поэтому
  не может быть представлен как Unicode. G17 остаётся RUNNING.

### 2026-09-25: G17 MIME parameter grammar checkpoint

- MIME structured parser теперь проверяет attribute/value grammar RFC 2045 §5.1:
  имена параметров должны быть допустимыми RFC 2231 attribute-char, некавыченные
  значения должны быть непустыми token, а tspecials разрешены только внутри
  quoted-string. Неэкранированная внутренняя кавычка и пустое некавыченное
  значение отвергаются; пустое quoted значение остаётся допустимым.
- RFC 2231 continuation допускает начальную секцию 0 и канонические следующие
  номера без ведущих нулей. Regressions также проверяют escaped DQUOTE,
  boundary-like quoted value и сохранение RFC 2231 filename continuation.
  Новые случаи пока подтверждены unit tests, независимый сетевой сценарий
  не запускался.
- PASS: Windows Rust 1.97.1 cargo test --all-targets --locked (366 тестов
  библиотеки, 1 тест бинаря), strict Clippy, fmt и diff check. Linux Docker
  Rust 1.97.1 release build, strict Clippy, 407 тестов библиотеки и 1 тест
  бинаря.
- Docker test-stage собран с --rm. После сборки в docker ps -a оставлен только
  litterkeep-g12-phone; G17 остаётся RUNNING.

### 2026-09-25: G17 MIME structured-value token checkpoint

- Общий MIME parser теперь удаляет вложенные RFC comments вне quoted-string и
  требует для Content-Type две непустые token-части `type/subtype`, разделённые
  ровно одним slash. Неизвестные корректные пары сохраняются в typed `Other`,
  malformed token-ы отвергаются. Sieve vacation использует тот же parser и
  общий comment scanner без дублирования обработки.
- Unit regressions проверяют comments вокруг slash, вложенные/незакрытые
  comments, скобки в quoted parameter, ошибочные media-type token-ы и прежний
  RFC 2231 filename continuation. Сетевой клиент для этих новых MIME случаев
  не запускался.
- PASS: Windows Rust 1.97.1 `cargo test --all-targets --locked` (364 теста
  библиотеки, 1 тест бинаря), strict Clippy, fmt и diff check. Docker Linux
  Rust 1.97 release build, strict Clippy, 405 тестов библиотеки и 1 тест бинаря.
- Docker build завершился с `--rm`; `docker ps -a` содержит только
  `litterkeep-g12-phone`. Временных контейнеров этапа не осталось. G17 остаётся
  RUNNING.

### 2026-09-25: G17 Sieve composite MIME transfer checkpoint

- RFC 2045 §6.4 запрещает base64 и quoted-printable для составных `multipart/*`
  и `message/*` entity. Vacation MIME validator теперь разбирает Content-Type
  в typed `MediaType`, удаляя разрешённые RFC comments вне quoted-string;
  классифицирует все `message/*` как составные и отвергает такие CTE. Повторный
  Content-Type тоже отклоняется. Multipart с 7bit остаётся допустимым, если
  проходит line/octet checks.
- Regression проверяет multipart mixed/related и message partial с запрещённой
  кодировкой, комментарий между type и slash, quoted boundary с круглыми
  скобками, допустимый multipart 7bit и duplicate Content-Type. Это unit
  CHECKSCRIPT coverage, без независимого сетевого ManageSieve клиента.
- PASS: Windows Rust 1.97.1 `cargo test --all-targets --locked` (361 тест),
  strict Clippy, fmt и diff check. Linux Docker Rust 1.97 release build,
  strict Clippy и 402 теста.
- Docker test-stage собран с `--rm`. Runtime-контейнеры не запускались;
  после сборки `docker ps -a` содержит только `litterkeep-g12-phone`, образ
  `litterkeep-linux-test:latest` обновлён. G17 остаётся RUNNING.

### 2026-09-25: G17 Sieve vacation MIME line profile checkpoint

- Проверка `:mime` выделена в отдельный MIME validator. Для 7bit проверяются
  ASCII, запрет NUL, только CRLF и предел 998 октетов на строку; для Base64 и
  quoted-printable проверяются корректные CRLF, допустимые wire octets и предел
  76 символов. Quoted-printable также отвергает сырые завершающие пробелы и
  пробел перед soft line break. Основа: RFC 2045 §§2.7, 6.7, 6.8.
- Regressions проверяют точную границу 998, превышение 999, превышение 76 для
  обеих кодировок, NUL, bare LF, повреждённый escape и trailing whitespace.
  Это unit-level validation, не независимая ManageSieve-сетевая проверка.
- PASS: Windows Rust 1.97.1 `cargo test --all-targets --locked` (360 тестов),
  strict Clippy, fmt и diff check. Linux Docker Rust 1.97 release build,
  strict Clippy и 401 тест.
- Test-stage собран с `--rm`, runtime-контейнеры не запускались; проверка
  `docker ps -a` оставила только `litterkeep-g12-phone`. G17 остаётся RUNNING.

### 2026-09-25: G17 Sieve vacation MIME transfer validation checkpoint

- `:mime` validation проверяет не только имя Content-Transfer-Encoding, но и
  декодируемость фактического body общим MIME decoder-ом. Duplicate CTE и
  unsupported 8bit/binary отклоняются; Base64 decoder больше не принимает
  данные после padding или ненулевые padding bits.
- Regressions проверяют повреждённый base64/QP, не-ASCII body под 7bit,
  duplicate CTE и повреждённый padding. Это unit-level CHECKSCRIPT validation;
  независимая ManageSieve-сетевая проверка именно этих отказов не запускалась.
- PASS: Windows Rust 1.97.1 `cargo test --all-targets --locked` (359 тестов),
  strict Clippy, fmt и diff check. Linux Docker Rust 1.97 release build,
  strict Clippy и 400 тестов.
- Docker build использовал test stage без именованного runtime-контейнера;
  после сборки `docker ps -a` содержит только `litterkeep-g12-phone`, образ
  `litterkeep-linux-test:latest` обновлён. G17 остаётся RUNNING.

### 2026-09-25: G17 RFC 5230 vacation history compaction checkpoint

- Vacation suppression journal компактизируется перед превышением лимита 8 MiB:
  учитываются только committed transaction IDs, для каждого ключа сохраняется
  последняя запись, просроченные claims удаляются, а при нехватке места
  oldest-first удаляются записи, сохраняя минимум 1000 identities по RFC 5230
  §4.2. Ниже этого минимума Store отказывает в новой записи, не теряя историю.
- Замена snapshot защищена temp/backup recovery. Тесты покрывают выбор
  committed/latest записей, истечение срока, oldest-first границу 1000 и
  восстановление прерывания по обе стороны публикации файла.
- PASS: Windows Rust 1.97.1 `cargo test --all-targets --locked` (358 тестов),
  strict Clippy, fmt и diff check. Docker Linux Rust 1.97 release build,
  strict Clippy и 399 тестов.
- Контейнеры не создавались. Проверка `docker ps -a` оставила только
  `litterkeep-g12-phone`; образ `litterkeep-linux-test:latest` обновлён.
  Физический отказ файловой системы в ходе `sync`/`rename` не симулировался.
  G17 остаётся RUNNING.

### 2026-09-25: G17 RFC 5230 vacation implementation checkpoint

- Реализованы typed parser/validator/executor для единственного `vacation`
  action, ManageSieve capability, RFC response generation, MIME/UTF-8 encoding,
  safety suppression и Store enqueue с durable history. Исходная доставка
  сохраняет implicit keep; ошибка enqueue после её commit не запрашивает у SMTP
  повтор исходного письма и журналируется в stderr.
- Локальные regressions: 9 parser/validator/message tests и 2 Store tests
  подтверждают MIME guards, UTF-8 response, автоматические/списочные guards,
  исходную доставку и suppression после reopen. Полный test suite выявил и
  исправил три regression: тест длины QP учитывал CR из CRLF, а старые тесты
  считали `require "vacation"` неизвестной capability.
- После исправлений Windows Rust 1.97.1 `cargo test --all-targets --locked`
  проходит (355 тестов), strict Clippy и fmt проходят. Docker Rust 1.97 Linux
  test-target проходит 396 тестов и release build. В `docker ps -a` остался
  только `litterkeep-g12-phone`; transient containers не создавались.
- RFC 5230 §4.5 проверен по первичному тексту: auto-reply требует адрес
  владельца в To/Cc/Bcc/Resent-*; конечный RCPT и `:addresses` являются
  дополнительными способами распознать адрес владельца, а `:addresses` не
  сужает список. Unit и сетевой regressions покрывают это правило.
- Linux black-box acceptance в `./data/g17-sieve-vacation-rfc45`: активный
  ManageSieve script получил три письма, два на supplemental `:addresses` alias
  и одно на не принадлежащий владельцу адрес; Linux queue-drain передал ровно
  один generated response независимому SMTP peer с `MAIL FROM:<>`. После
  перезапуска IMAP подтвердил все три оригинала у Bob. Python-клиенты независимы
  от реализации. Оба временных Docker контейнера удалены; оставлен только
  `litterkeep-g12-phone`. G17 остаётся RUNNING.

### 2026-09-25: G17 vacation history capacity checkpoint

- Уточнён RFC 5230 §4.2 capacity gate: отдельный recovery regression создаёт
  1000 committed response identities при максимальных разрешённых размерах
  account, sender и identity, затем читает все 1000 обратно. Ещё один regression
  добавляет оборванный tail и проверяет его удаление при сохранении claim.
- PASS: Windows Rust 1.97 `cargo test --all-targets --locked` (343 tests),
  strict Clippy, format и diff check. Linux Docker Rust 1.97 test target
  (384 tests), strict Clippy и release build. Runtime-контейнеры не создавались;
  в Docker остался только `litterkeep-g12-phone`.
- Это подтверждает только ёмкость и recovery журнала. Полная vacation action,
  удаление старых записей по сроку/лимиту, response generation и сетевой
  acceptance не реализованы.

### 2026-09-25: G17 durable vacation suppression foundation

- Store теперь имеет typed запрос generated vacation-ответа и append-only
  `journal/vacation-history.log`. Claim с account, envelope sender и response
  identity записывается до общей Store commit-точки, виден только для
  committed transaction ID и восстанавливается после reopen. Generated
  RFC822 blob и outbound job с `MAIL FROM:<>` используют ту же транзакцию.
- Regression покрывает подавление в пределах срока до и после reopen,
  повтор после истечения, другой response identity и сбой после записи claim,
  но до commit: после recovery job отсутствует, повтор разрешён.
- PASS: Windows Rust 1.97 `cargo test --all-targets --locked` (341 tests),
  strict Clippy, format и diff check. Linux Docker Rust 1.97 test target
  (382 tests), strict Clippy и release build. Новых runtime-контейнеров не
  создавалось; `docker ps -a` содержит только сохранённый
  `litterkeep-g12-phone`.
- Это storage foundation, не RFC 5230 vacation implementation: Sieve parser и
  executor пока не создают такой запрос; normalization response identity,
  days policy bounds, loop/list/daemon guards, MIME generation и сетевой
  ManageSieve -> SMTP -> IMAP acceptance остаются обязательными.

### 2026-09-25: G17 generated Sieve-response payload foundation

- Store получил отдельную границу `enqueue_outbound_payload`: она фиксирует
  собственный immutable RFC822 blob и outbound job общей transaction commit
  точкой, но не создаёт mailbox link. Queue journal кодирует null reverse-path
  как токен `<>`, чтобы whitespace replay не терял пустое `MAIL FROM`.
- Regression проверяет байт-в-байт payload и null sender до/после Store reopen
  и queue transition. Linux-only end-to-end Store->outbound service test
  передал generated body независимому SMTP peer, который потребовал ровно
  `MAIL FROM:<>`; доставка завершилась успешно.
- PASS: Windows Rust 1.97 `cargo test --all-targets --locked` (338 tests),
  strict Clippy, format и `git diff --check`. Docker Linux Rust 1.97 test target
  (379 tests), strict Clippy и release build. Временных контейнеров не было;
  `litterkeep-g12-phone` сохранён.
- Это только payload/transport foundation. `vacation` всё ещё не реализован и
  не рекламируется; durable response suppression, RFC 5230 limits/loop guards,
  response generation и ManageSieve acceptance остаются следующими задачами.

### 2026-09-25: G17 RFC 5228 same-mailbox delivery checkpoint

- Добавлен Store-level regression: два одинаковых `fileinto` не дают runtime
  error, создают одну mailbox entry и переживают reopen. Независимый acceptance
  исполняет два одинаковых `fileinto :copy`; IMAP reader требует ровно одно
  сообщение в INBOX и `Invoices` и сравнивает полные RFC822 bytes.
- PASS: Windows Rust 1.97, 336 тестов, strict Clippy, fmt, diff check и Python
  compile. Linux Docker Rust 1.97, strict Clippy и 376 тестов. Независимый
  ManageSieve -> SMTP -> IMAP acceptance в `./data/g17-sieve-same-mailbox`
  исполнил повторный `fileinto :copy`, потребовал ровно одно сообщение в INBOX
  и `Invoices` и подтвердил равенство полных RFC822 bytes. Временный контейнер
  удалён, `litterkeep-g12-phone` сохранён. Litterkeep app integration не
  проверялась. G17 остаётся RUNNING.

### 2026-09-25: G17 RFC 3894 Sieve copy checkpoint

- Реализованы `fileinto :copy` и `redirect :copy` с typed actions, обязательным
  `require "copy"`, дополнительным `require "fileinto"` для fileinto и
  обновлённым ManageSieve capability. Оба действия сохраняют implicit keep;
  unit regression также проверяет, что последующий `discard` всё ещё отменяет
  его.
- PASS: Windows Rust 1.97, 335 тестов и strict Clippy. Linux Docker Rust 1.97,
  strict Clippy и 375 тестов. Независимый ManageSieve -> SMTP -> IMAP acceptance
  в `./data/g17-sieve-copy-verified` сравнил полные RFC822 bytes письма в INBOX
  и `Invoices`; bytes совпали. `redirect :copy` проверен на уровне плана, не на
  внешнем SMTP peer. Временный контейнер удалён; `litterkeep-g12-phone`
  сохранён. Интеграция приложения Litterkeep не проверялась. G17 остаётся
  RUNNING.

### 2026-09-25: G17 RFC 5230 vacation gap inventory

- Сверен RFC 5230 §§4-5 с Sieve executor и durable outbound queue. `vacation`
  отсутствует и не объявляется. Его нельзя безопасно реализовать одной
  parser-командой: response payload должен быть отдельным от исходного blob,
  а история подавления должна быть durable и привязана к sender и ответу.
  Дополнительные требования и future acceptance перечислены в
  `docs/sieve-coverage.md`.
- Это inventory checkpoint, не реализация vacation и не PASS реализации.
  Отдельный durable response payload добавлен в более позднем checkpoint
  сверху; suppression journal, semantic guards и сетевой acceptance остаются
  обязательными до включения capability.

### 2026-09-25: G17 RFC 5228 envelope-test checkpoint

- Добавлен базовый Sieve `envelope` с typed `from`/`to`, ADDRESS-PART,
  comparator и обязательным `require "envelope"`; subaddress-проекции требуют
  также `require "subaddress"`. SMTP MAIL FROM до trace и исходный RCPT TO,
  сопоставленный локальному mailbox, передаются Store owner-ом до executor-а.
  Null reverse-path сравнивается пустой строкой независимо от ADDRESS-PART.
  ManageSieve greeting и CAPABILITY используют единый актуальный список.
- PASS: Windows Rust 1.97.1, 333 теста, strict Clippy, fmt, diff check и Python
  compile. Linux Docker Rust 1.97: strict Clippy и 373 теста. Независимый
  ManageSieve -> SMTP -> IMAP acceptance доставил письмо в `Invoices` с
  проверкой SMTP RCPT TO; MIME `To` при этом является другой group-form
  строкой. Проверка выполнена в `./data/g17-sieve-envelope`. Временный контейнер
  удалён, `litterkeep-g12-phone` сохранён. Приложение Litterkeep не проверялось.
  G17 остаётся RUNNING.

### 2026-09-25: G17 RFC 5228 multiline-string checkpoint

- Sieve lexer теперь распознаёт `text:` как многострочную строку, сохраняет
  CRLF, снимает ровно один dot-stuffing octet, поддерживает разрешённый
  hash-comment перед телом и ограничивает размер результата. Bracket-comment в
  заголовке literal, неверный separator и отсутствующий terminator отклоняются.
- PASS: Windows Rust 1.97.1, 329 тестов, strict Clippy, fmt, diff check и Python
  compile. Linux Docker Rust 1.97: strict Clippy и 369 тестов. Независимый TLS /
  SASL ManageSieve CHECKSCRIPT принял многострочную строку и отклонил literal
  без terminator в `./data/g17-sieve-multiline`. Временный контейнер удалён,
  `litterkeep-g12-phone` сохранён. SMTP -> Sieve -> IMAP проверка семантики
  matcher-а с фактическим входящим письмом ещё не выполнялась. G17 остаётся
  RUNNING.

### 2026-09-25: G17 RFC 5228 redirect address checkpoint

- Sieve `redirect` static validation теперь принимает только один корректный
  mailbox по RFC 5322: `addr-spec` или quoted phrase с `<addr-spec>`. Общий
  parser отвергает mailbox lists, source routes и group syntax. Unit tests и
  независимый ManageSieve CHECKSCRIPT проверяют допустимую форму и отказы.
- PASS: Windows Rust 1.97.1, fmt, 327 тестов, strict Clippy, Python compile и
  diff check. Linux Docker Rust 1.97: strict Clippy и 367 тестов. Независимый
  ManageSieve -> SMTP -> IMAP acceptance прошёл в
  `./data/g17-sieve-redirect-address`. Временный контейнер удалён,
  `litterkeep-g12-phone` сохранён. Доставку redirect во внешний SMTP и
  интеграцию с Litterkeep не проверяли. G17 остаётся RUNNING.

### 2026-09-25: G17 RFC 5228 comment line-ending checkpoint

- Lexer сначала проверяет весь Sieve source: NUL запрещён, CR и LF допустимы
  только парой. Hash-comment следует ABNF и обязан завершаться CRLF; достижение
  EOF внутри такого комментария является ошибкой. Block-comment также не может
  скрывать одиночный line ending или NUL. Unit regressions и независимый
  ManageSieve CHECKSCRIPT проверяют корректный CRLF и отказ на bare LF.
- PASS: Windows Rust 1.97.1, fmt, 326 тестов, strict Clippy, Python compile и
  diff check. Linux Docker Rust 1.97: strict Clippy и 366 тестов. Независимый
  ManageSieve -> SMTP -> IMAP acceptance прошёл в `./data/g17-sieve-comment-crlf`.
  Временный контейнер удалён, `litterkeep-g12-phone` сохранён. Litterkeep app
  integration не проверялась. G17 остаётся RUNNING.

### 2026-09-25: G17 RFC 5228 empty header strings checkpoint

- Static validation больше не отвергает пустые элементы в `header` и
  `address` string-list: пустой `:contains` key совпадает с существующим
  значением, пустой `:is` key не совпадает с непустым значением, а отсутствующий
  header не совпадает даже с пустым key. Пустое/невалидное имя header в тесте
  не вызывает ошибку скрипта и просто не совпадает. Unit regressions и
  независимый ManageSieve CHECKSCRIPT покрывают эти границы.
- PASS: Windows Rust 1.97.1, fmt, 325 тестов, strict Clippy, Python compile и
  diff check. Linux Docker Rust 1.97: strict Clippy и 365 тестов. Независимый
  ManageSieve -> SMTP -> IMAP acceptance прошёл в
  `./data/g17-sieve-empty-header-strings`. Временный контейнер удалён,
  `litterkeep-g12-phone` сохранён. Litterkeep app integration не проверялась.
  G17 остаётся RUNNING.

### 2026-09-25: G17 RFC 5228 header-name whitespace checkpoint

- Общий MIME header parser нормализует SP/HTAB перед `:` в имя поля вместо
  отказа на ранее допустимой для Sieve форме. Исходный raw header не меняется.
  Unit test закрепляет normalized name/value; независимый клиент отправляет
  `Subject \t:`, проверяет Sieve fileinto delivery через IMAP и подтверждает
  через FETCH, что wire bytes сохранены.
- PASS: Windows Rust 1.97.1, fmt, 324 теста, strict Clippy, Python compile и
  diff check. Linux Docker Rust 1.97: strict Clippy и 364 теста. Независимый
  ManageSieve -> SMTP -> IMAP acceptance прошёл в
  `./data/g17-sieve-header-colon-wsp-verified`. Временный контейнер удалён,
  `litterkeep-g12-phone` сохранён. Litterkeep app integration не проверялась.
  G17 остаётся RUNNING.

### 2026-09-25: G17 RFC 5228 fileinto capability checkpoint

- Static validation теперь требует `require "fileinto"` для каждого
  поддерживаемого `fileinto` action; scripts без capability отвергаются до
  создания DeliveryPlan. Unit regression и независимый ManageSieve CHECKSCRIPT
  проверяют отрицательный случай. Тот же клиент загружает допустимый script,
  отправляет письмо через SMTP и подтверждает folder delivery через IMAP.
- PASS: Windows Rust 1.97.1, fmt, 323 теста, strict Clippy, Python compile и
  diff check. Linux Docker Rust 1.97: strict Clippy и 363 теста; независимый
  ManageSieve -> SMTP -> IMAP acceptance прошёл в `./data/g17-sieve-fileinto-require`.
  Временный контейнер удалён, `litterkeep-g12-phone` сохранён. Прямое подключение
  Litterkeep не проверялось. G17 остаётся RUNNING.

### 2026-09-25: G17 RFC 5228 header whitespace checkpoint

- Sieve `header` теперь обрезает ведущие и завершающие SP/HTAB перед MIME
  декодированием и сравнением. Regression проверяет `:is` со значением,
  обрамлённым обоими видами whitespace; профиль обновлён со ссылкой на RFC
  5228 §5.7.
- PASS: Windows Rust 1.97.1, fmt, 323 теста, strict Clippy и diff check.
  Linux Docker Rust 1.97: strict Clippy и 363 теста. Stage использует
  `./data/g17-sieve-rfc2047`; контейнер test target завершился, иных временных
  контейнеров не создавалось. Прямое подключение Litterkeep не проверялось.
  G17 остаётся RUNNING.

### 2026-09-25: G17 RFC 5228 header charset comparison checkpoint

- Sieve `header` сравнивает RFC 2047 header text после общей MIME-декодировки
  в Unicode/UTF-8. Unit regression проверяет совместное декодирование
  ISO-8859-1 и UTF-8 encoded-word в Subject. Независимый acceptance расширен:
  ManageSieve загружает фильтр с Unicode условием `café☃`, Alice отправляет
  RFC 2047-кодируемое SMTP-письмо, а Bob подтверждает доставку в `Invoices`
  через IMAP.
- PASS: Windows Rust 1.97.1, fmt, 322 теста и strict Clippy; Python fixture
  compile и diff check. Linux Docker Rust 1.97: strict Clippy и 362 теста.
  Независимый ManageSieve -> SMTP -> IMAP сценарий прошёл. Рабочий каталог
  `./data/g17-sieve-rfc2047`; временный контейнер удалён, `litterkeep-g12-phone`
  сохранён. Прямое подключение Litterkeep не проверялось. G17 остаётся RUNNING.

### 2026-09-25: G17 RFC 5228 octet glob checkpoint

- `:matches` теперь токенизирует pattern по UTF-8 octets: базовые
  `i;octet`/`i;ascii-casemap` считают `?` одним octet, а `\*`/`\?` после
  Sieve lexical unescape становятся буквальными wildcard-символами. Добавлены
  matcher tests для многобайтного UTF-8, обоих comparator-ов и literal `*`;
  составной Sieve test включает header, escaped glob, subaddress и fileinto.
- PASS: Windows Rust 1.97.1, fmt, 321 тест, strict Clippy, Python compile и
  diff check. Linux Docker Rust 1.97: strict Clippy и 361 тест. Независимый
  ManageSieve -> SMTP -> IMAP acceptance с Subject `invoice* [invoice] paid`
  активировал reordered script и доставил письмо в `Invoices`. Первый прогон
  выявил неверный acceptance pattern без завершающего wildcard; исправленный
  шаблон и весь acceptance прошли на новом чистом stage
  `./data/g17-sieve-glob-octets-verified`. Контейнер удалён,
  `litterkeep-g12-phone` сохранён. G17 остаётся RUNNING.

### 2026-09-25: G17 RFC 5228 address-test semantics checkpoint

- Sieve `address` теперь игнорирует неадресные поля, поддерживает обязательные
  address-list headers вместе с полями `Resent-*` и переиспользует RFC 5322
  parser из MIME. Адреса членов group участвуют в сравнении, название группы
  нет; добавлены unit regressions для Subject, Resent-To и группы.
- PASS: Windows Rust 1.97.1, fmt, 318 тестов, strict Clippy, Python compile и
  diff check. Linux Docker Rust 1.97: strict Clippy и 358 тестов. Независимый
  ManageSieve -> SMTP -> IMAP acceptance прошёл с group-form заголовком To и
  фактической доставкой по subaddress; контейнер удалён, `litterkeep-g12-phone`
  сохранён. Stage `./data/g17-sieve-address-rfc`. Litterkeep app integration
  не проверялась. G17 остаётся RUNNING.

### 2026-09-25: G17 RFC 5228 address-tag ordering checkpoint

- Sieve `address` теперь разбирает `ADDRESS-PART`, `MATCH-TYPE` и
  `COMPARATOR` в любом порядке до позиционных списков. Каждая опция может
  встретиться один раз; конфликтующие дубликаты отклоняются. Выделен отдельный
  `address_options.rs`, файл содержит 66 строк.
- PASS: Windows Rust 1.97.1, fmt, 315 тестов, strict Clippy, Python compile и
  diff check. Linux Docker Rust 1.97: strict Clippy и 355 тестов. Независимый
  ManageSieve STARTTLS acceptance проверил переставленные теги в CHECKSCRIPT;
  активный script с перестановкой реально маршрутизировал SMTP письмо, которое
  было прочитано через IMAP. Stage `./data/g17-sieve-address-option-order`.
  Временный контейнер удалён, `litterkeep-g12-phone` сохранён. Интеграция с
  приложением Litterkeep не проверялась. G17 остаётся RUNNING.

### 2026-09-25: G17 RFC 5233 subaddress checkpoint

- `:user`/`:detail` теперь распознаются как `subaddress` extension и требуют
  `require "subaddress"`; ManageSieve greeting и CAPABILITY сообщают
  `fileinto subaddress`. Принятая политика выделяет detail по первому `+`:
  адрес без separator не совпадает с `:detail`, а `user+@domain` совпадает с
  пустой строкой. Ограничения отражены в protocol coverage.
- PASS: Windows Rust 1.97.1, fmt, 314 тестов, strict Clippy, Python compile и
  diff check. Linux Docker Rust 1.97: strict Clippy и 354 теста. Независимый
  ManageSieve STARTTLS-клиент проверил greeting, CHECKSCRIPT до/после require,
  literal upload и активацию; SMTP -> Sieve -> IMAP delivery в `Invoices`
  прошла. Stage `./data/g17-sieve-subaddress`. Временный контейнер удалён,
  `litterkeep-g12-phone` сохранён. Интеграция с приложением Litterkeep не
  проверялась. G17 остаётся RUNNING.

### 2026-09-25: G17 Sieve RFC 5228 string-list grammar checkpoint

- Квадратный `string-list` теперь требует одну или более строк; соседние
  элементы разделяются ровно одной запятой. Пустые списки, пропуск разделителя,
  ведущая/завершающая и повторная запятые отклоняются во всех командах и тестах,
  использующих общий parser.
- PASS: Windows Rust 1.97.1, fmt, 311 тестов, strict Clippy и diff check;
  Linux Docker Rust 1.97, strict Clippy и 351 тест. Независимый ManageSieve
  STARTTLS acceptance отверг список без запятой и принял корректный список;
  полный SMTP -> Sieve -> IMAP сценарий прошёл. Stage
  `./data/g17-sieve-string-list`; тестовый контейнер удалён,
  `litterkeep-g12-phone` оставлен. Интеграция с приложением Litterkeep не
  проверялась. G17 остаётся RUNNING.

### 2026-09-25: G17 Sieve RFC 5228 test-list grammar checkpoint

- `allof`/`anyof` теперь требуют `test-list` в круглых скобках, разделённый
  запятыми и содержащий минимум один тест. Квадратная форма string-list
  больше не принимается на этом месте; проверки покрывают пустой список,
  пропущенную/висячую запятую и вложенные тесты.
- PASS: Windows Rust 1.97.1, fmt, 310 тестов, strict Clippy и diff check;
  Linux Docker Rust 1.97, strict Clippy и 350 тестов. Независимый ManageSieve
  STARTTLS-клиент отверг квадратный `allof`, принял nested parenthesized
  `allof`/`anyof`, сохранил и активировал script. Сквозная SMTP -> Sieve ->
  IMAP доставка в `Invoices` прошла. Stage `./data/g17-sieve-test-list`;
  временный контейнер удалён, `litterkeep-g12-phone` сохранён. Интеграция с
  приложением Litterkeep не проверялась. G17 остаётся RUNNING.

### 2026-09-25: G17 Sieve RFC 5228 quoted-string checkpoint

- Quoted-string лексер сохраняет CRLF, принимает quoted-other escape как
  символ после обратной косой черты и отвергает NUL. Вне строки допускаются
  только space, HTAB и пары CRLF; непарные CR/LF отвергаются.
- PASS: Windows Rust 1.97.1, fmt, 309 тестов, strict Clippy и diff check;
  Linux Docker Rust 1.97, strict Clippy и 349 тестов. Независимый ManageSieve
  STARTTLS-клиент через CHECKSCRIPT принял CRLF в строке и `\a`, отверг NUL и
  одиночный LF, затем сохранил/активировал script. SMTP -> Sieve -> IMAP
  доставка в `Invoices` прошла. Stage `./data/g17-sieve-quoted-string`;
  временный контейнер удалён, `litterkeep-g12-phone` сохранён. Проверка
  приложения Litterkeep не выполнялась. G17 остаётся RUNNING.

### 2026-09-25: G17 Sieve RFC 5228 require-order checkpoint

- Parser отклоняет `require`, если до него встретилась любая обычная команда;
  несколько `require` до первой команды остаются допустимыми. Проверка
  закреплена unit regression и независимой ManageSieve сетевой загрузкой.
- PASS: Windows Rust 1.97.1, fmt, 307 тестов, strict Clippy и diff check;
  Linux Docker Rust 1.97, strict Clippy и 347 тестов. Независимый ManageSieve
  клиент проверил отказ позднего `require`, сохранил и активировал корректный
  script, после чего SMTP доставил письмо в `Invoices` и IMAP его прочёл.
  Stage `./data/g08-sieve-require-order`; временный контейнер удалён,
  `litterkeep-g12-phone` сохранён. Проверка приложения Litterkeep не заявляется.
  G17 остаётся RUNNING.

### 2026-09-25: G16 POP3 RFC 1939 repeated DELE checkpoint

- `DELE n` теперь возвращает `-ERR`, если `n` уже отмечен до `RSET`; решение
  кодируется типизированным `DeleteError`, а не проверкой текста. Это правило
  RFC 1939 §5, в котором номер удалённого сообщения недопустим для повторной
  команды DELE.
- PASS: Windows Rust 1.97.1, 306 тестов, strict Clippy, fmt, Python compile и
  diff check. Linux Docker test target: strict Clippy и 346 тестов. Независимый
  Python POP3-клиент отправил два письма по SMTP и проверил отказ повторного
  DELE, видимость STAT/LIST/UIDL, RSET и durable QUIT. Stage
  `./data/g16-pop3-dele-repeat-final2`; временный контейнер удалён,
  `litterkeep-g12-phone` сохранён. Интеграция с приложением Litterkeep не
  проверялась. G16 остаётся RUNNING.

### 2026-09-25: G14 RFC 3501 legacy RFC822 section FETCH checkpoint

- Реализованы обязательные для рекламируемого IMAP4rev1 data items
  `RFC822.HEADER` и `RFC822.TEXT`. Ответы сохраняют legacy labels, используют
  общий BODY section selector и возвращают точные байты. Header не меняет
  `\\Seen`; чтение TEXT сначала durable фиксирует `\\Seen`.
- PASS: Windows Rust 1.97.1, 306 тестов, strict Clippy, fmt и diff check;
  Linux Docker Rust 1.97, strict Clippy и 346 тестов. Независимый SMTP-to-IMAP
  acceptance проверил исходные header/body octets, response labels и Seen
  до/после обоих items. Stage `./data/g14-rfc822-fetch`. Тестовый контейнер
  удалён, `litterkeep-g12-phone` сохранён. Интеграция с приложением Litterkeep
  не проверялась. G14 остаётся RUNNING.

### 2026-09-25: G14 RFC 2047 address phrase checkpoint

- RFC 2047 encoded-word теперь распознаётся в отдельных словах unquoted
  display-name/group phrase; пробел между соседними encoded-word игнорируется.
  Некорректное слово сохраняется как видимый текст, чтобы оно не мешало
  обработке сообщения, а quoted display-name остаётся буквальным.
- PASS: Windows Rust 1.97.1, 305 тестов, strict Clippy, fmt и diff check;
  Linux Docker Rust 1.97, strict Clippy и 345 тестов. Независимый Linux
  STARTTLS IMAP acceptance проверил ENVELOPE и FROM SEARCH на encoded phrase,
  плюс Sender и quoted Reply-To. Stage `./data/g14-rfc2047-address-phrase`.
  Временный контейнер удалён; `litterkeep-g12-phone` сохранён. Интеграция с
  приложением Litterkeep не проверялась. G14 остаётся RUNNING.

### 2026-09-25: G16 POP3 RFC 1939 deleted-message visibility checkpoint

- Исправлено правило UPDATE: после `DELE` отмеченное сообщение немедленно
  исключается из `STAT`, многострочных `LIST`/`UIDL` и отклоняется для одиночных
  `LIST n`/`UIDL n`; номера остальных сообщений сохраняются в snapshot.
  `RSET` восстанавливает отображение, а успешный `QUIT` durable удаляет только
  отмеченный UID. Это соответствует RFC 1939 §§5-6.
- PASS: Windows Rust 1.97.1, 287 тестов и strict Clippy; Linux Docker Rust
  1.97, strict Clippy и 327 тестов. Независимый Linux runtime acceptance
  доставил два сообщения через SMTP и проверил POP3 STAT/LIST/UIDL после DELE,
  восстановление через RSET и окончательное удаление ровно одного UID по QUIT.
  Stage `./data/g16-pop3-deleted-visibility-final2`; тестовый контейнер удалён,
  `litterkeep-g12-phone` сохранён.
- G16 остаётся RUNNING.

### 2026-09-25: G16 POP3 RFC 8314 QUIT shutdown checkpoint

- После успешного POP3 QUIT reactor дожидается отправки `+OK`, инициирует
  TLS `close_notify`, и освобождает session/maildrop lease только после flush
  alert. QUIT в authorization и завершённый durable UPDATE проходят одним
  выходным lifecycle; plaintext POP3 также дожидается ответа перед закрытием.
- PASS: Windows Rust 1.97.1: 286 тестов и strict Clippy. Linux Docker Rust 1.97
  strict Clippy и 326 тестов. Независимый implicit-TLS socket test проверил
  greeting/authentication/QUIT и завершил TLS `unwrap()` без EOF/truncation;
  значит сервер отправил close alert после `+OK`. Stage `./data/g16-pop3-rfc8314-close-final`;
  тестовый контейнер удалён, `litterkeep-g12-phone` сохранён.
- Peer-initiated close alert без QUIT и аварийные transport closures остаются
  вне graceful-shutdown guarantee. G16 остаётся RUNNING.

### 2026-09-25: G16 POP3 MIME binary CTE checkpoint

- MIME tree рекурсивно определяет `Content-Transfer-Encoding: binary` во всех
  распознаваемых сущностях, в том числе в `multipart/*` подтипах, ранее
  трактовавшихся как непрозрачные листья. POP3 `TOP` и `RETR` отклоняют такое
  сообщение и в UTF8 mode, как требует RFC 6856 §2.1.
- PASS: Windows Rust 1.97.1: 286 тестов и strict Clippy. Linux Docker Rust
  1.97: strict Clippy и 326 тестов. Независимый Linux runtime acceptance
  сделал IMAP STARTTLS/APPEND multipart письма с `binary` CTE и NUL в теле,
  затем подтвердил отказ POP3 TOP/RETR даже после UTF8. Повторно прошли SMTP
  UTF8 delivery, POP3 LANG/UTF8/STLS сценарии. Данные находятся в
  `./data/g16-pop3-binary-cte-nested`; тестовый контейнер удалён, постоянный
  `litterkeep-g12-phone` сохранён.
- Ограничение: если MIME parser не строит дерево, наличие вложенного binary CTE
  не устанавливается и не считается проверенным. RFC 6856 в целом, conversion
  legacy charset и UTF-8 credentials остаются непокрытыми. G16 остаётся RUNNING.

### 2026-09-25: G16 POP3 SASL PLAIN over TLS checkpoint

- Реализован POP3 `AUTH PLAIN` с initial response и continuation, отменой `*`,
  строгой Base64/PLAIN-разборкой и credential path через существующую
  аутентификацию. До TLS механизм не рекламируется и AUTH отклоняется; после
  STLS capability меняется на `SASL PLAIN`, а `STLS` исчезает. После успешной
  аутентификации capability остаётся доступной, повторный AUTH отклоняется.
- PASS: Windows Rust 1.97.1: 276 тестов и strict Clippy. Чистая Linux Docker
  сборка Rust 1.97: strict Clippy и 316 тестов. Независимый socket acceptance
  подтвердил оба варианта AUTH PLAIN, cancel, TLS capability и ошибки для
  неподдерживаемого SCRAM. Проверка использовала отдельный
  `./data/g16-pop3-sasl-plain`; временный контейнер удалён,
  `litterkeep-g12-phone` сохранён.
- Последующая сверка RFC 5034 §4 выявила важную семантику: отказ AUTH не должен
  менять состояние так, будто AUTH не выполнялся, чтобы клиент мог повторить
  попытку или продолжить прежнюю USER/PASS пару. Добавлено восстановление этого
  состояния при malformed PLAIN, отмене, отказе backend и занятом maildrop.
- PASS update: Windows Rust 1.97.1: 278 тестов и strict Clippy; чистый Linux
  Docker Rust 1.97: strict Clippy и 318 тестов. Независимый Linux-target
  `tests/blackbox/pop3_reply_lines.py` повторил AUTH после неверного пароля и
  прошёл прежние SASL/CAPA/line-limit сценарии на отдельном
  `./data/g16-pop3-auth-failure-final`. Runtime image собран отдельно от
  verification target; тестовый контейнер удалён, постоянный
  `litterkeep-g12-phone` оставлен.
- G16 по-прежнему покрывает только SASL PLAIN. SCRAM и прочие механизмы,
  полная RFC 5034 совместимость и взаимодействие с litterkeep не проверены.

### 2026-09-25: G16 POP3 UTF8 mode checkpoint

- CAPA сообщает `UTF8` без аргумента `USER`, так как имена/пароли учётных
  записей остаются ASCII. Команда `UTF8` переключает сессию в UTF-8 mode;
  до переключения `LIST`, `TOP` и `RETR` отвечают типизированным `-ERR [UTF8]`,
  если письмо содержит корректные UTF-8 символы вне ASCII. `STLS` после UTF8
  отклоняется согласно разрешённому RFC 6856 поведению.
- PASS: Windows Rust 1.97.1, 281 тест и strict Clippy; чистый Linux Docker
  Rust 1.97, 321 тест и strict Clippy. Независимый socket клиент проверил
  SMTP 8BITMIME-доставку UTF-8 письма, `-ERR [UTF8]` на LIST/TOP/RETR до
  переключения, сохранение точных байтов после UTF8 command, capability
  до/после TLS и порядок STLS. На свежем `./data/g16-pop3-utf8-cache` прошли
  прежние POP3 SASL/RESP-CODES/line-limit regressions и UTF8 content acceptance
  с окончательным кэшем maildrop. Все временные
  контейнеры удалены, `litterkeep-g12-phone` сохранён.
- Это частичный RFC 6856 профиль: conversion из legacy charset и UTF-8
  credentials не реализованы; соответствие MIME binary CTE не проверялось.
  Детектор блокирует корректные
  non-ASCII UTF-8 последовательности, не классифицируя произвольные невалидные
  octets как UTF-8. G16 остаётся RUNNING.

### 2026-09-25: G16 POP3 LANG checkpoint

- Добавлена RFC 6856 capability `LANG`: сервер перечисляет `en English`,
  принимает регистронезависимый диапазон `en` и серверный выбор `*`, а
  синтаксически корректные неподдерживаемые диапазоны отклоняет без смены языка.
  Проверка command parser использует типизированный результат запроса.
- PASS: Windows Rust 1.97.1: 282 теста, strict Clippy, `cargo fmt --check`,
  Python syntax check и `git diff --check`. Linux Docker Rust 1.97: strict
  Clippy и 322 теста. Независимый socket acceptance на Linux runtime проверил
  CAPA и LANG listing/selection в AUTHORIZATION, CAPA и `LANG EN` после входа,
  выбор `*`, отказ `de`, UTF8/STLS семантику и байты UTF-8 письма. Отдельный
  чистый `./data/g16-pop3-lang-regression` прошёл прежние SASL, line-limit,
  длинный RETR и RESP-CODES regressions. Использован UTF8 stage
  `./data/g16-pop3-lang`; временные контейнеры удалены, постоянный
  `litterkeep-g12-phone` сохранён.
- Доступна только английская локаль, поэтому успешный выбор не меняет тексты;
  многоязычная локализация, conversion из legacy charset, UTF-8 credentials и
  соответствие MIME binary CTE не реализованы/не проверены. G16 остаётся RUNNING.

### 2026-09-25: G16 POP3 RFC 1939 LIST/UIDL checkpoint

- `LIST n` и `UIDL n` теперь сериализуются как одиночные `+OK` строки; формы
  без аргумента остаются многострочными. Maildrop кэширует размер, рассчитанный
  по передаваемым CRLF строкам, не считая дополнительный dot-stuffing октет.
- PASS: Windows Rust 1.97.1: 284 теста и strict Clippy. Linux Docker Rust 1.97:
  strict Clippy и 324 теста. Независимый Linux socket тест проверил одиночные
  ответы LIST/UIDL, многострочные формы, 700-октетную строку RETR, совпадение
  LIST octet count с unstuffed RETR и предыдущие SASL/RESP-CODES сценарии.
  Unit-тест проверил LF-only хранимое сообщение с dot-leading строкой и
  CRLF-нормализацией. Отдельно повторён UTF8/LANG acceptance. Данные изолированы
  в `./data/g16-pop3-list-uidl` и `./data/g16-pop3-list-uidl-utf8`; тестовые
  контейнеры удалены, `litterkeep-g12-phone` оставлен.
- Остальные требования RFC 1939 и полного POP3 профиля не покрыты; G16 остаётся
  RUNNING.

### 2026-09-25: G16 POP3 SASL empty initial response checkpoint

- RFC 5034 задаёт единственный `=` как wire representation пустого initial
  response. POP3 command parser теперь переводит его в отдельный enum вариант,
  не передавая маркер Base64 decoder-у. Для поддерживаемого PLAIN нулевой payload
  закономерно отклоняется как некорректные credentials, при этом AUTH состояние
  восстанавливается и следующая AUTH попытка остаётся возможной.
- PASS: Windows Rust 1.97.1: 285 тестов, strict Clippy, fmt и Python syntax.
  Linux Docker Rust 1.97: strict Clippy и 325 тестов. Независимый TLS socket
  acceptance проверил `AUTH PLAIN =`, отказ, повтор с неверным и затем верным
  payload; прошли существующие CAPA, AUTH PLAIN, response limits и POP3 delivery
  regressions. Stage `./data/g16-pop3-sasl-empty`; временный контейнер удалён,
  `litterkeep-g12-phone` сохранён.
- Другие SASL-механизмы и полный RFC 5034 профиль не реализованы; G16 остаётся
  RUNNING.

### 2026-09-25: POP3 RFC 1939 USER/PASS sequencing checkpoint

- AUTHORIZATION FSM различает состояние после успешного `USER`: повторный
  `USER` отклоняется и сбрасывает прежний выбор; неудачный пароль также очищает
  имя и требует новый `USER` перед следующей попыткой `PASS`. Пароль без `USER` отвечает
  `-ERR USER required`. STLS и отказ `IN-USE` также очищают незавершённый выбор.
- PASS: Windows Rust 1.97.1: 270 тестов, strict Clippy, fmt, diff и Python
  syntax checks. Чистый Linux Docker Rust 1.97: strict Clippy и 310 тестов.
  Независимый STLS socket acceptance подтвердил повторный USER rejection,
  failed PASS, запрет PASS retry без USER и затем `[IN-USE]` после нового USER.
- Сценарий использовал `tests/blackbox/pop3_reply_lines.py` и изолированный
  `./data/g15-pop3-auth-state`; SMTP delivery, CAPA response limits и длинная
  RETR строка остались зелёными. Runtime-контейнер удалён;
  `litterkeep-g12-phone` сохранён.
- Это проверка USER/PASS переходов, а не полной RFC 1939 auth grammar или POP3
  совместимости. Другие протоколы и G15 остаются в работе.

### 2026-09-25: POP3 RFC 2449 RESP-CODES/IN-USE checkpoint

- Объявленная `RESP-CODES` capability теперь подкреплена типизированным
  `ResponseCode::InUse`: после успешной проверки пароля конфликт занятого
  maildrop возвращает `-ERR [IN-USE] ...` через общий ограниченный wire encoder.
- PASS: Windows Rust 1.97.1: 269 тестов, strict Clippy, fmt, diff и Python
  syntax checks. Чистый Linux Docker Rust 1.97: strict Clippy и 309 тестов.
  Независимый STLS acceptance проверил RESP-CODES до и после AUTH, успешную
  первую maildrop lease и `[IN-USE]` при одновременном PASS второго клиента.
- Сквозной тест также подтвердил POP3 line limits и точную 700-октетную RETR
  строку после SMTP-доставки. Использованы `tests/blackbox/pop3_reply_lines.py`
  и `./data/g15-pop3-resp-codes`; runtime контейнер удалён, постоянный
  `litterkeep-g12-phone` сохранён.
- Проверены только RESP-CODES/IN-USE и указанные wire-сценарии. Прочие коды,
  расширения и POP3 состояния не объявляются полностью совместимыми; G15 RUNNING.

### 2026-09-25: POP3 RFC 1939/2449 response-line bound checkpoint

- POP3 status-строки сериализуются с пределом 512 октетов вместе с CRLF;
  CAPA/LIST/UIDL multiline frames проверяют каждую строку и dot terminator.
  RETR/TOP типизированы отдельно: их status ограничен, но RFC822 message lines
  не обрезаются протокольным лимитом.
- PASS: Windows Rust 1.97.1: 268 тестов, strict Clippy, fmt, diff и Python
  syntax checks. Чистый Linux Docker Rust 1.97: strict Clippy и 308 тестов.
  Независимый STLS клиент получил корректные ограниченные greeting/CAPA/LIST/
  UIDL ответы, доставил Alice->Bob сообщение и прочёл в RETR точную 700-октетную
  строку тела.
- Тест: `tests/blackbox/pop3_reply_lines.py`; отдельные stage data/TLS:
  `./data/g15-pop3-reply-lines`. Временный контейнер удалён;
  `litterkeep-g12-phone` оставлен.
- Проверены только перечисленные response-line и long-message сценарии.
  Остальные состояния POP3, extension semantics и совместимость клиентов не
  сертифицированы; G15 остаётся RUNNING.

### 2026-09-25: POP3 RFC 2449 command-line bound checkpoint

- POP3 parser принимает только завершённые CRLF команды из printable ASCII,
  разделённые одиночным SP. Предел CAPA-enabled RFC 2449 равен 255 октетам
  целиком с CRLF; более длинная строка получает `-ERR`, уже потреблённая строка
  не ломает синхронизацию следующей команды.
- PASS: Windows Rust 1.97.1: 263 теста, strict Clippy, fmt и diff checks.
  Linux Docker Rust 1.97: release image, strict Clippy и 303 теста. Независимый
  STLS клиент подтвердил 255 accepted, 256 rejected, затем успешные USER/PASS,
  STAT и QUIT в той же сессии.
- Тест: `tests/blackbox/pop3_command_length.py`; stage data/TLS:
  `./data/g15-pop3-command-length`. Временный контейнер удалён;
  `litterkeep-g12-phone` оставлен.
- Это только POP3 command framing/grammar checkpoint. Остальные состояния,
  ответы, extension semantics и взаимодействие с клиентами продолжают аудит.

### 2026-09-25: G15 RFC 5321 Postmaster acceptance checkpoint

- RCPT принимает bare `<Postmaster>` без домена и локальный
  `postmaster@domain` без учёта регистра local-part; сравнение domain следует
  DNS case-insensitive правилам. Оба варианта направляются в отдельный
  системный mailbox `postmaster`, не создавая парольную account-запись.
- PASS: Windows Rust 1.97.1: 259 тестов, strict Clippy, fmt и diff checks.
  Чистая Linux Docker Rust 1.97 сборка прошла strict Clippy и 299 тестов.
  Независимый STARTTLS SMTP black-box принял bare и qualified Postmaster,
  отклонил необслуживаемый домен 550, отправил DATA и подтвердил непустой
  долговечный `mailboxes/postmaster.journal`.
- Тест: `tests/blackbox/smtp_postmaster.py`; отдельные данные:
  `./data/g15-smtp-postmaster`. Временный контейнер удалён;
  `litterkeep-g12-phone` оставлен.
- Системный mailbox сохранён, но текущая account-аутентификация не предоставляет
  операторский доступ к нему. Quoted mailbox, address-literals, source routes и
  SMTPUTF8 остаются непокрытыми; G15 остаётся RUNNING.

### 2026-09-25: G15 RFC 5321 dot-atom mailbox checkpoint

- SMTP parser проверяет обычный ASCII dot-atom local-part и DNS domain labels
  в обеих командах MAIL FROM/RCPT TO: запрещены пустые/краевые/повторные точки,
  пустые или более 63 октетов labels, дефис по краям и не-ASCII домен.
- PASS: Windows Rust 1.97.1: 256 тестов, strict Clippy, fmt и diff checks.
  Linux Docker Rust 1.97: release image, strict Clippy и 296 тестов.
  Независимый STARTTLS SMTP black-box получил 501 для malformed mailbox в MAIL
  и RCPT, 250 для корректных путей; recipient-limit black-box и доставка Bob
  через IMAP также прошли.
- Проверки: `tests/blackbox/smtp_mailbox_syntax.py` и прежний
  `tests/blackbox/smtp_recipient_limit.py`; данные `./data/g15-smtp-mailbox-syntax`.
  Временный контейнер удалён, `litterkeep-g12-phone` оставлен.
- Ограниченный parser пока не принимает quoted local-parts, domain literals,
  source routes и SMTPUTF8; G15 остаётся RUNNING, RFC 5321 профиль не закрыт.

### 2026-09-25: G15 RFC 5321 recipient buffer limit checkpoint

- SMTP проверяет вместимость envelope до асинхронной проверки локальной политики.
  Первые 100 RCPT принимаются, следующий получает `452 4.5.3`; уже принятые
  адресаты и возможность завершить DATA сохраняются.
- PASS: Windows Rust 1.97.1: 251 тест, strict Clippy, fmt и diff checks. Linux
  Docker Rust 1.97: release-сборка, strict Clippy и 291 тест. Независимый
  STARTTLS SMTP/IMAP black-box принял 100 RCPT, получил 452 на 101-й, отправил
  DATA и нашёл сообщение у Bob через IMAP.
- Black-box: `tests/blackbox/smtp_recipient_limit.py`; stage data/TLS:
  `./data/g15-smtp-recipient-limit`. Временный контейнер удалён;
  `litterkeep-g12-phone` оставлен.
- G15 остаётся RUNNING: лимит буфера RCPT не является полной реализацией
  RFC 5321 и современных ESMTP-расширений.

### 2026-09-25: G15 RFC 5321 local-part octet checkpoint

- Для обычного SMTP mailbox адреса парсер ограничивает local-part 64 октетами;
  единая проверка действует для reverse-path MAIL FROM и forward-path RCPT TO.
  Ограничение учитывает UTF-8 byte length, а не число Unicode scalar values.
- PASS: Windows Rust 1.97.1: 251 тест, strict Clippy, fmt и diff checks.
  Linux Docker Rust 1.97: release image, strict Clippy и 291 тест.
  STARTTLS black-box подтвердил 64-октетный mailbox на parser/policy границе
  и `501` при 65 октетах для MAIL/RCPT; также прошли path, command, SIZE,
  8BITMIME и SMTP-to-IMAP regression проверки.
- Функциональный тест `tests/blackbox/smtp_local_part_length.py` проходит
  py_compile и runtime. Данные этапа `./data/g15-smtp-local-part`; временный
  контейнер удалён, `litterkeep-g12-phone` оставлен.
- G15 остаётся RUNNING: не реализованы полная RFC mailbox/address грамматика,
  Postmaster routing, SMTPUTF8 и прочие обязательные проверки профиля.

### 2026-09-25: G15 RFC 5321 reverse/forward path limit checkpoint

- SMTP parser измеряет reverse-path и forward-path в октетах вместе с `< >`.
  Пути длиной до 256 принимаются грамматикой, 257 отклоняются как syntax error;
  одинаковое правило применяется к MAIL FROM и RCPT TO.
- PASS: Windows Rust 1.97.1: 250 тестов, strict Clippy, fmt и diff checks.
  Linux Docker Rust 1.97: release-сборка, strict Clippy и 290 тестов.
  Независимый SMTP runtime проверил границы обоих путей, затем повторно прошли
  command-line 512/513, SIZE/8BITMIME и фактическая SMTP-to-IMAP доставка.
- Black-box `tests/blackbox/smtp_path_length.py` проходит py_compile и runtime.
  Stage data/TLS находятся в `./data/g15-smtp-path-length`; временный контейнер
  удалён, `litterkeep-g12-phone` оставлен.
- G15 остаётся RUNNING: ограничения адресных компонентов и полный RFC 5321
  профиль требуют отдельных проверок.

### 2026-09-25: G15 RFC 5321 command-line length checkpoint

- SMTP command reader проверяет полную строку вместе с CRLF до разбора команды.
  Строка длиннее 512 октетов получает 500; уже отделённая CRLF строка полностью
  потреблена, последующие команды остаются синхронизированы. DATA продолжает
  использовать отдельный лимит RFC 5321 для text lines.
- PASS: Windows Rust 1.97.1: `cargo test --all-targets` (249 тестов), strict
  Clippy, `cargo fmt --check`, `git diff --check`. Linux Docker Rust 1.97:
  release-сборка, strict Clippy, 289 тестов. Независимый runtime клиент получил
  250 на строку длиной 512, 500 на строку длиной 513 и 250 на следующий NOOP.
  Существующий SMTP SIZE/8BITMIME плюс доставка SMTP-to-IMAP повторно прошли.
- Black-box файлы разделены по функциям: `tests/blackbox/smtp_command_line.py`
  и `tests/blackbox/smtp_size_declaration.py`; оба проходят py_compile.
  Данные и тестовый TLS key этапа в `./data/g15-smtp-command-line`. Временный
  runtime контейнер удалён, `litterkeep-g12-phone` оставлен.
- G15 остаётся RUNNING: базовый command line bound не закрывает RFC 5321 целиком.

### 2026-09-25: G15 RFC 5321 repeated MAIL transaction checkpoint

- Открытая SMTP transaction теперь отклоняет повторный MAIL ответом 503; FSM
  сохраняет исходного sender и список RCPT. Отдельно проверены доменный FSM-тест
  и live delivery, после отказа отправив DATA прежнему получателю.
- PASS: Windows Rust 1.97.1: 249 тестов, строгий Clippy, fmt и diff checks.
  Linux Docker Rust 1.97: release image, strict Clippy и 289 тестов.
  Независимый Python black-box по TLS SMTP отправил повторный MAIL, проверил
  доставку исходного конверта и нашёл уникальное сообщение Bob через IMAP.
  Тем же прогоном проверен существующий SIZE/8BITMIME набор.
- Данные `./data/g15-smtp-mail-repeat`, включая stage-local тестовый TLS key.
  Временные контейнеры удалены; `litterkeep-g12-phone` оставлен.
- G15 остаётся RUNNING: этот transaction-order checkpoint не закрывает весь
  RFC 5321 и современные ESMTP расширения.

### 2026-09-25: G15 RFC 5321 greeting and transaction reset checkpoint

- В SMTP session введены `NotEstablished`, `Basic` и `Extended` greeting mode.
  MAIL/RCPT/DATA до HELO/EHLO получают 503. HELO разрешает базовый SMTP, но
  блокирует ESMTP параметры, AUTH и STARTTLS. Успешные HELO/EHLO сбрасывают
  незавершённый envelope и body mode; STARTTLS возвращает session к новому EHLO.
- PASS: Windows Rust 1.97.1: 248 тестов и строгий Clippy. Linux Docker Rust
  1.97: release-сборка, строгий Clippy, 288 тестов и независимый STARTTLS
  black-box. Клиент проверил MAIL до greeting, HELO extension gate, сброс
  envelope повторным HELO/EHLO и полный прежний SMTP SIZE/8BITMIME набор.
- Данные `./data/g15-smtp-rfc-greeting`; временный контейнер удалён,
  `litterkeep-g12-phone` оставлен. Коммит `9f64945`.
- G15 остаётся RUNNING: это начальный RFC 5321 transaction state, не полная
  SMTP-командная и ESMTP-совместимость.

### 2026-09-25: G15 RFC 5321 VRFY, EXPN, HELP and NOOP checkpoint

- SMTP parser распознаёт VRFY/EXPN/HELP и принимает необязательную строку NOOP.
  VRFY всегда возвращает privacy-safe 252 без раскрытия mailbox; EXPN возвращает
  502, поскольку mailing-list expansion не реализован. HELP предоставляет
  многострочный список команд. Эти команды не мутируют активный envelope.
- PASS: Windows Rust 1.97.1: 248 тестов и строгий Clippy. Linux Docker Rust
  1.97: release-сборка, строгий Clippy, 288 тестов и независимый STARTTLS
  black-box: VRFY=252, EXPN=502, HELP multiline, NOOP с параметром, сохранение
  активного envelope при VRFY/RCPT. Данные `./data/g15-smtp-rfc-commands`;
  временный контейнер удалён, `litterkeep-g12-phone` оставлен.
- G15 остаётся RUNNING: это совместимость команд RFC 5321, а не полный профиль.

### 2026-09-25: G15 RFC 6152 8BITMIME checkpoint

- Добавлена typed грамматика `BODY=7BIT/8BITMIME`; сервер рекламирует 8BITMIME
  только с поддержкой полного high-bit octet пути. DATA проверяет SMTP line
  limit и ASCII в 7BIT режиме; избыточное тело отбрасывается до terminator.
- Независимая проверка отправляет MIME text с UTF-8 octets через STARTTLS,
  читает BODY.PEEK[TEXT] через IMAP и сравнивает исходные bytes. Та же проверка
  убеждается, что high-bit DATA без BODY=8BITMIME отклоняется после dot и
  соединение остаётся синхронизировано.
- PASS: Windows Rust 1.97.1: 248 тестов, строгий Clippy. Linux Docker Rust
  1.97: release-сборка, строгий Clippy, 288 тестов и независимый SMTP/IMAP
  STARTTLS black-box. Подтверждены UTF-8 payload octets, 7BIT rejection,
  oversized DATA drain и последующий NOOP. Все изменённые исходники короче
  300 строк. Данные: `./data/g15-smtp-8bitmime`; временный контейнер удалён,
  `litterkeep-g12-phone` сохранён.
- Результат покрывает профиль передачи high-bit MIME text и проверенный
  round-trip, но не является исчерпывающей сертификацией RFC 6152 для всех
  MIME валидаторов и SMTP peer комбинаций. SMTPUTF8 ещё не рекламируется.

### 2026-09-25: G15 RFC 1870 SIZE checkpoint

- SMTP-командная грамматика теперь сохраняет `MAIL FROM SIZE=n`, проверяет
  формат RFC 1870 и отвергает неподдерживаемые ESMTP параметры с точным кодом.
  Объявленный предел и фактический лимит DATA используют одно значение.
- PASS: Windows Rust 1.97.1: 247 тестов и строгий Clippy. Linux Docker Rust
  1.97: release-сборка, строгий Clippy, 287 тестов. Независимый STARTTLS
  black-box проверил SIZE advertisement, ранний 552, malformed/duplicate
  параметры (501), unsupported MAIL/RCPT параметры (555) и восстановление
  сессии после отказа.
- Данные этапа: `./data/g15-smtp-size`. G15 остаётся RUNNING: покрыта только
  эта часть SMTP, полный RFC 5321/6409/8314 и SMTPUTF8 ещё не проверены.
  Временный контейнер удалён; `litterkeep-g12-phone` сохранён.

### 2026-09-25: G15 SMTP DATA oversize synchronization checkpoint

- При превышении лимита во время DATA сервер теперь ограничивает память,
  прекращая накопление тела, но остаётся в DATA до `.` и только затем отвечает
  `552`. Это сохраняет SMTP командную синхронизацию и не публикует частичное
  сообщение. Общий буферный предел для остальных протоколов не ослаблен.
- PASS: Windows Rust 1.97.1: 247 тестов. Linux Docker Rust 1.97: release,
  строгий Clippy, 287 тестов. Независимый TLS black-box прислал 25 MiB + 1
  байт без `SIZE`, получил 552 после завершающей точки и 250 на следующий
  NOOP. Временный контейнер удалён, `litterkeep-g12-phone` сохранён.
- Данные: `./data/g15-smtp-size`. G15 остаётся RUNNING: RFC 5321, SMTPUTF8,
  submission-расширения и остальные протоколы ещё требуют инвентаризации.

### 2026-09-25: G14 FETCH BODY item and FULL macro checkpoint

- Добавлен отдельный typed BODY structure item. FULL теперь раскрывается
  строго в FLAGS, INTERNALDATE, RFC822.SIZE, ENVELOPE и BODY; BODYSTRUCTURE
  остаётся доступным как отдельный item. До исправления FULL ошибочно выдавал
  BODYSTRUCTURE.
- PASS: Windows Rust 1.97.1, 247 тестов и строгий Clippy; Linux Docker Rust
  1.97, 287 тестов и строгий Clippy; runtime image и независимый SMTP/IMAP
  STARTTLS attachment black-box прошли. Клиент проверил BODY, BODYSTRUCTURE,
  каждый макрос и совпадение decoded байтов вложения.
- Данные этапа: ./data/g14-fetch-body-item. Временный контейнер удалён,
  litterkeep-g12-phone сохранён. Проверка приложения Litterkeep не выполнялась;
  G14 остаётся RUNNING.

### 2026-09-25: G14 RFC 9051 BINARY checkpoint

- FETCH parser и typed модель принимают BINARY, BINARY.PEEK и BINARY.SIZE.
  Собственный MIME path resolver выбирает leaf part, декодирует transfer
  encoding и применяет partial к decoded octets; текстовые части выдаются с
  CRLF canonicalization, NUL сериализуется literal8, UNKNOWN-CTE сохраняет
  стандартизованный response code. BINARY без PEEK фиксирует Seen, PEEK и SIZE
  его не меняют.
- PASS: Windows Rust 1.97.1, 246 тестов, строгий Clippy; Linux Docker Rust 1.97,
  286 тестов и строгий Clippy; runtime image собран. Независимый STARTTLS
  black-box доставил MIME вложение через SMTP, проверил BINARY.SIZE, каждый
  decoded байт, literal8 с NUL, partial, Seen/PEEK и tagged UNKNOWN-CTE.
  Подключение приложения Litterkeep не проверялось.
- Данные этапа: ./data/g14-binary-fetch. Временный контейнер удалён,
  litterkeep-g12-phone сохранён. G14 остаётся RUNNING: полный IMAP4rev2
  baseline и остальные RFC 9051 команды/расширения ещё не закрыты.

The actually available Goal interface was inspected. It supports creating and
updating one active root goal (`create_goal`, `get_goal`, `update_goal`), but
has no subgoal/dependency operation. The G00-G12 rows below are therefore the
authoritative local subgoal ledger, rather than invented Goal API calls.

## Protocol-completeness extension

Root Goal ID: `01a0ba65-3922-7d21-ac75-0963ef79819a`
Objective: довести стандартизованные профили SMTP, IMAP, POP3, MIME, Sieve и
ManageSieve до заявленного набора современных форм, подтверждая каждую
возможность сетевыми и отрицательными тестами. «Полный» означает перечисленные
в матрицах RFC-команды и расширения, а не неограниченное обещание реализовать
любую спецификацию почты.

| Goal | Depends on | Acceptance criterion | Status |
|---|---|---|---|
| G13 | G12 | IMAP FETCH data-item list, ALL/FAST/FULL, UID response semantics, literal wire and attachment-byte acceptance | PASS |
| G14 | G13 | IMAP4rev2 RFC 9051 baseline plus tested IMAP4rev1 client compatibility | RUNNING |
| G15 | G12 | SMTP RFC 5321/6409/8314 baseline, including SMTPUTF8 RFC 6531 | NOT RUN |
| G16 | G12 | POP3 RFC 1939 extensions RFC 2449/5034/6856 and secure TLS profile | RUNNING |
| G17 | G12 | MIME RFC 5322/2045-2049 updates and Sieve RFC 5228/ManageSieve RFC 5804 profile | RUNNING |
| G18 | G14,G15,G16,G17 | Cross-protocol Linux acceptance, truthful capabilities and final coverage matrix | NOT RUN |

### 2026-09-25: G14 RFC 5322 address comments checkpoint

- Address projection removes nested RFC 5322 comments and honors quoted-pairs
  outside quoted strings before ENVELOPE serialization and address SEARCH.
  Parentheses inside quoted display names remain literal data. Invalid unclosed
  comments fail instead of leaking partial address data. This closes only the
  comments portion of RFC 5322 §3.2.2, not the obsolete-address grammar.
- PASS: Windows Rust 1.97.1, 290 tests and strict Clippy; Linux Docker Rust
  1.97, strict Clippy and 330 tests. Updated independent STARTTLS IMAP acceptance
  verified nested and escaped comments in From/To, ENVELOPE values without
  comment text, group markers, and shared SEARCH projection. Stage
  `./data/g14-envelope-comments-final2`; temporary container removed,
  `litterkeep-g12-phone` preserved. Litterkeep app integration was not tested.
- G14 remains RUNNING.

### 2026-09-25: G14 quoted addr-spec separator checkpoint

- Address parsing now selects the RFC 5322 addr-spec `@` separator outside
  quoted local-parts and domain literals. A quoted local-part containing `@`
  is retained as one ENVELOPE mailbox component; malformed multiple external
  separators are rejected. SEARCH uses the resulting shared projection.
- PASS: Windows Rust 1.97.1, 292 tests, strict Clippy and fmt check; Linux
  Docker Rust 1.97, strict Clippy and 332 tests. Independent Linux STARTTLS
  IMAP acceptance verified exact ENVELOPE mailbox/hostname components and
  address SEARCH for `"a@b"@example.test`. Litterkeep app integration was not
  tested. Data: `./data/g14-quoted-localpart-final`.
- Temporary container `litterkeep-g14-quoted-localpart-debug` was removed;
  persistent `litterkeep-g12-phone` was preserved. G14 remains RUNNING.

### 2026-09-25: G14 trailing group CFWS checkpoint

- RFC 5322 group terminator `;` may be followed by CFWS. The address scanner
  now accepts a trailing comment there and omits it from ENVELOPE, while
  rejecting actual content after the terminator. The independent STARTTLS
  fixture verifies the comment does not leak in the group/mailbox projection.
- PASS: Windows Rust 1.97.1, 294 tests, strict Clippy and fmt; Linux Docker
  Rust 1.97, strict Clippy and 334 tests; independent Linux IMAP acceptance.
  Data remains under `./data/g14-quoted-localpart-final`; the temporary
  container was removed and `litterkeep-g12-phone` was preserved. The app was
  not tested. G14 remains RUNNING.

### 2026-09-25: G14 obsolete source-route checkpoint

- RFC 5322 obsolete route-list inside angle-addr is validated as nonempty and
  ignored during interpretation; only the destination addr-spec contributes
  to IMAP ENVELOPE and address SEARCH. Malformed route and empty destination
  are rejected. This is the source-route subset only, not full obsolete
  addressing support.
- PASS: Windows Rust 1.97.1, 297 tests, strict Clippy and fmt; Linux Docker
  Rust 1.97 strict Clippy and 337 tests; independent Linux STARTTLS IMAP
  acceptance verifies destination projection and absence of route domains.
  Data: `./data/g14-quoted-localpart-final`. Temporary container removed,
  `litterkeep-g12-phone` preserved. Litterkeep app integration not tested.
- G14 remains RUNNING.

### 2026-09-25: G14 obsolete dotted CFWS checkpoint

- RFC 5322 obsolete local-part/domain may contain CFWS around period-separated
  words. The ENVELOPE parser now removes only that separator-adjacent CFWS;
  whitespace and punctuation inside quoted local-parts and domain-literals
  remain unchanged. Both normalized ENVELOPE fields and address SEARCH are
  tested, including comments on each side of a dot.
- PASS: Windows Rust 1.97.1, 300 tests, strict Clippy and fmt; Linux Docker
  Rust 1.97, strict Clippy and 340 tests; independent STARTTLS IMAP acceptance
  verified the parsed mailbox/hostname and SEARCH. Data:
  `./data/g14-quoted-localpart-final`. Temporary container removed,
  `litterkeep-g12-phone` preserved. Litterkeep app integration not tested.
- G14 remains RUNNING.

### 2026-09-25: G14 quoted display-name pair checkpoint

- RFC 5322 quoted-string display-names now remove the quoting backslash from
  quoted-pairs before ENVELOPE serialization and shared address SEARCH.
  Independent STARTTLS IMAP acceptance checks the exact ENVELOPE octets for
  an escaped DQUOTE in From. Existing quoted local-part handling is unchanged.
- PASS: Windows Rust 1.97.1, 301 tests, strict Clippy and fmt; Linux Docker
  Rust 1.97, strict Clippy and 341 tests; independent IMAP acceptance PASS.
  Data: `./data/g14-quoted-localpart-final`. Temporary container removed,
  `litterkeep-g12-phone` preserved. Litterkeep app integration not tested.
- G14 remains RUNNING.

### 2026-09-25: G14 obsolete null address-list members checkpoint

- RFC 5322 obsolete leading, repeated, and trailing null members in address
  lists are ignored without fabricating addresses or reordering mailboxes.
  The independent STARTTLS fixture exercises those forms through FETCH
  ENVELOPE alongside the group, comment, quoted-pair, and route cases.
- PASS: Windows Rust 1.97.1, 302 tests, strict Clippy and fmt; Linux Docker
  Rust 1.97, strict Clippy and 342 tests; independent IMAP acceptance PASS.
  Data: `./data/g14-quoted-localpart-final`. Temporary container removed,
  `litterkeep-g12-phone` preserved. Litterkeep app integration not tested.
- G14 remains RUNNING.

### 2026-09-25: G14 RFC 5322 address groups checkpoint

- ENVELOPE address parsing now preserves typed mailbox, group-start, and
  group-end entries. FETCH ENVELOPE emits RFC 9051 group markers and an empty
  group remains represented. SEARCH address keys use the same decoded
  projection, including the group phrase and member mailbox components.
- Unit tests cover multiple members, empty groups, and unbalanced boundaries.
  Independent STARTTLS black-box verified FETCH ENVELOPE plus TO/CC/BCC search
  over a three-member group, an empty Cc group, and a Bcc member. Это не тест
  подключения из приложения Litterkeep.
- PASS: Windows Rust 1.97.1 strict Clippy и 237 tests; Linux Docker Rust 1.97
  strict Clippy и 272 tests; runtime image build и сетевой black-box PASS.
  Данные: `./data/g14-envelope-groups`.
- Obsolete address forms and remaining RFC 5322 grammar mismatches are still
  limitations. Контейнер этапа удалён, `litterkeep-g12-phone` сохранён. G14
  остаётся RUNNING.

### 2026-09-25: G14 базовый SEARCH key inventory checkpoint

- Сверка parser-а с базовым перечнем ключей RFC 9051 подтвердила, что все
  стандартные ключи уже представлены в typed модели и evaluator-е. Добавлен
  единый исчерпывающий parser-тест, который отдельно проверяет принятие каждого
  ключа, включая sequence-set, UID и обе формы keyword.
- В матрицах устранено устаревшее заявление, что полный базовый набор ключей
  отсутствует. SEARCH остаётся PARTIAL по другим причинам: ограниченный charset,
  не поддержанные SEARCH string literals и не базовые расширения. RFC 5322
  address groups больше не перечисляются как незакрытый пробел; на дату этого
  checkpoint comments и obsolete forms оставались ограничениями parser-а.
- PASS: Windows Rust 1.97.1 strict Clippy и 238 tests; Linux Docker Rust 1.97
  release build, strict Clippy и 273 tests. Сетевой MIME/SEARCH и UID SEARCH
  acceptance остаются отдельными ранее проверенными сценариями; приложение
  Litterkeep этим этапом не проверялось.
- Данные этапа: `./data/g14-search-key-inventory`. Цель G14 остаётся RUNNING.

### 2026-09-25: G14 SEARCH synchronizing literals checkpoint

- IMAP session reactor принимает bounded synchronizing literals в SEARCH и
  UID SEARCH без blocking wait и передаёт восстановленную quoted UTF-8 команду
  существующему typed parser-у. Лимит одного literal: 64 KiB; octet count
  проверяется до чтения, несколько literals в команде обрабатываются последовательно.
- Rust tests проверяют фрагментацию на каждой границе, экранирование кавычек и
  обратной косой черты, пустой literal, неверный UTF-8 и превышение бюджета.
  Независимый STARTTLS black-box проверил BODY, UID SEARCH, два literals в
  HEADER, Unicode UTF-8 и пустую строку. Это не подключение приложения Litterkeep.
- PASS: Rust 1.97 Windows strict Clippy и 238 tests; Linux Docker release build,
  strict Clippy и 278 tests; runtime black-box PASS. Данные сохранены в
  `./data/g14-search-literal`; временный контейнер удалён, `litterkeep-g12-phone`
  сохранён.
- G14 SEARCH остаётся PARTIAL: LITERAL+ не объявляется, MIME charset и прочие
  IMAP4rev2 пробелы не закрыты. Цель остаётся RUNNING.

### 2026-09-25: G14 SEARCHRES UID EXPUNGE checkpoint

- `UID EXPUNGE $` принимает типизированный saved-result set, снимок результата
  фиксируется при постановке операции в очередь, а удаляются только сообщения
  одновременно с `\\Deleted` и сохранённым UID.
- Независимый STARTTLS black-box проверил два удалённых флага: сохранённый UID
  удалён, несохранённый остался; пустой saved result не изменил mailbox.
- Проверки: Rust 1.97.1, 232 Windows unit tests, строгий Clippy, Linux Docker
  Rust 1.97, 267 tests и Clippy, затем runtime black-box PASS. Это не тест
  подключения из приложения Litterkeep. Данные этапа: `./data/g14-searchres-expunge`.
- SEARCHRES capability остаётся скрытой: `$` в COPY/UID COPY и корректный
  порядок конвейерных зависимостей SEARCH SAVE пока не реализованы.
- Временный контейнер этапа удалён после проверки; сохранённый
  `litterkeep-g12-phone` не затрагивался. G14 остаётся RUNNING.

### 2026-09-25: G14 SEARCHRES COPY checkpoint

- `COPY $` и `UID COPY $` разбираются как `MessageSet::Saved`. Очередь
  фиксирует сохранённый UID set при поступлении команды; единый source snapshot
  сопоставляет его UID независимо от sequence/UID addressing, затем выполняется
  существующий durable COPY/COPYUID путь.
- Независимый STARTTLS сценарий создал отдельные назначения и проверил, что
  каждый вариант скопировал ровно одно выбранное письмо. Это не проверка
  подключения из приложения Litterkeep.
- PASS: Windows Rust 1.97.1 strict Clippy и 233 tests; Linux Docker Rust 1.97
  release build, strict Clippy и 268 tests; независимый сетевой black-box.
  Данные: `./data/g14-searchres-copy`.
- Остаточный пробел SEARCHRES: исполнение pipelined SAVE-зависимостей до
  tagged completion. Capability SEARCHRES по-прежнему не рекламируется.
- Временный контейнер `litterkeep-g14-searchres-copy` удалён;
  `litterkeep-g12-phone` сохранён. G14 остаётся RUNNING.

### 2026-09-25: G14 SEARCHRES pipelining and capability checkpoint

- Текущий reactor удерживает последующие строки одной IMAP session до
  завершения ожидающего store operation, затем повторно разбирает буфер после
  tagged response. Это уже обеспечивает требуемый порядок; сетевой тест теперь
  отправляет SEARCH RETURN (SAVE) и COPY `$` подряд без чтения ответа и проверяет
  копирование ровно одного найденного письма.
- Поскольку SEARCH SAVE, ESEARCH result options, `$` consumers, empty-set
  поведение и ordering проверены, защищённый и plaintext CAPABILITY теперь
  объявляет `ESEARCH SEARCHRES`. Linux runtime acceptance проверяет capability.
- PASS после изменения: Windows Rust 1.97.1 strict Clippy и 233 tests; Linux
  Docker Rust 1.97 strict Clippy и 268 tests; независимый STARTTLS black-box,
  включая pipelining. Данные: `./data/g14-searchres-pipeline`.
- Заявление ограничено SEARCHRES, не означает завершение IMAP4rev1/rev2:
  полный набор SEARCH keys, MIME charset и другие RFC пробелы остаются в матрице.
- Этапный контейнер удалён; `litterkeep-g12-phone` оставлен. G14 остаётся
  RUNNING.

### 2026-09-25: G14 SEARCHRES STORE checkpoint

- `STORE $` и `UID STORE $` теперь используют тот же typed saved-result,
  сохранённый при постановке чтения. Поскольку session result хранит UID,
  sequence STORE преобразует его через текущий snapshot; UID STORE напрямую
  выбирает те же UID. Durable flags mutation остаётся одной batch-операцией.
- Независимый STARTTLS black-box сохранил SEARCH и UID SEARCH results,
  выполнил оба STORE варианта, проверил флаги только на выбранных письмах и
  подтвердил корректный tagged OK для пустого `$`. Это не клиент Litterkeep.
- PASS: Windows Rust 1.97.1 format, strict Clippy и 232 tests; Linux Docker
  Rust 1.97 release build, strict Clippy и 267 tests; независимый Python
  STARTTLS acceptance.
- Изолированные данные сохранены в `./data/g14-searchres-store`; созданный
  black-box контейнер остановлен и удалён, `litterkeep-g12-phone` сохранён.
- SEARCHRES остаётся не рекламируемым: `$` в COPY/UID COPY/UID EXPUNGE и
  последовательное выполнение pipelined SAVE-зависимостей ещё не реализованы.

### 2026-09-24: G14 SEARCHRES FETCH checkpoint

- `FETCH $` и `UID FETCH $` принимают typed saved-result selection из RFC 5182.
  В сессии результат хранится по UID; обычный FETCH разрешает эти UID в текущие
  sequence numbers, поэтому выборка остаётся корректной после EXPUNGE. Снимок
  saved UID set фиксируется при постановке read-запроса и переносится через
  durable `\\Seen` follow-up для BODY/RFC822.
- Независимый STARTTLS black-box создал UID gap, сохранил отдельные SEARCH и
  UID SEARCH результаты, проверил FETCH mapping sequence 2 / UID 3, UID FETCH,
  `\\Seen` для BODY и пустой saved result без untagged FETCH. Это не проверка
  приложения Litterkeep.
- PASS: Windows Rust 1.97.1 format, strict Clippy и 231 test; Linux Docker
  Rust 1.97 release build, strict Clippy и 266 tests; Python STARTTLS black-box.
- Данные этапа сохранены в `./data/g14-searchres-fetch`; тестовый контейнер
  после проверки остановить и удалить, `litterkeep-g12-phone` оставить.
- Это не полная реализация SEARCHRES: `$` в STORE/COPY/UID EXPUNGE,
  pipelined ordering/dependency и capability advertisement остаются
  непроверенными. SEARCHRES не рекламируется, G14 остаётся RUNNING.

### 2026-09-24: G14 LIST reference-name checkpoint

- `LIST reference pattern` разрешает относительный шаблон в контексте папки:
  непустая reference получает `/` separator при необходимости, wildcard `*` и
  `%` применяется к полному каноническому имени. Ответ содержит полное имя.
  Пустой pattern выполняет delimiter/root discovery для заданного reference.
- Независимый STARTTLS-клиент проверил root discovery, `%` без перехода на
  следующий уровень, `*` с переходом на вложенный уровень и комбинацию
  reference с `RETURN (CHILDREN)`. Это не проверка пользовательского клиента.
- PASS: Windows Rust 1.97.1 format/strict Clippy/230 tests; Linux Docker
  Rust 1.97 release image, strict Clippy/265 tests и независимый Python
  STARTTLS black-box.
- Данные этапа сохранены в `./data/g14-list-reference`; black-box контейнер
  остановить и удалить после проверки, `litterkeep-g12-phone` не затрагивать.
- Граница реализации: сервер имеет только personal namespace с delimiter `/`;
  namespace breakout и multi-namespace naming не реализованы. Поэтому G14
  остаётся RUNNING, хотя относительный reference profile проверен.

### 2026-09-24: G14 LIST RETURN STATUS checkpoint

- Добавлен `LIST RETURN (STATUS (...))` для MESSAGES, UIDNEXT, UIDVALIDITY,
  UNSEEN, DELETED и SIZE. Один Store-owner snapshot строит точные счётчики из
  mailbox journal и текущих durable flags; тела сообщений не читаются, а SIZE
  вычисляется только по длинам blob-файлов, если поле запрошено.
- Для каждого реально существующего mailbox, совпавшего с pattern и selection,
  ответ STATUS отправляется сразу после LIST. Синтетические родители от
  RECURSIVEMATCH и отсутствующие подписанные mailbox получают LIST по правилам
  выбора, но не получают STATUS. Обычный LIST исключает отсутствующие имена;
  они выбираются только `SUBSCRIBED` selection. `RETURN (SUBSCRIBED)` лишь
  добавляет точный атрибут уже выбранным именам.
- Принимается пустой initial selection `LIST ()`. `RECENT` намеренно отвергается
  внутри rev2 `RETURN STATUS`, но остаётся доступным в совместимой STATUS
  команде rev1. Неизвестные status items отвергаются.
- Независимый STARTTLS-клиент проверил точные счётчики после APPEND с `\\Deleted`,
  STATUS непосредственно после LIST, отсутствие STATUS для несуществующей
  подписанной папки и пустой selection. Это не тест пользовательского клиента.
- Windows Rust 1.97.1: форматирование, strict Clippy, 228 тестов PASS. Linux
  Docker Rust 1.97: release build, strict Clippy, 263 теста PASS; финальный
  независимый STARTTLS black-box прошёл.
- Данные этапа сохранены в `./data/g14-list-status`; контейнер
  `litterkeep-g14-list-status` остановлен и удалён, `litterkeep-g12-phone`
  оставлен нетронутым.
- STATUS return теперь проверен; полный reference-name pattern, прочие
  LIST-EXTENDED элементы и завершение G14 остаются в работе.

### 2026-09-24: G14 LIST RETURN CHILDREN checkpoint

- Добавлены типизированные `RETURN (CHILDREN)` и `RETURN (SUBSCRIBED)` для
  базового LIST, включая комбинацию опций и устранение дубликатов. CHILDREN
  вычисляется по фактическому каталогу mailbox из одного snapshot, а не по
  подпискам: каждый ответ получает ровно `\\HasChildren` либо
  `\\HasNoChildren`. SUBSCRIBED return отображает durable состояние для всех
  совпавших имён; отсутствующие имена выбираются только initial selection
  `SUBSCRIBED`.
- Независимый STARTTLS-клиент проверяет mailbox с дочерней папкой, лист, имя
  подписки без mailbox и комбинированный CHILDREN+SUBSCRIBED. Это не проверка
  пользовательского litterkeep-клиента.
- Windows: Rust 1.97 `cargo fmt --check`, strict Clippy и 225 тестов PASS.
- Linux Docker release build Rust 1.97, strict Clippy и 261 тест PASS; runtime
  image собран, независимый Python STARTTLS black-box прошёл.
- Финально после отрицательного parser-теста: Windows Rust 1.97 format,
  strict Clippy и 226 тестов PASS. Данные сохранены в
  `./data/g14-list-children`; контейнер `litterkeep-g14-list-children`
  остановлен и удалён. `litterkeep-g12-phone` не затронут.
- LIST теперь покрывает CHILDREN и SUBSCRIBED return; полный reference-name
  pattern profile и прочие расширенные элементы остаются непроверенными.
  G14 остаётся RUNNING.

### 2026-09-24: G14 LIST RECURSIVEMATCH checkpoint

- Parser переводит selection options `SUBSCRIBED`, `SPECIAL-USE`,
  `RECURSIVEMATCH` и `REMOTE` в typed criteria. Дубликаты устраняются;
  `RECURSIVEMATCH` без другого selection option возвращает BAD. `REMOTE`
  принимается как no-op, поскольку сервер не имеет удалённых mailbox.
- Storage owner возвращает одним ответом snapshot каталога и подписок.
  Проекция extended LIST выдаёт подходящих потомков и родителей, только если
  родитель совпадает с LIST pattern, дедуплицирует имена и добавляет точный
  `CHILDINFO` для SUBSCRIBED/SPECIAL-USE. Родитель, которого нет в каталоге,
  получает `\\NonExistent`; `\\Subscribed` комбинируется с ним независимо.
- Независимый STARTTLS-клиент проверил подписанные существующие и отсутствующие
  папки, рекурсивный `%` с CHILDINFO для существующего и отсутствующего
  родителя, `*` без лишнего CHILDINFO, SPECIAL-USE selection и BAD для
  одиночного RECURSIVEMATCH: `PASS imap-list-subscribed`. Это не проверка
  пользовательского litterkeep-клиента.
- PASS: Windows Rust 1.97 форматирование, strict Clippy и 224 теста; Linux
  Docker release build Rust 1.97, strict Clippy и 259 тестов; Python TLS
  black-box. Собственный контейнер `litterkeep-g14-list-subscribed` удалён,
  `litterkeep-g12-phone` оставлен. Данные этапа: `./data/g14-list-subscribed`.
- LIST остаётся PARTIAL: STATUS return, полный reference-name profile и другие
  LIST-EXTENDED return options остаются следующими пробелами.
  G14 остаётся RUNNING.

### 2026-09-24: G14 LIST SUBSCRIBED checkpoint

- `LIST (SUBSCRIBED) "" pattern` теперь проходит как типизированная команда
  LIST, отдельно от legacy LSUB. Фильтрация идёт по durable subscription list,
  имена совпадают по каноническому wildcard pattern, включая папки, которых
  сейчас нет в каталоге. Повтор selection-опции принимается как одна опция.
  Ответ LIST включает `\\Subscribed`; отсутствующая папка также помечается
  `\\NonExistent`, как показывает пример RFC 9051. Стандарт требует точного
  набора подписанных имён, допускает отсутствующие папки и требует поддержку
  этого initial selection option.
- Независимый STARTTLS-клиент создал существующую папку, подписал её и
  отсутствующее имя, затем проверил оба точных атрибута в `LIST` response:
  `PASS imap-list-subscribed`. Это не подключение через пользовательский
  litterkeep-клиент.
- PASS: Windows Rust 1.97 `cargo fmt --check`, strict Clippy и 225 тестов;
  Linux Docker Rust 1.97 release build, strict Clippy и 260 тестов; независимая
  Python TLS-проверка. Свой контейнер `litterkeep-g14-list-subscribed`
  остановлен и удалён; пользовательский `litterkeep-g12-phone` оставлен.
  Stage data: `./data/g14-list-subscribed`.
- LIST остаётся PARTIAL: `RECURSIVEMATCH`, `CHILDREN` и LIST RETURN STATUS не
  покрыты; G14 остаётся RUNNING, полный IMAP4rev2 baseline не заявляется.

### 2026-09-24: G14 durable keyword flags and SEARCH checkpoint

- Реализованы пользовательские IMAP keywords как регистронезависимые typed
  atoms. APPEND записывает начальное состояние в journal v4; durable flags
  journal пишет полный mixed system/keyword batch в v2 frame с checksum и
  commit marker. Восстановление продолжает читать APPEND v1-v3 и flags v1,
  обрезает только неполный v2 tail и отвергает повреждённый завершённый frame.
- STORE FLAGS/+/− применяет оба класса флагов одной durable операцией;
  внутреннее выставление `\\Seen` меняет только системную часть. FETCH FLAGS,
  IDLE notifications, COPY/MOVE и повторное открытие сохраняют keywords.
  SELECT сообщает `\\*` в PERMANENTFLAGS. SEARCH KEYWORD/UNKEYWORD использует
  typed KeywordFlag, неизвестные `\\flag-extension` не принимаются.
- PASS: Windows `cargo fmt --check`, strict Clippy и 219 тестов; независимые
  тесты legacy flags v1 и recovery неполного v2 frame. Linux Docker Rust 1.97
  собрал release binary, strict Clippy и 254 теста прошли. IMAP black-box в
  Linux-контейнере проверил APPEND keyword, mixed STORE add/remove/replace,
  FETCH FLAGS, KEYWORD/UNKEYWORD; `verify-only` после перезапуска на актуальном
  образе подтвердил персистентность после смешанной замены flags. Stage data:
  `./data/g14-keywords-replacement`; все созданные для проверки контейнеры
  удалены, user-owned `litterkeep-g12-phone` сохранён.
- Контракт keyword atom сверён с RFC 9051. Ограничение остаётся честным:
  сервер всё ещё рекламирует IMAP4rev1, G14 не закрыт, полный SEARCH набор и
  полный RFC 9051 response grammar ещё не выполнены.

### G13: IMAP FETCH baseline compatibility checkpoint

- `FETCH` is now represented as a typed list of data items across parser,
  pending request and reactor completion. UID FETCH always emits UID, while a
  requested duplicate UID item is not repeated.
- The standard ALL, FAST and FULL macros expand into their protocol-defined
  typed data-item sets. A combined response is bounded by the session output
  limit before publication.
- `tests/blackbox/smtp_imap_attachment.py` now authenticates over STARTTLS,
  verifies grouped FETCH, ALL/FAST/FULL, UID FETCH auto-response semantics, and
  exact decoded attachment bytes.
- PASS: `cargo fmt --check`; `cargo clippy --all-targets --locked -- -D warnings`;
  `cargo test --all-targets --locked` (192 Windows tests); Linux Docker target
  `docker build -f docker/Dockerfile.linux-test --target test -t
  litterkeep-server:g13-test .` (strict Clippy, 222 tests); Python syntax check;
  Linux STARTTLS SMTP -> IMAP black-box attachment verifier returned
  `sha256=3434bddb77229b18aab16f5a2894661d44505548389f84c11bce6d6b50efa4a4`.
- Data is isolated under `./data/g13-protocol-profile`; black-box TLS trust was
  deliberately disabled for the local generated test certificate. Production
  certificate validation is not part of this test.
- G13 does not close IMAP4rev1: BODY header selectors, RFC side effects, the full
  SEARCH key grammar and advertised extensions remain for G14.
- Standards scope was corrected after RFC lookup: the modern IMAP target is
  IMAP4rev2 (RFC 9051), not just the currently advertised IMAP4rev1. SMTP,
  POP3, MIME and Sieve milestones likewise name their base standards and the
  selected current updates/extensions rather than claiming every optional
  extension exists.
- Next: G14, inventory each mandatory IMAP4rev2 command/data grammar against
  executable tests, then implement missing semantics without widening CAPABILITY.

### 2026-09-24: G14 BODY sections and durable Seen checkpoint

- Реализованы RFC BODY section selectors `HEADER`, `TEXT`,
  `HEADER.FIELDS`/`.NOT`, вложенные `message/rfc822` selectors, MIME part bytes
  и partial octet ranges. Field matching не зависит от регистра, повторяющиеся
  поля и исходные folded bytes сохраняются.
- Обычный `BODY[...]` и `RFC822` для writable SELECT теперь сначала записывают
  `\\Seen` через единственный Store owner, затем выдают FETCH. `BODY.PEEK[...]`
  и FETCH в `EXAMINE` не меняют флаг; другие ожидающие IDLE-клиенты получают
  durable FLAGS notification.
- PASS: Windows format/strict Clippy/199 тестов; Linux Docker release build,
  strict Clippy и 231 тест, включая два reactor unit-теста Seen; независимый
  STARTTLS SMTP -> IMAP-клиент сверил `HEADER`, `TEXT`, `HEADER.FIELDS` и
  `.NOT`, подтвердил точные октеты вложения (`sha256=4c7872cd574aabe5cdc9a29678b88b178ef4aac98f051455ff87e207eeea9759`),
  отсутствие `\\Seen` после PEEK и появление после BODY. После остановки и
  повторного запуска сервера отдельное подключение подтвердило сохранение
  `\\Seen`. Данные: `./data/g14-imap-body-sections`.
- G14 остаётся RUNNING: этот checkpoint не подтверждает полный RFC 9051 набор
  команд/расширений и совместимость всего IMAP4rev1 профиля. G15-G18 не начаты.

### 2026-09-24: G14 decoded MIME SEARCH checkpoint

- `BODY`/`TEXT` теперь сравнивают декодированные `text/*` leaf parts: Base64 и
  quoted-printable переводятся в октеты, затем применяется ограниченный charset
  профиль US-ASCII, UTF-8 и ISO-8859-1. RFC 2047 encoded-word декодируется в
  outer headers и MIME-part headers; `HEADER` использует decoded outer values.
  `BODY` не ищет внешние заголовки и не сканирует
  `application/octet-stream` attachment bytes. Projection строится один раз на
  сообщение в пределах одного SEARCH.
- Неизвестная charset и некорректный текст не дают raw-octet совпадений. На
  момент этого checkpoint оставался пробел: неизвестный Content-Transfer-Encoding
  пропускался, а RFC 9051 требует tagged `NO [UNKNOWN-CTE]`. Исправление этого
  поведения зафиксировано отдельным checkpoint ниже. Другие MIME charset пока
  не поддерживаются и не дают совпадений.
- PASS: Windows `cargo fmt --check`, strict Clippy и 209 тестов; Docker Linux
  Rust 1.97 test target прошёл strict Clippy и 241 тест; независимый Python
  STARTTLS-клиент подтвердил Base64 UTF-8, quoted-printable UTF-8,
  RFC 2047 outer/MIME заголовки, различие BODY/TEXT и исключение бинарного
  вложения (`PASS imap-search-mime`). Python syntax и structural audit также
  прошли: 241 Rust-файл, каждый не более 300 строк, `mod.rs` отсутствует.
  Тестовый контейнер `litterkeep-g14-search-mime` удалён после проверки;
  данные сохранены в `./data/g14-search-mime`.
- SEARCH остаётся PARTIAL: неподдерживаемые MIME charset, SAVE/`$`, полный
  mandatory SEARCH key set и полная IMAP4rev2 grammar не закрыты. CAPABILITY
  по-прежнему не объявляет IMAP4rev2.

### 2026-09-24: G14 UNKNOWN-CTE error response checkpoint

- Если BODY/TEXT projection встречает `text/*` часть с неизвестным
  Content-Transfer-Encoding, typed MIME error теперь доходит до SEARCH
  completion. Сервер завершает tagged команду ответом `NO [UNKNOWN-CTE]` и не
  публикует уже вычисляемый или частичный список совпадений. Неизвестная charset
  остаётся отдельным ограничением и не вызывает raw-octet matching.
- PASS: Windows format, strict Clippy, 210 тестов; Docker Rust 1.97 release
  build, strict Clippy и 242 теста. Независимый STARTTLS-клиент повторно
  проверил MIME positive/negative cases и добавил неизвестный CTE в mailbox с
  иным совпадающим сообщением: ответ был `NO [UNKNOWN-CTE]`, без untagged SEARCH
  результата (`PASS imap-search-mime`). Контейнер `litterkeep-g14-search-cte`
  удалён, данные остались в `./data/g14-search-mime`.
- G14/Search остаётся PARTIAL: charset вне текущего профиля, SAVE/`$`, полный
  mandatory SEARCH key set и полная IMAP4rev2 grammar ещё не закрыты.

### 2026-09-24: G14 SEARCH SAVE session-result checkpoint

- SEARCH RETURN принимает SAVE как типизированный ESEARCH option. Успешная
  команда сохраняет UID projection в конкретной IMAP-сессии. Один SAVE подавляет
  ESEARCH response; SAVE вместе с ALL/COUNT сохраняет весь результат, а SAVE
  вместе только с MIN/MAX сохраняет соответствующие крайние сообщения.
  `$` распознаётся как отдельный SearchKey и работает в SEARCH и UID SEARCH,
  независимо от того, каким из этих двух режимов набор был сохранён.
- Обычный SEARCH не изменяет набор. SEARCH RETURN SAVE, завершившийся NO,
  очищает его; BAD grammar не меняет. Успешный SELECT/EXAMINE очищает набор,
  EXPUNGE/MOVE удаляет из него выбывшие UIDs. Независимый STARTTLS тест
  проверяет сохранение, cross-mode UID/sequence projection, отсутствие утечки
  в другую сессию, не-SAVE NO сохранение, SAVE NO очистку и SELECT reset.
- PASS: Windows Rust 1.97 `cargo fmt --check`, strict Clippy и 212 тестов;
  Docker Linux Rust 1.97 release build, strict Clippy и 246 тестов. Независимый
  Python STARTTLS клиент подтвердил MIME SEARCH и SEARCH SAVE сценарии
  (`PASS imap-search-mime`). Python syntax и structural audit прошли: 241
  Rust-файл, все не более 300 строк, `mod.rs` отсутствует. Созданный
  `litterkeep-g14-searchres` удалён; этапные данные сохранены в
  `./data/g14-search-mime`.
- Это не завершённый SEARCHRES профиль: `$` в FETCH/STORE/COPY/UID EXPUNGE
  не принимается, поэтому `SEARCHRES` capability не рекламируется. G14/Search
  остаётся PARTIAL вместе с другими незакрытыми SEARCH keys, MIME charset и
  полным IMAP4rev2 baseline.

### 2026-09-24: G14 ESEARCH empty-result projection checkpoint

- Исправлена сериализация пустого `SEARCH RETURN` результата: ESEARCH остаётся
  в ответе; ALL/MIN/MAX опускаются; COUNT 0 включается только если COUNT был
  запрошен. Пустой `RETURN ()` продолжает означать ALL и не подменяется COUNT.
- PASS: Windows Rust 1.97 format, strict Clippy и 212 тестов; Linux Docker
  Rust 1.97 release build, strict Clippy и 246 тестов. Независимый STARTTLS
  `tests/blackbox/imap_esearch.py` подтвердил непустой ESEARCH и пустые выборки
  ALL, ALL/MIN/MAX/COUNT и `RETURN ()`. Контейнер `litterkeep-g14-esearch-empty`
  удалён после проверки; использованы этапные данные `./data/g14-esearch`.
- G14 остаётся RUNNING: SEARCH/ESEARCH все ещё PARTIAL; ещё требуются `$`
  message sets в других командах, полный mandatory search-key set, charset
  профиль и остальной IMAP4rev2 baseline.

### 2026-09-24: G14 SEARCH key RFC inventory checkpoint

- Сверка с перечнем search keys RFC 9051 отделила реализованные parser/evaluator
  от фактической семантики. `KEYWORD` и `UNKEYWORD` отсутствуют, так как message
  model и durable FLAGS journal поддерживают только системные биты. SUBJECT,
  FROM, TO, CC и BCC пока сканируют decoded RFC 5322 header values, а не
  единую IMAP ENVELOPE projection; на malformed и необычных заголовках это
  оставляет несовместимость. Границы записаны в `docs/imap-profile.md`.
- Этот checkpoint намеренно не помечает SEARCH полным: следующий этап должен
  безопасно добавить durable custom flags и их APPEND/STORE/COPY/MOVE/FETCH/replay
  семантику, а отдельный сетевой этап должен выровнять address criteria с
  ENVELOPE. Источник нормы: RFC 9051, раздел 6.4.4.
- G14 остаётся RUNNING; capability IMAP4rev2/SEARCHRES не расширялась.

### 2026-09-24: G14 envelope-based address SEARCH checkpoint

- SUBJECT/FROM/TO/CC/BCC получили отдельный typed `Envelope` search key и
  сравниваются с decoded ENVELOPE projection, общей с FETCH ENVELOPE. Для
  адреса сопоставляются display name, mailbox, host и канонический
  `mailbox@host`; wire delimiters вроде `<` не создают ложное совпадение.
  Обычный HEADER поиск остаётся привязан к исходным header values.
- PASS: Windows Rust 1.97 strict Clippy и 213 тестов; Linux Docker Rust 1.97
  strict Clippy, release build и 248 тестов, включая Linux-only evaluator test.
  Независимый STARTTLS `tests/blackbox/imap_search_mime.py` проверил пять
  envelope criteria, отрицательный `<alice` match и отдельное положительное
  `HEADER FROM` совпадение. Контейнер `litterkeep-g14-envelope-search` удалён;
  данные этапа `./data/g14-search-mime` сохранены. Structural audit: 241 Rust
  файл, все не длиннее 300 строк, `mod.rs` отсутствует.
- Ограничения общего ENVELOPE address parser-а на группы и malformed syntax
  остаются; `KEYWORD`/`UNKEYWORD` требуют отдельного durable custom-flags этапа.
  G14 остаётся RUNNING и IMAP4rev2 capability не объявляется.

### 2026-09-24: G14 NAMESPACE and UNSELECT checkpoint

- Добавлены typed-команды `NAMESPACE` и `UNSELECT`. Персональный namespace
  сообщает prefix `""` и delimiter `/`, а недоступные other-user/shared классы
  остаются `NIL`. `UNSELECT` возвращает сессию в authenticated state, очищает
  selected projection и не expunge-ит сообщения с `\\Deleted`.
- `CAPABILITY` теперь рекламирует `NAMESPACE` и `UNSELECT` вместе с прежним
  IMAP4rev1 профилем. Это расширение не рекламирует IMAP4rev2 и не меняет
  существующую TLS/authentication политику.
- PASS: Windows typed parser test/strict Clippy; Linux Docker release build,
  strict Clippy и 232 теста; независимый STARTTLS Python `imaplib` тест
  подтвердил advertised capabilities, формат `NAMESPACE`, UNSELECT -> повторный
  SELECT и сохранение помеченного `\\Deleted` сообщения. Тестовая папка:
  `./data/g14-imap-body-sections`.
- G14 остаётся RUNNING: IMAP4rev2 search/ESEARCH, untagged/response grammar и
  остальной RFC 9051 acceptance ещё не закрыты.

### 2026-09-24: G14 ENABLE negotiation checkpoint

- Добавлена типизированная команда `ENABLE`; capability атомы разбираются в
  enum, а неизвестные/неподдерживаемые возможности игнорируются по RFC. Ответ
  содержит пустой `ENABLED`, поскольку включаемых сервером расширений пока нет;
  `IMAP4rev2` и `UTF8=ACCEPT` не активируются и не рекламируются.
- PASS: Windows `cargo fmt --check`, strict Clippy и 201 тест; Linux Rust 1.97
  Docker test target прошёл strict Clippy и 233 теста. Независимый STARTTLS
  клиент проверил ENABLE, NAMESPACE и UNSELECT. Повторная сквозная SMTP -> IMAP
  проверка BODY подтвердила байты вложения SHA256
  `4c7872cd574aabe5cdc9a29678b88b178ef4aac98f051455ff87e207eeea9759`, а после
  restart отдельная IMAP-сессия подтвердила durable `\\Seen`. Данные находятся
  в bind mount `./data/g14-imap-body-sections`.
- G14 остаётся RUNNING: обязательные IMAP4rev2 команды, поисковая и response
  грамматики и полный клиентский профиль ещё не закрыты. G15-G18 не начаты.

### 2026-09-24: G14 IMAP atom and quoted-string distinction checkpoint

- IMAP lexer сохраняет различие между `atom` и `quoted string` в Enum токенов.
  Команды с атомарной grammar могут точно отклонять quoted string, тогда как
  текстовые аргументы сохраняют обработку обоих wire-форматов.
- PASS: Windows `cargo fmt --check`, strict Clippy и 202 теста, structural audit
  (нет `mod.rs`, Rust-файлы не превышают 300 строк); Docker Linux Rust 1.97
  прошёл strict Clippy и 234 теста. STARTTLS blackbox подтвердил BAD для
  `ENABLE "X-LITTERKEEP-UNKNOWN"`, затем успешные ENABLE/NAMESPACE/UNSELECT;
  SMTP -> IMAP вложение совпало по SHA256
  `4c7872cd574aabe5cdc9a29678b88b178ef4aac98f051455ff87e207eeea9759`.
  Временный контейнер удалён после тестов; данные сохранены в
  `./data/g14-imap-body-sections`.
- G14 остаётся RUNNING: полная RFC 9051 команда и response grammar ещё не
  реализована. G15-G18 не начаты.

### 2026-09-24: G14 SEARCH charset checkpoint

- SEARCH типизирует `CHARSET`; без параметра используется UTF-8, явно
  поддерживаются UTF-8 и US-ASCII. Неизвестный charset получает tagged NO с
  `BADCHARSET (US-ASCII UTF-8)`, а не BAD. Unicode-критерий при заявленном
  US-ASCII отвергается отдельным NO до сравнения результатов.
- PASS: Windows format/strict Clippy и 204 теста; Docker Linux Rust 1.97
  strict Clippy и 236 тестов. Независимый STARTTLS клиент APPEND-ил UTF-8 тело,
  нашёл его BODY search с UTF-8 и с неявным charset, проверил US-ASCII, NO для
  unsupported charset и NO для несовместимого US-ASCII criteria. Повторно
  прошли ENABLE/NAMESPACE/UNSELECT, точные байты вложения
  `4c7872cd574aabe5cdc9a29678b88b178ef4aac98f051455ff87e207eeea9759` и Seen
  после restart. Тестовые данные: `./data/g14-imap-body-sections`; созданный
  для сетевого теста контейнер удалён.
- Search остаётся PARTIAL: evaluator ещё не декодирует MIME transfer encodings
  и encoded-word заголовки перед сопоставлением; ESEARCH result options и
  полный mandatory search-key набор также не закрыты. G15-G18 не начаты.

### 2026-09-24: G14 Windows-1251 MIME search checkpoint

- MIME text decoder поддерживает явно названные Windows-1251/CP1251 aliases,
  преобразует кириллические и punctuation octets в Unicode и отвергает
  неопределённый октет `0x98`. Неизвестные charset по-прежнему пропускаются без
  raw-byte search. SEARCH CHARSET остаётся отдельным UTF-8/US-ASCII контрактом.
- Добавлены unit-тест MIME projection и независимая STARTTLS black-box проверка:
  письмо с Windows-1251 body находится Unicode-критерием `BODY "Привет"`.
- PASS: Windows `cargo fmt --check`, `cargo clippy --all-targets --locked --
  -D warnings` и `cargo test --all-targets --locked` (220 тестов); Docker image
  `litterkeep-server:g14-test` на Rust 1.97 прошёл release build, strict
  Clippy и 255 тестов. `tests/blackbox/imap_search_mime.py` после STARTTLS
  подтвердил Windows-1251 MIME body и остальные SEARCH кейсы: `PASS
  imap-search-mime`. Использовался только независимый Python IMAP-клиент,
  подключение из litterkeep не проверялось. Stage data: `./data/g14-charset`.
  Контейнер `litterkeep-g14-charset` остановлен и удалён; в `docker ps -a`
  остался только пользовательский `litterkeep-g12-phone`.
- G14 остаётся RUNNING: поддержка иных MIME charset, все RFC 9051 criteria и
  полный IMAP4rev2 command/response baseline не закрыты.

### 2026-09-24: G14 strict no-argument command grammar checkpoint

- CAPABILITY, LOGOUT, NOOP, IDLE и STARTTLS теперь отвергают любой лишний
  аргумент до dispatch. Reactor возвращает tagged `BAD invalid command syntax`,
  после чего соединение остаётся пригодным для следующей корректной команды.
- RFC 9051 разрешает пустой `resp-text`; добавление текста здесь сделано ради
  совместимости с независимым стандартным `imaplib`, который аварийно завершал
  разбор такого пустого `BAD`. Это interoperability requirement, не MUST RFC.
- PASS: Windows format/strict Clippy/221 тест; Docker Rust 1.97 release build,
  strict Clippy и 256 тестов. Независимый STARTTLS black-box `imaplib`
  проверил лишний аргумент до TLS, в authenticated и selected states, затем
  успешный NOOP в той же сессии (`PASS imap-command-grammar`). MIME SEARCH
  smoke также прошёл (`PASS imap-search-mime`). Litterkeep-подключение не
  проверялось. Данные: `./data/g14-command-grammar`; контейнер удалён, в Docker
  остался только пользовательский `litterkeep-g12-phone`.
- G14 остаётся RUNNING: данный parser checkpoint не закрывает весь mandatory
  IMAP4rev2 grammar или command/response baseline.

### 2026-09-24: G14 basic LIST delimiter discovery checkpoint

- Базовый `LIST "" ""` теперь типизирован как root discovery и возвращает
  `* LIST (\\Noselect) "/" ""`, после чего tagged OK. Extended LIST с пустым
  pattern остаётся обычным пустым совпадением, а не ошибочно выдаёт root.
- PASS: Windows `cargo fmt --check`, strict Clippy и 223 теста; Docker Rust
  1.97 release build, strict Clippy и 258 тестов. Независимый STARTTLS Python
  IMAP-клиент сравнил точный delimiter/root LIST item: `PASS imap-list-root`.
  Данные: `./data/g14-list-root`; контейнер удалён, пользовательский
  `litterkeep-g12-phone` сохранён.
- RFC 9051 LIST остаётся PARTIAL: обязательные SUBSCRIBED selection и другие
  LIST-EXTENDED варианты ещё не реализованы; подключение через litterkeep не
  проверялось, IMAP4rev2 не рекламируется.

### 2026-09-24: G14 ESEARCH result checkpoint

- SEARCH RETURN принимает только типизированные ALL/COUNT/MIN/MAX; пустой
  RETURN трактуется как ALL. Ответ ESEARCH содержит TAG correlator и UID marker
  для UID SEARCH, ALL с точным сжатием полных последовательностей, COUNT,
  MIN/MAX; при пустом результате сохраняется обязательная непустая response
  data. Неизвестный/неподдержанный RETURN option отвергается вместо молчаливого
  успеха. Без RETURN сохраняется legacy SEARCH профиля IMAP4rev1.
- PASS: Windows `cargo test --all-targets --locked` (206 tests), strict Clippy,
  format и structural audit. Docker Rust 1.97 test target: strict Clippy и 238
  tests. Независимый STARTTLS blackbox создал отдельный mailbox, проверил
  correlator, UID marker, ALL/COUNT/MIN/MAX, пустой результат и сохранение
  legacy SEARCH; вывод `PASS imap-esearch`. Временный серверный контейнер
  использует только `./data/g14-esearch` и после тестов удаляется.
- Search остаётся PARTIAL: SAVE и `$`, расширения с параметрами, полный набор
  criteria, MIME transfer decoding и encoded-word decoding не реализованы.
  IMAP4rev2 не рекламируется; G14 остаётся RUNNING.

| Goal | Depends on | Acceptance criterion | Status |
|---|---|---|---|
| G00 | none | build, help, lock, dependency report, saved goal | PASS |
| G01 | G00 | lifecycle/lease/wake tests and Linux poll integration | PASS |
| G02 | G01 | durable store, journal, blob, recovery | PASS |
| G03 | G02 | TLS, AUTH, bounded admission | PASS |
| G04 | G03 | durable SMTP submission and local delivery | PASS |
| G05 | G04 | MIME + IMAP, independently checked attachment | PASS |
| G06 | G05 | IMAP folders, flags, UID, IDLE | PASS |
| G07 | G02 | POP3 backed by the shared store | PASS |
| G08 | G02 | Sieve and ManageSieve delivery effect | PASS |
| G09 | G04 | outbound SMTP, pool, retry/DSN | PASS |
| G10 | G06,G07,G08,G09 | failure matrix, replay, resource plateaus | PASS |
| G11 | G10 | offline check/backup and repeatable seed | PASS |
| G12 | G11 | litterkeep and final acceptance report | PASS |

## Checkpoints

### 2026-09-20: G12 final clean acceptance and report

- На одном чистом container-local `/data` выполнены repeatable init, полный seed,
  независимая manifest-проверка, process restart, повтор seed/manifest, offline
  check и восстановимая backup-копия. Финальное состояние после реальной отправки
  litterkeep: 5 mailboxes, 37 links, 36 referenced blobs, 0 orphan blobs и
  0 pending deliveries; финальная backup содержит 63 файла и 14,381,601 bytes.
- Независимый STARTTLS IMAP-сценарий с полным логином Bob нашёл ровно одно письмо
  с темой `G12-Litterkeep-UI`, которое отправил Android-клиент. Это подтверждает
  server commit отдельно от UI status отправителя.
- Итоговая человеческая матрица находится в `docs/acceptance.md`, машинная в
  `docs/acceptance.tsv`. Из 46 T/R критериев 44 имеют PASS, T23 имеет точный
  NOT RUN, T25 имеет BLOCKED с фактической причиной. Server-protocol,
  client-integration и manual-device evidence разделены.
- Добавлены содержательные документы runtime lifecycle, storage format, protocol
  coverage, client setup и security. G12 PASS означает завершение требуемого
  отчёта и доступной приёмки, а не ложное утверждение о third-party MTA или полном
  attachment-сценарии litterkeep.

### 2026-09-20: G12 real litterkeep connection and full-address login checkpoint

- Локальный исходный код litterkeep проверен без изменения его dirty worktree. Физический
  `TECNO CH6i` был доступен через ADB, установленный package
  `ru.thpt.letterkeep` использован без очистки существующих данных и cache.
- Первый реальный setup доказал TLS policy: неизвестный CA был отклонён, затем
  выбранный пользователем `litterkeep-g12-ca.pem` принят как account trust anchor.
  После этого обнаружена фактическая несовместимость: litterkeep аутентифицируется
  полным адресом `alice@litterkeep.test`, а сервер принимал только короткое имя.
- Добавлена единая typed normalization boundary. Короткий login сохраняется, полный
  адрес разрешается только через тот же enum route-профиль локальных доменов;
  внешний домен отклоняется до credential path. IMAP, SMTP, POP3 и ManageSieve
  получают одно canonical имя владельца.
- После пересборки litterkeep успешно проверил IMAP и SMTP STARTTLS, показал
  «Почта подключена», синхронизировал каталог папок и отправил письмо Alice -> Bob.
  UI подтвердил приём сервером и принятого получателя. Это реальная client-integration
  проверка, а не результат Python-клиента.
- Полный T25 не пройден: текущий compose screen не показывает control выбора
  вложения; повторный setup Bob завершился generic TLS connection error до выбора
  trust anchor. Получение и скачивание вложения именно litterkeep не объявляются
  проверенными. Сырые факты: `data/g12-clean/litterkeep-device-measurements.txt`.
- PASS gates изменения: Windows 190 тестов и strict Clippy; Docker Rust 1.97
  strict Clippy и 220 Linux тестов, images `litterkeep-server:g12-login-test` и
  `litterkeep-server:g12-login`.

### 2026-09-20: G12 repeatable secure init checkpoint

- Команда bootstrap приведена к единственному typed интерфейсу
  `init --data-dir <directory>`. Она создаёт ровно четыре acceptance-аккаунта:
  `alice`, `bob`, `qa`, `pop`, используя CSPRNG установленного Rust/TLS стека.
- Пароли не передаются в argv и не печатаются. Локальный файл
  `test-accounts.tsv` создаётся до PHC-записей, синхронизируется и получает
  права `0600` на Linux. Повторный init валидирует typed набор владельцев и
  существующие PHC, не генерирует новые пароли и не перезаписывает hashes.
- Unit-тест доказал идентичность secret-файла и всех PHC после повтора, а также
  аутентификацию каждого сохранённого пароля. Windows gate: 189 тестов и strict
  Clippy. Docker Rust 1.97 gate: 219 тестов и strict Linux Clippy; release image
  `litterkeep-server:g12-init` успешно собран.
- Linux CLI acceptance на container-local `/data`: первый запуск сообщил
  `created=4 existing=0`, второй `created=0 existing=4`; SHA-256 secret-файла и
  четырёх PHC совпал до байта, mode secret-файла равен `600`. Сырые измерения:
  `data/g12-init/measurements.txt`.
- G12 остаётся RUNNING до чистого полного прогона продукта, фактической проверки
  доступной интеграции litterkeep и итоговой матрицы acceptance без завышения
  статуса непроверенных сценариев.

### 2026-09-20: G11 multi-recipient and bounded maintenance completion

- Fixture `seed-009-bcc` теперь выполняет одну реальную SMTP transaction с двумя
  RCPT, Bob и QA, и одним DATA. Seed проверяет наличие письма отдельными IMAP
  TLS-сессиями обоих владельцев и считает частичное состояние ошибкой, а не
  создаёт слепой дубль.
- Чистый Linux прогон снова дал `created=35 existing=0`, повтор
  `created=0 existing=35`; независимый manifest verifier подтвердил Bob и QA,
  все 35 fixture IDs и восемь attachment payloads. Сырые данные:
  `data/g11-multi-recipient.txt`.
- Явным тестом доказан предел Sieve diagnostic journal в 1 MiB с отказом
  `StorageFull`. Вместе с ранее проверенными frame payload bounds, blob quota,
  bounded scheduler handoff и offline check/backup это закрывает maintenance
  limits без небезопасного удаления журналов.
- Финальный gate G11: Windows 187 тестов и strict Clippy; Docker Rust 1.97
  strict Linux Clippy и 217 тестов в `litterkeep-server:g11-final-test`.
- PASS G11. Следующая активная подцель G12: повторяемый init, чистый запуск,
  интеграция litterkeep при фактической доступности и итоговый acceptance report.

### 2026-09-20: G11 full 35-message seed and independent manifest checkpoint

- Seed расширен до 35 сообщений: русский/английский текст, emoji, ответы,
  message/rfc822 forwarding, HTML/plain alternative, Bcc fixture, Sent, Drafts,
  Archive, флаги и семь attachment-сценариев с восемью файлами.
- Вложения включают TXT, валидный минимальный PNG, binary, пустой файл,
  Unicode filename, два одинаковых имени и точный 10 MiB payload. MIME generator
  разделён по предметным файлам и остаётся bounded полным лимитом сообщения.
- На чистом Linux Store первый запуск дал `created=35 existing=0`, второй
  `created=0 existing=35`. После полного process restart результат снова
  `created=0 existing=35`.
- Независимый Python `imaplib`/`email`/`hashlib` verifier скачал все сообщения и
  подтвердил `fixtures=35 attachments=8` по TSV manifest. Отдельное ручное
  письмо сохранилось после повторного seed и restart в единственном экземпляре.
- Offline checker после наполнения: `message_links=35`, `referenced_blobs=35`,
  `orphan_blobs=0`. Сырые результаты находятся в
  `data/g11-full-seed-acceptance.txt` и `data/g11-full-seed-measurements.txt`.
- PASS gate checkpoint-а: Windows 186 тестов и strict Clippy; Docker Rust 1.97
  strict Linux Clippy и 216 тестов в `litterkeep-server:g11-full-seed-test`.
- G11 остаётся RUNNING до multi-recipient fixture и явной итоговой проверки
  существующих bounded journal/diagnostic contracts.

### 2026-09-20: G11 first five network seed fixtures checkpoint

- Добавлена команда `seed`, которая читает пароли Alice и Bob из файлов, открывает
  проверенные implicit-TLS SMTP/IMAP соединения и не пишет Store напрямую.
- Входящие fixtures доставляются SMTP submission после SASL PLAIN, исторические
  Sent/Drafts создаются IMAP APPEND. Перед созданием и после commit выполняется
  IMAP SEARCH по закрытому `X-Litterkeep-Fixture` в папке настоящего владельца.
- Первый чистый Linux запуск сообщил `created=5 existing=0`; немедленный повтор
  на том же Store сообщил `created=0 existing=5`. Набор содержит English,
  русский UTF-8, multipart/alternative, Sent и Draft с binary attachment.
- Начальный manifest находится в `tests/fixtures/seed-manifest.tsv`, включая
  MIME path, размер и независимо рассчитанный SHA-256 вложения. Сырые результаты:
  `data/g11-seed/measurements.txt`; runtime использовал container-local `/data`.
- PASS gate checkpoint-а: Windows 186 тестов и strict Clippy; Docker Rust 1.97
  strict Linux Clippy и 216 тестов в image `litterkeep-server:g11-seed-test`.
- G11 остаётся RUNNING: набор требуется расширить до 30–50, добавить остальные
  типы fixtures, независимую сверку manifest, ручное письмо и restart-повтор.

### 2026-09-20: G11 consistent offline backup checkpoint

- Добавлена команда `backup --data-dir <source> --output <new-directory>`.
  Она удерживает exclusive Store lock на всём протяжении копирования, заранее
  проверяет ссылки, не копирует `writer.lock`/`runtime.control`, запрещает
  symlink и назначение внутри source, синхронизирует файлы и каталоги на Linux.
- Destination обязан отсутствовать. Частично созданный самой командой каталог
  удаляется при ошибке копирования. Готовая копия автоматически открывается и
  проходит тот же integrity checker.
- Тест восстановления доказал сохранение исходного UID и точных байтов письма.
  Linux CLI создал и проверил `/backup`, а параллельная попытка при работающем
  Store owner-е была отклонена. Сырые результаты: `data/g11-backup/measurements.txt`;
  source и backup находились на container-local filesystem.
- PASS gate checkpoint-а: Windows 185 тестов и strict Clippy; Docker Rust 1.97
  strict Linux Clippy и 215 тестов в image `litterkeep-server:g11-backup-test`.
- G11 остаётся RUNNING до bounded maintenance limits и полного повторяемого seed.

### 2026-09-20: G11 offline integrity checker checkpoint

- Добавлена команда `check --data-dir`, которая получает эксклюзивную блокировку
  Store, выполняет штатный recovery framing и полностью читает каждый blob,
  принадлежащий mailbox либо ожидающей исходящей доставке.
- Checker раздельно сообщает mailbox links, уникальные referenced blobs, pending
  deliveries и orphan blobs. Orphan-файлы не удаляются. Отсутствующая ссылка
  приводит к ошибке, а redirect-only очередь правильно считается владельцем.
- Linux CLI acceptance подтвердил отказ при работающем Store owner-е с текстом
  `store writer is already active`, затем успешную проверку после остановки.
  Сырые результаты сохранены в `data/g11-check/measurements.txt`; runtime data
  находились в container-local `/data`, так как bind mount не заявлен рабочим.
- PASS gate checkpoint-а: Windows strict Clippy и 182 теста; Docker Rust 1.97
  strict Linux Clippy и 212 тестов в image `litterkeep-server:g11-check-test`.
- G11 остаётся RUNNING: ещё требуются согласованный backup/restore и повторяемый
  acceptance seed с manifest и проверкой отсутствия дубликатов.

### 2026-09-20: G10 final SMTP reply-loss and completion checkpoint

- Независимый SMTP-клиент прошёл STARTTLS/AUTH/MAIL/RCPT/DATA, отправил полный
  terminator и закрыл transport, не читая финальный ответ. Исчезновение session
  generation не отменило уже принятое задание единственного Store owner-а.
- Новая STARTTLS IMAP-сессия нашла целое письмо по Subject. Затем server process
  полностью перезапущен на том же container-local `/data`; повторный IMAP SEARCH
  снова нашёл commit. После обеих фаз lifecycle baseline был нулевым.
- Сырые сведения сохранены в `data/g10-final-reply-loss/measurements.txt`.
- PASS G10: высокоуровневые и низкоуровневые storage failures/replay, lost wake
  и completion metadata plateau, two-instance peer, 2000 TCP churn connections,
  32 simultaneous IMAP IDLE, exact 10 MiB attachment, 256 deadline batch,
  10 cooperative cancellations и финальная SMTP reply-loss граница доказаны.
  Следующая активная подцель G11.

### 2026-09-20: G10 deadline and cooperative cancellation series checkpoint

- Независимый клиент одновременно занял все 256 admission slots неполным TLS
  handshake. 257-е соединение закрыто. Через пятиисекундный deadline все 256
  owners освобождены, после чего новый полный TLS handshake получил SMTP greeting.
- Отдельная серия удерживала 10 завершённых TLS-сессий. Control `tasks` показал
  десять разных TaskId и socket lease; для каждого выполнен настоящий
  `runtime cancel`. Все команды приняты, все десять клиентов увидели закрытие.
- После deadline batch и после cancel batch все task/resource/queue/wait/event
  счётчики вернулись к нулю. Server process сохранил 2 потока. Сырые значения
  сохранены в `data/g10-deadline-cancel/measurements.txt`.
- G10 остаётся RUNNING только до матрицы потери финального SMTP-ответа после
  durable commit и общей проверки накопленных black-box доказательств.

### 2026-09-20: G10 exact 10 MiB attachment and TLS streaming checkpoint

- Аудит до теста обнаружил, что прежний лимит всего MIME-сообщения 10 MiB не
  мог вместить 10 MiB binary attachment после base64. Full-message limit и
  рекламируемый SMTP SIZE увеличены до разрешённых ТЗ 25 MiB.
- Реальный тест выявил два независимых TLS backpressure-дефекта. Input терял
  непринятый хвост большого record, потому что игнорировал partial `read_tls`;
  теперь каждый record проходит цикл read/process/drain с bounded 64 KiB.
  Output пытался одним `write_all` заполнить bounded rustls buffer; теперь
  plaintext передаётся частями и остаток остаётся у session до `POLLOUT`.
- Независимый `smtplib` через STARTTLS/AUTH отправил точное вложение 10 485 760
  байт. Полный MIME имел 14 349 525 байт. `imaplib` через STARTTLS скачал RFC822,
  стандартный Python MIME parser декодировал вложение, размер совпал, SHA-256:
  `aecf3c2ab8aca74852bca07b54136cecb3fdafdc35540068ed952c0b89538e0d`.
- После передачи control socket показал нулевой lifecycle baseline; процесс
  сохранил 2 потока. Сырые значения находятся в локальном
  `data/g10-large-attachment/measurements.txt`; runtime data были container-local.
- PASS gate: Windows 179 tests и strict Clippy; Docker Rust 1.97 strict Linux
  Clippy и 209 tests прошли в image `litterkeep-server:g10-large-test`.
- G10 остаётся RUNNING до deadline/cancel series и потери финального SMTP-ответа
  после durable commit.

### 2026-09-20: G10 simultaneous IMAP IDLE plateau checkpoint

- Независимый raw socket client открыл 32 одновременные IMAP-сессии, для каждой
  выполнил STARTTLS с проверкой CA/hostname, LOGIN, SELECT и IDLE. Во время
  удержания control socket показал 32 task и 32 leased TCP/TLS resource.
- Процесс сохранил ровно 2 потока. После `DONE`, tagged completion и LOGOUT все
  task/resource/queue/wait/event счётчики вернулись к нулевому baseline.
- После первого authentication churn allocator удержал RSS 219288 KiB. Поэтому
  профиль выполнен повторно: после второго полного цикла RSS равен 219324 KiB,
  прирост только 36 KiB, число потоков осталось 2. Это bounded plateau, а не
  ложное требование возврата allocator к начальному RSS.
- Сырые значения сохранены в `data/g10-imap-idle/measurements.txt`. Runtime
  использовал container-local `/data`; shared bind mount не заявлен рабочим.
- G10 остаётся RUNNING: ещё требуются 10 MiB transfer, deadline/cancel series и
  потеря финального SMTP-ответа после durable commit.

### 2026-09-20: G10 TCP churn resource plateau checkpoint

- Независимый Python socket client выполнил исходный, не уменьшенный профиль:
  10 одинаковых циклов по 100 соединений, в каждом 50 SMTP и 50 IMAP, с
  ограниченной одновременностью 20. Затем профиль полностью повторён для
  одновременного измерения RSS и числа потоков.
- После каждого из 20 циклов настоящий local control socket вернул нулевые
  ready/wait/cancel/terminal task records, нулевые available/leased/quarantined/
  closed resources, нулевые ready queue, wait registrations и tracked events.
- Во всех десяти циклах повторного прогона Linux `docker top` показал RSS
  3316 KiB и ровно 2 потока server process. После нагрузки дополнительные
  SMTP/IMAP соединения прошли, поэтому плато не получено ценой остановки reactor.
- Сырые измерения сохранены в локальной поддиректории
  `data/g10-resource-plateau/measurements.txt`. Runtime использовал
  container-local `/data`: зависающий bind mount диска H не заявлен рабочим.
- Runtime image: `litterkeep-server:g10-churn`; container:
  `litterkeep-g10-churn`. G10 остаётся RUNNING до 32 IMAP IDLE, 10 MiB transfer,
  deadline/cancel series и границы финального SMTP-ответа.

### 2026-09-20: G10 low-level blob I/O failure checkpoint

- Production-путь публикации blob получил тестовый перехватчик пяти реальных
  I/O-операций: создание temp-файла, partial/short write, file `sync_all`,
  атомарный rename и directory sync. Проверка не подменяет код отдельной
  тестовой реализацией.
- Short-write инъекция действительно сохраняет только половину payload в temp
  и возвращает I/O error. После полного переоткрытия Store recovery удаляет
  temp, неполное письмо остаётся невидимым, а следующий append проходит.
- Ошибка directory sync моделирует неопределённость после rename: окончательное
  orphan-имя может существовать, но без transaction commit оно не публикуется.
  Следующая транзакция выбирает другой blob id и сохраняет правильные байты.
- PASS checkpoint: Windows 179 tests; strict Clippy, fmt, diff и структурный
  аудит. Docker Rust 1.97 strict Linux Clippy и 209 tests прошли в image
  `litterkeep-server:g10-blob-failures`.
- G10 остаётся RUNNING: нужны границы финального SMTP-ответа и сетевой resource
  plateau с сохранёнными сырыми измерениями.

### 2026-09-20: G10 append failure and replay boundary checkpoint

- Append pipeline получил девять типизированных failpoint-границ: публикация
  blob, запись outbound queue, Sieve diagnostics, первая mailbox frame, sync
  mailbox directory, recent journal, sync commit frame, sync transaction
  directory и граница перед публикацией результата в памяти.
- Для каждой границы тест создаёт новое хранилище, получает ошибку операции,
  проверяет переход единственного writer в `Poisoned`, запрет последующей
  записи тем же владельцем, затем полностью переоткрывает данные с диска.
- До durable commit письмо после replay невидимо. Начиная с синхронизированной
  commit frame письмо видимо и его исходные байты совпадают. Тем самым отдельно
  зафиксирована допустимая неопределённость ответа вызывающему после durable
  commit без повторной или частичной публикации сообщения.
- PASS checkpoint: Windows 178 tests; strict Clippy, fmt, diff и структурный
  аудит. Docker Rust 1.97 strict Linux Clippy и 208 tests прошли в image
  `litterkeep-server:g10-store-failure-matrix`.
- G10 остаётся RUNNING: нужны низкоуровневые инъекции short write, rename и
  fsync, а также сетевой churn plateau по live resources и buffers.

### 2026-09-20: G10 bounded event metadata plateau checkpoint

- Аудит обнаружил фактический unbounded рост: каждый уникальный scheduler
  `Completion(request_id)` оставался в `event_versions` после пробуждения.
  При долгой работе это превращало registry в бесконечную историю завершений.
- Event lifecycle вынесен в отдельный `runtime/event_control.rs`. Одноразовое
  событие удаляется только после фактического signal, снятия wait registration
  и перехода владельца в Ready/Cancelling. Сигнал без waiter сохраняет version,
  поэтому защита от lost wake не ослаблена.
- Retirement task дополнительно удаляет его Timer/Cancel metadata. Runtime
  `Stats` и CLI теперь показывают `tracked_events`, поэтому плато наблюдаемо, а
  не скрыто внутри HashMap.
- Churn test выполнил 10 000 разных completion cycles для одного task и получил
  `tracked_events=0`, `wait_registrations=0`, `ready_queue_depth=1`.
- PASS checkpoint: Windows 177 tests и strict Clippy. Docker Rust 1.97 strict
  Linux Clippy и 207 tests прошли в image
  `litterkeep-server:g10-event-plateau`. Runtime image
  `litterkeep-server:g10-runtime` через реальный control socket вернул baseline
  `tracked_events=0`.
- G10 остаётся RUNNING: требуется расширить storage failpoint/crash matrix и
  выполнить сетевой churn plateau по live resources/buffers.

### 2026-09-20: G09 two-instance exchange and completion checkpoint

- Два одновременно работающих экземпляра нашего бинарника запущены в отдельной
  Docker-сети с разными container-local `/data`: source с постоянным outbound
  scheduler и destination с собственным plaintext peer SMTP listener.
- Независимые ManageSieve/SMTP clients создали две последовательные redirect
  delivery `remote@peer.test`. Source scheduler после первой и второй job
  вернулся в Waiting, его observed event version вырос с 82 до 108.
- На destination после первой доставки реальный lifecycle показал SMTP
  `TASK id=1:1 state=Ready held=1:1`. После второй доставки сохранились тот же
  TaskId и тот же socket ResourceId. Это доказывает две SMTP transaction через
  одно принятое TCP-соединение, без второго accept/handshake.
- После полного restart destination запущен с IMAP STARTTLS. Независимый
  `imaplib` нашёл оба разных Subject в ящике `remote`, подтвердив долговечную
  доставку, а не только прохождение scheduler state machine.
- Ранее пройденные Linux real-socket tests подтверждают implicit-TLS route,
  проверку CA/SAN, deadline, dirty/reset pool policy, durable retry и DSN.
  Текущий внешний обмен проверял локальный plaintext MTA peer, что соответствует
  выбранному локальному межсерверному профилю и не заявляется публичным MTA.
- PASS G09: обмен двух экземпляров и server-side reuse доказаны вместе с
  retry/DSN/cancellation invariants. Следующая активная подцель G10.

### 2026-09-20: G08 independent runtime-error and completion checkpoint

- Новый независимый socket/smtplib/imaplib test загрузил через ManageSieve
  активный script с 17 redirect actions и отдельно script с redirect при восьми
  итоговых `Received` hops. SMTP в обоих случаях принял письмо, а IMAP доказал
  RFC-safe implicit keep вместо потери или частичного применения плана.
- После полного restart контейнера оба письма снова найдены через IMAP SEARCH.
  Offline `sieve-errors /data bob` прочитал долговечные typed diagnostics
  `redirect-limit` и `redirect-loop` с transaction/script version.
- На том же текущем бинарнике повторно прошёл полный положительный независимый
  путь ManageSieve literal/commands -> SMTP -> Sieve `fileinto Invoices` ->
  IMAP UID SEARCH/FETCH.
- Runtime image использует container-local `/data`, endpoints:
  SMTP `127.0.0.1:22587`, IMAP `127.0.0.1:22143`, ManageSieve
  `127.0.0.1:24190`. Shared host `./data` не заявлен проверенным из-за ранее
  зафиксированного зависания Docker Desktop bind I/O.
- PASS G08: сетевой upload/activate реально влияет на доставку, invalid update
  не заменяет active version, bounded execution при ошибке сохраняет письмо,
  diagnostic/replay и лимиты доказаны. Следующая активная подцель G09.

### 2026-09-20: G06 independent network and completion checkpoint

- Независимый Python `imaplib` через реальный TCP выполнил STARTTLS, LOGIN,
  APPEND и полный заявленный SEARCH matrix: persistent/session flags,
  sequence/UID sets, OR/NOT, TEXT, INTERNALDATE и RFC 5322 sent date.
- Вторая одновременно открытая IMAP-сессия прочитала тот же snapshot. После
  полного restart контейнера повторная сессия подтвердила сохранённые UID,
  flags, content/date results и переход присвоенного `Recent` в `Old`.
- Повторён обязательный сквозной smoke: независимый `smtplib` отправил письмо
  Alice через STARTTLS/AUTH, `imaplib` прочитал его у Bob, MIME-вложение совпало
  побайтно; SHA-256
  `f3a25aa93aa2fbba28d79260535bbd6a5eb0fc1c24a8b0f04e12b484c1dfe363`.
- Runtime image: `litterkeep-server:g06-network`; host endpoints проверки:
  SMTP `127.0.0.1:21587`, IMAP `127.0.0.1:21143`. Данные этой проверки жили в
  `/data` container layer, потому что Docker Desktop daemon всё ещё не способен
  завершить старые процессы на shared `./data`. Это не заявлено как проверка
  bind-mount durability.
- PASS G06: папки/Sent/Drafts, flags, UID/UIDPLUS, IDLE, две сессии, Recent,
  partial FETCH и базовый SEARCH имеют unit/integration и независимые сетевые
  доказательства; ожидающая IDLE-сессия не получает отдельный поток.
- Следующая активная подцель G08. G07 уже PASS; G08 и G09 ещё RUNNING.

### 2026-09-20: G06 sent-date SEARCH checkpoint

- SEARCH AST, parser и evaluator поддерживают `SENTBEFORE`, `SENTON` и
  `SENTSINCE`. Они сравнивают календарный UTC-день RFC 5322 `Date`, тогда как
  `BEFORE`, `ON` и `SINCE` по-прежнему используют сохранённый INTERNALDATE.
- Парсер отправленной даты вынесен в отдельный файл. Он принимает базовый RFC
  5322 date-time с необязательным weekday, временем с секундами либо без них и
  числовой timezone. Некорректная дата, obsolete named zone и отсутствие
  `Date` не подменяются INTERNALDATE и не дают ложного совпадения.
- Header block по-прежнему разбирается не более одного раза на сообщение даже
  при boolean-композиции sent-date, header и body criteria.
- PASS: Windows 176 tests, strict Clippy и format. Docker Rust 1.97 strict Linux
  Clippy и 206 tests прошли в image
  `litterkeep-server:g06-search-sent-date`.
- G06 остаётся RUNNING: полный обязательный сетевой
  SEARCH/concurrent/reconnect matrix ещё не завершён; расширения за пределами
  заявленного профиля продолжают получать BAD.

### 2026-09-20: G06 sequence-set SEARCH checkpoint

- SEARCH принимает самостоятельный message sequence-set и отдельный
  `UID <sequence-set>` criterion. Пространство фильтра не смешивается с
  `SEARCH`/`UID SEARCH` addressing, которое определяет только числа ответа.
- Существующий bounded `SequenceSet` переиспользован без строковых сравнений и
  разворачивания диапазонов. Для SEARCH добавлен allocation-free lookup с
  лимитом 64 wire-диапазона; `*` разрешается относительно snapshot maximum
  sequence либо maximum UID.
- Evaluator получает типизированные координаты одного immutable snapshot.
  Production evaluator и его unit tests разделены по файлам, оба меньше 300
  строк; прямого индексирования коллекций не добавлено.
- PASS: Windows 174 tests, strict Clippy и format. Docker Rust 1.97 strict Linux
  Clippy и 203 tests прошли в image `litterkeep-server:g06-search-sets`.
- G06 остаётся RUNNING: sent-date SEARCH criteria и независимая сетевая
  SEARCH/concurrent/reconnect matrix ещё не завершены.

### 2026-09-20: G06 complete system and session flag SEARCH checkpoint

- SEARCH AST и parser теперь типизированно поддерживают `ANSWERED`,
  `UNANSWERED`, `FLAGGED`, `UNFLAGGED`, `DRAFT`, `UNDRAFT`, `RECENT`, `OLD` и
  `NEW` вместе с ранее реализованными `SEEN`, `UNSEEN`, `DELETED` и
  `UNDELETED`.
- Persistent flags читаются из immutable mailbox snapshot. `RECENT`, `OLD` и
  `NEW` вычисляются только по session-local `selected_recent`; `NEW` означает
  `RECENT` и отсутствие `Seen`, не создавая ложного persistent-флага.
- Исправлена композиция boolean SEARCH: вложенные `NOT` и `OR`, содержащие
  `HEADER` или `BODY`, теперь заранее требуют единственный разбор header block.
  Отдельный regression test фиксирует эту границу.
- PASS: Windows 172 tests, strict Clippy, format, structural gate и
  `git diff --check`. Docker Rust 1.97 strict Linux Clippy и 200 tests прошли в
  image `litterkeep-server:g06-search-flags`.
- G06 остаётся RUNNING: sequence-set/UID и sent-date SEARCH criteria, а также
  независимая сетевая SEARCH/concurrent/reconnect matrix ещё не завершены.

### 2026-09-20: G06 SEARCH date and boolean checkpoint

- SEARCH AST поддерживает `BEFORE`, `ON`, `SINCE`, `NOT`, бинарный `OR` и
  вложенные parenthesized groups. Последовательность criteria внутри и вне
  группы остаётся неявным AND по IMAP4rev1, незакрытая или лишняя скобка
  отвергается parser-ом.
- Date parser вынесен в отдельный typed `SearchDate`: wire-строка проверяется
  существующей Gregorian/UTC арифметикой `MailDate`, после чего executor
  сравнивает только номер календарного дня с сохранённым INTERNALDATE.
- Unit tests покрывают parser tree, невалидную дату, границы ON/BEFORE/SINCE и
  short-circuit OR/NOT. Неподдержанные SENT-date и прочие keys всё ещё получают
  BAD, поэтому профиль не выдаёт неполную семантику за пустой успех.
- PASS: Windows 171 tests, strict Clippy и format. Docker Rust 1.97 strict
  Linux Clippy и 197 tests прошли в финальном image
  `litterkeep-server:g06-search-boolean-date-final`.
- G06 остаётся RUNNING: расширение остальных базовых flag/date criteria и
  независимая сетевая SEARCH/concurrent/reconnect matrix ещё не завершены.

### 2026-09-20: G06 typed content SEARCH checkpoint

- SEARCH больше не ограничен пятью flag predicates. Typed AST поддерживает
  `FROM`, `TO`, `CC`, `BCC`, `SUBJECT`, произвольный `HEADER`, `BODY`, `TEXT`,
  `LARGER`, `SMALLER` и неявный AND нескольких последовательных criteria для
  обычного и UID SEARCH. Неизвестный criterion и отсутствующий аргумент
  по-прежнему дают BAD, а не ложный пустой SEARCH.
- Сопоставление работает над immutable bytes согласованного mailbox snapshot.
  ASCII-поиск регистронезависим; BODY исключает header block, TEXT включает всё
  сообщение. MIME header block разбирается не более одного раза на сообщение и
  только для запроса, которому нужны header/body boundaries.
- PASS: Windows 169 tests, strict Clippy и format. Docker Rust 1.97 strict
  Linux Clippy и 194 tests прошли в image
  `litterkeep-server:g06-search-profile`.
- G06 остаётся RUNNING: date criteria, отрицание/OR и независимый сетевой
  SEARCH/concurrent/reconnect matrix ещё не завершены.

### 2026-09-20: G06 IDLE fan-out and Recent ownership checkpoint

- RFC 3501 оставляет неопределённым, какая из одновременно selected sessions
  увидит новое письмо с `\Recent`. Профиль продукта фиксирует более простую
  проверяемую политику: все уже selected sessions получают session-local
  marker вместе с durable EXISTS; durable кандидат остаётся до первого
  будущего SELECT, который атомарно claim-ит его. Следующий SELECT marker уже
  не получает, поэтому `\Recent` не становится persistent message flag.
- Интеграционный Store/reactor test подтверждает live notification двум
  владельцам, однократный durable claim будущим SELECT и пустой следующий
  SELECT. Отдельный профиль создаёт 32 настоящих socket lease в одном
  Orchestrator, доставляет каждому независимый bounded EXISTS buffer и
  проверяет ровно 32 task/resource без session threads.
- PASS: Windows 167 tests, strict Clippy и format. Docker Rust 1.97 strict
  Linux Clippy и 191 tests прошли в image
  `litterkeep-server:g06-idle-matrix`.
- G06 остаётся RUNNING: расширенный SEARCH профиль и независимая сетевая
  concurrent/reconnect matrix ещё не завершены; Docker bind mount `./data`
  на этой машине остаётся непригодным для честного black-box запуска.

### 2026-09-20: G06 isolated IMAP IDLE overflow checkpoint

- Unsolicited IMAP notifications больше не возвращают общую reactor error при
  переполнении bounded session output. Отстающий подписчик получает typed
  `SessionTermination::NotificationOverflow`; его неотправленные буферы
  очищаются, а lease закрывается единым путём lifecycle cleanup после dispatch.
- Остальные selected/IDLE-сессии продолжают получать durable EXISTS, FLAGS,
  EXPUNGE и COPY/MOVE projection. Отдельный Linux test заполняет очередь одного
  подписчика до `MAX_OUTPUT`, проверяет корректный EXISTS у второго и доказывает
  удаление только одного task/resource owner без утечки metadata.
- PASS: Windows `cargo test` (167 tests), strict Clippy и format. Docker Rust
  1.97 strict Linux Clippy и 189 tests прошли в image
  `litterkeep-server:g06-idle-overflow-final`.
- Docker Desktop file sharing перепроверен фактическим `alpine:3.22` bind mount
  `H:/github/mail-server/data:/data`. Обращение к `/data` снова зависло, а
  daemon не смог завершить контейнер в штатный срок. Поэтому сетевой black-box
  сценарий не объявлен выполненным, G06 остаётся RUNNING.

### 2026-09-20: G08 redirect route and loop policy checkpoint

- Общий `redirect_policy` разбирает mailbox в typed `Domain::PeerTest`,
  проверяет bounded dot-atom и канонизирует domain. Первый локальный профиль
  разрешает только `peer.test`; `litterkeep.test`, неизвестный domain и
  malformed address не могут попасть в outbound queue.
- Store применяет policy после bounded Sieve execution. Запрещённый route или
  восемь уже существующих `Received` fields отбрасывают весь частичный план,
  выполняют implicit keep и атомарно публикуют versioned Sieve diagnostic.
- Sieve projection теперь читает ordered MIME header fields напрямую, поэтому
  повторные `Received` не схлопываются `BTreeMap` и loop budget проверяет
  фактическую цепочку trace headers.
- Outbound worker повторяет ту же policy до получения connection из pool.
  Повреждённый/старый queue job не открывает socket; typed `InvalidInput`
  становится terminal `PermanentFailure` с DSN `5.7.1 RoutePolicy`, а не
  расходует transient retry budget.
- PASS: Windows `cargo test --all-targets --locked` (167 tests), strict Clippy,
  format. Docker Rust 1.97 strict Linux Clippy и 188 tests прошли в image
  `litterkeep-server:g08-redirect-policy-gate`.
- G08 остаётся RUNNING только до независимого сетевого runtime-error/loop
  matrix. Его запуск всё ещё невозможен на `./data` из-за зависшего runtime
  bind mount Docker Desktop; unit/integration доказательство не выдано за
  black-box PASS.

### 2026-09-20: G08 transactional Sieve runtime-error checkpoint

- Bounded executor возвращает typed `ExecutionError`, а не строку. При runtime
  error частично построенный DeliveryPlan отбрасывается полностью и применяется
  RFC-safe implicit keep, поэтому уже принятое письмо не теряется и redirect из
  недовыполненного плана не публикуется.
- Диагностика содержит account, точную version активного script, transaction ID
  и typed error code. Она записывается до общей Store commit-точки и при replay
  фильтруется по committed transaction IDs вместе с mailbox/queue state.
  Оборванная transaction поэтому не показывает ложную ошибку пользователю.
- Добавлена offline-команда `litterkeep-server sieve-errors <data-dir>
  <account>` со стабильным TSV-выводом. Она фактически запущена на локальном
  `./data/g08-sieve-diagnostics`; пустой новый Store вернул только корректный
  header, не создавая фиктивных записей.
- Store tests вызывают реальный runtime redirect limit: исходное письмо попадает
  в INBOX, outbound queue остаётся пустой, версия и причина переживают reopen.
  Failpoint после mailbox frame доказывает, что до global commit не видны ни
  письмо, ни предварительно записанная Sieve diagnostic.
- Повторный RCPT одного account дедуплицируется через `HashSet` до исполнения
  правила, поэтому не создаёт несколько одинаковых redirect jobs или ошибок.
- Recovery обрезает только оборванный tail diagnostic journal до выдачи writer,
  но отклоняет завершённый повреждённый frame. После ошибки записи Store
  становится poisoned и не дописывает данные за неопределённым tail.
- Diagnostic journal имеет явную квоту 1 MiB. Её исчерпание останавливает
  transaction до global commit и SMTP success, сохраняя проверяемую семантику.
- PASS: Windows `cargo test --all-targets --locked` (161 tests), strict Clippy и
  format. Docker Rust 1.97 strict Linux Clippy и 182 tests прошли в image
  `litterkeep-server:g08-runtime-error-gate`.
- G08 остаётся RUNNING: независимый сетевой runtime-error test не запускался,
  поскольку Docker Desktop bind mount по-прежнему зависает на `init /data`.
  Осталась отдельная проверка redirect route/loop policy и полный replay matrix.

### 2026-09-20: G08 Sieve implicit keep semantics checkpoint

- Исполнитель Sieve теперь хранит typed-состояние `Implicit`, `Cancelled` или
  `Explicit`. Действия `redirect`, `fileinto` и `discard` отменяют только
  implicit keep; ранее выполненный explicit `keep` сохраняется независимо от
  порядка последующих delivery actions.
- Исправлена ошибочная Store-проверка, которая считала INBOX обязательным для
  одиночного `redirect`. Теперь redirect-only transaction создаёт durable
  outbound job без mailbox link, сохраняет исходный envelope sender и после
  terminal transition не восстанавливает job при reopen.
- Отдельный Store test подтверждает составной план `keep; redirect`: INBOX link
  и outbound job фиксируются вместе и оба восстанавливаются после повторного
  открытия Store. Unit test языка дополнительно проверяет сочетание explicit
  keep, redirect и fileinto.
- Тесты Store вынесены в отдельный предметный файл, поэтому production-файл
  снова укладывается в лимит 300 строк без `mod.rs` и без смешивания тестовой и
  рабочей ответственности.
- PASS: Windows `cargo test --all-targets --locked` (156 tests), strict Clippy,
  format и `git diff --check`. Docker Rust 1.97 strict Linux Clippy и 177 tests
  прошли в image `litterkeep-server:g08-implicit-keep-gate`.
- G08 остаётся RUNNING: ещё не завершены полный runtime-error/replay matrix и
  независимый сетевой redirect-only сценарий без дополнительного `discard`.
  Его нельзя объявлять PASS по одним unit/integration тестам; runtime bind mount
  Docker Desktop ранее зависал даже после включения file sharing.

### 2026-09-20: G06 IMAP AUTHENTICATE PLAIN checkpoint

- Устранена ложная capability: `AUTHENTICATE PLAIN` теперь имеет typed
  mechanism, optional SASL initial response, continuation state, явную отмену
  `*`, общий authzid policy и асинхронную проверку тем же Argon2 Store worker,
  что LOGIN/SMTP/POP3/ManageSieve. Tagged completion различает LOGIN и
  AUTHENTICATE через enum, а не строковое восстановление команды.
- LOGIN и AUTHENTICATE запрещены до реального TLS и после уже успешной
  аутентификации. Plaintext listener больше не рекламирует `AUTH=PLAIN`.
  `SASL-IR` объявляется в TLS capability, поскольку initial response реально
  принимается и проверяется.
- Общий входной dispatcher отделён от IMAP command dispatcher; SASL
  continuation обрабатывается до обычной command grammar и остаётся
  собственностью одной session. Это сохраняет file-level ответственность и
  лимит 300 строк.
- Добавлена фактическая матрица rev1 в `docs/imap-profile.md`: реализованные
  команды, ограниченный SEARCH профиль и явно не заявленные расширения.
- PASS: Windows `cargo test --all-targets --locked` (154 tests), strict Clippy,
  format. Docker Rust 1.97 strict Linux Clippy и 175 tests прошли в image
  `litterkeep-server:g06-imap-auth-gate`. Linux integration использует реальный
  TCP socket, event wake через `poll`, account journal и Argon2 worker; отдельно
  проверены успешный continuation, cancel, plaintext denial и повторный вход.
- Независимый `tests/blackbox/imap_authenticate.py` проверяет обе wire-формы и
  capability до/после STARTTLS; `py_compile` PASS, сетевой запуск NOT RUN из-за
  зафиксированного зависания runtime bind mount Docker Desktop.
- G06 остаётся RUNNING до завершения всей concurrent-change/error matrix и
  независимой сетевой проверки AUTHENTICATE после восстановления bind-mount
  runtime Docker Desktop.

### 2026-09-20: G06 COPY/MOVE multi-session synchronization checkpoint

- После успешного durable COPY/MOVE reactor обновляет destination projection
  каждой уже selected сессии: увеличивает EXISTS и добавляет destination UID в
  session-local `\\Recent`. IDLE-получатель немедленно получает агрегированный
  `EXISTS`, обычная selected-сессия сохраняет согласованное внутреннее состояние.
- MOVE дополнительно уменьшает EXISTS source-сессий, удаляет перенесённые UID
  из их `\\Recent` и публикует рассчитанные source sequence numbers как
  `EXPUNGE` ожидающим IDLE-сессиям. Инициатор исключён только из source
  broadcast, поскольку его completion уже содержит эти EXPUNGE; destination
  projection инициатора при совпадении mailbox не теряется.
- Поиск source sequence numbers переведён с повторного линейного `Vec::contains`
  на `BTreeSet`, поэтому построение проекции не становится квадратичным.
- PASS: Windows `cargo test --all-targets --locked` (153 tests), strict Clippy,
  format, `git diff --check`, отсутствие `mod.rs` и Rust-файлов длиннее 300
  строк. Docker Rust 1.97 strict Linux Clippy и 172 tests прошли в image
  `litterkeep-server:g06-transfer-sync-gate`; новый тест использует три
  настоящих TCP socket и три эксклюзивных lifecycle lease.
- Независимый сетевой сценарий `tests/blackbox/imap_idle_move.py` подготовлен и
  проходит `py_compile`, но NOT RUN по протоколу. После разрешения file sharing
  финальный image собрался, однако новый `docker run --rm -v
  H:\\github\\mail-server\\data\\g06-idle-move:/data ... init /data bob
  bob-password` завис до вывода результата, как и прежний runtime-контейнер.
  Поэтому unit/integration доказательство не выдано за сетевой black-box PASS.
- G06 остаётся RUNNING: требуется завершить оставшиеся классы concurrent-change
  matrix и реально выполнить подготовленный COPY/MOVE watcher после
  восстановления runtime bind-mount I/O Docker Desktop.

### 2026-09-20: G09 scheduler TLS success and SAN failure checkpoint

- Linux integration test запускает настоящий постоянный scheduler, durable
  Store worker, bounded TLS transport worker и отдельный rustls SMTP peer.
  Успешный trusted route проходит queue `Pending -> InFlight -> Delivered`,
  после reopen Store больше не содержит pending job.
- Второй тест использует доверенный CA, но неверный SNI. rustls отклоняет SAN,
  неопределённое соединение уничтожается, а scheduler фиксирует durable
  `Deferred`, `attempts=1` и ненулевой restart-safe retry deadline.
- Оба сценария завершаются cooperative cancellation scheduler task, закрытием
  job channel, `join` transport thread и повторным открытием Store.
- PASS: Windows `cargo test --all-targets --locked` (153 tests), strict Clippy;
  Docker Rust 1.97 strict Linux Clippy и 171 test в image
  `litterkeep-server:g09-scheduler-tls-gate`. Новые Linux tests используют
  реальные TCP/rustls sockets, а не mock результата transport-а.
- G09 остаётся RUNNING: нужен внешний обмен двух одновременно запущенных
  service instances на отдельных `./data` и доказательство pool reuse в этом
  black-box процессе. Текущий Docker Desktop shared mount всё ещё занят старым
  зависшим контейнером, поэтому этот отдельный сценарий не объявлен PASS.

### 2026-09-20: G09 typed TLS route pool checkpoint

- Outbound worker принимает `Route::Plain` либо `Route::ImplicitTls`; variant
  выбирается до запуска thread. Enum-owned `Pool` физически не позволяет
  вернуть rustls stream в plaintext pool или смешать разные transport types.
- Implicit-TLS connection теперь остаётся non-cloneable владельцем rustls
  stream между доставками. Route key закрепляет числовой address, SNI и CA;
  channel возвращается в pool только после successful DATA либо accepted RSET.
- Постоянный reactor scheduler принимает typed route. Новый CLI
  `serve-all-mail-outbound-tls-starttls` загружает outbound CA отдельно от
  входящего server certificate и запускает тот же lifecycle/queue state machine.
- Unit test провёл две SMTP transaction через один TLS connection и подтвердил
  один idle channel. Linux scheduler test продолжает проверять plaintext variant.
- PASS: Windows `cargo test --all-targets --locked` (153 tests), strict Clippy;
  Docker Rust 1.97 strict Linux Clippy и 169 tests в image
  `litterkeep-server:g09-tls-route-gate`. Format и structural gate выполнены
  перед commit.
- G09 остаётся RUNNING до внешнего обмена двух service instances и негативной
  проверки wrong CA/SAN именно через постоянный scheduler.

### 2026-09-20: G09 bounded outbound network deadlines checkpoint

- Plaintext и implicit-TLS transport используют один connector contract:
  route разбирается в typed `SocketAddr` до worker thread, TCP connect получает
  deadline 5 секунд, read/write получают deadline 10 секунд.
- Hostname route отклоняется как `InvalidInput`, поэтому внутри scheduler-а нет
  скрытого blocking DNS. `ConnectionPool` хранит уже проверенный `SocketAddr` и
  не разбирает строку заново для каждой доставки.
- Детерминированный peer-test принимает TCP, не отправляет greeting и ждёт EOF.
  Клиент получает системный timeout через 100 ms test profile, уничтожает
  неопределённый channel, после чего peer наблюдает реальное закрытие без sleep.
- PASS: Windows `cargo test --all-targets --locked` (152 tests), strict Clippy;
  Docker Rust 1.97 strict Linux Clippy и 168 tests в image
  `litterkeep-server:g09-deadline-gate`. Format, `git diff --check` и structural
  gate выполнены перед commit.
- G09 остаётся RUNNING: постоянный scheduler ещё использует plaintext route;
  нужны typed TLS route key и двухсерверная проверка service mode.

### 2026-09-20: G09 cooperative cancellation checkpoint

- Local control protocol и CLI получили typed команду
  `runtime cancel <task-id>`. `request_cancel` снимает registrations ожидания,
  но не выдаёт занятый ресурс повторно и не удаляет task с живым lease.
- Reactor после обработки готовой poll-пачки закрывает отменённую клиентскую
  сессию через общий `session_close`: socket, POP3 maildrop lease, resource и
  task metadata освобождаются одним владельцем. Отложенный Store completion
  всё ещё коррелируется по generation и безопасно отбрасывается.
- Outbound scheduler в idle завершается сразу. При активной операции отмена
  дренирует текущую доставку до durable terminal transition, затем закрывает
  bounded worker channel, присоединяет thread и только после этого удаляет task.
- PASS: Windows `cargo test --all-targets --locked` (150 tests), strict Clippy;
  Docker Rust 1.97 strict Linux Clippy и 166 tests, включая реальное закрытие
  отменённого socket lease, в `litterkeep-server:cancel-bind-gate-final`.
  Format, `git diff --check` и structural gate также пройдены.
- BLOCKED: новое разрешение Docker file sharing проверено успешным read-only
  запуском с `-v H:/github/mail-server/data:/data`, но write/init остаётся в
  `Created`. Старый контейнер всё ещё завис внутри прежнего shared mount, поэтому
  bind-mount black-box отмены не объявлен PASS до перезапуска Docker Desktop.
- G09 остаётся RUNNING: нет deadline для отмены зависшего transport I/O, TLS
  route для постоянного scheduler-а и полного межсерверного service matrix.

### 2026-09-20: G09 scheduler lifecycle ownership checkpoint

- Постоянный outbound scheduler теперь создаёт реальную task запись в общем
  `Orchestrator<Session>`, а не только пишет логи или держит локальный enum.
- Каждая Store и SMTP transport операция паркует task на собственном
  `Event::Completion(request_id)`. Completion сначала выполняет versioned
  `signal`, проверяет переход task в `Ready`, затем следующая фаза снова
  регистрирует ожидание. Пустая очередь использует отдельный typed
  `Event::Timer(TaskId)` и пробуждается общим poll deadline.
- Linux integration test после запуска и после полной доставки проверяет
  `waiting_tasks=1`, `wait_registrations=1` и `ready_queue_depth=0`. Тот же тест
  подтверждает реальную SMTP-транзакцию и отсутствие pending job после reopen.
- Listener binding/регистрация вынесены в отдельный файл одной ответственности;
  все Rust/Python файлы остаются не длиннее 300 строк, `mod.rs` отсутствует.
- PASS: Windows 149 tests, strict Clippy и format; Docker Rust 1.97 strict Linux
  Clippy и 163 tests в image `litterkeep-server:lifecycle-gate-2`.
- G09 остаётся RUNNING: lifecycle task пока нельзя отменить через control API,
  а завершение process всё ещё не присоединяет transport worker управляемо.

### 2026-09-20: G09 long-lived reactor scheduler checkpoint

- `serve-all-mail-outbound-starttls` запускает один постоянный single-flight
  scheduler в общем Linux `poll` loop. Scheduler использует тот же Store
  worker, отдельный bounded SMTP transport owner и отдельный wake fd.
- Store completions больше не теряются: session dispatcher возвращает
  не принадлежащие session ответы, а scheduler принимает их только при точном
  совпадении верхнего диапазона request ID и текущей typed-фазы. Transport
  completion также проверяет job ID перед durable transition.
- Пустая очередь переводит scheduler в timed idle. Общий poll deadline выбирает
  минимум TLS handshake и следующего queue scan; отдельный timer-thread не
  создаётся. После terminal transition или DSN scheduler немедленно запрашивает
  следующую due delivery.
- Linux integration test `scheduler_drains_without_explicit_queue_command`
  создал durable job, провёл его через настоящий TCP SMTP peer, wakeup Store и
  transport workers и подтвердил после повторного открытия Store отсутствие
  pending job без вызова `queue-drain`.
- PASS: Windows 149 tests, strict Clippy и format; Docker Rust 1.97 gate теперь
  обязательно устанавливает Clippy и выполнил strict Linux Clippy плюс 163
  теста. Все Rust/Python файлы не превышают 300 строк, `mod.rs` отсутствует.
- Попытка дополнительного bind-mount black-box выявила зависание Docker Desktop
  внутри Unix `bind(2)` на shared volume. Control socket переведён на безопасный
  `/tmp` socket с owner-only locator в `./data`. Уже зависший контейнер Docker
  Desktop не смог завершить даже через `docker rm -f`, поэтому этот внешний
  сценарий не объявлен пройденным; доказательством checkpoint является Linux
  integration test внутри image build.
- G09 остаётся RUNNING: ещё нужны scheduler cancellation/clean shutdown,
  TLS route policy для постоянного сервиса и обмен двух одновременно работающих
  экземпляров именно через новый service mode.

### 2026-09-20: G10 local control socket foundation

- Linux reactor теперь владеет bounded Unix-domain control listener-ом и
  короткими nonblocking control-сессиями в том же `poll` loop, что SMTP, IMAP,
  POP3, ManageSieve и Store wakeup. Для диагностики не создаётся отдельный
  поток и не открывается второй Store writer.
- CLI добавил `runtime stats`, `runtime tasks`, `runtime resources` и
  `runtime inspect <task-id>` с обязательным `--data-dir`. Ответы строятся из
  реальных records `Orchestrator`: task state, owner и held resource IDs;
  значения ресурсов, тела почты, пароли и ключи не сериализуются.
- Control admission ограничен 16 одновременными session slots; лишний local
  peer закрывается без роста таблицы и без вытеснения уже обслуживаемого запроса.
- Docker Desktop shared bind mount не поддерживает Unix socket inode. Это
  выявлено фактическим Linux запуском, а не скрыто: только при таком bind error
  server создаёт owner-only locator `runtime.control` к process-local socket в
  `/tmp`; CLI проверяет, что locator не выходит из временного каталога.
- PASS: Linux Docker image `litterkeep-server:control-runtime` запущен с
  `./data/g10-control`; независимый `docker exec` выполнил `runtime stats`,
  `tasks`, `resources` и `inspect 2:1` при удерживаемой SMTP session. Ответ
  показал `TASK id=2:1 state=Ready held=2:1` и соответствующий leased resource;
  locator имел права `0600`. До final gate выполнены Windows `cargo test
  --all-targets --locked` (149 tests), strict Clippy, format и structural audit.
- G10 остаётся NOT RUN: зависимости G06/G08/G09 ещё не завершены; отсутствуют
  stop/cancel/restart counters, bounded event ring, полный failure/replay matrix
  и resource plateau acceptance.

### 2026-09-20: G08 ManageSieve literal and shared-reactor checkpoint

- ManageSieve получил собственный ограниченный RFC 5804 literal receiver для
  `PUTSCRIPT` и `CHECKSCRIPT`. Он принимает `{n+}` и синхронизирующий `{n}`
  до 65 536 байтов, не выделяет память по неограниченной длине и сохраняет
  pipeline-байты следующей команды. Это не копия IMAP APPEND: completion
  наступает ровно после n байтов без вымышленного завершающего CRLF.
- Добавлен `serve-all-mail-starttls`: SMTP, IMAP, POP3 и ManageSieve работают
  в одном Linux poll-reactor с одним Store owner. Отдельные процессы не
  конкурируют за файловый lock и не дают ложной проверки общей семантики.
- PASS для checkpoint: Windows `cargo test --all-targets --locked` (117
  tests), strict Clippy, format и structural audit без `mod.rs` и Rust-файлов
  больше 300 строк; Docker Linux test target прошёл 124 tests. Независимый Python socket
  сценарий `tests/blackbox/managesieve_literal_delivery.py` проверил STARTTLS,
  SASL PLAIN, `CHECKSCRIPT {n}` continuation, fragmented `PUTSCRIPT {n+}` и
  `SETACTIVE`, затем standard-library SMTP отправил Alice -> Bob, а IMAP
  прочитал письмо из `Invoices`. Использован только bind mount
  `./data/g08-managesieve`, CA `./data/pki/ca.pem`.
- G08 остаётся RUNNING: базовый RFC Sieve ещё не покрывает все обязательные
  tests/comparators/actions, redirect не связан с outbound queue, а полный
  внешний ManageSieve matrix всех команд и response codes ещё не завершён.

### 2026-09-20: G08 Sieve elsif checkpoint

- Parser представляет `elsif` вложенным `Command::If` в единственной
  else-ветви. Поэтому executor использует прежний bounded алгоритм и единый
  лимит операций, а следующие ветви не исполняются после первого совпадения.
- PASS: Windows `cargo test --all-targets --locked` (118 tests), strict
  Clippy, format и `git diff --check`. G08 остаётся RUNNING: ещё требуются
  address/size, match types/comparators, redirect через outbound queue и
  полный внешний ManageSieve matrix.

### 2026-09-20: G08 Sieve matcher and size checkpoint

- AST теперь явно хранит enum match type (`:is`, `:contains`, `:matches`) и
  comparator (`i;ascii-casemap`, `i;octet`), а не восстанавливает семантику
  из строк во время доставки. Matcher вынесен в отдельный bounded компонент:
  glob не использует regex и не делает экспоненциальный backtracking.
- `header` принимает списки имён и ключей, `size :over`/`:under` сравнивает
  именно число октетов исходного письма, переданное Store до journal commit.
  Неподдержанный comparator, переполнение и неверный quantifier отклоняются
  до публикации script version.
- PASS: Windows `cargo test --all-targets --locked` (121 tests), strict
  Clippy, format и Docker Linux test target (128 tests). G08 остаётся
  RUNNING: отсутствуют `address`-условия, redirect ещё не идёт через outbound
  queue, нужен полный внешний ManageSieve command/error matrix.

### 2026-09-20: G08 Sieve address checkpoint

- `address` test получил отдельный bounded extractor для `:all`, `:localpart`,
  `:domain`, `:user` и `:detail`; только затем выбранные значения передаются
  тому же typed matcher-у. Quoted display name и angle-bracket mailbox не
  ломают delimiter comma, некорректный адрес не становится совпадением.
- PASS: Windows `cargo test --all-targets --locked` (123 tests), strict
  Clippy, format и Docker Linux test target (130 tests). G08 остаётся
  RUNNING: redirect ещё не проходит outbound queue, не проверены все команды
  и response codes ManageSieve независимым сетевым matrix.

### 2026-09-20: G08 ManageSieve positive command matrix

- Независимый socket-клиент теперь исполняет по TCP/TLS CAPABILITY, STARTTLS,
  AUTHENTICATE, HAVESPACE, CHECKSCRIPT, PUTSCRIPT, LISTSCRIPTS, SETACTIVE,
  GETSCRIPT, RENAMESCRIPT, DELETESCRIPT, NOOP и LOGOUT. Он читает
  multi-line LIST и literal GETSCRIPT как wire-протокол, а не через наш parser.
- PASS: Linux Docker image с единым `serve-all-mail-starttls` reactor-ом;
  положительный matrix завершился SMTP Alice -> Bob и IMAP-чтением `Invoices`.
  Данные: bind mount `./data/g08-managesieve-matrix`, CA `./data/pki/ca.pem`.
  После проверки контейнер остановлен. G08 остаётся RUNNING: ещё нет
  отрицательного matrix response codes и redirect не подключён к durable
  outbound queue.

### 2026-09-20: G08 ManageSieve negative command matrix

- Тот же независимый TCP-клиент подтвердил отказ AUTHENTICATE до STARTTLS,
  HAVESPACE сверх лимита, невалидного PUTSCRIPT и DELETESCRIPT действующей
  версии. После неуспешной загрузки LISTSCRIPTS/GETSCRIPT всё ещё возвращают
  исходную active `invoices`, затем SMTP применяет её к доставке.
- PASS: Linux Docker `serve-all-mail-starttls`, отдельный bind mount
  `./data/g08-managesieve-negative`, CA `./data/pki/ca.pem`; тестовый
  контейнер остановлен. G08 остаётся RUNNING только из-за неинтегрированного
  redirect/outbound queue и полного RFC Sieve failure/replay coverage.

### 2026-09-20: G08 advanced Sieve network fixture

- Независимый uploaded script теперь реально использует `allof`, `header
  :contains`, `address :detail :is` и `size :over`. SMTP envelope recipient
  остаётся `bob@litterkeep.test`, а header `To` содержит plus-detail, поэтому
  проверка не подменяет Sieve routing разрешением другого RCPT account.
- PASS: Linux Docker SMTP -> Store -> IMAP прочитал письмо в `Invoices` из
  bind mount `./data/g08-sieve-advanced-network-3`; client serializes RFC 5322
  с CRLF через standard-library `email.policy.SMTP`. Windows полный suite:
  124 tests, strict Clippy и format. G08 остаётся RUNNING из-за redirect queue
  и replay/transaction integration.

### 2026-09-20: G08 envelope sender handoff checkpoint

- `AppendMany` теперь переносит SMTP `MAIL FROM` как отдельный
  `envelope_sender` от reactor через bounded storage worker к Store. Значение
  взято до `smtp_trace::prepend`, не выводится из MIME `From` и сохраняет
  допустимый null reverse-path. Это готовит корректный sender для Sieve
  redirect и исключает подмену envelope данными письма.
- PASS: Windows `cargo test --all-targets --locked` (124 tests), strict Clippy,
  format и `git diff --check`. G08 остаётся RUNNING: durable queue frame должен
  войти в тот же Store transaction commit, затем требуются replay и SMTP peer
  проверки redirect.

### 2026-09-20: G09 transaction-aware queue format checkpoint

- Durable queue теперь имеет typed `OwnedDelivery`: transaction ID immutable
  blob, envelope sender, recipient и enum state. `pending_owned` возвращает
  job только когда его ID присутствует в Store global committed set; тест
  подтверждает невидимость физически записанного, но незафиксированного job.
- PASS: Windows `cargo test --all-targets --locked` (125 tests), strict Clippy,
  format и `git diff --check`; `src/queue.rs` 263 lines. G08/G09 остаются
  RUNNING: Store ещё должен записывать owned jobs до общего commit, recovery
  должен использовать committed IDs, затем redirect и SMTP transport worker.

### 2026-09-20: G08 redirect and Store queue transaction checkpoint

- Store владеет queue и записывает Sieve redirect job после durable blob, но до
  единственного global commit. Новый unit test подтверждает: локальный INBOX
  link и `OwnedDelivery` становятся видимыми вместе, job хранит SMTP envelope
  sender, а не MIME `From`. `src/store/append.rs` отделяет эту ответственность
  от Store model, все файлы остаются не длиннее 300 строк.
- PASS: Windows `cargo test --all-targets --locked` (126 tests), strict Clippy,
  format и `git diff --check`. G08/G09 остаются RUNNING: recovery должен
  загрузить committed jobs для worker-а, затем нужны delivery state transitions,
  retry и независимый SMTP peer.

### 2026-09-20: G09 committed queue recovery checkpoint

- Store сохраняет recovered global committed transaction IDs и обновляет набор
  только после успешной global commit publication. `pending_outbound` отдаёт
  queue jobs через этот набор; restart test проверяет, что committed Sieve
  redirect снова доступен после `Store::open`.
- PASS: Windows `cargo test --all-targets --locked` (126 tests), strict Clippy,
  format и `git diff --check`. G09 остаётся RUNNING: jobs ещё не получают
  durable state transitions и не передаются исходящему SMTP worker/peer.

### 2026-09-20: G09 durable owned queue transitions checkpoint

- `OwnedDelivery` получил append-only `transition_owned`; при recovery reader
  выбирает последнее состояние transaction/recipient и не выдаёт повторно
  `Delivered` job. Тест подтверждает это через reopen. Тесты вынесены из
  production queue journal: `queue.rs` 226 lines, `queue_tests.rs` 91 lines.
- PASS: Windows `cargo test --all-targets --locked` (127 tests), strict Clippy,
  format и `git diff --check`. G09 остаётся RUNNING: Store должен предоставить
  worker-у transition command и blob read, затем нужен реальный SMTP peer.

### 2026-09-20: G09 Store-owned transport handoff checkpoint

- Store получил typed API `pending_outbound`, `outbound_blob` и
  `transition_outbound`. Они принимают только committed `OwnedDelivery`, не
  раскрывают worker-у путь blob и сохраняют queue state через владельца Store.
  Unit test прошёл pending -> immutable payload -> Delivered -> restart без
  повторной выдачи terminal job.
- PASS: Windows `cargo test --all-targets --locked` (127 tests), strict Clippy,
  format и `git diff --check`. G09 остаётся RUNNING: storage worker protocol
  и SMTP transport worker ещё не используют этот handoff against real peer.

### 2026-09-20: G09 bounded worker outbound protocol checkpoint

- Bounded `StorageWorker` получил typed requests/responses для pending jobs,
  immutable blob и state transition. Единственный Store owner исполняет их,
  а reactor correlation ID покрывает все новые variants, поэтому completion не
  теряется и transport не получает Store/file access напрямую.
- PASS: Windows `cargo test --all-targets --locked` (127 tests), strict Clippy,
  format и `git diff --check`; `message.rs` 258 lines, `execution.rs` 284.
  G09 остаётся RUNNING: отдельный bounded SMTP transport worker ещё не делает
  TCP peer transaction и не переводит результаты в эти completion requests.

### 2026-09-20: G09 SMTP outcome classification checkpoint

- Собственный outbound SMTP client возвращает typed `DeliveryOutcome`:
  positive completion, `Deferred` для SMTP 4xx и `PermanentFailure` для 5xx.
  Системные I/O ошибки остаются `io::Error`; CLI явно показывает оба вида
  отказа. Это даёт очереди корректное решение о durable следующем state.
- PASS: Windows `cargo test --all-targets --locked` (127 tests), strict Clippy,
  format и `git diff --check`. G09 остаётся RUNNING: outcome ещё не вызывает
  Store worker transition в bounded queue-drain loop и не проверен peer-ом.

### 2026-09-20: G09 bounded queue drain and crash recovery checkpoint

- Добавлен `outbound_queue::drain`: он берёт не больше указанного числа
  committed jobs, фиксирует `InFlight`, запускает собственный SMTP transport и
  записывает `Delivered`, `Deferred` либо `PermanentFailure`. I/O ошибка peer
  становится `Deferred`, не потерей письма. Recovery mapping переводит stale
  `InFlight` в `Deferred`; reopen test это подтверждает.
- PASS: Windows `cargo test --all-targets --locked` (128 tests), strict Clippy,
  format и `git diff --check`; queue files не длиннее 300 lines. G09 остаётся
  RUNNING: drain ещё не имеет CLI/service entrypoint и не проверен против
  независимого Linux SMTP peer.

### 2026-09-20: G09 queue drain CLI checkpoint

- CLI получил `queue-drain <data-dir> <peer-address> [limit]`: открывает
  единственный Store writer, запускает bounded drain и печатает durable
  delivered/deferred/permanent counters. Нулевой или некорректный limit
  отвергается до доступа к хранилищу; default limit равен 8.
- PASS: Windows `cargo test --all-targets --locked` (128 tests), strict Clippy,
  format и `git diff --check`; `main.rs` 292 lines. G09 остаётся RUNNING до
  независимой Linux SMTP peer проверки queue-drain и service integration.

### 2026-09-20: G09 Linux outbound completion mapping checkpoint

- Linux Docker release build обнаружил неполный cfg-gated `Response` mapping
  reactor-а для новых outbound completions. `completion_id` теперь exhaustively
  покрывает все variants без wildcard; образ `litterkeep-server:g09-queue-runtime`
  собран из текущего исходного кода.
- PASS: Windows `cargo test --all-targets --locked` (128 tests), strict Clippy,
  format, `git diff --check` и Linux Docker release build. G09 остаётся RUNNING:
  независимый SMTP peer queue-drain ещё не выполнен.

### 2026-09-20: G09 Linux queue drain peer acceptance

- Независимый `queue_redirect_seed.py` по ManageSieve STARTTLS загрузил
  `redirect "remote@peer.test"` и отправил SMTP письмо source instance. После
  остановки source writer `queue-drain /data peer:2526 1` в отдельном Linux
  container выдал `delivered=1 deferred=0 permanent_failures=0`. Независимый
  Python `imaplib` container прочёл `queue redirect peer` из peer `remote`
  mailbox. Route policy теперь принимает documented peer domain `peer.test`.
- PASS: Windows `cargo test --all-targets --locked` (128 tests), strict Clippy,
  format, Linux Docker release build и два listener/container peer scenario.
  Все state data только в bind mounts `./data/g09-peer-source` и
  `./data/g09-peer-dest`; тестовые контейнеры остановлены. G09 остаётся
  RUNNING: отсутствуют automatic service drain, retry schedule/TTL, TLS peer
  transport и connection pool reuse/cancellation acceptance.

### 2026-09-20: G09 outbound 4xx/5xx wire peer checkpoint

- Отдельный standard-library `TcpListener` peer получил реальный SMTP greeting,
  EHLO, MAIL и RCPT от нашего client-а, затем вернул 450 или 550. Regression
  tests подтверждают `DeliveryOutcome::Deferred` для 4xx и
  `PermanentFailure` для 5xx, без DATA после RCPT rejection. Tests вынесены в
  `outbound_tests.rs`, production transport остаётся 125 lines.
- PASS: Windows `cargo test --all-targets --locked` (129 tests), strict Clippy,
  format и `git diff --check`. G09 остаётся RUNNING: отсутствует persistent
  retry schedule/TTL и automatic service drain/pool.

### 2026-09-20: G09 durable retry budget checkpoint

- Owned queue frames теперь несут `attempts`; reader сохраняет backward
  compatibility с прежним форматом без счётчика. Drain увеличивает attempts
  перед durable `InFlight`, поэтому crash не обнуляет budget. Третья 4xx/I/O
  deferred попытка становится `PermanentFailure`, что подтверждено unit test.
- PASS: Windows `cargo test --all-targets --locked` (130 tests), strict Clippy,
  format, `git diff --check`; queue/transport files не длиннее 300 lines. G09
  остаётся RUNNING: есть bounded count, но ещё нет persisted next-at/TTL delay,
  automatic service drain и TLS/pool lifecycle peer acceptance.

### 2026-09-20: G08 redirect-only durable delivery checkpoint

- Sieve `redirect; discard;` теперь публикует durable outbound job без
  фиктивной INBOX link. Store разрешает пустой список mailbox links только при
  непустом outbound plan; обычный пустой APPEND по-прежнему получает
  `InvalidInput`. Queue job, blob и global commit остаются одной transaction.
- PASS: Windows `cargo test --all-targets --locked` (131 tests), strict Clippy,
  format и `git diff --check`. G08/G09 остаются RUNNING: redirect-only peer
  scenario ещё не проверен в Linux; автоматический retry timing, TLS и pool
  остаются неполными.

### 2026-09-20: G08/G09 redirect-only Linux recovery checkpoint

- Recovery отличает redirect-only transaction от повреждённого commit: ID без
  mailbox link допустим только при owned SMTP job; blob такой transaction
  проверяется при открытии Store. Это устраняет ошибку после restart перед
  `queue-drain`, не ослабляя проверку повреждённых committed mail transactions.
- PASS: Linux image `litterkeep-server:g09-redirect-discard`, source и peer
  используют bind-mounted `./data/g09-redirect-*`. ManageSieve загрузил
  `redirect "remote@peer.test"; discard;`, SMTP принял письмо, source Store
  после restart drained его как `delivered=1`, а независимый Python IMAP
  клиент прочитал его у Linux peer. В source mailbox links нет, queue journal
  содержит terminal `delivered`. Windows: 131 tests, strict Clippy, format и
  `git diff --check` PASS. G08/G09 остаются RUNNING: нет automatic delayed
  retry, outbound TLS, pool и cancellation acceptance.

### 2026-09-20: G09 owned SMTP pool checkpoint

- `ConnectionPool` владеет настоящими TCP channels одного маршрута и имеет
  capacity 2 для bounded `queue-drain`. После positive final DATA channel
  возвращается в idle и применяется к следующему job; SMTP reject, EOF и I/O
  уничтожают channel и освобождают slot, поэтому грязное соединение не
  выдаётся новому recipient. Разовый CLI `deliver` остаётся отдельным
  standalone transport.
- PASS: детерминированные peer tests подтвердили два сообщения на одном TCP
  connection и новую TCP accept после RCPT 450. Windows: 133 tests, strict
  Clippy, format и `git diff --check`; Linux Docker test stage: 140 tests.
  G09 остаётся RUNNING: pool пока живёт только в одном bounded drain, нет
  runtime service ownership, persisted next-at/backoff, TLS route keys, DSN
  и cancellation lifecycle acceptance.

### 2026-09-20: G09 durable retry deadline checkpoint

- Owned queue frames теперь сохраняют `next_attempt_epoch` как restart-safe
  Unix epoch. После deferred delivery используется bounded exponential backoff
  60/120/240... seconds до 3600 seconds; terminal state не получает deadline.
  `queue-drain` не выдаёт job раньше deadline. Reader сохраняет совместимость
  с прежними owned frames, присваивая им immediate retry, а crash в InFlight
  возвращает job в Deferred без ложной задержки.
- PASS: restart test подтверждает due-gate на границе epoch, отдельный test
  читает прежний attempts frame. Windows: 136 tests, strict Clippy, format и
  `git diff --check`; Linux Docker test stage: 143 tests. G09 остаётся
  RUNNING: не реализованы service-owned automatic drain, TLS route keys,
  DSN и cancellation lifecycle acceptance.
- `Store::due_outbound` закрепляет due-отбор за единственным владельцем queue:
  transport больше не читает и не фильтрует durable schedule самостоятельно.

### 2026-09-20: G09 outbound TLS transport checkpoint

- Добавлен отдельный implicit-TLS SMTP transport с явными `server-name` и
  DER CA параметрами. Он строит свой rustls client config, проверяет SNI/CA и
  только после TLS handshake выполняет SMTP greeting, EHLO, MAIL, RCPT и DATA.
  Plaintext pool с ним не смешивается, поскольку TLS identity является другим
  route key. CLI `deliver-tls` делает этот путь доступным без runtime Python.
- PASS: собственный rustls TLS peer с выданным на лету CA принял полный SMTP
  transaction; malformed DER CA отвергается до TCP-подключения. Windows: 138
  tests, strict Clippy, format и `git diff --check`; Linux Docker test stage:
  145 tests. G09 остаётся RUNNING: TLS ещё не
  подключён к durable pool/service route, также отсутствуют automatic drain,
  DSN и cancellation lifecycle acceptance.

### 2026-09-20: Linux POP3 ownership correction

- Linux release build обнаружил cfg-specific перенос POP3 `Command` при
  проверке STLS до передачи команды state machine. Проверка теперь сопоставляет
  заимствованную команду, поэтому ownership остаётся у единственного `apply`.
- PASS: Windows `cargo test --all-targets --locked` (138 tests), strict Clippy,
  format и `git diff --check`; Docker Linux test stage: 145 tests. Контекст
  Docker равен 83 KiB и не содержит `./data`; runtime данные остаются только в
  явно подключаемых bind mounts из `./data`.

### 2026-09-20: G09 SMTP pool reset checkpoint

- После известного SMTP 4xx/5xx pool больше не уничтожает channel только по
  классу ответа. Он читает полный ответ, отправляет `RSET` и возвращает TCP
  connection в idle исключительно после положительного полного RSET reply.
  EOF, I/O error и отказ RSET уничтожают channel до следующего recipient.
- PASS: отдельный TCP peer подтвердил reuse одного соединения после RCPT 450
  и `RSET 250`; отдельный peer подтвердил новый accept после `RSET 500`.
  Windows `cargo test --all-targets --locked`: 139 tests; strict Clippy,
  format и `git diff --check` PASS. Docker Linux test stage: 146 tests.
- G09 остаётся RUNNING: TLS route keys/service-owned drain, DSN, cancellation
  lifecycle и независимый сторонний MTA ещё не проверены.

### 2026-09-20: G09 local DSN checkpoint

- `queue-drain` после durable `PermanentFailure` строит multipart/report DSN
  с `message/delivery-status` и исходным RFC822 body. Пустой reverse-path и
  внешний либо отсутствующий local account не создают bounce. 5xx и исчерпанный
  retry budget выбирают разные typed причины; публикацию выполняет только
  Store owner после account/local-route проверки.
- PASS: Linux Docker scenario с bind mount `./data/g09-dsn-source`: независимый
  ManageSieve/SMTP client создал redirect job от Alice, независимый socket peer
  вернул RCPT `550`, `queue-drain` выдал `permanent_failures=1`, а независимый
  Python IMAP STARTTLS client прочитал у Alice `report-type=delivery-status`,
  `Action: failed` и исходный subject. Тестовые Docker container/network
  остановлены и удалены; данные и peer log сохранены в stage directory.
- PASS: Windows `cargo test --all-targets --locked` (146 tests), strict Clippy,
  format и `git diff --check`; Docker Linux test stage: 153 tests. Structural
  audit: нет `mod.rs`, Rust files не длиннее 300 lines, запрещённых составных
  `if` не найдено. G09 остаётся RUNNING: DSN пока выполняется через explicit
  `queue-drain`, TLS route keys/service lifecycle, cancellation и сторонний MTA
  ещё не приняты.

### 2026-09-20: G09 bounded outbound worker checkpoint

- `OutboundWorker` владеет единственным `ConnectionPool` в выделенном thread и
  принимает только immutable SMTP jobs через bounded `sync_channel`. Worker не
  получает `Store`, не создаёт thread на job и возвращает completion без
  блокирования reactor. Явный `shutdown` закрывает jobs channel и присоединяет
  владельца transport ресурса.
- PASS: независимый TCP peer принял ровно одно TCP-соединение и подтвердил два
  последовательных SMTP jobs на этом connection. Windows
  `cargo test --all-targets --locked`: 147 tests; strict Clippy, format,
  `git diff --check` и structural audit PASS. Docker Linux test stage: 154
  tests.
- G09 остаётся RUNNING: worker ещё не подключён к reactor/durable queue;
  отсутствуют service wakeup, cancellation, TLS route keys и acceptance со
  сторонним MTA.

### 2026-09-20: G09 outbound completion wakeup checkpoint

- Linux `WakingOutboundWorker` создаёт Unix wake socket одновременно с
  bounded transport worker-ом. После каждого completion поток пытается
  записать только coalesced hint; authoritative result остаётся в channel.
  Поэтому заполненный wake socket не теряет outcome, а reactor не опрашивает
  transport в холостом цикле. Shutdown сначала присоединяет владельца pool.
- PASS: Linux unit test через собственный `poll(2)` подтвердил readable wake
  socket, вычитал hint и получил `Delivered` completion. Windows
  `cargo test --all-targets --locked`: 147 tests, strict Clippy, format,
  `git diff --check` и structural audit PASS. Docker Linux test stage: 155
  tests.
- G09 остаётся RUNNING: wakeup ещё не включён в server loop и не получает
  durable jobs; сохраняются обязательные service scheduler, cancellation,
  TLS route keys и независимый MTA acceptance.

### 2026-09-20: G09 bounded scheduler contract checkpoint

- Retry state, restart-safe deadline и DSN reason вынесены в единую typed
  `outbound_policy`; `queue-drain` больше не содержит отдельную копию этих
  решений. Store worker получил `OutboundDue`, который возвращает не более
  явно запрошенного лимита committed due jobs. Следующий service scheduler
  будет запрашивать только свою свободную capacity у Store owner.
- PASS: новый Store test создал две durable redirect-only jobs и подтвердил
  выдачу ровно одной при `limit=1`. Windows `cargo test --all-targets --locked`:
  148 tests; strict Clippy, format и `git diff --check` PASS. Docker Linux
  test stage: 156 tests.
- G09 остаётся RUNNING: контракт ещё не означает запуск scheduler-а в server
  loop. Требуются durable transition before submit, processing completion,
  service wakeup/timer, cancellation, TLS route keys и independent MTA test.

### 2026-09-20: G09 event-driven service drain checkpoint

- Linux `queue-drain` теперь использует `outbound_service`, а не открывает
  Store рядом с transport loop. Service создаёт bounded Store worker и
  bounded outbound pool worker, ожидает только их Unix wake sockets, durable
  фиксирует `InFlight` до SMTP submit и фиксирует final state до DSN. Один
  job является единственным mutable phase владельцем, поэтому completion не
  может быть применён к другому recipient.
- PASS: Linux integration test создаёт committed outbound job, независимый
  TCP peer принимает SMTP transaction, service получает wake completions и
  после final transition reopened Store не видит pending job. Docker Linux
  test stage: 157 tests. Windows: 148 tests, strict Clippy, format,
  `git diff --check` and current structural audit PASS.
- G09 остаётся RUNNING: это bounded explicit drain, не long-lived scheduler
  в `server_loop`; future retry timer, cancellation policy, TLS route keys и
  independent two-server acceptance ещё не завершены.

### 2026-09-20: G09 independent service drain DSN acceptance

- Под Linux Docker создана отдельная stage directory
  `./data/g09-service-blackbox`. Независимые Python ManageSieve и SMTP clients
  загрузили `redirect "remote@peer.test"; discard;` и передали письмо source
  server-у. После остановки source writer контейнерный `queue-drain /data
  host.docker.internal:3555 1` использовал event-driven service owners.
- PASS: независимый `smtp_550_peer.py` получил обычные EHLO, MAIL, RCPT 550 и
  RSET, а CLI сообщил `delivered=0 deferred=0 permanent_failures=1`. После
  restart source server независимый `queue_dsn_read.py` через IMAP STARTTLS
  прочитал multipart/report с `Action: failed` и исходным subject. Test
  container остановлен и удалён; данные и peer logs остаются в указанной
  `./data` directory.
- Этот PASS доказывает explicit service drain и DSN, но не long-lived runtime
  scheduler: G09 остаётся RUNNING до server-loop lifecycle, cancellation,
  TLS route keys и independent two-server acceptance.

### 2026-09-20: G10 lifecycle snapshot foundation

- `Orchestrator::stats` строит read-only snapshot только из актуальных task,
  resource, ready queue и wait registrations. Snapshot не удерживает lease,
  не пишет секреты и не создаёт историю terminal records. Unit test проверяет
  leased/quarantined resource и parked task на настоящих lifecycle transitions.
- PASS: Windows `cargo test --all-targets --locked` 149 tests, strict Clippy,
  format и `git diff --check`. G10 остаётся NOT RUN: нет local control socket,
  runtime CLI, plateau/failure matrix и Linux acceptance данного интерфейса.

### 2026-09-20: lifecycle parking correction

- Snapshot обнаружил, что `park_on` оставлял Waiting task в ready queue.
  Переход теперь удаляет task из ready queue только после атомарной проверки
  event version и регистрации wait. `signal` остаётся единственным путём,
  который вернёт parked task в ready state. Это восстанавливает требование
  освобождать execution slot во время ожидания.
- PASS: Windows 149 tests, strict Clippy, format и `git diff --check`.

### 2026-09-20: correction of scheduler contract structure

- Первый commit scheduler contract временно увеличил `src/store/tests.rs` до
  321 physical lines. Нарушение найдено тем же обязательным audit до передачи
  результата. Тест вынесен в `store/outbound_tests.rs`; история не
  переписывалась, чтобы факт и исправление были проверяемы отдельным commit.
- Current structural audit обязан показать не более 300 lines для каждого Rust
  file перед следующим checkpoint.

### 2026-09-20: structural condition-rule compliance checkpoint

- Устранены все найденные в `src` условия `if` с `&&` или `||`: ветвления
  выражены через typed tuples, `match` и guards. Для reactor read/write guards
  сохранено ленивое выполнение I/O, поэтому отсутствие readiness не вызывает
  лишнего read/flush. Проверка не нашла `mod.rs`, файлов длиннее 300 lines или
  прямых array-index expressions.
- PASS: Windows 137 tests, strict Clippy, format и `git diff --check`.

### 2026-09-20: Sieve capability enum checkpoint

- `require` преобразуется parser-ом в typed `Capability`; validation проверяет
  `Capability::Unsupported`, а не сравнивает предметное состояние со строкой.
- PASS: Windows 137 tests, strict Clippy, format и `git diff --check`.

### 2026-09-20: G07 POP3 core foundation

- Заглушка POP3 заменена typed grammar и FSM AUTHORIZATION/TRANSACTION/UPDATE.
  Реализованы USER/PASS, STAT, LIST, RETR, DELE, NOOP, RSET, QUIT, UIDL, TOP,
  CAPA и STLS на session-local `Maildrop` snapshot. UIDL строится из durable
  mailbox UID, RETR/TOP делают dot-stuffing, а QUIT возвращает delete plan и
  не объявляет UPDATE до отдельного completion `commit_deletes`.
- PASS только для core: Windows `cargo test --all-targets --locked` (108
  tests), strict Clippy, format, structural audit и Docker Linux test target.
  G07 не отмечен PASS:
  Linux POP3 listener, auth/store worker, maildrop lease, durable QUIT и
  независимый poplib test ещё не реализованы.

### 2026-09-20: G07 reactor loop extraction checkpoint

- Poll-loop SMTP/IMAP вынесен из фасада reactor в отдельный
  `reactor/server_loop.rs`. Владелец socket leases, wakeup, deadline и
  cancellation остаётся тем же: это структурная подготовка к третьему POP3
  listener без создания потоков на соединение и без изменения wire-семантики.
- PASS: Windows `cargo test --all-targets --locked` (108 tests), strict
  Clippy, format, `git diff --check`, audit Rust-файлов (не более 300 строк),
  Linux release build и Docker Linux test target (112 tests).
- G07 остаётся NOT RUN: сетевой POP3 listener, auth/store worker, maildrop
  lease, durable QUIT и независимый poplib test ещё не реализованы.

### 2026-09-20: G07 SMTP to POP3/STLS checkpoint

- Общий Linux poll reactor теперь может владеть только выбранными listener-ами,
  поэтому `serve-pop3-starttls` использует те же session leases, wakeup
  storage-worker и TLS deadline без потока на POP3-соединение. POP3 PASS
  создаёт worker request, успешная аутентификация открывает atomарный snapshot
  каталогового INBOX, а DELE становится durable EXPUNGE только после QUIT.
- Исправлена общая delivery граница: SMTP local recipient теперь разрешается в
  его durable INBOX key до `append_many`. Поэтому SMTP, IMAP и POP3 читают одну
  mailbox projection, а не разные строковые ключи.
- PASS: Windows `cargo test --all-targets --locked` (109 tests), strict
  Clippy, format, `git diff --check`, audit Rust-файлов (не более 300 строк),
  Linux release image и Docker test target (113 tests). Независимый standard
  library `tests/blackbox/pop3_starttls_delivery.py` выполнил SMTP STARTTLS
  Alice -> Bob, POP3 `CAPA`/STLS/USER/PASS/UIDL/RETR/DELE/QUIT и сравнил байты
  MIME attachment; отдельный POP3 restart подтвердил durable удаление. Данные:
  bind mount `./data/g07-pop3`, CA `./data/pki/ca.pem`.
- G07 остаётся NOT RUN: maildrop lease для конкурентных POP3 sessions,
  implicit-TLS POP3 и расширенный interoperability/failure matrix ещё не
  реализованы. Этот checkpoint не объявляет их проверенными.

### 2026-09-20: G07 maildrop lease and implicit-TLS checkpoint

- POP3 account получает отдельную bounded maildrop lease только после worker
  authentication. Она принадлежит `(slot, generation)`, не удерживает Store и
  освобождается после durable QUIT, TCP EOF, poll error или TLS deadline.
  Поэтому второй POP3 client не получает тот же snapshot, но SMTP/IMAP и
  storage-worker продолжают работать независимо.
- Добавлен `serve-pop3-tls`: implicit TLS не выдаёт plaintext greeting до
  handshake, переиспользуя существующие poll, deadline и socket lease.
- PASS: Windows `cargo test --all-targets --locked` (109 tests), strict
  Clippy, format, `git diff --check`, structural audit. Docker Linux test
  target прошёл 114 tests. Независимый `poplib` подтвердил конкурентный отказ
  второй POP3 session и получение maildrop после EOF первой, а `POP3_SSL`
  подтвердил implicit TLS с доверенным CA. Данные: `./data/g07-pop3`.
- G07 остаётся NOT RUN: политика конфликтующего IMAP EXPUNGE во время POP3
  snapshot и полный destructive/interoperability matrix ещё не завершены.

### 2026-09-20: G07 POP3 snapshot versus IMAP EXPUNGE checkpoint

- Политика зафиксирована и проверена: POP3 maildrop содержит immutable snapshot
  bytes, поэтому уже открытый RETR остаётся доступен после IMAP EXPUNGE. Однако
  Store не допускает повторный tombstone отсутствующего UID: POP3 QUIT получает
  `-ERR delete commit failed` и не делает ложного обещания. IMAP удаление
  остаётся durable и побеждает конфликт.
- Добавлен `serve-all-starttls`, объединяющий SMTP, IMAP и POP3 listener-ы в
  один poll-reactor и один Store owner для межпротокольных проверок.
- PASS: независимый `tests/blackbox/pop3_imap_conflict.py` на Linux Docker
  открыл POP3 snapshot `pop`, удалил то же письмо через IMAP STORE/EXPUNGE,
  прочитал snapshot через RETR, получил отказ QUIT и подтвердил пустой INBOX.
  Данные: `./data/g07-conflict`, CA: `./data/pki/ca.pem`.
- G07 остаётся NOT RUN: destructive matrix RSET/disconnect до QUIT и полный
  interoperability profile ещё не завершены.

### 2026-09-20: G07 empty-QUIT lease completion checkpoint

- Даже пустой POP3 delete plan теперь передаётся storage worker-у как пустой
  durable EXPUNGE completion. Только после него session переходит в UPDATE и
  освобождает maildrop lease, поэтому QUIT без DELE не оставляет account
  искусственно занятым до TCP EOF.
- PASS: Windows `cargo test --all-targets --locked` (110 tests), strict
  Clippy, format, structural audit и `git diff --check`. Linux regression
  target для этого уточнения ещё не запускался.
- G07 остаётся NOT RUN: требуется Linux destructive matrix RSET и disconnect
  до QUIT, а также полный interoperability profile.

### 2026-09-20: G07 Linux POP3 delete matrix checkpoint

- Независимый POP3 client подтвердил `DELE + RSET` и `DELE + TCP disconnect`:
  оба пути сохраняют сообщение. Затем тот же SMTP-delivered MIME message прошёл
  UIDL/RETR/DELE/QUIT, а restart подтвердил durable удаление. Пустой QUIT также
  прошёл worker completion как часть повторного входа после RSET/disconnect.
- PASS: Linux Docker image с одним `serve-all-starttls` poll-reactor; данные
  только в bind mount `./data/g07-pop3`, CA `./data/pki/ca.pem`.
- G07 остаётся NOT RUN: destructive fixture ещё использует `bob`, тогда как
  цель требует отдельный `pop@litterkeep.test`, и полный external
  interoperability profile не завершён.

### 2026-09-20: G07 PASS

- Отдельный fixture `pop@litterkeep.test` создан как account `pop` и проверен
  независимыми `smtplib`/`poplib` клиентами в Linux Docker. SMTP STARTTLS
  доставил сообщение в общий INBOX; POP3 STLS подтвердил UIDL и RETR, `RSET`
  и TCP disconnect не опубликовали DELE, а QUIT выполнил durable удаление.
- Отдельная maildrop lease проверена на конкурентный отказ и повторную выдачу
  после EOF. Политика IMAP EXPUNGE против POP3 snapshot документирована и
  проверена отдельным client-сценарием без второго Store writer-а.
- PASS: критерий G07 `UIDL/RETR, правильное удаление и освобождение maildrop
  lease` выполнен. Неподдержанные внешние клиенты и расширенная failure matrix
  остаются задачами G10, а не ложным claim о всеобщей interoperability.

### 2026-09-20: G08 Sieve language foundation

- Заглушка `fileinto` заменена собственными отдельными слоями lexer, parser,
  AST, static validation и bounded execution. Поддерживаются comments,
  `require`, `if`/`else`, `true`/`false`, `header :contains`, `exists`,
  `not`, `allof`, `anyof`, `keep`, `discard`, `fileinto`, `redirect` и `stop`.
  Результат исполнения является `DeliveryPlan`, а не записью напрямую в Store.
- Unit-тест доказал обязательное правило `Invoices`: заголовок `[invoice]`
  выдаёт только folder-plan `Invoices`; отдельно проверены implicit keep,
  comments и отказ неподдержанного `require`/unsafe mailbox.
- Проверено локально: `cargo test --all-targets --locked` (112 passed) и
  `cargo clippy --all-targets --locked -- -D warnings`. G08 ещё не завершён:
  требуется versioned script storage, транзакционное применение delivery plan,
  ManageSieve listener и независимый сетевой SMTP -> IMAP сценарий.

### 2026-09-20: G08 versioned script storage checkpoint

- Единственный Store owner теперь хранит scripts в `sieve/<account>.state`.
  PUT сначала parser-ом проверяет source, строит candidate snapshot, fsync-ит
  temporary file и только затем atomically заменяет state. Account, script,
  версия, active flag и quotas ограничены, поэтому ошибочная загрузка не
  заменяет действующий script.
- Unit-тест создал и активировал `invoices`, отверг невалидное обновление,
  перепроверил прежний source и active metadata после restart Store.
- Проверено локально: `cargo test --all-targets --locked` (113 passed),
  `cargo clippy --all-targets --locked -- -D warnings`, формат и лимит файлов.
  G08 остаётся RUNNING: storage должен быть подключён к worker/SMTP delivery,
  после чего нужен самостоятельный ManageSieve listener и внешний сценарий.

### 2026-09-20: G08 transactional delivery-plan checkpoint

- `AppendMany` storage worker теперь перед публикацией message links получает
  active script каждого local recipient у того же Store owner. Sieve plan
  вычисляется до первой записи journal: `fileinto "Invoices"` заменяет
  implicit INBOX, обычное письмо остаётся в INBOX. Redirect пока возвращает
  явную delivery error, не теряя внешний адрес молча до подключения G09 queue.
- Unit-тест активировал invoice-rule, доставил invoice и контрольное письмо,
  затем подтвердил по folder snapshots ровно одно сообщение в `Invoices` и
  одно в `INBOX`. Это Store-level proof, сетевой ManageSieve/SMTP сценарий
  ещё не заявляется выполненным.

### 2026-09-20: G08 ManageSieve FSM foundation

- Добавлено собственное ManageSieve ядро с typed commands, quoted-string
  framing boundary и состояниями plaintext/authenticated/closed. CAPABILITY,
  STARTTLS, AUTHENTICATE PLAIN, LOGOUT, HAVESPACE, PUTSCRIPT, LISTSCRIPTS,
  SETACTIVE, GETSCRIPT, DELETESCRIPT, RENAMESCRIPT, CHECKSCRIPT и NOOP
  представлены отдельными typed действиями; password запрещён до TLS.
- Unit-тесты подтверждают TLS-before-AUTH, смену authenticated state, передачу
  PUTSCRIPT source и строгий отказ лишней arity. Это не является claim о
  listener: TCP literals, storage worker completion и Linux acceptance ещё
  требуются до G08 PASS.

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

### 2026-09-19: TLS SMTP AUTH PLAIN submission checkpoint

- SMTP AUTH PLAIN теперь разбирается на протокольной границе, а пароль
  проверяет bounded storage worker. Reactor не читает credential-файлы и не
  получает владение store. Успешный completion связывает account с конкретной
  session и её generation.
- TLS EHLO рекламирует только реализованный `AUTH PLAIN`. Plaintext listener
  AUTH не рекламирует и отвечает `538`. Защищённый submission требует AUTH до
  MAIL/RCPT/DATA и разрешает envelope sender только для вошедшего локального
  account. Отдельный peer listener сохраняет возможность пустого reverse-path.
- SMTP и IMAP state transitions разделены по файлам. Новый
  `reactor/smtp_protocol.rs` содержит 239 строк, `reactor/protocol.rs` 165
  строк; `mod.rs` не добавлялся.
- PASS на Windows: `cargo fmt --check`; `cargo test --locked` (29 passed);
  `cargo clippy --locked --all-targets -- -D warnings`; `git diff --check`.
- PASS в новом Linux Docker image `litterkeep-server:smtp-auth` и чистом volume
  `litterkeep-smtp-auth-test`: стандартный Python `SMTP_SSL.login` отправил от
  Alice письмо Bob, отдельный `IMAP4_SSL` получил MIME-вложение, байты совпали.
  SHA-256: `ae8cbf6572ca0f8f11c8890b0eb2eb081bd0c42fa8c92286a4bc1983737c7e66`.
- PASS `smtp_auth_policy.py`: неверный пароль получил `535`, MAIL до AUTH
  получил `530`, sender Bob под Alice получил `553`, plaintext AUTH получил
  `538`, а plaintext EHLO не объявил AUTH.
- G03 остаётся NOT RUN: отсутствуют handshake timeout, полноценный admission
  control и STARTTLS. G04 также остаётся NOT RUN до Received/Return-Path,
  идемпотентности delivery и остальных критериев этапа.

### 2026-09-19: повторный аудит правил исходников

- Устранено прямое позиционное чтение первого сообщения в store-тесте. Старые
  Python blackbox-клиенты больше не читают `sys.argv[n]`, а используют
  `argparse` и именованные поля. Присваивание MIME-заголовков через mapping API
  `EmailMessage` не является индексированием массива.
- Составные логические цепочки заменены исчерпывающим `match` или `if let` в
  MIME base64, queue, SMTP path, orchestrator ownership/wakeup, Sieve mailbox,
  store mailbox и recovery. Внешние строковые состояния queue уже переводятся
  в `State`, домены в `LocalRoute`, а SMTP reply в новых тестах в `Reply`.
- PASS: все Rust/Python файлы не превышают 300 строк, `mod.rs` отсутствует;
  `cargo fmt --check`; `cargo test --locked` (29 passed); `cargo clippy
  --locked --all-targets -- -D warnings`; все изменённые Python CLI приняли
  `--help`; `git diff --check`.

### 2026-09-19: TLS handshake deadline checkpoint

- Каждая implicit-TLS session получает отдельный пятиисекундный handshake
  deadline. Reactor вычисляет ближайший deadline и передаёт его в `poll(2)`,
  поэтому не использует heartbeat, busy-loop или поток-таймер на соединение.
- После пробуждения отдельный `reactor/deadline.rs` удаляет только session, у
  которой rustls всё ещё находится в handshake и deadline истёк. Завершённый
  handshake снимает deadline и дальше живёт по протокольным правилам.
- PASS в Linux Docker image `litterkeep-server:handshake-timeout`:
  `tls_handshake_timeout.py` открыл TCP и не послал ClientHello; сервер закрыл
  его через 5.013 секунды. Сразу после освобождения slot полный TLS SMTP AUTH
  to IMAP attachment сценарий и вся негативная AUTH policy снова прошли.
- PASS на Windows для доступных целей: format, 29 unit tests, Clippy. Unit-тест
  Linux-only deadline модуля на Windows не компилируется и отдельно не
  засчитывается; фактическое Linux-поведение подтверждено blackbox-тестом.
- G03 остаётся NOT RUN: требуется доказать общий admission limit и исчерпание
  лимита без утечки, а также реализовать обязательный STARTTLS-профиль задания.

### 2026-09-19: TLS admission acceptance checkpoint

- Независимый `tls_admission_limit.py` открыл 256 незавершённых TLS sessions.
  257-е соединение было немедленно закрыто сервером, то есть ограничение
  проверено по сетевому поведению, а не по наличию `MAX_SESSIONS` в исходнике.
- После handshake deadline тест подтвердил закрытие всех 256 соединений. Затем
  стандартный TLS SMTP AUTH to IMAP attachment сценарий снова прошёл, что
  подтверждает освобождение и безопасное повторное использование session slot.
- Реализованные части критерия G03 теперь имеют независимые доказательства:
  TLS trust/SAN, верный и неверный пароль, AUTH только под TLS, handshake
  timeout, buffer bounds и connection admission. G03 остаётся RUNNING, потому
  что его зависимость G02 ещё не завершена и итоговый профиль также требует
  STARTTLS. Статусы ранее начатых G01-G05/G09 в верхней таблице исправлены с
  вводящего в заблуждение `NOT RUN` на `RUNNING`.

### 2026-09-19: durable directory sync and crash checkpoint

- После синхронизации временного blob и атомарного rename storage worker теперь
  вызывает Linux `fsync` каталога `blobs`. После синхронизации mailbox journal
  он также синхронизирует каталог `mailboxes`. Эта блокирующая работа остаётся
  вне reactor в единственном storage worker-е.
- PASS в Linux Docker image `litterkeep-server:durable-dir`: независимый
  TLS SMTP-клиент дождался финального success для письма с бинарным вложением;
  процесс был завершён `docker kill` с exit code 137; новый процесс открыл тот
  же volume и независимый TLS IMAP-клиент нашёл то же письмо и точные байты.
- Это доказательство сохранности при аварии процесса после подтверждённого
  commit, а не заявление о моделировании физической потери питания или лжи
  накопителя. G02 остаётся RUNNING: текстовый journal нужно заменить требуемым
  binary framing, добавить атомарный multi-recipient transaction и дисковую
  квоту с проверенными отказами.

### 2026-09-19: versioned binary journal and UID index checkpoint

- Mailbox journal переведён с текстовых строк на собственный binary frame:
  magic `LKJF`, version 1, big-endian sequence UID, transaction id, bounded
  payload length, payload, FNV-1a checksum и отдельный commit marker. Decoder
  использует проверяемый cursor и не индексирует недоверенный массив напрямую.
- Recovery выполняет один линейный проход `O(n)`: недописанный последний frame
  обрезается до начала frame, а полный frame с неверными magic/version/length,
  transaction id/checksum/commit marker останавливает открытие store. Поиск
  следующего случайного magic не выполняется.
- Добавлен долговечный `format-version`. Неизвестный или старый неразмеченный
  store с mail data получает диагностируемый отказ; автоматического мигратора
  пока нет. Это намеренная несовместимость прежних временных test volumes.
- Оптимизирован hot path: single-writer восстанавливает `next UID` каждого
  mailbox один раз при старте и хранит его в `HashMap`. Append использует в
  среднем `O(1)` lookup и обновляет cache только после durable commit, вместо
  повторного полного replay и накопительного `O(n^2)` поведения.
- PASS: 31 Windows unit test, format и Clippy. В чистом Linux Docker volume
  проверены marker, binary prefix `4c 4b 4a 46 00 01`, последовательные
  `bob-1.eml`..`bob-3.eml`, kill с exit 137, recovery и повторный SMTP AUTH to
  IMAP attachment с совпавшими байтами.
- G02 остаётся RUNNING: один общий атомарный multi-recipient transaction,
  failpoint matrix и дисковая quota ещё отсутствуют.

### 2026-09-19: bounded blob quota checkpoint

- Добавлен отдельный `Quota` budget неизменяемых blobs с документированным
  пределом 25 MiB. При старте он одним линейным проходом учитывает все файлы,
  включая orphan; далее single writer делает `O(1)` preflight и обновление
  счётчика. Проверка выполняется до создания временного файла.
- Unit-тест с budget 4 bytes подтвердил `StorageFull` для сообщения 5 bytes,
  пустой mailbox и отсутствие temp/final blob после отказа. Полный набор вырос
  до 32 tests; format и Clippy прошли.
- PASS Linux Docker build `litterkeep-server:blob-quota`. Новый бинарник открыл
  ранее созданный version-1 store и повторил независимый TLS SMTP AUTH to IMAP
  attachment сценарий с точным совпадением байтов.
- G02 остаётся RUNNING до атомарного multi-recipient transaction и failpoint
  matrix. Квота пока ограничивает blobs, но не journal/accounts metadata; это
  явно не заявляется полной квотой всего filesystem tree.

### 2026-09-19: atomic multi-recipient Unit of Work checkpoint

- Store format обновлён до `litterkeep-store-v2`. Один transaction ID теперь
  публикует один immutable `message-<id>.eml` и несколько mailbox links. Все
  mailbox frames синхронизируются раньше единственного global commit frame в
  `journal/transactions.log`; materialized view обновляется только после него.
- Повторный RCPT одному mailbox дедуплицируется с сохранением порядка первого
  появления. Один blob совместно используется всеми mailbox links, поэтому
  объём вложения не умножается на число локальных получателей.
- При ошибке после публикации blob writer переходит из типизированного
  `Writable` в `Poisoned` и больше не принимает изменения до recovery. UID и
  transaction ID из незавершённых frames не переиспользуются после restart,
  но сами frames без global commit остаются невидимыми.
- PASS 35 unit tests. Failpoint после первого mailbox frame подтвердил: нет
  частичной видимости Bob/Alice, повторная запись отвергается poisoned writer,
  recovery продолжает с новым UID. Проверены format и Clippy.
- PASS в чистом Linux Docker volume: независимый TLS SMTP-клиент отправил один
  DATA для Alice и Bob; оба TLS IMAP-клиента получили идентичные RFC822 bytes и
  вложение. На диске был ровно один blob, два mailbox journal по 51 byte и один
  commit frame 26 bytes. После `docker kill` exit 137 оба link сохранились;
  обычный attachment regression также прошёл.
- На этом checkpoint G02 сохранял RUNNING из-за незавершённой зависимости G01.
  Реализованный multi-recipient commit больше не являлся причиной удерживать
  этап незавершённым; полная failpoint matrix относится к G10.

### 2026-09-19: G01 и G02 PASS, реальные session leases

- Reactor теперь регистрирует каждую живую TCP/TLS `Session` как значение
  `ResourceLease<Session>`. `TaskId` является владельцем эксклюзивного lease,
  а не записью в параллельном журнале наблюдения. I/O, storage completion и
  deadline получают session только через lease.
- Все terminal-пути EOF, I/O error и handshake timeout вызывают единый
  `Orchestrator::close`: реальный socket/TLS resource уничтожается, а task,
  resource, ready и wait metadata удаляются. Admission использует фактический
  `resource_count`, поэтому registry является источником решения, а не логом.
- PASS три последовательных цикла независимого Linux blackbox-теста: по 256
  незавершённых TLS sessions, немедленный отказ 257-й, quiescence после
  пятиисекундного deadline и успешный новый TLS-клиент. Тест посылает первый
  байт TLS record до измерения overflow, чтобы не зависеть от TCP backlog.
- PASS Docker Linux test target `litterkeep-server:linux-tests`: 38/38. В него
  входят реальный `UnixStream` wake через `poll(2)`, округление deadline,
  double-issue, lost-wake, cancellation ownership и удаление terminal registry
  records. На Windows PASS 36/36, format и Clippy с `-D warnings`.
- G01 получает PASS: один poll-reactor обслуживает idle sessions без потока на
  соединение, реальный ресурс находится во владении lease, а обязательные wake
  и ownership-тесты прошли на Linux. Файл модели состояний отделён в
  `runtime/model.rs`; `mod.rs` нет, каждый Rust/Python файл не длиннее 300 строк.
- G02 получает PASS по критериям этапа: durable binary journal и immutable blob
  пережили kill/restart, второй writer отвергается единым кроссплатформенным
  `AlreadyExists`, а принятый commit завершается storage worker даже после
  отмены requester. Полная failpoint matrix остаётся обязательной для G10 и не
  объявлена выполненной.
- Текущий этап остаётся G03. Его implicit TLS, AUTH, admission и timeout уже
  проверены, но обязательный STARTTLS-профиль ещё не реализован, поэтому G03
  сохраняет RUNNING.

### 2026-09-19: SMTP STARTTLS transport checkpoint

- Добавлен отдельный типизированный `TlsMode::StartTls`, а SMTP parser получил
  `Command::StartTls`. До upgrade EHLO рекламирует STARTTLS и не рекламирует
  AUTH; после успешного handshake клиент обязан повторить EHLO и получает AUTH.
- Upgrade выполняется только после полной plaintext-записи ответа `220`.
  Накопленный plaintext input очищается, envelope и authentication сбрасываются,
  затем session создаёт новый `ServerTls` с тем же ограниченным handshake
  deadline. TLS records остаются в существующем poll-reactor без нового потока.
- PASS стандартным Python `smtplib.SMTP.starttls()` с проверкой цепочки и SAN:
  Alice прошла AUTH, отправила Bob MIME-письмо, отдельный IMAP-клиент прочитал
  его, SHA-256 вложения `f3a25aa93aa2fbba28d79260535bbd6a5eb0fc1c24a8b0f04e12b484c1dfe363`.
- PASS: Windows 37/37, format, Clippy; Docker Linux target 39/39. Все исходники
  сохраняют лимит 300 строк, `mod.rs` не добавлен.
- На этом checkpoint G03 оставался RUNNING: SMTP STARTTLS был проверен, но IMAP
  STARTTLS и его независимый клиентский сценарий ещё не были реализованы.
  Implicit TLS для обоих клиентских протоколов уже был проверен.

### 2026-09-19: G03 PASS, IMAP STARTTLS

- IMAP parser переводит `STARTTLS` в отдельный `Command::StartTls`. До upgrade
  CAPABILITY объявляет `STARTTLS LOGINDISABLED`, а LOGIN возвращает
  `NO [PRIVACYREQUIRED]`; пароль не передаётся storage worker по plaintext.
- После tagged `OK` transport переключается на тот же неблокирующий rustls FSM,
  который использует SMTP. Plaintext input очищается, authentication state
  сбрасывается, handshake получает ограниченный deadline и остаётся реальным
  ресурсом lifecycle lease.
- PASS независимым стандартным `imaplib.IMAP4.starttls()`: неизвестный CA
  отвергнут, plaintext LOGIN отвергнут, доверенный CA и IP SAN приняты, Bob
  вошёл и прочитал доставленное через SMTP STARTTLS MIME-вложение. SHA-256:
  `f3a25aa93aa2fbba28d79260535bbd6a5eb0fc1c24a8b0f04e12b484c1dfe363`.
- PASS Windows: 38/38, format, Clippy. PASS Docker Linux: 40/40, включая poll,
  deadline и оба typed STARTTLS parser tests.
- G03 получает PASS: независимо проверены implicit TLS и STARTTLS, доверие и
  отказ неизвестному CA, правильный и неправильный пароль, plaintext policy,
  handshake timeout, ограничение буферов, admission и освобождение resources.
  Текущий последовательный этап теперь G04.

### 2026-09-19: G04 PASS, envelope trace headers

- SMTP transaction теперь передаёт в delivery типизированный `Envelope` с
  reverse-path и уже принятыми RCPT. Получатели по-прежнему не извлекаются из
  `To`, `Cc` или `Bcc`.
- Отдельный `smtp_trace.rs` добавляет `Return-Path` и `Received` перед исходными
  RFC822 bytes. Дата вычисляется за O(1) без таблиц с прямым индексированием;
  исходное MIME-тело не разбирается и не сериализуется повторно.
- Unit-тесты проверили обычный и пустой reverse-path, сохранение исходного Bcc и
  нулевого байта. PASS Windows 40/40 и Docker Linux 42/42, format и Clippy.
- PASS независимым SMTP/IMAP STARTTLS blackbox: Alice отправила Bob вложение,
  сохранённый RFC822 начинается с envelope-derived `Return-Path` и `Received`,
  а SHA-256 вложения остался
  `f3a25aa93aa2fbba28d79260535bbd6a5eb0fc1c24a8b0f04e12b484c1dfe363`.
- G04 получает PASS по критерию этапа: Alice -> Bob, успех DATA только после
  durable storage completion, сохранность после kill/restart и отсутствие
  публикации оборванной DATA уже проверены предыдущими checkpoint. Расширенная
  матрица потери финального ответа и storage failpoints остаётся в G10 и не
  объявлена выполненной. Текущий последовательный этап теперь G05.

### 2026-09-19: G05 MIME byte-boundary checkpoint

- Вместо прежнего UTF-8-only map добавлен отдельный байтовый header parser.
  Он сохраняет порядок и повторы полей, точные raw ranges и body offset,
  выполняет unfolding continuation lines и отвергает bare LF. Бинарное значение
  одного поля больше не делает недоступным весь RFC822 source.
- Добавлен отдельный однопроходный quoted-printable decoder: hex octets, CRLF
  soft breaks, строгий отказ bare CR/LF, усечённых и неверных escape. Алгоритм
  работает за O(n) и выделяет не больше начального размера encoded input.
- PASS Windows: 44/44, format, Clippy; лимит 300 строк и запрет `mod.rs`
  соблюдены. G05 остаётся RUNNING: multipart line delimiters, MIME parameters,
  encoded-word, charset profiles и IMAP BODYSTRUCTURE ещё не готовы и не
  объявляются проверенными.

### 2026-09-19: G05 charset, encoded-word и parameter checkpoint

- Внешние имена charset преобразуются в `Charset::UsAscii`, `Utf8` или
  `Iso8859_1`; неизвестный профиль не угадывается. US-ASCII и UTF-8 проверяются
  строго, Latin-1 отображается по байтам без повреждения raw source.
- Отдельный encoded-word decoder поддерживает B и Q, underscore-space и hex
  escapes. Неверная оболочка, неизвестная transfer encoding, charset или Q
  escape дают диагностируемый отказ.
- Structured parameter parser переводит известные media types в enum, учитывает
  quoted semicolon и escape, extended value, упорядоченные RFC 2231 segments и
  percent encoding. Пропущенный segment и неизвестный charset отклоняются.
- PASS 49 Windows unit tests, format и Clippy. Независимый blackbox запустил
  собранный `mime_probe` и сравнил encoded UTF-8 subject и кириллический
  `filename*0*`/`filename*1*` с Python `email`: результаты совпали.
- G05 остаётся RUNNING до line-aware multipart дерева, вложенности, raw ranges
  частей и IMAP ENVELOPE/BODYSTRUCTURE.

### 2026-09-19: G05 line-aware multipart checkpoint

- Добавлен bounded MIME tree: максимум 16 уровней и 256 entities. Каждый узел
  хранит абсолютные ranges raw entity, headers и body; исходное сообщение не
  копируется и не сериализуется заново.
- Multipart boundary распознаётся только как полная delimiter line с CRLF и
  допустимым transport padding. Произвольная подстрока `--boundary` внутри тела
  часть не разрезает. Preamble, epilogue и обязательный closing delimiter
  учитываются отдельно; отсутствие закрытия является ошибкой.
- PASS 51 Windows unit test, format и Clippy. Расширенный независимый
  `mime_python_comparison.py` подтвердил совпадение числа частей и точных байтов
  первого body с Python `email`, дополнительно к encoded-word и RFC 2231 имени.
- G05 остаётся RUNNING: нужны декодирование transfer encoding на уровне entity,
  BODYSTRUCTURE/ENVELOPE и корректные IMAP section/partial responses.

### 2026-09-19: G05 IMAP BODYSTRUCTURE checkpoint

- Каждый MIME entity теперь хранит `TransferEncoding` enum и собственные
  parameters. Leaf body декодируется только из абсолютного raw range; 7bit
  проверяет ASCII, base64 и quoted-printable используют наши bounded decoder-ы,
  неизвестная encoding не угадывается.
- Собственный BODYSTRUCTURE encoder строит multipart children в исходном
  порядке, экранирует строковые значения и сообщает размер encoded raw body в
  октетах. Для text parts отдельно считается число CRLF lines.
- IMAP FETCH принимает типизированные одиночные атрибуты `RFC822`,
  `RFC822.SIZE`, `BODYSTRUCTURE`. Неподдержанный атрибут остаётся BAD, а
  повреждённый MIME при BODYSTRUCTURE возвращает NO вместо ложной структуры.
- PASS сетевой SMTP/IMAP STARTTLS blackbox: доставленное multipart/mixed письмо
  вернуло BODYSTRUCTURE с `MIXED`, отдельный RFC822.SIZE и исходный RFC822;
  вложение сохранило SHA-256
  `f3a25aa93aa2fbba28d79260535bbd6a5eb0fc1c24a8b0f04e12b484c1dfe363`.
- PASS Windows 56/56, Clippy и format; PASS Docker Linux 58/58. G05 остаётся
  RUNNING до ENVELOPE, BODY section/PEEK и partial FETCH по октетам.

### 2026-09-19: G05 ENVELOPE и BODY section checkpoint

- FETCH parser преобразует `BODY[]`, `BODY.PEEK[part]`, `BODY[part.MIME]` и
  `<offset.count>` в `BodySection`/`Partial`, не передавая строковые команды в
  reactor. Part numbers проходят 1-based tree traversal через безопасный `get`.
- Отдельный `imap_fetch.rs` выбирает raw encoded body либо raw MIME headers по
  абсолютным ranges. Partial применяется после выбора section и считает
  октеты; BODY.PEEK не меняет состояние сообщения.
- Добавлен собственный ENVELOPE encoder. Имена заголовков типизированы,
  From/Sender/Reply-To/To/Cc/Bcc разбираются в name/mailbox/host tuples,
  Sender и Reply-To получают fallback к From, encoded subject декодируется.
- PASS сетевой STARTTLS blackbox: `BODY.PEEK[1]<0.5>` вернул ровно `trans`,
  `BODY.PEEK[2.MIME]` вернул Content-Type вложения, ENVELOPE содержал subject и
  отдельные Alice/Bob mailbox/host, полный RFC822 и SHA-256 вложения совпали.
- PASS Windows 59/59, Clippy и format; PASS Docker Linux 61/61. На этом
  checkpoint G05 оставался RUNNING: ещё требовались составные encoded-word и
  рекурсия `message/rfc822`; HEADER.FIELDS был перенесён в IMAP-этап G06.

### 2026-09-19: G05 PASS, composite words и message/rfc822

- Encoded-word decoder обрабатывает последовательность B/Q words внутри
  неструктурированного header и удаляет только folding whitespace между двумя
  соседними encoded words. Plain prefix и обычный текст сохраняются.
- `message/rfc822` получает дочерний entity с абсолютными raw ranges и
  ограничивает transfer encoding значениями, для которых raw mapping остаётся
  однозначным. BODYSTRUCTURE включает вложенные ENVELOPE, structure и line count.
- PASS 62 Windows unit test, format и Clippy; PASS Docker Linux 64/64.
  Независимый Python differential
  test подтвердил composite UTF-8/Latin-1 header, вложенное message/rfc822 body,
  multipart ranges и RFC 2231 filename.
- G05 получает PASS по критерию этапа: собственный MIME parser и IMAP reading
  проверены стандартным независимым клиентом, включая ENVELOPE, BODYSTRUCTURE,
  raw section, octet partial и точное совпадение бинарного вложения после
  SMTP -> store -> IMAP. Нереализованные HEADER.FIELDS и полноценный
  инкрементальный IMAP grammar остаются явными задачами G06, а не считаются
  выполненными. Текущий последовательный этап теперь G06.

### 2026-09-19: G06 IMAP lexer checkpoint

- Удалён разбор IMAP через `split_whitespace`. Однопроходный lexer за `O(n)`
  выделяет atoms, границы списков и quoted strings с escape; число токенов
  ограничено, чтобы одна команда не создавала неограниченное число объектов.
- LOGIN теперь сохраняет пробелы внутри quoted username/password, а FETCH
  разбирает parenthesized attribute как синтаксические токены, не удаляя
  скобки строковой эвристикой.
- PASS 64 Windows unit test, format и Clippy. G06 остаётся RUNNING: literals,
  sequence-set, folders, UID, flags, APPEND и IDLE ещё не реализованы.

### 2026-09-19: G06 sequence-set и UID FETCH checkpoint

- После сверки RFC 9051 sections 4.1.1, 6.4.5 и 6.4.9 FETCH принимает
  одиночные номера, списки, прямые/обратные диапазоны и `*`. `UID FETCH`
  сохраняет отдельный enum пространства адресации до получения store snapshot.
- Диапазоны не разворачиваются. Они один раз разрешаются относительно snapshot,
  сортируются и объединяются за `O(r log r)`, затем каждое сообщение проверяется
  бинарным поиском за `O(log r)`. Число исходных диапазонов ограничено 64.
- Reactor возвращает все выбранные сообщения в порядке mailbox. UID FETCH
  не подменяет номер untagged FETCH: снаружи остаётся sequence number, а UID
  автоматически включается в каждый ответ.
- PASS 68 Windows unit tests, format и Clippy; PASS Docker Linux 70/70.
  Обычный Windows cross-check не засчитан: в PATH нет Linux C compiler для
  `ring`, поэтому authoritative Linux проверкой служит Docker build/test.
- G06 остаётся RUNNING: literals, flags, APPEND, folders, UIDVALIDITY/UIDNEXT и
  IDLE ещё не реализованы.

### 2026-09-19: G06 durable SELECT metadata checkpoint

- Store read теперь возвращает согласованный `MailboxSnapshot`: messages,
  UIDVALIDITY и UIDNEXT берутся одним owner-ом из одного materialized view.
  Snapshot вынесен в отдельный файл, поэтому чтение не смешано с записью.
- SELECT и EXAMINE различают READ-WRITE/READ-ONLY, возвращают FLAGS,
  PERMANENTFLAGS, UIDVALIDITY и UIDNEXT. INBOX преобразуется в enum на границе
  parser-а. FETCH до успешного SELECT получает NO, а не читает mailbox по одной
  лишь аутентификации.
- UIDVALIDITY равен 1 при действующем контракте: журналы mailbox не удаляются,
  UID не перераспределяются. Будущий DELETE обязан сохранить allocator либо
  назначить новую validity; это явно зафиксировано у реализации snapshot.
- PASS 69 Windows unit tests, format и Clippy; PASS Docker Linux 71/71.
  Независимый STARTTLS SMTP/IMAP blackbox подтвердил отказ FETCH до SELECT,
  `UIDVALIDITY 1`, `UIDNEXT 2`, `UID FETCH 1:*`, MIME attachment SHA-256
  `f3a25aa93aa2fbba28d79260535bbd6a5eb0fc1c24a8b0f04e12b484c1dfe363`.
  После restart того же контейнера и volume отдельный клиент повторно получил
  один message, неизменный UIDVALIDITY, UIDNEXT 2 и UID 1.
- G06 остаётся RUNNING: literals, APPEND, mutable flags, folders и IDLE ещё не
  реализованы.

### 2026-09-19: G06 synchronized APPEND checkpoint

- APPEND INBOX принимает синхронизирующий `{octets}` literal: continuation
  отправляется только после TLS-аутентификации, проверки mailbox и лимита 10
  MiB. Необъявленный LITERAL+ не принимается и не рекламируется.
- `LiteralReceiver` принадлежит конкретной session, поглощает вход частями и
  отделяет ровно объявленные октеты от завершающего CRLF и следующей команды.
  Большой literal накапливается в bounded receiver, а общий input buffer не
  обязан вместить сообщение целиком.
- Store request ставится только после полного literal. Tagged OK с
  `APPENDUID 1 uid` выдаётся лишь после durable append completion; ошибка store
  возвращает NO. Оборванная сессия не публикует частичный literal.
- PASS unit-тест всех внутренних границ двух фрагментов; PASS 70 Windows tests,
  format и Clippy; PASS Docker Linux 73/73.
- Независимый Python `imaplib` через STARTTLS добавил 97 октетов с NUL,
  получил `APPENDUID 1 1`, затем SELECT/FETCH вернули точные исходные байты.
  После restart с тем же volume подтверждены один message, UIDVALIDITY 1,
  UIDNEXT 2 и UID 1.
- G06 остаётся RUNNING: APPEND flags/date, mutable flags, folders и IDLE ещё не
  реализованы.

### 2026-09-19: G06 durable FLAGS checkpoint

- Системные `Answered/Flagged/Deleted/Seen/Draft`, набор и операция
  replace/add/remove вынесены в общий enum-домен. Parser сравнивает wire atoms
  только на границе и передаёт дальше bitset, а не строки.
- FETCH FLAGS, STORE, STORE.SILENT и UID STORE работают через sequence-set.
  Двухфазный completion сначала разрешает snapshot в устойчивые UID, затем
  ставит отдельную mutation владельцу store. Tagged OK и untagged FETCH FLAGS
  выдаются только после успешного durable completion; `.SILENT` подавляет
  только untagged response.
- Store пишет весь batch UID одним append-only frame с version, count,
  checksum и commit marker, затем `sync_all` и directory sync. Диапазоны UID
  проверяются бинарным поиском, дубликаты сортируются и удаляются до записи.
  Неизвестный UID отклоняется до создания журнала; uncertain write отравляет
  writer до recovery.
- PASS 73 Windows tests, format и Clippy; PASS Docker Linux 76/76.
  Независимый `imaplib` подтвердил обычный STORE response, отсутствие FETCH у
  `.SILENT`, UID STORE и последующий FETCH FLAGS. После restart того же volume
  `\\Seen` у первого и `\\Draft` у второго сообщения сохранились.
- G06 остаётся RUNNING: APPEND flags/date, folders, COPY/MOVE/EXPUNGE и IDLE
  ещё не реализованы.

### 2026-09-19: G06 event-driven IDLE checkpoint

- `IDLE` и его единственная continuation `DONE` представлены enum-типами на
  границе parser-а. Selected session сохраняет lease реального соединения и
  остаётся зарегистрированной в Linux `poll`; отдельный поток или busy loop на
  ожидающую сессию не создаётся.
- `EXISTS` публикуется всем подходящим IDLE-сессиям только после успешного
  durable `Append`/`AppendMany` completion worker-а. Список затронутых mailbox
  хранится в типизированном pending-контексте, сортируется и дедуплицируется,
  поэтому одна фиксация не увеличивает счётчик одной сессии дважды.
- `DONE` завершает исходный tag ответом `OK IDLE completed`. Типизированный
  IMAP `NOOP` после этого подтверждает возврат к обычной грамматике команд.
- PASS 75 Windows tests, format и Clippy; PASS Docker Linux 78/78.
- Независимый Python 3.14 `imaplib` двумя STARTTLS-соединениями подтвердил
  `CAPABILITY IDLE`, пустой SELECT, durable APPEND из второй сессии, `* 1
  EXISTS` в первой, успешные DONE и следующий NOOP.
- G06 остаётся RUNNING: APPEND flags/date, folders, COPY/MOVE/EXPUNGE, очередь
  событий с определённым поведением при отставании и профиль 32 одновременных
  IDLE-сессий ещё не реализованы или не проверены.

### 2026-09-19: G06 APPEND metadata checkpoint

- APPEND grammar принимает ограниченный список системных flags и quoted
  INTERNALDATE перед синхронизирующим literal. Строковые flag atoms и названия
  месяцев преобразуются в enum/bitset на границе parser-а.
- Собственный Gregorian-конвертер проверяет календарную дату, время и numeric
  zone, хранит секунды Unix и выдаёт канонический UTC INTERNALDATE без locale и
  timezone процесса. FETCH INTERNALDATE читает значение из mailbox snapshot.
- Mailbox journal frame v2 фиксирует UID, transaction id, initial flags,
  INTERNALDATE и blob reference под одной checksum и commit marker. Поэтому
  APPENDUID не может быть подтверждён раньше метаданных. Reader продолжает
  распознавать существующие v1 frames с безопасными значениями по умолчанию.
- Последующие STORE mutations перекрывают initial flags, но при отсутствии
  flag journal snapshot использует flags атомарного APPEND frame.
- PASS 80 Windows tests, format и Clippy; PASS Docker Linux 83/83.
  Независимый Python 3.14 `imaplib` передал `\\Seen`, `\\Draft` и
  `19-Sep-2026 12:34:56 +0300`; FETCH вернул оба флага и
  `19-Sep-2026 09:34:56 +0000`. После restart контейнера с тем же volume
  отдельный verify-only запуск получил те же значения.
- G06 остаётся RUNNING: folders, COPY/MOVE/EXPUNGE, HEADER.FIELDS, UID SEARCH,
  subscriptions, Recent, очередь IDLE-событий и расширенный concurrency profile
  ещё не реализованы или не проверены.

### 2026-09-19: G06 durable folder catalog checkpoint

- Store содержит account-scoped каталог со стабильными монотонными folder ID.
  Пользовательское имя находится только в checksum-защищённом journal frame;
  путь mailbox строится из проверенного account и числового ID, поэтому даже
  Unicode или вложенное имя не становится filesystem path.
- При первом обращении идемпотентно создаются INBOX, Sent, Drafts, Trash, Junk,
  Archive, Invoices и тестовый дочерний `Invoices/2026`. SPECIAL-USE хранится
  enum-значением, а не выводится из английского имени. Прерванный хвост журнала
  обрезается, повреждённый полный frame отвергается.
- Типизированные CREATE и LIST проходят через bounded storage worker. LIST
  возвращает delimiter `/`, специальные атрибуты и различает поддержанные
  профили `*`, `%` и exact name. Повторный CREATE получает ALREADYEXISTS.
- PASS 82 Windows tests, format и Clippy; PASS Docker Linux 85/85.
  Независимый Python 3.14 `imaplib` подтвердил default folders, SPECIAL-USE,
  CREATE `Projects/2026`, LIST `*`, отсутствие дочерней папки в LIST `%` и
  восстановление тех же результатов после restart с неизменным volume.
- G06 остаётся RUNNING: named-folder SELECT/APPEND, RENAME/DELETE,
  subscriptions, STATUS, COPY/MOVE/EXPUNGE, modified UTF-7, HEADER.FIELDS,
  UID SEARCH, Recent и полный LIST wildcard profile ещё не готовы.

### 2026-09-19: G06 named-folder mailbox checkpoint

- SELECT разрешает account-scoped имя и получает внутренний mailbox key вместе
  со snapshot одним worker-действием. Дальнейшие FETCH/STORE используют key, а
  не повторяют строковый поиск имени и не могут перейти в каталог другого
  account. Unit test подтверждает разные keys для одинаковых имён Bob и Alice.
- APPEND сначала отправляет bounded `ResolveFolder` единственному владельцу
  store. Literal continuation выдаётся только после успешного resolution;
  неизвестная папка получает `NO [TRYCREATE]` до передачи тела. Session затем
  сохраняет проверенный key до durable append completion.
- Один и тот же внутренний key используется selected session, IDLE notification
  и APPEND pending context, поэтому поступление в named folder не смешивается с
  INBOX или одноимённой папкой другого аккаунта.
- PASS 82 Windows tests, format и Clippy; PASS Docker Linux 85/85.
  Независимый Python 3.14 `imaplib` подтвердил TRYCREATE, CREATE
  `Projects/2026`, APPEND с `\\Draft` и NUL-байтом, SELECT, точный RFC822 FETCH
  и FLAGS. После restart того же container volume отдельный запуск получил
  один message с теми же байтами и флагом.
- G06 остаётся RUNNING: RENAME/DELETE, subscriptions, STATUS,
  COPY/MOVE/EXPUNGE, modified UTF-7, HEADER.FIELDS, UID SEARCH, Recent и полный
  LIST wildcard profile ещё не реализованы.

### 2026-09-20: G06 durable folder lifecycle checkpoint

- `RENAME` и `DELETE` разобраны в типизированные варианты до reactor-а и
  выполняются bounded storage worker-ом, который единолично владеет store.
  Tagged `OK` выдаётся только после `write_all`, `sync_all` и directory sync.
- Каталог использует append-only create/rename/delete events. Переименование
  поддерева является одним checksum-защищённым frame и сохраняет внутренние
  ID. Это O(1) запись в журнал и O(n) неизбежное обновление затронутого snapshot,
  без полного переписывания каталога.
- Replay хранит high-water mark ID отдельно от активного списка. После DELETE
  и повторного CREATE старый ID не возвращается, а недописанный mutation tail
  отсекается без публикации операции. Отдельный crash-unit-test это проверяет.
- INBOX нельзя удалить или переименовать. DELETE родителя с потомками
  отклоняется `[CANNOT]`; RENAME в существующее имя, внутрь собственного
  поддерева и из отсутствующей папки получает типизированный `NO`.
- PASS: 84 Windows tests and 87 Linux Docker tests, format, Clippy
  `-D warnings`, `git diff --check`,
  отсутствие `mod.rs`, файлы не длиннее 300 строк и запрещённых смешанных
  boolean-условий.
- Независимый `imaplib` через STARTTLS проверил subtree RENAME с сохранением
  сообщения, отказ DELETE родителя, DELETE leaf, повторный CREATE с пустым
  новым mailbox и тот же результат после restart контейнера.
- G06 остаётся RUNNING: subscriptions/LSUB, STATUS, COPY/MOVE/EXPUNGE/CLOSE,
  HEADER.FIELDS, UID SEARCH, modified UTF-7, полный LIST wildcard/SPECIAL-USE
  профиль, Recent и согласование sequence-map двух сессий ещё не завершены.

### 2026-09-20: G06 subscriptions and STATUS checkpoint

- `SUBSCRIBE`, `UNSUBSCRIBE`, `LSUB` и поддержанный профиль `STATUS` проходят
  через типизированный parser, bounded worker и одного владельца store.
- Подписки хранятся независимо от существования mailbox в append-only журнале.
  В памяти используется `BTreeSet`: идемпотентное обновление имеет O(log n),
  а LSUB получает детерминированный порядок без дополнительной сортировки.
- LSUB возвращает удалённую либо ещё не созданную подписку с `\Noselect`.
  SUBSCRIBE не создаёт mailbox, а DELETE/RENAME не переписывают подписки.
- STATUS читает согласованный snapshot и поддерживает `MESSAGES`, `UIDNEXT`,
  `UIDVALIDITY`, `UNSEEN`, не меняя selected state. `RECENT` сознательно
  получает BAD как неподдержанный до реализации нормативной сессионной модели,
  вместо ложного успешного `RECENT 0`.
- PASS: 86 Windows tests, strict Clippy и format. Linux Docker: 89 tests.
  Независимый `imaplib` через STARTTLS проверил две записи с разными flags,
  durable subscribe/unsubscribe, `\Noselect` и точные STATUS counters до и
  после restart.
- По уточнённому требованию Docker использует один фиксированный образ и bind
  mount локальной `./data/g06-subscriptions-status`; named volumes и новые
  runtime-папки для каждого прогона больше не создаются. `.dockerignore`
  исключает `data`, `runtime`, `target` и `.git` из build context.
- G06 остаётся RUNNING: COPY/MOVE/EXPUNGE/CLOSE/CHECK, UID SEARCH,
  modified UTF-7, полный LIST/SPECIAL-USE профиль, Recent и согласование
  sequence-map двух сессий ещё не завершены.

### 2026-09-20: G06 durable EXPUNGE checkpoint

- `EXPUNGE` сначала читает snapshot выбранной папки, вычисляет sequence number
  каждой `\Deleted` записи, затем передаёт устойчивые UID единственному writer.
  Untagged `EXPUNGE` и tagged OK выдаются только после fsync tombstone frame.
- Новый append-only `.expunges` journal replay-ится после mailbox и flags
  журналов. Он скрывает только окончательно удалённые UID, не меняет UID
  оставшихся и не откатывает UIDNEXT. Неподтверждённый хвост frame не публикуется.
- EXAMINE сохраняет `selected_read_only` в session и возвращает
  `NO [READ-ONLY]` для EXPUNGE. Бинарные blobs пока намеренно не удаляются:
  они могут разделяться между mailbox, а отдельный GC нужен вместе с анализом
  ссылок и quota semantics.
- Исправлен decoder mailbox journal: frame всегда содержит сначала глобальный
  transaction id, затем UID mailbox. Новый regression test создаёт между двумя
  письмами Bob независимую транзакцию Alice, поэтому больше не маскирует
  различие идентификаторов и проверяет restart.
- PASS: 88 Windows tests, strict Clippy, format и structural audit. Linux
  Docker target `test` успешно выполнил `cargo test --locked`; runtime image
  прошёл проверку реальным Python `imaplib` по STARTTLS. В чистой папке
  `ExpungeFresh` независимый клиент проверил два `\Deleted` сообщения,
  sequence responses `1`, затем `2`, сохранение второго сообщения. После
  restart того же фиксированного контейнера tombstone был повторно подтверждён.
  Отдельно существующий mailbox `Expunge` подтвердил запрет EXPUNGE после
  EXAMINE и durable replay. Все данные остались в `./data/g06-expunge`.
- G06 остаётся RUNNING: COPY/MOVE/CLOSE/CHECK, UID EXPUNGE и UIDPLUS,
  UID SEARCH, modified UTF-7, полный LIST/SPECIAL-USE профиль, Recent и
  согласование sequence-map двух сессий ещё не завершены.

### 2026-09-20: G06 CLOSE and CHECK checkpoint

- `CHECK` допускается только в selected state. Все текущие mutating store
  операции уже подтверждаются fsync до IMAP OK, поэтому команда не создаёт
  ложную дополнительную запись и подтверждает именно выбранный mailbox.
- `CLOSE` в read-write state следует двухфазному пути EXPUNGE: сначала
  snapshot определяет Deleted UID и sequence-map, затем один writer durable
  фиксирует tombstone. До этой фиксации selected state сохраняется. После неё
  CLOSE отправляет только tagged OK, без untagged EXPUNGE, и снимает selection.
- `CLOSE` после EXAMINE не пытается изменять mailbox: он только снимает
  read-only selection. Некорректные аргументы `CHECK` и `CLOSE` отклоняются
  parser-ом как BAD.
- PASS: 89 Windows tests, format и strict Clippy. Независимый Python
  `imaplib` по STARTTLS создал `CloseFresh` в `./data/g06-expunge`, выполнил
  CHECK, STORE `\\Deleted`, CLOSE и новый SELECT, который вернул ровно 0
  сообщений. Linux runtime image был собран из текущих исходников.
- G06 остаётся RUNNING: COPY/MOVE, UID EXPUNGE и UIDPLUS, UID SEARCH,
  modified UTF-7, полный LIST/SPECIAL-USE профиль, Recent и согласование
  sequence-map двух сессий ещё не завершены.

### 2026-09-20: G06 SEARCH checkpoint

- Базовый `SEARCH` и `UID SEARCH` имеют отдельные typed варианты addressing
  и key. Поддерживаются `ALL`, `SEEN`, `UNSEEN`, `DELETED`, `UNDELETED`.
  Другие search keys не имитируются пустым ответом и получают BAD в parser-е.
- Результат строится из единого worker snapshot: обычный SEARCH возвращает
  sequence number, а UID SEARCH возвращает устойчивый UID той же записи.
  Поэтому последующие удаления соседних сообщений не меняют identity UID.
- PASS: 90 Windows tests, format, strict Clippy и structural audit. Linux
  runtime image с STARTTLS прошёл независимый Python `imaplib` сценарий в
  `./data/g06-expunge`: UNSEEN вернул sequence `2 3`, UID SEARCH SEEN вернул
  `1`, UID SEARCH DELETED вернул `3`.
- G06 остаётся RUNNING: COPY/MOVE, UID EXPUNGE и UIDPLUS, modified UTF-7,
  полный LIST/SPECIAL-USE профиль, Recent и согласование sequence-map двух
  сессий ещё не завершены.

### 2026-09-20: G06 UID EXPUNGE checkpoint

- `UID EXPUNGE <uid-set>` проходит typed parser и ограничивает durable
  tombstone только UID из множества, уже имеющие `\\Deleted`. Обычный EXPUNGE
  остаётся операцией по всем Deleted сообщениям выбранной папки.
- UIDPLUS объявляется только вместе с реально исполняемыми APPENDUID и UID
  EXPUNGE, а не как несвязанный capability token.
- PASS: 91 Windows tests, format, strict Clippy и structural audit.
- G06 остаётся RUNNING: COPY/MOVE, modified UTF-7, полный LIST/SPECIAL-USE
  профиль, Recent и согласование sequence-map двух сессий ещё не завершены.

### 2026-09-20: G06 UID EXPUNGE Linux black-box checkpoint

- Добавлен независимый STARTTLS-сценарий
  `tests/blackbox/imap_uid_expunge.py`. Он создаёт три письма, помечает UID 1
  и UID 2 как `\\Deleted`, исполняет `UID EXPUNGE 1`, затем проверяет,
  что в mailbox осталось два письма и точные байты письма UID 2 доступны через
  `UID FETCH 2 RFC822`.
- Прогон в фиксированном контейнере `litterkeep-linux-test` с bind mount
  `./data/g06-expunge` завершился PASS как до restart (`committed`), так и
  после него (`replayed`). Имена mailbox `UidExpungeFresh` и
  `UidExpungeFresh2` оставлены в том же локальном каталоге данных; named
  volumes и дополнительные runtime-каталоги не создавались.
- PASS: `python -m py_compile tests/blackbox/imap_uid_expunge.py`,
  `cargo test --all-targets --locked` (91 tests), strict Clippy, format,
  `docker build -f docker/Dockerfile.linux-test --target test -t
  litterkeep-server:linux-test .` и независимая проверка реальным Python
  `imaplib` через STARTTLS.
- G06 остаётся RUNNING: COPY/MOVE, modified UTF-7, полный LIST/SPECIAL-USE
  профиль, Recent и согласование sequence-map двух сессий ещё не завершены.

### 2026-09-20: G06 modified UTF-7 checkpoint

- Добавлен собственный `imap::modified_utf7` без внешней codec-зависимости.
  Он строго декодирует wire-имя до Unicode на parser boundary, канонически
  кодирует имена в LIST/LSUB/STATUS и отвергает некорректные shift/Base64/UTF-16
  последовательности вместо подстановки повреждённого текста.
- Новой логической папке соответствует уже существующий account-scoped Unicode
  каталог. Поэтому wire-имя `&ZeVnLIqe-` не может стать filesystem path:
  folder catalog всё ещё выдаёт только числовой internal key.
- PASS: `cargo test --all-targets --locked` (95 tests), strict Clippy,
  format, Linux `docker build -f docker/Dockerfile.linux-test --target test
  -t litterkeep-server:linux-test .` и независимый `imaplib` STARTTLS
  сценарий `tests/blackbox/imap_modified_utf7.py`. Он создал, SELECT-нул и
  получил через LIST RFC-имя `&ZeVnLIqe-`, затем подтвердил его после restart
  фиксированного контейнера c bind mount `./data/g06-expunge`.
- G06 остаётся RUNNING: COPY/MOVE, полный LIST/SPECIAL-USE профиль, Recent и
  согласование sequence-map двух сессий ещё не завершены.

### 2026-09-20: G06 durable COPY checkpoint

- Реализованы `COPY` и `UID COPY` через один owner Store. Новый journal
  frame v3 отделяет transaction ссылки от transaction immutable blob, поэтому
  одна COPY transaction может atomically опубликовать несколько destination
  links на уже зафиксированные blobs без копирования payload и без частично
  видимого sequence-set. Старые v1/v2 frames продолжают декодироваться.
- Reactor сначала получает единый source snapshot, преобразует sequence-set в
  устойчивые UID, затем resolve-ит destination account-scoped folder и только
  после durable commit отправляет `[COPYUID 1 source-uids destination-uids]`.
  Пустая выборка не получает фиктивный пустой COPYUID.
- PASS: `cargo test --all-targets --locked` (97 tests), strict Clippy,
  format. Независимый `imaplib` STARTTLS сценарий
  `tests/blackbox/imap_copy.py` проверил обычный COPY, низкоуровневый UID
  COPY с tagged COPYUID, неизменный source, copied `\\Seen` и состояние
  destination до и после restart фиксированного Linux-контейнера. Данные
  добавлены только в bind mount `./data/g06-expunge`.
- G06 остаётся RUNNING: MOVE, полный LIST/SPECIAL-USE профиль, Recent и
  согласование sequence-map двух сессий ещё не завершены.

### 2026-09-20: G06 LIST wildcard and SPECIAL-USE checkpoint

- LIST pattern больше не ограничен тремя exact значениями. Для wildcard внутри
  имени используется bounded NFA: `*` проходит delimiter, `%` не проходит
  slash, состояние не разворачивается рекурсивным backtracking.
- Реализована selection форма RFC 6154 `LIST (SPECIAL-USE) "" "*"`.
  Она выдаёт только папки с durable enum `SpecialUse`; CAPABILITY объявляет
  `SPECIAL-USE` именно потому, что эта extended form исполняется. Обычный
  LIST по-прежнему может вернуть attributes для папок, но не рекламирует
  неисполненную CREATE-SPECIAL-USE/RETURN форму.
- PASS: `cargo test --all-targets --locked` (98 tests), strict Clippy,
  format и structural audit. Независимый STARTTLS `imaplib` сценарий
  `tests/blackbox/imap_list_special_use.py` подтвердил CAPABILITY,
  `LIST "" "Invoices/%"`, RFC-строку extended LIST и отсутствие
  `Invoices/2026` в выборке только special-use папок.
- G06 остаётся RUNNING: MOVE, часть extended LIST RETURN/CREATE-SPECIAL-USE,
  Recent и согласование sequence-map двух сессий ещё не завершены.

### 2026-09-20: G06 atomic MOVE checkpoint

- `MOVE` и `UID MOVE` получают typed enum transfer на parser boundary и
  исполняются единственным владельцем Store. Хранилище сначала fsync-ит
  destination links и transaction-зависимый source tombstone, затем публикует
  одну global commit record. До этой записи restart не показывает ни link, ни
  tombstone; после неё source и destination меняются вместе.
- Unit tests подтверждают оба варианта recovery: незакоммиченный tombstone не
  скрывает source, а завершённый MOVE переживает reopen. Reactor выдаёт
  `COPYUID`, untagged EXPUNGE с согласованными sequence numbers и уменьшает
  selected EXISTS только после успешного durable completion.
- PASS: Windows `cargo test --all-targets --locked` (101 tests), strict Clippy,
  format и structural audit без Rust-файлов более 300 строк. Собран Linux image
  `litterkeep-server:linux` и Docker test target (105 tests); независимый `tests/blackbox/imap_move.py` через
  STARTTLS выполнил MOVE `1,3`, подтвердил source=1 и destination=2, затем
  повторил проверку после restart того же контейнера. Использован только bind
  mount `./data/g06-expunge`; CA для клиента: `./data/pki/ca.pem`.
- G06 остаётся RUNNING: часть extended LIST RETURN/CREATE-SPECIAL-USE, Recent
  и согласование sequence-map двух сессий ещё не завершены.

### 2026-09-20: G06 two-session EXPUNGE synchronization checkpoint

- После успешного durable EXPUNGE reactor публикует уже рассчитанные sequence
  numbers всем другим IDLE-сессиям, selected в том же mailbox. Инициатор
  получает собственный completion один раз, поэтому не возникает дублирования
  EXPUNGE или двойного уменьшения EXISTS. Не-IDLE selected session также
  обновляет свой внутренний EXISTS, не получая несвоевременный wire event.
- PASS: Windows `cargo test --all-targets --locked` (101 tests), strict Clippy,
  format и Docker Linux test target (105 tests). Независимый STARTTLS
  `tests/blackbox/imap_idle_expunge.py` открыл
  две сессии, создал три сообщения, удалил два из второй сессии и получил в
  первой IDLE-сессии последовательность `EXPUNGE 1`, `EXPUNGE 1`; DONE и
  повторный SELECT подтвердили ровно одно оставшееся сообщение. Данные остаются
  в bind mount `./data/g06-expunge`.
- G06 остаётся RUNNING: часть extended LIST RETURN/CREATE-SPECIAL-USE, Recent
  и полная синхронизация всех change classes между несколькими сессиями ещё не
  завершены.

### 2026-09-20: G06 LIST RETURN SPECIAL-USE checkpoint

- Parser принимает `LIST "" "*" RETURN (SPECIAL-USE)` как extended форму
  обычного LIST. Она не фильтрует папки, а требует special-use attributes в
  ответах выбранного набора, которые уже берутся из durable enum каталога.
- PASS: Windows 102 unit tests и strict Clippy; независимый STARTTLS клиент
  `tests/blackbox/imap_list_return_special_use.py` получил `OK` и `\Sent`.
- G06 остаётся RUNNING: CREATE-SPECIAL-USE, Recent и остальные change classes
  multi-session синхронизации ещё не завершены.

### 2026-09-20: G06 CREATE-SPECIAL-USE checkpoint

- CREATE `(USE (\\Archive))` проходит typed parser, bounded worker и durable
  folder journal; LIST возвращает сохранённый special-use attribute.
- PASS: 104 Windows tests, strict Clippy, STARTTLS Linux create/list и replay
  `Archive2` после restart с bind mount `./data/g06-expunge`.
- G06 остаётся RUNNING: Recent и полная multi-session синхронизация.

### 2026-09-20: G06 durable IMAP Recent checkpoint

- Добавлен отдельный durable журнал `journal/recent.log`: delivery links
  становятся кандидатами `\Recent` только после общего transaction commit,
  SELECT атомарно получает snapshot и фиксирует claim до wire-success. Поэтому
  `\Recent` не попадает в persistent MessageFlags, не выдаётся второму SELECT
  и не возвращается после restart. COPY, MOVE и EXPUNGE синхронизируют ту же
  projection; incomplete tail журнала обрезается без потери committed delivery.
- SELECT сообщает `RECENT`, FETCH/STORE возвращают session-local `\Recent`, а
  STATUS поддерживает typed `RECENT`. Корреляция нового worker request вынесена
  в отдельный file-level компонент, сохраняя production reactor в лимите
  300 строк без `mod.rs`.
- PASS: Windows `cargo test --all-targets --locked` (107 tests), strict
  Clippy, format, `git diff --check` и audit всех Rust-файлов (не более 300
  строк). Linux release image собран; Docker test target прошёл 111 tests.
  Независимый STARTTLS `tests/blackbox/imap_recent.py` подтвердил durable
  APPEND -> STATUS RECENT -> SELECT/FETCH `\Recent` одной сессией, а его
  `--verify-restart` подтвердил отсутствие повторной выдачи после restart.
  Данные остаются в bind mount `./data/g06-recent-4`, CA: `./data/pki/ca.pem`.
- G06 остаётся RUNNING: полная синхронизация остальных change classes между
  несколькими selected/IDLE сессиями ещё не реализована.

### 2026-09-20: G06 two-session FLAGS synchronization checkpoint

- Успешный durable `STORE` теперь публикует другим IDLE-сессиям того же
  selected mailbox untagged `FETCH (UID ... FLAGS ...)`. Инициатор не получает
  вторую копию completion. `\Recent` кодируется из собственного состояния
  получателя, поэтому session-local marker не становится общим persistent flag.
- PASS: Windows `cargo test --all-targets --locked` (107 tests), strict
  Clippy, format, `git diff --check`, structural audit. Docker Linux test
  target прошёл 111 tests. Независимый STARTTLS
  `tests/blackbox/imap_idle_flags.py` подтвердил, что watcher в IDLE получает
  `\Seen` от writer с `+FLAGS.SILENT`, после чего тот же watcher читает
  durable `\Seen` через UID FETCH. Данные: bind mount `./data/g06-flags-2`.
- G06 остаётся RUNNING: остаются согласование delivery/Recent с уже selected
  сессиями и другие multi-session change classes.

### 2026-09-20: G06 selected-session Recent delivery checkpoint

- После durable APPEND reactor сопоставляет опубликованный UID с каждым уже
  selected mailbox и добавляет его в session-local `\Recent`. Поэтому
  существующая IDLE-сессия сохраняет проверяемый `EXISTS` event и может сразу
  получить `\Recent` через FETCH без записи этого marker в MessageFlags.
- PASS: Windows `cargo test --all-targets --locked` (107 tests), format;
  Linux release image собран. Независимый STARTTLS
  `tests/blackbox/imap_idle_recent.py` подтвердил APPEND в selected mailbox,
  `EXISTS 1` в IDLE и последующий `UID FETCH 1 (FLAGS)` с `\Recent`.
  Данные: bind mount `./data/g06-idle-recent`.
- G06 остаётся RUNNING: ещё нужна полная matrix concurrent change classes и
  явное решение политики `\Recent` для вновь подключающихся selected clients.

### 2026-09-25: G17 Sieve runtime-error outcome checkpoint

- Диагностический контракт и offline-команда `sieve-errors` теперь явно
  сообщают фактически применённый результат `implicit-keep-only`. Он следует
  из атомарности delivery plan: при runtime error частичные Sieve actions не
  публикуются, письмо остаётся только по правилу implicit keep. Store regression
  проверяет durable diagnostic и fallback после restart; CLI regression
  проверяет точную TSV-строку с кодом ошибки и outcome.
- PASS: Rust 1.97 Windows `cargo test --all-targets --locked` (337 tests),
  strict Clippy, `cargo fmt --check`, `git diff --check`. Docker Linux Rust 1.97
  test target: 377 tests, strict Clippy и release build. Независимый
  `tests/blackbox/sieve_runtime_errors.py` через ManageSieve загрузил
  limit/loop scripts, отправил письма SMTP и прочитал их через IMAP. После
  остановки того же Linux сервера `sieve-errors /data bob` прочитал его bind
  mount `./data/g17-sieve-cli-live` и вывел две пары committed diagnostics:
  `redirect-limit` и `redirect-loop`, обе с `implicit-keep-only`. Сценарий
  повторён после обновления клиента для чтения bootstrap credentials. Временный
  `litterkeep-g17-sieve-cli` удалён; `litterkeep-g12-phone` сохранён.
- G17 остаётся RUNNING: RFC 5230 vacation и другие неподдержанные Sieve/MIME
  extension scenarios остаются вне текущего подтверждённого покрытия.

### 2026-09-25: G17 RFC 5230 :mime ManageSieve checkpoint

- Добавлен независимый клиент `tests/blackbox/managesieve_vacation_mime.py`.
  Он проходит STARTTLS/SASL PLAIN и передаёт MIME entity literal-ом в
  CHECKSCRIPT. Корректные 7bit, quoted-printable и Base64 приняты; malformed
  Base64, duplicate Content-Transfer-Encoding и Base64 для multipart/mixed
  отвергнуты. Это проверка собственного ManageSieve сервера, не Litterkeep.
- PASS: Python compile, Rust 1.97.1 Windows `cargo test --all-targets --locked`
  (412 library + 1 binary), strict Clippy, format и diff check. После разнесения
  Sieve Store-кода Docker Linux test target заново собран: 453 library tests,
  1 binary test, doc-tests, strict Clippy и release build. Независимый клиент
  прошёл к актуальному Linux release binary с bind mount
  `./data/g17-sieve-vacation-mime-checkscript`. Так как корневого Dockerfile
  нет, обе стадии собраны с `-f docker/Dockerfile.linux-test`. Временные
  контейнеры и теги образа удалены; `docker ps -a` оставил только ранее
  существовавший `litterkeep-g12-phone`.
- G17 остаётся RUNNING: полный RFC 5228 grammar, все Sieve extensions,
  сторонний MTA и оставшиеся MIME network cases всё ещё требуют покрытия.

### 2026-09-25: G17 RFC 2231 language metadata checkpoint

- Начальная RFC 2231 extended секция теперь сохраняет optional language tag в
  `StructuredValue` и во всех MIME tree entities. Новый `LanguageTag` проверяет
  BCP 47 syntax: normal, extension, private-use и grandfathered формы, ASCII,
  границы subtags и повтор variant/singleton, сохраняя исходный регистр.
  IANA registry membership не проверяется и не заявляется.
- Исправлен допустимый omitted charset: ASCII payload работает с пустыми
  charset/language полями как в одиночном extended parameter, так и в
  continuation. High-bit payload без charset отклоняется без guess-codec.
  Unit regressions проверяют acceptance/rejection и сохранение language в
  nested MIME entity. Network acceptance не запускался.
- В работе; G17 остаётся RUNNING. RFC 2231 tag validity по актуальной IANA
  registry и прочие SMTP/IMAP/POP3/MIME/Sieve пробелы не закрыты.

### 2026-09-25: G17 RFC 2231 encoded-word language checkpoint

- RFC 2231 §5 `charset*language` расширение encoded-word теперь разбирается в
  отдельный `DecodedWord`: charset декодирует текст, optional BCP 47 tag
  сохраняется как typed metadata. Старые `decode`, `decode_header` и
  `decode_phrase` сохраняют text-only contracts. Неверный suffix остаётся raw
  в header projection, чтобы повреждённое слово не скрывало письмо.
- PASS: Rust 1.97.1 Windows `cargo test --all-targets --locked` (419 library
  tests + 1 binary test), strict Clippy, `cargo fmt --check` и `git diff --check`.
  Linux Docker test target: 460 library tests, 1 binary test, doc-tests,
  strict Clippy и release build. Сетевой acceptance не запускался, так как
  language metadata не меняет существующие text projections.
- G17 остаётся RUNNING: IANA registry validity и остальные RFC gaps не закрыты.

### 2026-09-25: G14 BODYSTRUCTURE disposition checkpoint

- MIME tree сохраняет единственное корректное Content-Disposition в typed
  модели. Расширение BODYSTRUCTURE выдаёт RFC 2183 disposition token и
  нормализованные параметры, включая RFC 2231 filename, в разных extension
  slots leaf, multipart и message/rfc822. Network comparison BODY labels на
  том этапе не выполнялся.
- Некорректное или повторное optional поле не ломает доступность письма и не
  попадает в структуру; исходный заголовок остаётся в raw MIME headers.
- Последующая проверка wiring выявила общий encoder для `BODY` и
  `BODYSTRUCTURE`; добавленные extensions могли утечь в basic `BODY`. Профили
  разведены и отдельно проверены следующим checkpoint ниже.
- PASS: Rust 1.97.1 Windows `cargo test --all-targets --locked` (427 library
  tests + 1 binary test), strict Clippy, `cargo fmt --check`, `git diff --check`
  и аудит лимита в 300 строк. Linux Docker test target: 467 library tests,
  1 binary test, doc-tests, strict Clippy и release build. Сетевой acceptance
  не запускался. Content-Language и Content-Location остаются незакрытыми
  RFC 9051 fields.
- G14 остаётся RUNNING: полный IMAP4rev2 grammar и расширенные BODYSTRUCTURE
  language/location поля ещё не подтверждены.

### 2026-09-25: G14 BODYSTRUCTURE language and location checkpoint

- MIME entity теперь сохраняет единственное Content-Language поле как список
  структурно проверенных BCP 47 tags и единственное Content-Location как
  непустой unfolded текст. URI grammar и registry membership не проверяются;
  percent-encoded URI не преобразуется.
- BODYSTRUCTURE сериализует Content-Disposition, language list и location в
  RFC 9051 порядке с нужными NIL placeholders. Basic BODY и extended
  BODYSTRUCTURE теперь выбирают разные typed profiles; regression подтверждает,
  что MIME extensions не попадают в BODY.
- PASS: Rust 1.97.1 Windows `cargo test --all-targets --locked` (436 library
  tests + 1 binary test), strict Clippy, `cargo fmt --check` и `git diff --check`.
  Linux Docker Rust 1.97 test target: 477 library tests, 1 binary test,
  doc-tests, strict Clippy и release build. Независимый Python `imaplib`
  STARTTLS-клиент подтвердил разные wire-проекции `BODY`/`BODYSTRUCTURE`,
  language, location и RFC 2231 filename. Временный runtime/test контейнеры и
  image tags удалены; `litterkeep-g12-phone` сохранён. Content-ID и
  Content-Description basic fields и прочие RFC gaps остаются открытыми.
- G14 остаётся RUNNING: этот unit-only checkpoint не закрывает полный IMAP4rev2
  grammar или все MIME body fields.
