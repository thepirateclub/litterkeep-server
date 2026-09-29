# Goal ledger

### 2026-09-29: G12 Sieve glob byte captures foundation, PARTIAL T421

- Вынесен glob matcher в отдельную ответственность и добавлен capture API для
  `*`/`?`: byte ranges копируются только после полного match, escaped wildcard не
  захватывает данные, а matcher сохраняет shortest-first поведение ранних `*`.
  Существующий boolean `:matches` использует тот же код, поэтому не создаётся
  второй алгоритм с отличающейся истиной/ложью семантикой.
- Четыре focused tests проверяют wildcard order/minimum, UTF-8 как octets для
  поддерживаемых comparator-ов, escapes и backtracking. Windows Rust 1.97.1
  all-target: 1085 passed, 4 ignored; Linux Docker Rust 1.97.1 all-target:
  1211 passed, 4 ignored. Strict Clippy, rustfmt и diff-check прошли на обеих
  платформах. Linux использовал существующие `rust:1.97-bookworm` и
  `./data/g23-cargo-home`, `./data/g18-relayed-linux-target`; `--rm`, image не
  создавался.
- Capture helper ещё не связан с Sieve variable state. `set`, interpolation,
  modifiers и `${0}`..`${9}` не реализованы; CAPABILITY не изменена. T421 PARTIAL,
  G12 ACTIVE.

### 2026-09-29: G12 unimplemented Sieve extensions fail closed, PARTIAL T420

- Добавлена регрессия для `require` неподдержанных Variables, Body, Reject,
  IMAP4flags, IMAP events, mailboxid, Mailbox metadata и Special-use. Ни одна из
  этих capability не принимается parser/validator-ом молча. Скрипты используют
  корректный базовый `keep`, поэтому исход теста проверяет именно capability.
- Целевой Windows Rust 1.97.1 тест
  `sieve::parser_tests::selected_unimplemented_extension_requirements_fail_closed`
  прошёл (1/1), `git diff --check` прошёл. CAPABILITY и production behavior не
  расширялись; это fail-closed регрессия, не реализация расширений и не PASS.
  T420 PARTIAL, G12 ACTIVE.

### 2026-09-29: G12 modern Sieve extension inventory, PARTIAL T419

- Сверен фактический `SIEVE` capability ManageSieve с текущими источниками:
  `fileinto subaddress envelope copy vacation vacation-seconds`. Создан отдельный
  RFC inventory выбранных современных расширений, которые не рекламируются и
  не должны приниматься за реализованные: Variables, Body, Reject/Ereject,
  IMAP4flags, IMAP events, mailboxid, Mailbox metadata и Special-use delivery.
- Для каждого gap зафиксирована техническая граница следующего этапа; в
  частности `ereject` требует отказа в SMTP до принятия DATA и не может быть
  подменён поздним DSN после доставки в Store.
- Это аудит покрытия документацией, не реализация этих расширений и не PASS.
  RFC Editor подтверждает обновления RFC 5228 документами RFC 5229, 5429, 6785
  и 9042; полный IANA Sieve registry и все сочетания extensions ещё не проверены.
  T419 PARTIAL, G12 ACTIVE.

### 2026-09-29: G18 delayed DSN same-drain SMTP acceptance, PARTIAL T418

- Расширен Linux Store/outbound-service peer test: первый SMTP `421` переводит
  original в retry; после контролируемого due перехода второй `drain` с бюджетом
  2 выполняет retry, сохраняет/publishes delayed DSN и следующей bounded работой
  того же drain доставляет report независимому peer.
- Peer проверяет `MAIL FROM:<>`, original sender в RCPT, DATA completion,
  `Action: delayed`, `Status: 4.0.0`, отсутствие исходного тела и graceful QUIT.
  Drain возвращает один deferred original плюс одну delivered report job;
  после reopen остаётся original с `attempts=2`, report job уже подтверждён.
- Первоначальный T417 GAP был вызван ошибкой диагностического harness: peer
  завершал listener после двух временных отказов и не принимал третий SMTP
  session, который bounded drain правильно использовал для report. T417 помечен
  SUPERSEDED; непрошедший отдельный Worker эксперимент не используется как
  доказательство transport-дефекта.
- Windows Rust 1.97.1 all-target: 1080 library + 1 CLI passed, 4 ignored;
  strict Clippy, rustfmt и diff-check прошли. Linux Docker Rust 1.97.1
  all-target: 1206 library + 1 CLI passed, 4 ignored; strict Clippy и
  rustfmt прошли. Переиспользованы `rust:1.97-bookworm`,
  `./data/g23-cargo-home` и `./data/g18-relayed-linux-target`; контейнер `--rm`,
  образ не создавался.
- Litterkeep client interop и полный RFC 3461 профиль остаются
  непроверенными. T418 PARTIAL, G18 ACTIVE.

### 2026-09-29: G18 delayed DSN report transport diagnostic, SUPERSEDED T417

- Начальная расширенная проверка T416 завершила независимый peer после двух
  временных отказов и не приняла третий SMTP-сеанс, который bounded drain
  использует для отправки delayed report. Из-за неполного test harness попытка
  была записана как GAP; отдельный standalone Worker эксперимент также остался
  без надёжного peer exchange и не давал основания считать серверный transport
  сломанным.
- T418 добавил принимающий третий сеанс и проверил ту же последовательность
  целиком: report доставлен, SMTP поля и MIME проверены, исходная задача осталась
  Deferred. Поэтому T417 SUPERSEDED; прежний эксперимент не считается ни PASS,
  ни доказательством transport-дефекта.

### 2026-09-29: G18 RFC 3461 delayed DSN after retry, PARTIAL T416

- После второй временно неудачной SMTP попытки job, которая остаётся в
  `Deferred`, может сформировать durable `Notice::Delayed`. Политика разрешает
  уведомление при omitted NOTIFY или явном DELAY; FAILURE-only, SUCCESS-only и
  NEVER его не получают. Порог является локальной optional policy, а не
  требованием RFC. Повторная попытка остаётся запланированной; обычный terminal
  failure после исчерпания попыток создаёт отдельный failure notice.
- RFC 3464 сериализует `Action: delayed`, `Status: 4.0.0`; третья MIME часть с
  исходным письмом опущена. Notice имеет отдельный journal tag и восстанавливается
  как due work после перезапуска; idempotency key различает его от последующего
  failure notice. Linux Store/outbound-service integration выполняет два
  последовательных временных 421 ответа, затем проверяет durable null-sender
  report job и retryable исходное сообщение; payload содержит delay DSN поля и
  не содержит исходное тело. Отдельная доставка этого DSN через SMTP peer пока
  не проверена. Unit tests дополнительно проверяют policy, MIME и recovery.
- Windows Rust 1.97.1 all-target: 1080 library + 1 CLI passed, 4 ignored;
  strict Clippy, rustfmt и diff-check прошли. Linux Docker Rust 1.97.1 all-target:
  1205 library + 1 CLI passed, 4 ignored; strict Clippy и rustfmt прошли.
  Переиспользованы `rust:1.97-bookworm`, `./data/g23-cargo-home` и
  `./data/g18-relayed-linux-target`; контейнер `--rm`, образ не создавался.
- SMTP/Store процесс с двумя временными отказами и самостоятельная доставка
  созданного DSN независимому peer-у теперь проверены в T418; клиент Litterkeep
  и полный RFC 3461 lifecycle не проверены. T416 PARTIAL, G18 ACTIVE.

### 2026-09-29: G18 multi-origin relay DSN local strategy, PARTIAL T415

- При route, объединяющем несколько исходных RCPT policy, downstream SMTP
  transaction больше не откладывается. Применяется локальная стратегия RFC
  3461 §5.2.7.3: исходные ENVID/RET/NOTIFY/ORCPT не передаются следующему peer,
  даже если он объявил DSN; исходная политика остаётся durable у Store owner.
  После принятой доставки результат `Relayed` инициирует локальный RFC 3464
  report только для origins с явным `NOTIFY=SUCCESS`; failure policy каждого
  origin по-прежнему используется при отказе downstream RCPT.
- Plain OutboundWorker независимый TCP peer и implicit-TLS pool тест проверяют
  точные команды без downstream DSN параметров, принятие DATA и `Relayed`.
  Unit test отчёта подтверждает включение SUCCESS origin и исключение
  FAILURE-only origin.
- Windows Rust 1.97.1 all-target: 1077 library + 1 CLI passed, 4 ignored;
  strict Clippy и rustfmt прошли. Linux Docker Rust 1.97.1 all-target:
  1202 library + 1 CLI passed, 4 ignored; strict Clippy и rustfmt прошли.
  Переиспользованы `rust:1.97-bookworm`, `./data/g23-cargo-home` и
  `./data/g18-relayed-linux-target`; контейнер запущен с `--rm`, образ не
  создавался, тестировался независимый peer, Litterkeep client interop не
  заявляется.
- Ограничение: DSN-aware downstream-generated SUCCESS/DELAY и расширение
  нескольких исходных адресов в полный RFC 3461 alias/expanded lifecycle ещё
  не реализованы. T415 PARTIAL, G18 остаётся ACTIVE.

### 2026-09-29: G18 single-origin relay SUCCESS notice without downstream DSN, PARTIAL T414

- Исходящий SMTP transport различает принятую relay доставку (`Relayed`) от
  обычного downstream DSN-aware результата. Для peer без `DSN` при явном
  `NOTIFY=SUCCESS` сохраняемый `Notice::Relayed` приводит к RFC 3464
  `Action: relayed`, `Status: 2.0.0`; исходное тело не возвращается. Без явного
  SUCCESS report не создаётся. DSN параметры не отправляются peer без
  capability.
- Queue journal и ключ Store idempotency обобщены до typed `DsnNotice`; старые
  failure tags остаются читаемыми, новый `relayed` tag восстанавливается как
  due work после перезапуска. Linux Store acceptance проходит исходное письмо и
  report через две независимые SMTP session, проверяет MIME поля, отсутствие
  тела source и очистку durable queue. Отдельно проверены worker outcome и
  Store idempotency после reopen.
- Windows Rust 1.97.1 `cargo test --all-targets`: 1075 passed, 4 ignored;
  strict Clippy прошёл. Linux Docker Rust 1.97.1: 1199 library + 1 CLI passed,
  4 ignored; strict Clippy и `cargo fmt --check` прошли. Использован
  существующий `rust:1.97-bookworm`, target `./data/g18-relayed-linux-target`,
  CARGO_HOME `./data/g23-cargo-home`; `--rm` удалил созданный контейнер,
  образ не создавался. Клиент Litterkeep не проверялся.
- T414 закрывает только одно-origin relay SUCCESS при peer без DSN. T415
  добавляет локальную multi-origin стратегию. Delay DSN, downstream DSN-aware
  SUCCESS/DELAY processing и реальная совместимость клиента остаются GAP; G18
  остаётся PARTIAL.

### 2026-09-29: G18 downstream single-origin DSN forwarding, PARTIAL T413

- Outbound `Job` теперь переносит durable typed DSN metadata из Store через
  обычный и TLS pool. EHLO parser распознаёт `DSN` без параметров. Для одного
  source RCPT с одним forwarding destination transport отправляет сохранённые
  MAIL `RET`/`ENVID`, RCPT `NOTIFY`/`ORCPT` только при объявленной peer
  capability. RFC 3461 xtext сохраняет ASCII octets; RFC 6533 UTF-8 ORCPT
  сериализуется 7-bit-safe unitext с проверкой UTF-8 и CR/LF/NUL.
- Два независимых локальных TCP peer теста, подключённых через реальный
  OutboundWorker, проверили точные MAIL/RCPT wire-команды с завершённым DATA и
  deferred multi-origin без MAIL/RCPT. Formatter tests проверяют omission DSN
  параметров без capability, RFC 6533 unitext и отказ malformed UTF-8/CRLF.
  Grouped jobs сейчас переходят в durable Deferred после EHLO и до MAIL/RCPT,
  пока не реализована выбранная политика RFC 3461 §5.2.7.3.
- Windows Rust 1.97.1: 1070 library + 1 CLI tests pass, 4 ignored; строгий
  Clippy и rustfmt pass. Linux Docker Rust 1.97.1: 1194 library + 1 CLI pass,
  4 ignored; rustfmt и strict Clippy pass. Предыдущая flake в Store journal
  прошла отдельно и при повторном full suite.
- Использованы существующий `rust:1.97-bookworm`, `./data/g31-rust-cargo-home`
  и `./data/g36-outbound-dsn-target`; компоненты rustfmt/clippy установлены
  только в одноразовый слой контейнера. `--rm` удалил контейнер, образ не
  создавался. Изменены два существующих Clippy lint-а без suppression.
- Исторический T413 был PARTIAL до T414: одно-origin relay-result для peer без
  DSN теперь реализован. Multi-origin alias strategy, downstream SUCCESS/DELAY,
  независимая forwarding acceptance на implicit TLS и Litterkeep app
  interoperability остаются непроверенными.

### 2026-09-29: G18 RFC 3464 multi-origin failure groups, PARTIAL T412

- Добавлена unit-регрессия сериализатора: для двух исходных RCPT с явным
  `NOTIFY=FAILURE` один failure DSN содержит две отдельные recipient-группы,
  оба `Original-Recipient` и один общий `Original-Envelope-ID`.
- Точечный `cargo test --lib failure_report_keeps_every_origin_recipient_group`
  и группа `cargo test --lib dsn_report` прошли. Полный Windows Rust 1.97.1
  all-target: 1063 library + 1 CLI, 4 ignored. Полный Linux Rust 1.97.1:
  1187 library + 1 CLI, 4 ignored. Строгий Clippy, rustfmt и `git diff --check`
  прошли.
- Исправлена актуальная RFC-сводка: T385/T404 обозначены историческими
  checkpoint-ами; multi-origin report теперь описан как реализованный и
  проверенный unit/Store тестами. Независимая wire acceptance нескольких
  report-групп ещё не проводилась. Delay DSN и downstream SMTP parameter
  propagation остаются GAP; T385/G18 остаётся PARTIAL.
- Linux прогон использовал существующий `rust:1.97-bookworm`, `./data` и
  отдельный `./data/g35-dsn-groups-target`. Одноразовый контейнер с `--rm`
  удалён; новых Docker images не создавалось.

### 2026-09-29: G20 owned client-first SCRAM transcript, PARTIAL T411

- Добавлены consuming parser-ы SCRAM client-first. POP3 и ManageSieve передают
  владение уже декодированным `String`; GS2 prefix удаляется на месте, client
  nonce хранится как диапазон внутри bare transcript. Большой client-first
  больше не копируется целиком вместе с отдельной копией nonce.
- Регрессия на 2 MiB проверяет pointer identity исходной allocation, bare
  transcript и nonce. Windows Rust 1.97.1 all-target: 1062 library + 1 CLI
  passed, 4 ignored; Linux Rust 1.97.1 all-target: 1186 library + 1 CLI passed,
  4 ignored. 56 SCRAM-тестов, строгий Clippy, fmt и diff-check прошли.
- Независимый Linux POP3 TLS peer снова завершил SCRAM-SHA-256 на 2 MiB nonce,
  проверил server signature и выполнил STAT/QUIT. Пиковая память всего сетевого
  стека не измерялась; этот результат не доказывает общий верхний предел
  доступной памяти или поведение при allocator failure. Litterkeep app interop
  не заявляется.
- Использованы существующий `rust:1.97-bookworm` и `./data`; disposable listener
  автоматически удалён, Docker image не создавался.

### 2026-09-29: G20 borrowed SCRAM attributes and transcript, PARTIAL T410

- SCRAM attribute values теперь заимствуются из исходного сообщения вместо
  отдельной `String` на каждое поле. SHA-1 и SHA-256 client-final хранят срез
  unsigned transcript, а AuthMessage формируется единым заранее выделенным
  буфером. Регрессия подтверждает, что 2 MiB значение указывает прямо в исходные
  байты.
- Windows Rust 1.97.1: 1061 library + 1 CLI passed, 4 ignored; Linux Rust 1.97.1:
  1185 library + 1 CLI passed, 4 ignored. 55 SCRAM unit tests, strict Windows
  Clippy, fmt и diff-check прошли. Независимый Linux TLS peer повторно завершил
  POP3 SCRAM-SHA-256 с 2 MiB nonce и дошёл до STAT/QUIT.
- Это уменьшает временные копии, но не peak-memory proof: Base64 decode и
  сохранённый client-first transcript по-прежнему требуют память, пропорциональную
  сообщению. T377 остаётся PARTIAL; Litterkeep app interop не заявляется.
- Повторно использованы существующий `rust:1.97-bookworm` и каталоги `./data`;
  одноразовый listener удалён, образ не создавался.

### 2026-09-29: G20 POP3 SCRAM transcript beyond fixed wire caps, PARTIAL T409

- Удалены фиксированные 700 KiB/700 KiB+136 parser limits и 1 MiB ceiling
  накопителя POP3 SASL continuation. Для POP3 SASL challenge не применяется
  общий MAX_OUTPUT; input/output Vec резервируются fallibly, а ошибка input
  reserve дренирует текущую строку до CRLF и отклоняет только AUTH session.
- SCRAM unit tests принимают 2 MiB opaque nonce и client-final field.
  Независимый Python TLS peer отправляет 2 MiB nonce: transcript Base64
  превышает 1 MiB; клиент независимо считает proof, проверяет server signature
  и доходит до `STAT`/`QUIT`. Linux Rust 1.97.1 all-target: 1185 library + 1
  CLI passed, 4 ignored; Windows: 1061 library + 1 CLI passed, 4 ignored;
  strict Windows Clippy, rustfmt и diff-check прошли.
- T377 продвинут до PARTIAL, не PASS. Реальная проверка покрывает 2 MiB, но
  декодирование и атрибуты пока создают пропорциональные копии, поэтому
  исчерпание памяти и полный произвольный диапазон не проверены. Использованы
  существующий `rust:1.97-bookworm`, локальные `./data/g32-pop3-unbounded-sasl`
  и `./data/g32-pop3-unbounded-target`; disposable контейнер автоматически
  удалён, образ и постоянный контейнер не создавались.

### 2026-09-29: G18 failure DSN RET=FULL MIME return, PARTIAL T408

- Добавлена регрессия failure DSN с `RET=FULL`: сериализованный report
  содержит исходные заголовки, MIME metadata и тело в части `message/rfc822`.
  Существующий соседний тест `RET=HDRS` продолжает проверять исключение тела.
- Windows Rust 1.97.1 all-target: 1061 library + 1 CLI passed, 4 ignored;
  strict Clippy, rustfmt и diff-check прошли. Linux Rust 1.97.1 all-target:
  1185 library + 1 CLI passed, 4 ignored. Специализированная регрессия
  RET=FULL требует полный исходный byte sequence в message/rfc822 MIME part.
- Linux Store/queue acceptance также запускает независимые downstream SMTP peers:
  после permanent RCPT failure durable job доставляет внешний DSN с RET=FULL;
  получатель сверяет MIME type, оригинальные заголовки и исходное тело.
- Проверка SMTP delivery использует независимый тестовый peer и уже сохранённую
  typed envelope, но не повторяет ingress negotiation RFC параметров. RFC 3461
  §4.3 задаёт RET как выбор полного содержимого или заголовков для failure DSN;
  Litterkeep app interop не проверялся.

### 2026-09-29: G18 outbound SMTP diagnostic in external failure DSN, PARTIAL T407

- Исходящий plain SMTP и implicit TLS теперь сохраняют точный bounded SMTP
  reply code/text и типизированную Remote-MTA identity в durable delivery job.
  После terminal failure внешний DSN содержит peer enhanced status, Diagnostic-Code
  и Remote-MTA (DNS name или RFC address literal), сохраняя RET=HDRS и recipient
  correlation из предыдущего этапа.
- Независимые unit и Linux SMTP peer проверки подтверждают восстановление
  diagnostic token, DSN-поля, null reverse-path, sender recipient и отсутствие
  исходного тела. Windows Rust 1.97.1 all-target: 1060 library + 1 CLI pass,
  4 ignored; strict Clippy и rustfmt прошли. Linux Rust 1.97.1 all-target:
  1184 library + 1 CLI pass, 4 ignored; сквозной failure DSN peer test прошёл.
- Этап остаётся PARTIAL: RFC 3461 negotiation/ENVID/RET/NOTIFY/ORCPT не полностью
  связан со всеми transport сценариями, SUCCESS/DELAY и SMTPUTF8 edge cases
  не завершены; проверялся независимый тестовый peer, не Litterkeep app.
- Использован существующий `rust:1.97-bookworm`; disposable `--rm` контейнер
  удалён автоматически. Новых образов и постоянных контейнеров не создавалось.

### 2026-09-29: G18 external failure DSN durable publication, PARTIAL T406

- Универсальная типизированная цель DSN теперь поддерживает локальный mailbox
  и исходящий адрес. Failure DSN для внешнего MAIL FROM создаётся как
  идемпотентная Store-транзакция и outbound job с пустым reverse-path; пустой
  sender и отсутствующий локальный аккаунт по-прежнему подавляются.
- Добавлен Linux-only сетевой тест: первоначальный независимый SMTP peer
  отклоняет доставку, затем тест требует durable DSN job и проверяет второй
  независимый peer на null MAIL FROM, исходного отправителя, multipart/report,
  Final-Recipient, Action, Status и ограниченное включение исходных заголовков.
- Windows Rust 1.97.1 all-target: 1053 library + 1 CLI passed, 4 ignored;
  strict Clippy, rustfmt и diff-check прошли. Linux execution не подтверждён:
  существующий Rust container не имеет рабочего linker (`cc`); попытка
  одноразового container была остановлена и сам container удалён. Linux test
  source написан, но не компилировался и не запускался. Активная goal CLI и
  подключение Litterkeep для этой проверки не использовались.

### 2026-09-29: G18 duplicate RCPT local SUCCESS intent correlation, PARTIAL T402

- Добавлена Store regression: два SMTP RCPT, разрешающиеся в один локальный
  mailbox и оба запрашивающие SUCCESS, создают одну доставленную mailbox-запись,
  но две раздельные recipient-indexed DSN intents.
- Reopen Store подтверждает сохранение обоих `(transaction_id, recipient_index)`
  ключей. Windows Rust 1.97.1 all-target: 1043 passed, 4 ignored; strict Clippy,
  rustfmt и diff-check прошли. Linux suite не запускался.
- Это local-success coverage. Один outbound redirect от coalesced account пока
  хранит один source RCPT ordinal; отдельная failure DSN корреляция остаётся GAP.

### 2026-09-29: G20 POP3 SCRAM configured-size successful proof, PARTIAL T401

- Добавлен независимый TLS POP3 acceptance для успешного SCRAM-SHA-256 на
  настроенном пределе: client-first 700 KiB и соответствующий client-final
  700 KiB + 72 октета. Python самостоятельно вычисляет client proof и
  сверяет server signature, затем проверяет `STAT` и `QUIT`.
- Linux Rust 1.97.1 release build и wire acceptance прошли; Python 3.14.7
  `py_compile` и `git diff --check` прошли. Использован существующий образ
  `rust:1.97-bookworm`; временный `--rm` контейнер удалён, новых образов нет.
- Это evidence только конечного локального предела. T377 остаётся GAP: RFC 5034
  произвольный размер механизма и RFC 5802 nonce без максимума не покрыты.
  Litterkeep app acceptance не выполнялся.

### 2026-09-29: G18 SUCCESS DSN пакетная задержка повторов, PARTIAL T400

- Ошибки пакетного SUCCESS DSN sweep больше не вызывают повтор Store-запроса
  на каждом 100 ms idle poll. Внутрипроцессная задержка растёт от 100 ms до
  максимума 30 s и сбрасывается после успешной порции.
- Deterministic unit tests проверяют начальный срок, удвоение, cap и reset.
  Полный Windows Rust 1.97.1 прогон: 1042 passed, 4 ignored; Linux Docker
  Rust 1.97.1: 1163 passed, 4 ignored; strict Clippy, fmt и diff-check прошли.
- Это пакетный in-memory pacing, а не durable per-intent retries: срок повтора
  не переживает рестарт, dead-letter policy отсутствует. RFC 3461 wire-поддержка
  не заявляется, независимый Litterkeep acceptance не выполнялся.

### 2026-09-29: G18 fair bounded SUCCESS DSN backlog continuation, PARTIAL T399

- Pending recipient intents теперь индексируются упорядоченным `BTreeSet` по
  `(transaction_id, recipient_index)`. Владелец Store переносит cursor между
  bounded страницами по 100 записей и оборачивается в начало, поэтому
  необрабатываемая ранняя запись не заслоняет остальные в текущем процессе.
  После committed report intent удаляется из pending-индекса, а recovery сразу
  фильтрует подтверждённые ключи; поэтому scan bound ограничивает и посещённые
  элементы, не только число найденных pending-результатов.
- Worker делает следующий ограниченный проход после 100 ms ожидания канала,
  поэтому backlog продолжается без новых клиентских запросов. Linux reactor
  получает coalesced wake после Store-команды или если idle sweep создал отчёты.
- Regression проверяет стабильные cursor pages и wrap-around после Store reopen.
  Runtime acceptance формирует 101 SUCCESS intent, оставляет worker idle и
  подтверждает публикацию всех отчётов и очистку intents после единственной
  последующей проверки. Это тест StorageWorker, не Litterkeep-клиент.
- Windows Rust 1.97.1 all-target: 1039 passed, 4 ignored; Linux Docker Rust
  1.97.1 all-target: 1160 passed, 4 ignored. Strict Windows Clippy, rustfmt,
  `git diff --check` и Linux suite прошли. Docker использовал существующий
  `rust:1.97-bookworm` и `./data/g24-dsn-success-order-linux-target`, контейнер
  `--rm`, образ не создавался.
- Ошибки отдельного intent пока повторяются с cadence worker-а без per-intent
  exponential backoff; pacing/DLQ policy остаются GAP. G18 остаётся ACTIVE,
  SMTP DSN wire capability выключена.
- Источник: [RFC 3461 §5.2.3](https://www.rfc-editor.org/rfc/rfc3461.html).

### 2026-09-29: G18 outbound SUCCESS DSN to external reverse-path, PARTIAL T398

- Положительное уведомление теперь получает typed target: локальный mailbox
  либо внешний reverse-path. Для внешнего адреса Store атомарно фиксирует
  очередь сообщения с пустым SMTP reverse-path и idempotency key отчёта;
  обычный outbound SMTP service доставляет его только в явно настроенный relay.
- Linux независимый SMTP peer проверяет `MAIL FROM:<>`, внешний RCPT, `Action:
  delivered`, оригинальные заголовки без исходного тела и очистку очереди после
  успешной передачи. Этот peer не является публичным MX и не доказывает DNS/MX,
  интернет-доставку или совместимость с приложением Litterkeep.
- Sweep выполняется также в Linux reactor-worker после каждой bounded-команды,
  а не только в отдельном StorageWorker loop. Это исправлено после того, как
  end-to-end тест обнаружил непубликуемый pending intent в runtime-варианте.
- Расширен typed outbound redirect recipient grammar для внешних доменов;
  server-owned локальные домены остаются запрещены как relay targets. Прямой
  SMTP RCPT путь не расширен и по-прежнему ограничен локальной маршрутизацией.
- Windows Rust 1.97.1 all-target: 1037 passed, 4 ignored; Linux Docker Rust
  1.97.1 all-target: 1158 passed, 4 ignored. Strict Clippy, rustfmt и
  `git diff --check` прошли. Переиспользован образ
  `sha256:0e2bcaef56d041a486784e54104a81aebe0da44bd03019bd70bc0401e42e4a97`,
  cache `./data/g23-dsn-external-success-linux-target`; контейнер `--rm`, образ
  не создавался.
- Fair continuation/retry pacing при backlog не проверены. EHLO DSN и MAIL/RCPT
  DSN параметры остаются выключены; G18 ACTIVE, T391 GAP.
- Источник: [RFC 3461 §5.2.3](https://www.rfc-editor.org/rfc/rfc3461.html).

### 2026-09-29: G18 automatic local SUCCESS DSN recovery sweep, PARTIAL T397

- Store owner now scans at most 100 pending positive notices when it starts and
  after each bounded storage request. Local sender accounts receive reports
  automatically after recovery without a separate protocol/client command.
- A failed sweep is written to stderr and leaves its intent durable for retry;
  source SMTP delivery success is not retroactively changed by a report error.
  A StorageWorker acceptance checks that startup processing commits one report,
  removes the intent, and a later explicit retry does not duplicate it.
- The scan is bounded per pass. Reports for external reverse-path senders are
  not yet sent through outbound SMTP and remain pending; retry pacing and fair
  continuation across a large external backlog need follow-up. SMTP DSN
  negotiation remains disabled, so G18 is still ACTIVE and T391 remains GAP.
- Windows Rust 1.97.1 all-target suite: 1035 passed, 4 ignored; Linux Docker
  Rust 1.97.1 all-target suite: 1155 passed, 4 ignored. Strict all-target
  Clippy, rustfmt, and diff checks passed. Docker reused
  `sha256:0e2bcaef56d041a486784e54104a81aebe0da44bd03019bd70bc0401e42e4a97`
  with `./data/g23-dsn-success-report-linux-verify-target`; the container was
  removed by `--rm`, and no image was created.
- Источник: [RFC 3461 §5.2.3](https://www.rfc-editor.org/rfc/rfc3461.html).

### 2026-09-29: G18 local delivered DSN consumer, PARTIAL T396

- The bounded StorageWorker now accepts a typed command to consume a committed
  local SUCCESS intent. It reloads the source reverse-path, per-recipient DSN
  policy and immutable source bytes through the Store owner, verifies that the
  sender maps to an existing local account, serializes `Action: delivered`,
  and appends the report into that sender mailbox.
- A checksummed sidecar ties the success-intent key to the report transaction.
  The report transaction is the durable acknowledgement: recovery exposes the
  intent until the report commit exists, then deduplicates retries after reopen.
  A failpoint vector verifies recovery between sidecar write and global commit.
- At this checkpoint the consumer handled only local sender mailboxes and
  required an explicit worker request. T397 and T398 subsequently add automatic
  sweep and external relay delivery. SMTP DSN negotiation is still disabled.
- Windows Rust 1.97.1 all-target suite: 1035 passed, 4 ignored; Linux Docker
  Rust 1.97.1 all-target suite: 1155 passed, 4 ignored; strict Clippy, format,
  and diff checks passed. The Linux run reused image
  `sha256:0e2bcaef56d041a486784e54104a81aebe0da44bd03019bd70bc0401e42e4a97`
  and cache `./data/g23-dsn-success-report-linux-verify-target` with Cargo
  incremental compilation disabled; `--rm` removed the container and no image
  was built. Reusing the older target cache once failed on a missing incremental
  object in the bind mount, so the final Linux run used this separate cache.
- T391 remains GAP and G18 remains ACTIVE. No SMTP peer or Litterkeep client
  was used to claim RFC 3461 wire conformance.
- Источник: [RFC 3461 §5.2.3](https://www.rfc-editor.org/rfc/rfc3461.html),
  [RFC 3464 §§2.2-2.3](https://www.rfc-editor.org/rfc/rfc3464.html).

### 2026-09-28: G18 commit-linked local SUCCESS DSN intent, PARTIAL T395

- Sieve delivery now identifies each source RCPT with explicit `NOTIFY=SUCCESS`
  when its plan contains a local mailbox target. Pure discard and redirect-only
  plans produce no local success intent; repeated RCPTs retain independent
  ordinals and policy.
- The per-RCPT intent is fsynced before the source global commit, then exposed
  in Store memory only after commit. Recovery filters intents by source commit
  ID, so a crash before commit cannot publish a success notice. The journal is
  checksummed, trims only incomplete trailing frames, and is capped at 16 MiB.
- Five Store integration vectors cover durable reopen, duplicate RCPT policy,
  FAILURE-only suppression, discard suppression, and uncommitted recovery.
- Windows Rust 1.97.1 all-target suite: 1031 library + 1 CLI passed, 4 ignored.
  Linux Docker Rust 1.97.1 all-target suite: 1151 library + 1 CLI passed,
  4 ignored. Strict all-target Clippy, rustfmt and `git diff --check` passed.
- Docker reused image ID
  `sha256:0e2bcaef56d041a486784e54104a81aebe0da44bd03019bd70bc0401e42e4a97`;
  target cache is `./data/g22-dsn-success-intent-linux-target`. The disposable
  container was removed with `--rm`, no image was created, and no test container
  remains.
- This is only durable intent discovery; no consumer generates/appends the
  report or acknowledges the intent yet. DSN wire capability remains disabled,
  so T391 stays GAP and G18 ACTIVE.
- Источник: [RFC 3461 §5.2.3](https://www.rfc-editor.org/rfc/rfc3461.html).

### 2026-09-28: G18 suppress unsolicited delivered DSN, PARTIAL T394

- Публичный success renderer теперь проверяет recipient-scoped policy и
  сериализует отчёт только при явном `NOTIFY=SUCCESS`. Отсутствующий NOTIFY,
  только FAILURE и NEVER возвращают отсутствие отчёта; это предотвращает
  unsolicited positive DSN до появления durable delivery worker-а.
- Targeted vectors прошли. Windows Rust 1.97.1 all-target suite: 1026 library
  + 1 CLI passed, 4 ignored; strict all-target Clippy, fmt и diff-check прошли.
  Linux Docker all-target suite: 1146 library + 1 CLI passed, 4 ignored.
- Для Linux переиспользован существующий `rust:1.97-bookworm`, Cargo target
  сохранён в `./data/g21-dsn-delivered-linux-target`; одноразовый контейнер
  удалён через `--rm`, image не создавался. Независимый SMTP acceptance не
  выполнялся.
- Это policy guard вокруг сериализатора, а не durable outbox: local-delivery
  commit ещё не создаёт и не восстанавливает success notice. T391 остаётся GAP,
  G18 ACTIVE.
- Источник: [RFC 3461 §5.2.3](https://www.rfc-editor.org/rfc/rfc3461.html).

### 2026-09-28: G18 RFC 3464 delivered DSN serialization, PARTIAL T393

- DSN renderer получил отдельный typed `Delivered` outcome: он формирует
  `Action: delivered`, `Status: 2.0.0` и Success summary, не используя failure
  enum/status. Для positive report исходный MIME-part всегда содержит только
  заголовки, включая при `RET=FULL`; UTF-8 media type выбирается по этим
  возвращаемым октетам. Пустой reverse-path подавляет генерацию.
- Unit-векторы проверяют поля report, отсутствие тела при обычном policy и
  `RET=FULL`, и подавление null reverse-path. Отдельный T394 проверяет, что без
  явного SUCCESS отчёт не сериализуется. Первый вектор обнаружил и помог
  исправить несоответствие MIME media type при позитивном DSN.
- Windows Rust 1.97.1 all-target suite: 1025 library + 1 CLI passed, 4 ignored;
  strict all-target Clippy, rustfmt и `git diff --check` прошли. Linux Docker
  Rust 1.97.1 all-target suite: 1145 library + 1 CLI passed, 4 ignored.
- Docker переиспользовал существующий `rust:1.97-bookworm`; test target находится
  в `./data/g21-dsn-delivered-linux-target`. Контейнер удалён через `--rm`, новых
  images не создавалось. Независимый SMTP acceptance и durable crash/recovery
  acceptance не запускались.
- Это только сериализация. Local-delivery commit ещё не создаёт durable success
  notice; report не ставится в очередь и DSN wire capability выключена. T391
  остаётся GAP, G18 ACTIVE.
- Источники: [RFC 3461 §§5.2.3, 6.2](https://www.rfc-editor.org/rfc/rfc3461.html),
  [RFC 3464 §2.3.3](https://www.rfc-editor.org/rfc/rfc3464.html#section-2.3.3).

### 2026-09-28: G18 RFC 3461 outcome eligibility policy, PARTIAL T392

- Выбор recipient-scoped NOTIFY вынесен в общую typed policy для FAILURE,
  SUCCESS и DELAY. Без NOTIFY FAILURE разрешён, SUCCESS не запрошен, DELAY
  допускается как необязательное уведомление; явный список допускает только
  перечисленные outcomes, а NEVER запрещает все.
- Три unit-вектора проверяют default, явный набор и NEVER. Windows Rust 1.97.1
  all-target suite: 1023 library + 1 CLI passed, 4 ignored; прежняя точечная
  проверка failure policy и `cargo clippy --all-targets -- -D warnings` прошли.
- Linux Rust 1.97.1 Docker all-target suite: 1143 library + 1 CLI passed,
  4 ignored. Использован существующий `rust:1.97-bookworm` image и локальный
  `./data/g20-dsn-policy-linux-target`; одноразовый контейнер удалён через
  `--rm`, новых images не создавалось.
- Это только policy helper, не генерация delivered/delayed report и не SMTP
  DSN wire support. EHLO DSN и параметры MAIL/RCPT остаются выключены; T391
  остаётся GAP, G18 ACTIVE. Независимый SMTP wire acceptance для этого шага
  не запускался.
- Источник: [RFC 3461 §§5.2.3, 5.2.5-5.2.6](https://www.rfc-editor.org/rfc/rfc3461.html).

### 2026-09-28: G18 local-delivery SUCCESS DSN conformance audit, GAP T391

- Сверка RFC 3461 §5.2.3 и §6 с `src/reactor/smtp_data.rs`,
  `src/storage_worker/local_delivery.rs`, `src/store/sieve_delivery.rs` и
  `src/store/append_transaction.rs` показывает: Store теперь фиксирует
  commit-linked success intent для явно запрошенного RCPT с локальным mailbox
  target, но ещё не сериализует/публикует report из этого intent.
- RFC определяет успешную доставку в mailbox как момент доступности письма для
  retrieval и требует `delivered` DSN, когда SUCCESS был запрошен. У нас эта
  wire-функция сейчас безопасно недоступна: `EHLO DSN` не объявляется, а
  параметризованные MAIL/RCPT отвергаются. Поэтому T391 фиксирует отсутствие
  conformance при включённом профиле, а не дефект уже рекламируемой функции.
- Durable outbound queue notice и report idempotency key всё ещё типизированы
  только для failure. Serializer и commit-linked source intent умеют отдельный
  delivered outcome/SUCCESS, но ещё отсутствуют recovery consumer, идемпотентный
  report append и durable acknowledgement pending intent.
- Acceptance для закрытия GAP: unit vectors для RFC 3461 SUCCESS/NEVER/default;
  crash recovery между local mailbox commit и report append/ack; независимый
  SMTP peer после последующего включения DSN wire должен получить ровно один
  RFC 3464 `Action: delivered` report при RET=HDRS и ни одного при NEVER.
  Независимый runtime сценарий пока не выполнен.
- Источники: [RFC 3461 §§5.2.3, 6.2-6.3](https://www.rfc-editor.org/rfc/rfc3461.html),
  [RFC 3464 §§2.2-2.3](https://www.rfc-editor.org/rfc/rfc3464.html).

### 2026-09-28: G18 durable failure DSN outbox and idempotent Store append, PARTIAL T390

- Queue terminal transition теперь атомарно записывает pending failure-notice tag
  в тот же fsync-журнал. Recovery выбирает такой terminal job для обработки до
  новых SMTP попыток; после успешного/подавленного report queue transition
  очищает notice как acknowledgement.
- Store связывает DSN report idempotency key с глобальной transaction через
  checksummed sidecar, фильтруемый по commit journal. Повтор после сбоя между
  report commit и queue acknowledgement возвращает прежний transaction ID;
  незакоммиченный mapping не скрывает retry. Есть проверки checksum/conflict,
  recovery оборванного tail и failpoint перед commit.
- Все три пути исходящей очереди используют intent, idempotent append и ack:
  reactor scheduler, bounded event-driven service и synchronous bounded drain.
  При восстановлении terminal notice не увеличивает attempts и не запускает
  повторную SMTP доставку.
- Windows Rust 1.97.1: `cargo check --all-targets`, полный suite (1020 passed,
  4 ignored) и `cargo clippy --all-targets -- -D warnings` прошли. Linux Docker,
  существующий image `rust:1.97-bookworm` ID
  `sha256:0e2bcaef56d041a486784e54104a81aebe0da44bd03019bd70bc0401e42e4a97`:
  all-target suite 1140 passed, 4 ignored. Первый Linux запуск поймал moved-value
  defect при bounded submit; после `SubmitError::Full(Box<Request>)` повторный
  запуск прошёл. Контейнер использовал `--rm`; после завершения контейнера нет,
  image не создавался. Build/cache каталоги находятся в `./data/g19-dsn-outbox-*`.
- Не выполнялись независимый runtime crash-kill acceptance и interoperability
  с приложением Litterkeep. Это recovery-unit/failpoint evidence, не полная
  RFC 3461 wire поддержка: EHLO `DSN`, MAIL/RCPT параметры остаются выключены;
  success/delay reports и RCPT aggregation остаются GAP. G18 ACTIVE.
- Источники: [RFC 3461 §§4.3, 5.2.6-5.2.7](https://www.rfc-editor.org/rfc/rfc3461.html),
  [RFC 3464](https://www.rfc-editor.org/rfc/rfc3464.html).

### 2026-09-28: G18 failure DSN crash-window audit, now PARTIAL T389

- Исходный статический аудит зафиксировал разрыв между terminal queue state и
  публикацией `AppendDsn`. Последующий commit `a609b6f` связал failure notice с
  durable queue transition: queue recovery снова выдаёт terminal delivery с
  неподтверждённым notice, Store append идемпотентен по ключу отчёта, а
  scheduler записывает acknowledgement только после публикации.
- Повторно пройдены `queue_tests::terminal_failure_notice_is_recovered_as_due_work`
  и `store::failure_matrix_tests::append_failure_matrix_has_atomic_replay_boundary`.
  Первый закрывает/reopen-ит journal и проверяет восстановление и ack. Это не
  OS process-kill acceptance. T389 теперь PARTIAL: RFC 3461 SMTP wire negotiation
  остаётся выключенной; success/delay policy и внешняя interop не покрыты.
- Источники: [RFC 3461 §§5.2.6-5.2.7](https://www.rfc-editor.org/rfc/rfc3461.html).

### 2026-09-28: G18 RFC 3461 failure DSN fields and RET=HDRS, PARTIAL T388

- Scheduler, event-driven service и bounded queue drain передают сохранённые
  recipient-scoped MAIL/RCPT параметры в Store owner. Генератор отчёта включает
  декодированный `Original-Envelope-ID`, типизированный `Original-Recipient`,
  фактический `Final-Recipient` и корректное разделение DSN-блоков.
- `RET=HDRS` возвращает только заголовки до первого `CRLF CRLF`; используются
  `text/rfc822-headers` или RFC 6533 `message/global-headers` по charset.
  UTF-8 ORCPT и адрес получателя выбирают `message/global-delivery-status`,
  ASCII-адреса сохраняют `message/delivery-status`. MIME boundary выбирается
  по реально возвращаемым байтам; unit regressions проверяют эти варианты.
- Windows Rust 1.97.1 all-target: 1014 library + 1 CLI прошли; 4 library
  теста и 2 fixture-зависимых POP3 integration-теста ignored. Строгий Clippy,
  fmt и diff-check прошли. Linux `rust:1.97-bookworm` / Rust 1.97.1 all-target:
  1134 library + 1 CLI прошли; те же 4 + 2 теста ignored. Linux Clippy не
  запускался: image не содержит `cargo-clippy`; image не менялся.
- Docker test container был запущен с `--rm` и отсутствует после тестов.
  Повторное использование bind-mounted incremental target теряло `.o`, поэтому
  итоговый полный Linux suite прошёл на изолированном
  `./data/g18-rfc3461-report-final-linux-target`; Cargo cache лежит в
  `./data/g18-rfc3461-report-cargo`. Новых image не создавалось.
- Это внутренний failure-report checkpoint, не RFC 3461 wire acceptance:
  SMTP `DSN` не рекламируется и параметры MAIL/RCPT по-прежнему отвергаются.
  Успешные/задержанные отчёты, duplicate-RCPT aggregation, полная проверка
  ENVID/ORCPT на независимом wire-клиенте и interoperability Litterkeep
  остаются неподтверждёнными. G18 остаётся ACTIVE.
- Источники: [RFC 3461 §§4.3, 6.3](https://www.rfc-editor.org/rfc/rfc3461.html),
  [RFC 3464 §§2.2-2.3](https://www.rfc-editor.org/rfc/rfc3464.html),
  [RFC 6533 §§3-4](https://www.rfc-editor.org/rfc/rfc6533.html).

### 2026-09-28: G18 RFC 3461 terminal failure NOTIFY policy, PARTIAL T387

- Store owner связывает committed outbound job с typed MAIL/RCPT DSN metadata
  через transaction ID и сохранённый ordinal; повреждённый ordinal даёт
  `InvalidData`, а не fallback на чужую RCPT policy.
- Outbound scheduler теперь применяет RFC 3461 §5.2.6 к failure DSN: без
  NOTIFY действует default failure, явный FAILURE разрешает report, SUCCESS,
  DELAY и NEVER подавляют failure report. Политика хранится в компактном
  boxed metadata, не раздувающем каждое storage Request.
- Проверены policy unit cases и Store reopen round-trip. Windows Rust 1.97.1:
  1010 library + 1 CLI прошли, 4 ignored; strict Clippy/fmt/diff прошли. Linux
  rust:1.97-bookworm Rust 1.97.1: 1130 library + 1 CLI прошли, 4 ignored.
  Повтор на прежнем bind-mounted incremental target встретил отсутствующий `.o`;
  полный запуск на отдельном `./data/g18-rfc3461-notify-linux-target` прошёл.
- Linux strict Clippy не выполнен: image не содержит `rustup`/`cargo-clippy`;
  image не изменялся. `--rm` контейнер удалён автоматически. Это не wire
  acceptance: EHLO `DSN` и параметризованные MAIL/RCPT остаются выключены;
  SUCCESS/DELAY reports, RET, ORCPT, ENVID и объединение duplicate RCPT ещё GAP.
- Источник: [RFC 3461 §5.2.6](https://www.rfc-editor.org/rfc/rfc3461.html).

### 2026-09-28: G18 RFC 3461 durable redirect recipient ordinal, PARTIAL T386

- Outbound queue frame теперь сохраняет ordinal SMTP RCPT, чей Sieve plan
  породил redirect. Queue recovery и transition сохраняют его; queue key
  различает одинаковые конечные адреса, если исходные ordinal различны.
- Совместимый reader продолжает принимать все предыдущие owned queue frame
  форматы, у которых ordinal отсутствует.
- Regression проверяет два разных локальных mailbox RCPT, переадресованных на
  одинаковый внешний адрес, после закрытия и повторного открытия Store.
- Windows Rust 1.97.1: full all-target suite 1006 library + 1 CLI, 4 ignored;
  strict Clippy и fmt прошли. Linux Rust 1.97.1 контейнер: 1126 library + 1 CLI,
  4 ignored прошли. Linux strict Clippy не выполнен: в имеющемся image отсутствуют
  `rustup` и `cargo-clippy`; образ не изменялся. Новые временные контейнеры
  использовали `--rm`, отдельные target/cache сохранены в `./data`.
- Это recipient-link groundwork, не DSN scheduling или RFC 3461 PASS. Несколько
  RCPT, схлопываемых в один локальный mailbox, ещё не агрегируются; scheduler
  не применяет сохранённый ordinal. EHLO `DSN` остаётся выключенным.

### 2026-09-28: G18 RFC 3461 queue recipient correlation audit, GAP T385

- Повторно проверены текущие `Store`, `OwnedDelivery` и `deliver_sieve_many`.
  DSN sidecar индексируется исходной transaction ID, а outbound queue journal
  сохраняет только конечные sender/recipient и состояние попытки.
- Sieve redirect создаёт job для нового адреса, не сохраняя связь с исходным
  RCPT DSN metadata. Без recipient-scoped корреляции нельзя безопасно
  применять NOTIFY, ORCPT, RET или ENVID к generated failure report.
- EHLO `DSN` остаётся выключенным. Это инвентаризация подтверждённого GAP,
  не реализация RFC 3461 и не PASS.
- Источник: RFC 3461 §§5.2.7 и 6; локально сверены
  `src/store/sieve_delivery.rs`, `src/store/append.rs`, `src/queue.rs`,
  `src/queue_owned.rs` и `src/store/outbound_access.rs`.

### 2026-09-29: G18 durable multi-origin queue recovery, T404 PARTIAL

- Sieve delivery groups source RCPT indices by resolved local account. A single
  redirect route for duplicate recipients now journals its full ordered origin
  set rather than only the first index. Owned queue recovery and retry
  transitions retain the list; report idempotency keys serialize the same set.
- Existing single-index queue and report-journal tokens still recover as a
  one-element set. Writers reject duplicate/unordered origins and enforce the
  SMTP envelope's current 100-recipient bound.
- Focused Windows tests pass for queue recovery, legacy journal compatibility,
  retry transition recovery, Store reopen for coalesced same-account redirects,
  and multi-ordinal DSN report-journal identity.
- Full Windows Rust 1.97.1 gate passes with 1050 library + 1 CLI tests
  (4 ignored), strict Clippy and rustfmt. Linux Rust 1.97.1 gate passes with
  1171 library + 1 CLI tests (4 ignored), strict Clippy and rustfmt using the
  existing `rust:1.97-bookworm` image. Its disposable test container was removed;
  no image was created.
- This is not complete T385: outbound relay policy still uses the first
  recipient's DSN metadata and generated failure DSN still contains one
  per-recipient block. EHLO DSN is not advertised, MAIL/RCPT DSN parameters are
  refused, and no new independent wire or Litterkeep app test was run.
- Normative basis: RFC 3461 §§5.2.7-5.2.8 and RFC 3464 §§1.2, 2.3.

### 2026-09-29: G18 RFC 3461 multi-origin failure policy, T385 PARTIAL

- Store now resolves every durable source RCPT ordinal to its typed DSN
  metadata. A coalesced Sieve redirect preserves the first recipient policy
  only for the downstream SMTP RCPT command, while failure policy and report
  serialization retain the full origin set.
- Failure publication is allowed when at least one source RCPT requests
  FAILURE (or uses the RFC default). RFC 3464 output contains one recipient
  field group per eligible origin and omits NOTIFY=NEVER and explicit policies
  that do not include FAILURE. MAIL ENVID and RET remain common message fields.
- Regression checks cover mixed NEVER/FAILURE aliases, recipient-group count,
  ORCPT filtering, and aggregate failure selection. No external relay policy
  or independent SMTP/IMAP wire test was exercised in this checkpoint.
- Windows Rust 1.97.1: `cargo test` passed (1052 library + 1 CLI, 4 ignored),
  strict Clippy and rustfmt passed. Linux Rust 1.97.1 in the existing image:
  `cargo test` passed (1173 library + 1 CLI, 4 ignored), strict Clippy and
  rustfmt passed. The initial Linux test shared a target directory with an
  already-running user container and printed incremental-copy warnings; a
  repeat on a distinct `./data/g27-sieve-case-insensitive-dsn/target` passed
  cleanly. Both disposable containers were removed; no image was created.
- T385 remains PARTIAL: EHLO DSN is not advertised, DSN MAIL/RCPT parameters
  remain refused, and downstream outbound policy still sends one RCPT command
  for a coalesced route. No Litterkeep phone acceptance is claimed.
- Follow-up Store regression reopens a coalesced two-origin job with distinct
  `NOTIFY=FAILURE` and `NOTIFY=NEVER` plus distinct ORCPT values, then exercises
  the real recovered metadata through failure policy and report rendering.
  Windows gate: 1053 library + 1 CLI tests, 4 ignored; Linux gate: 1174 library
  + 1 CLI, 4 ignored. Strict Clippy and rustfmt passed in both environments.
- The extra disposable Linux container used the separate
  `./data/g27-sieve-case-insensitive-dsn/target`; it was removed. The active
  user container was left running and no Docker image was created.

### 2026-09-29: G18 SMTP DSN ESMTP negotiation enabled, PARTIAL T385

- EHLO now advertises the parameter-free RFC 3461 `DSN` capability in clear,
  STARTTLS-capable, and TLS-established profiles. HELO remains basic SMTP and
  DSN parameters are rejected there. The ESMTP gate no longer rejects typed
  `MAIL RET/ENVID` and `RCPT NOTIFY/ORCPT` parameters.
- Linux reactor tests verify capability output, ESMTP acceptance, persistence
  of decoded ENVID/RET/NOTIFY/ORCPT in the accepted envelope, and continued
  rejection after HELO. The earlier Store recovery and DSN report tests cover
  sidecar durability and report generation; this step did not run an
  independent TCP client against a running server.
- Windows Rust 1.97.1 full gate: 1053 library + 1 CLI passed, 4 ignored,
  strict Clippy and rustfmt passed. Linux Rust 1.97.1 full gate: 1176 library
  + 1 CLI passed, 4 ignored, strict Clippy and rustfmt passed. Linux artifacts
  used `./data/g18-smtp-dsn-wire-clean/target`, with incremental compilation
  disabled to avoid the user's concurrent Cargo workload. The disposable
  container was removed; the existing image and user container were untouched.
- T385 remains PARTIAL: wire behavior has only reactor unit coverage, not an
  independent network acceptance. Delivery of delay DSNs and external
  downstream parameter propagation are not claimed. G18 remains ACTIVE.
- RFC basis: [RFC 3461 §§5.2.7.3, 5.2.8](https://www.rfc-editor.org/rfc/rfc3461.html#section-5.2.8)
  permits shared DSN reporting for recipient aliases while applying each
  recipient's notification entitlement; [RFC 3464 §2.3](https://www.rfc-editor.org/rfc/rfc3464.html#section-2.3)
  defines recipient-scoped field groups.

### 2026-09-29: G18 independent SMTP DSN negotiation acceptance, PARTIAL T405

- Built the Linux release binary with Rust 1.97.1, initialized a dedicated
  `./data/g18-smtp-dsn-blackbox/runtime`, and ran the server in a disposable
  container on loopback. Two independent Python raw-socket peers verified
  EHLO's `250-DSN`, accepted MAIL `RET/ENVID` and RCPT `NOTIFY/ORCPT`, completed
  DATA, and verified that the same parameters remain rejected after HELO.
- Commands passed: `python tests/blackbox/smtp_dsn_negotiation.py 127.0.0.1 2525`
  and `python tests/blackbox/smtp_dsn_refusal.py 127.0.0.1 2525`. The server
  container was stopped and removed; `rust:1.97-bookworm` was reused, not
  created. Build and Store artifacts remain under `./data`.
- This verifies plain ESMTP negotiation and transaction acceptance only.
  It does not yet verify emitted failure/success/delay DSNs on the network,
  parameter propagation to an external relay, or Litterkeep app behavior.

### 2026-09-28: G18 RFC 3464 Final-Recipient mailbox syntax, T382 PASS

- Исправлен generated failure DSN: `Final-Recipient` теперь содержит
  `rfc822; mailbox` без угловых скобок. Unit regression проверяет точное поле
  и отсутствие прежней SMTP path формы.
- Независимый Linux acceptance: Bob redirect настроен ManageSieve STARTTLS;
  Alice отправляет письмо через SMTP. Файловая очередь доставляет job к
  независимому SMTP peer, который подтверждает последовательность EHLO, MAIL,
  RCPT, `550 5.1.1`, RSET и QUIT. `queue-drain` сообщает
  `permanent_failures=1`. Стандартный независимый IMAP STARTTLS клиент читает
  DSN из Alice INBOX и проверяет MIME report type, `Action: failed` и точный
  `Final-Recipient: rfc822; remote@peer.test`.
- Linux Rust 1.97.1: all-target suite (1115 library + 1 CLI, 4 ignored), strict
  Clippy и release build прошли. Windows Rust 1.97.1: all-target suite (999
  library + 1 CLI, 4 ignored), strict Clippy и fmt прошли. Python syntax и diff
  check прошли. Переиспользованы `rust:1.97-bookworm`, `python:3.14-slim` и
  `./data`; временные контейнеры удалены, новые образы не создавались.
  Независимые тестовые клиенты, не приложение Litterkeep. RFC 3461 DSN
  capability остаётся неподдержанным.
- Источник: [RFC 3464 §2.3.2](https://www.rfc-editor.org/rfc/rfc3464.html#section-2.3.2).

### 2026-09-28: G18 RFC 3502 zero-length MULTIAPPEND rollback, T381 PASS

- RFC 3502 §3.1 требует ответить tagged `NO` на нулевой message literal и
  отменить весь APPEND. Добавлена Store regression: batch с допустимым первым
  сообщением и пустым вторым отклоняется; после закрытия и повторного открытия
  ни один элемент пакета не виден.
- Независимый STARTTLS клиент передаёт тот же пакет по wire: первый literal
  получает continuation, второй имеет размер `{0}`, команда завершается `NO`,
  последующий SELECT показывает ноль сообщений. Встроенный прогон также
  повторно проверяет LITERAL+, SEARCH, успешный MULTIAPPEND и точные байты.
- Linux Rust 1.97.1: fmt, all-target suite (1115 library + 1 CLI, 4 ignored),
  strict Clippy и release build прошли; Windows Rust 1.97.1: fmt, all-target
  suite (999 library + 1 CLI, 4 ignored) и strict Clippy прошли. Python syntax
  прошёл. Использованы существующие `rust:1.97-bookworm` и
  `./data`; созданный listener остановлен и автоматически удалён, новый image
  не создавался. Проверен независимый клиент, не приложение Litterkeep.
- Источник: [RFC 3502 §3.1](https://www.rfc-editor.org/rfc/rfc3502.html).

### 2026-09-28: G18 RFC 5322 military-zone grammar vectors, T380 PASS

- Расширен unit regression для obsolete military timezone: проверяются все
  однобуквенные зоны A-I и K-Z в верхнем и нижнем регистре; J остаётся
  отвергнутой. Каждый разрешённый вариант нормализуется к unknown offset и
  сохраняет ожидаемый UTC-день.
- Целевая Rust 1.97.1 unit проверка прошла; strict Clippy и форматирование
  проходят. Это unit evidence общего RFC 5322 Date parser для IMAP SEARCH и MIME
  date projection, не независимый IMAP wire acceptance и не полное покрытие
  obsolete date-time grammar.
- Источник: [RFC 5322 §4.3](https://www.rfc-editor.org/rfc/rfc5322.html#section-4.3).

### 2026-09-28: G19 POP3 near-transport-budget SCRAM, T379 PARTIAL

- Общий bounded SCRAM message limit увеличен с 256 KiB до 700 KiB. При
  server nonce suffix и обязательных client-final полях Base64 continuation
  остаётся внутри POP3 reactor input budget 1 MiB.
- Независимый STARTTLS peer проверил client-first ровно 700 KiB, затем
  соответствующий SHA-256 client-final ровно 700 KiB + 72 октета. Он дошёл до
  proof verification и ожидаемого `invalid-proof`; размерного отказа нет.
  Это негативный proof path и транспортная граница, не успешный вход.
- Полная Linux Rust 1.97.1 suite, strict Clippy, release build, Python syntax и
  живая acceptance прошли. Использованы существующие `rust:1.97-bookworm` и
  `./data`; временный listener удалён, новый image не создавался. T377 остаётся
  GAP: конечный parser cap не подтверждает безграничные nonce/response размеры.

### 2026-09-28: G19 POP3 larger bounded SCRAM transcript, T378 PARTIAL

- Увеличен общий bounded SCRAM attribute-message limit с 16 KiB до 256 KiB;
  отдельный client-final limit автоматически стал 256 KiB + 136 октетов.
  POP3 continuation wire line остаётся в пределах существующего 1 MiB транспортного
  бюджета, при этом SCRAM nonce expansion и channel-binding overhead укладываются
  в parser profile.
- Независимый STARTTLS peer передал client-first ровно 256 KiB, проверил
  сохранённый nonce и послал соответствующий SHA-256 client-final длиной
  256 KiB + 72 октета. Сообщение дошло до proof verification и ожидаемого
  `invalid-proof`; сессия завершила AUTH обычным отказом. Это проверяет parser
  и wire-boundary path, но не успешный вход с большим payload.
- Rust 1.97.1 Linux suite прошла (1113 library + 1 CLI, 4 ignored), strict
  Clippy, release build, Python syntax и живая acceptance прошли. Переиспользованы
  существующие `rust:1.97-bookworm` и `./data`; временный listener удалён, новый
  image не создавался. T377 остаётся GAP: RFC 5034/RFC 5802 не дают конечной
  верхней границы для supported SCRAM nonce.

### 2026-09-28: G19 POP3 arbitrary SASL response size, GAP T377

- RFC 5034 §4 says encoded SASL challenges and responses may have arbitrary
  mechanism-dependent length and requires each peer to handle the largest
  messages generated by mechanisms it supports. RFC 5802 defines SCRAM nonce as
  printable ASCII and does not set a maximum. Current shared SCRAM attribute
  parser used to cap client-first at 16 KiB and client-final at 16 KiB + 136
  octets. T378 raised those limits to 256 KiB and 256 KiB + 136; T379 raised
  them again to 700 KiB and 700 KiB + 136. POP3 still drains a continuation
  above 1 MiB. Thus boundary tests prove only the larger local limits, not
  complete size interoperability for advertised SCRAM.
- This remains an explicit GAP, not PASS. Closing it requires a deliberate
  bounded streaming/input policy and end-to-end size contract across POP3
  Base64 framing, shared SCRAM parsing and SASL mechanism state; raising one
  parser constant alone would leave the wire path incompatible. The next
  acceptance must demonstrate the maximum advertised profile end to end or
  stop advertising a mechanism whose accepted limits are not interoperable.
- Source: [RFC 5034 §4](https://www.rfc-editor.org/rfc/rfc5034.html#section-4)
  and [RFC 5802 §5.1](https://www.rfc-editor.org/rfc/rfc5802.html#section-5.1).

### 2026-09-28: G19 POP3 SASL CAPA after successful AUTH, T376 PASS

- RFC 5034 §3 requires `CAPA` to keep advertising the `SASL` capability after
  successful AUTH, while §4 prohibits another AUTH in that session. Added a
  session regression test and independent STARTTLS wire check that authenticate
  Bob with PLAIN, read CAPA in TRANSACTION, and confirm its SASL mechanism line
  still includes PLAIN. Existing transaction command flow then continues.
- Rust 1.97.1 Linux suite passed (1113 library + 1 CLI, 4 ignored), strict
  Clippy and release build passed; Python syntax and live acceptance passed.
  Clippy component was installed in the disposable test container only.
  Existing `rust:1.97-bookworm` and `./data` were reused; listener container
  removed, no image created. This is RFC 5034 §3 scoped coverage, not complete
  POP3 conformance or Litterkeep application interoperability.

### 2026-09-28: G19 POP3 SCRAM nonce-expansion final bound, T375 PASS

- На момент T375 SCRAM client-first был ограничен 16 KiB. Его допустимый nonce
  мог расшириться серверным nonce suffix, channel-binding и proof полями в
  client-final; единый лимит ранее отвергал корректный ответ этого обмена.
  Client-final теперь использует отдельный жёсткий лимит 16 KiB + 136 октетов,
  сохраняя исходный лимит для остальных SCRAM сообщений.
- Независимый STARTTLS peer тогда послал client-first ровно 16 KiB и получил
  challenge с сохранённым nonce. Соответствующий SHA-256 client-final длиной
  16 KiB + 72 октета прошёл parser и дошёл до ожидаемого `invalid-proof`;
  сессия завершилась обычным отказом аутентификации. Это проверка границы
  проекта, не полный размерный аудит или заявление о полном RFC 5034.
- Rust 1.97.1 Linux полная suite прошла (1113 library + 1 CLI, 4 ignored),
  strict Clippy и release build прошли; Python syntax и независимая живая
  acceptance прошли. Переиспользованы `rust:1.97-bookworm` и `./data`;
  временный listener удалён, новый image не создавался. Litterkeep app не
  проверялась.

### 2026-09-28: G19 POP3 PLAIN maximum supported password continuation, T374 PASS

- Независимый STARTTLS peer отправляет RFC 5034 PLAIN continuation с паролем
  ровно 4096 байт, что соответствует верхней границе установленного SCRAM
  credential verifier. Вместе с `bob` и двумя NUL разделителями Base64
  response занимает 5468 октетов. Сервер принимает его как SASL response и
  доходит до ожидаемого `-ERR authentication failed` для намеренно неверного
  пароля; ошибка лимита POP3 command или invalid Base64 не возвращается.
- Проверен именно парольный максимум текущего credential профиля, а не
  наибольшие теоретические SCRAM nonce/все валидные authcid. RFC 5034 §4
  обязывает peer-ы обрабатывать наибольшие ответы поддерживаемых механизмов;
  полный такой размерный аудит остаётся открытым.
- Python 3.14.7 syntax и полный независимый STARTTLS POP3 acceptance прошли на
  имеющемся Linux Rust 1.97.1 release listener-е. `rust:1.97-bookworm` и
  `./data/g19-pop3-scram-linux` переиспользованы; временный listener удалён,
  новый image не создавался. Litterkeep app этим тестом не проверялась.

### 2026-09-28: G19 oversized POP3 SASL continuation bounded discard, T373 PASS

- Reactor больше не закрывает POP3 session только из-за превышения общего
  входного буфера во время SASL continuation. Для ответа длиннее 1 MiB сервер
  ограниченно отбрасывает bytes до CRLF, возвращает `-ERR authentication
  failed` и сохраняет синхронизацию с pipelined командой. Состояние discard
  хранит только признак предыдущего CR, не тело oversized строки.
- Три Linux unit-теста покрывают полную строку с pipelined хвостом, CRLF между
  чтениями и неизменность общего лимита для других состояний. Независимый Python
  STARTTLS peer отправил continuation длиной 1 MiB + 1 и pipelined CAPA; затем
  тот же сценарий подтвердил последующий PASS для ранее выбранного USER Bob.
  Это тест стороннего клиента, не приложение Litterkeep.
- Rust 1.97.1 Linux полная suite прошла (1112 library + 1 CLI, 4 ignored),
  strict Clippy и три целевых unit-теста прошли; release listener и Python
  acceptance прошли. Использованы `./data/g19-pop3-scram-linux`
  и существующий `rust:1.97-bookworm`; временный listener-контейнер удалён,
  новый Docker image не создавался. Транспортный input остаётся ограничен 1 MiB:
  этот отказной сценарий не доказывает поддержку произвольно больших PLAIN
  payload и не закрывает весь RFC 5034 профиль.

### 2026-09-28: G19 POP3 AUTH initial-response size fallback, T372 PASS

- Независимый Python peer теперь выбирает AUTH PLAIN initial-response только
  если вся команда, включая CRLF, укладывается в 255 октетов. Короткий пароль
  проходит initial-response ветвь; 256-байтовый пароль кодируется в 348-октетный
  Base64 payload, из-за чего полная AUTH команда превышает предел, и peer
  корректно начинает exchange без initial-response, получает пустой `+ ` и
  передаёт payload continuation строкой. Сервер обрабатывает его как SASL,
  не как команду, и отвечает результатом проверки credentials.
- RFC 5034 §4 требует не включать initial-response, если AUTH команда превышает
  лимит RFC 2449, а перейти к challenge/response форме. Это проверка поведения
  acceptance peer, а не реального клиентского приложения.
- Python 3.14.7 `py_compile`, полный независимый Linux STARTTLS acceptance и
  `git diff --check` прошли. Существующие Rust 1.97.1 release listener/image и
  `./data` fixture переиспользованы; контейнер удалён и отсутствие проверено,
  новый image не создавался. Litterkeep app этим этапом не проверялась.
- Проверка единственной SASL capability выражена структурным `match`, без
  прямой индексации списка; после этой корректировки полный acceptance повторно
  прошёл.

### 2026-09-28: G19 POP3 SASL continuation beyond command limit, T371 PASS

- RFC 5034 §4 отделяет ограничение initial-response команды (255 октетов по
  RFC 2449) от последующих SASL challenge/response строк, длина которых зависит
  от механизма. Независимый STARTTLS acceptance отправляет корректный PLAIN
  continuation с 256-байтовым паролем; Base64 строка превышает 255 октетов.
  Сервер принимает framing как SASL response и возвращает результат проверки
  credentials, а не ошибку длины обычной POP3 команды.
- Полный Python STARTTLS acceptance `pop3_sasl_prior_user.py` прошёл на Linux
  Rust 1.97.1 listener-е, включая не рекламируемый DIGEST-MD5 и сохранение USER
  через отказы. Python 3.14.7 `py_compile` и `git diff --check` прошли.
- Переиспользованы `rust:1.97-bookworm` и локальный runtime под `./data`;
  listener удалён и отсутствие проверено, образ не создавался. Эта граница
  подтверждена для 348-октетного Base64 PLAIN payload; не заявляется испытание
  максимального разрешённого transport buffer или всех SASL-механизмов.

### 2026-09-28: G19 reject unadvertised SASL mechanism, T370 PASS

- Исправлен устаревший независимый acceptance `pop3_sasl_prior_user.py`: после
  SCRAM-SHA-256 стал поддержанным, его ошибочно называли unsupported. Теперь
  STARTTLS peer сначала читает CAPA, подтверждает, что `DIGEST-MD5` не объявлен,
  отправляет его с валидным Base64 initial response и проверяет точный
  `-ERR unsupported SASL mechanism`. Затем тот же сценарий подтверждает, что
  USER bob сохранён через отказы пустого/битого PLAIN, cancellation, malformed
  continuation, неверный пароль и неизвестный account; завершает PLAIN login,
  STAT и QUIT.
- Нормативная база: RFC 5034 §3 задаёт CAPA SASL механизмами, а §4 требует
  отклонить неподдерживаемый механизм. Это приёмка сервера, а не требование к
  клиенту отправлять initial response для не объявленного механизма.
- Python 3.14.7 `py_compile` прошёл. Независимый Python STARTTLS client прошёл
  весь сценарий против Linux Rust 1.97.1 release listener-а. Использован уже
  существующий `rust:1.97-bookworm` image и `./data/g19-pop3-scram-linux/runtime`;
  disposable listener удалён и его отсутствие проверено, нового image нет.
  Историческую запись T276 не переписывали; этот этап подтверждает исправленную
  текущую acceptance семантику. Litterkeep app и полный RFC 5034 профиль не
  проверялись.

### 2026-09-28: G19 POP3 SCRAM-SHA-256-PLUS, T365 PASS

- POP3 теперь разбирает типизированный `SCRAM-SHA-256-PLUS` и рекламирует его
  только после TLS 1.3, когда завершённый `ServerTls` предоставляет 32-октетный
  RFC 9266 `EXPORTER-Channel-Binding`. GS2 допускает `p=tls-exporter,,`; сервер
  связывает canonical `c=` с точной GS2 header и exporter, затем проверяет
  proof до доступа к maildrop. Обычный SCRAM-SHA-256 сохраняет прежний путь.
- Unit tests проверяют parser, отсутствие PLUS без exporter, capability,
  корректный binding/proof, server signature и acknowledgement. Независимый
  Rustls Linux peer проверяет CA и TLS 1.3, получает greeting, CAPA, независимо
  экспортирует channel binding, проходит полный PLUS обмен, сверяет server
  signature и выполняет STAT/QUIT. SHA-256 verifier старой acceptance учётки
  создаётся предварительным штатным USER/PASS. Crypto peer повторно использует
  crate SHA/HMAC/PBKDF2 helpers, поэтому он независим по transport/protocol,
  но не является независимым crypto oracle и не проверяет приложение Litterkeep.
- Windows Rust 1.97.1 all-target: 996 library + 1 CLI passed, 4 ignored; fmt,
  strict Clippy и `git diff --check` прошли. Linux release build прошёл в
  `rust:1.97-bookworm`; Linux integration peer прошёл (1/1). Windows all-target:
  один явно требующий listener peer ожидаемо ignored.
- Существующий image ID `sha256:0e2bcaef56d041a486784e54104a81aebe0da44bd03019bd70bc0401e42e4a97`
  переиспользован; созданный POP3 listener удалён и его отсутствие проверено;
  новый image не создавался. Linux target/cache и artifacts лежат в
  `./data/g20-pop3-scram-plus`; предыдущие G19 runtime fixtures использованы
  для существующей test account. Изменённые source/test файлы меньше 300 строк
  каждый. POP3 PLUS wire этап принят, G19 в целом остаётся ACTIVE.

### 2026-09-28: G19 SCRAM-SHA-256-PLUS failure recovery, T366 PASS

- Независимый TLS 1.3 peer расширен отрицательными wire сценариями: transcript
  с синтаксически корректным, но чужим exporter отклоняется; правильный exporter
  с валидно сформированным proof на неверном пароле также отклоняется после
  server-final acknowledgement. После каждого отказа `USER bob` принимается,
  что проверяет возврат в POP3 AUTHORIZATION согласно RFC 5034 §4. Успешный PLUS
  вход, server signature и доступ к maildrop остаются частью того же acceptance.
- Первоначальная попытка ожидала `NOOP` после отказа, но это противоречит RFC
  1939 §5, где NOOP допустим лишь в TRANSACTION; тест скорректирован на допустимый
  USER. Никакое серверное ограничение или тестовая семантика не ослаблялись.
- Windows Rust 1.97.1 all-target: 996 library + 1 CLI passed, 4 ignored; fmt,
  strict Clippy и `git diff --check` прошли. Linux live peer прошёл (1/1) на
  существующем release binary; выделенный wire target собран с incremental off.
- Существующий rust image переиспользован; listener удалён и проверен отсутствующим;
  нового image нет. Acceptance sources разделены на peer и scenario файлы по
  лимиту 300 строк; кэш расположен в `./data/g20-pop3-scram-plus`. Это расширяет
  доказательства отказов/восстановления, G19 и общий RFC 5034 профиль остаются ACTIVE.

### 2026-09-28: G19 SCRAM-SHA-256-PLUS cancellation, T367 PASS

- Независимый POP3 TLS 1.3 peer выбирает `USER bob`, начинает `AUTH
  SCRAM-SHA-256-PLUS` без initial response, отменяет запрос client-first через
  RFC 5034 `*`, затем успешно отправляет `PASS` с тем же паролем и завершает
  соединение через `QUIT`. Это проверяет восстановление прежней AUTHORIZATION
  пары USER/PASS при отмене до передачи verifier lookup.
- Windows Rust 1.97.1 all-target: 996 library + 1 CLI passed, 4 ignored; fmt и
  strict Clippy прошли. Linux независимый implicit-TLS acceptance прошёл (1/1),
  включая cancellation, wrong exporter/proof failure recovery, успешный PLUS,
  server signature, STAT и QUIT. `git diff --check` прошёл.
- Существующий `rust:1.97-bookworm` образ переиспользован; listener удалён,
  отсутствие проверено; нового образа нет. Peer остаётся отдельным файлом меньше
  300 строк, артефакты в `./data/g20-pop3-scram-plus`. Доказательство отмены
  добавлено, полный RFC 5034 профиль и G19 остаются ACTIVE.

### 2026-09-28: G19 reject AUTH after success, T368 PASS

- RFC 5034 §4 требует отклонять повторный AUTH в той же session после успешной
  SASL-аутентификации. Независимый TLS 1.3 POP3 peer теперь после успешного
  SCRAM-SHA-256-PLUS отправляет второй AUTH, проверяет `-ERR`, затем выполняет
  `STAT` в уже открытой TRANSACTION и закрывает её через QUIT. Так проверяется
  не только код отказа, но и сохранение активной maildrop session.
- Windows Rust 1.97.1 all-target: 996 library + 1 CLI passed, 4 ignored; fmt,
  strict Clippy и `git diff --check` прошли. Linux Rust 1.97.1 live acceptance
  прошёл (1/1), сохранив проверки PLUS success/failure/cancellation.
- Переиспользован `rust:1.97-bookworm`; listener удалён и отсутствие проверено,
  новый образ не создавался. Изменение локализовано в wire acceptance под
  `./data/g20-pop3-scram-plus`; полный RFC 5034 профиль остаётся ACTIVE.

### 2026-09-28: G19 POP3 SCRAM-SHA-256-PLUS over STLS, T369 PASS

- Независимый peer начинает с plaintext POP3: проверяет отсутствие SASL и
  наличие STLS, получает положительный STLS boundary, согласует TLS 1.3 и
  проверяет сертификат локальным CA. После upgrade CAPA включает
  `SCRAM-SHA-256-PLUS`; peer экспортирует RFC 9266 channel binding, проходит
  PLUS proof/server signature, получает maildrop через STAT и закрывает QUIT.
  Verifier для исходной acceptance учётки создаётся перед сценарием обычным
  USER/PASS через тот же STLS listener.
- Windows Rust 1.97.1 all-target: 996 library + 1 CLI passed, 4 ignored; fmt,
  strict Clippy и `git diff --check` прошли. Linux Rust 1.97.1 независимый
  STARTTLS live test прошёл (1/1); существующий implicit-TLS плюс suite остаётся
  отдельным сценарием T365–T368.
- Использован существующий `rust:1.97-bookworm`; временный STLS listener удалён,
  отсутствие проверено; нового Docker image нет. Peer лежит в
  `tests/support/`, source-файлы не превышают 300 строк, cache в
  `./data/g20-pop3-scram-plus`. Это подтверждает PLUS через POP3 STLS, не
  совместимость приложения Litterkeep и не полный RFC 5034 профиль.

### 2026-09-28: G18 MIME parameter separator whitespace, T364 PASS

- RFC 2045 §5.1 и RFC 2231 §7: parser отклоняет SP/HTAB непосредственно до и
  после `=` в regular, extended и continuation параметрах. Допустимый OWS после
  `;` и комментарии RFC 822 не превращаются в ложные ошибки.
- Unit regressions и независимый IMAP STARTTLS wire acceptance проверяют
  fallback для испорченного Content-Type, отбрасывание только некорректного
  filename при сохранении `attachment` в BODYSTRUCTURE и исходные RFC822 bytes
  без изменений. Peer отправляет обычный literal RFC 9755 после ENABLE.
- Windows Rust 1.97.1: 995 library + 1 CLI, 4 ignored; all-target tests, fmt,
  strict Clippy и Python 3.14 syntax check прошли. Linux Rust 1.97.1:
  1108 library + 1 CLI, 4 ignored; fmt, strict Clippy, all-target tests и
  release build прошли. Независимый IMAP STARTTLS acceptance прошёл.
- Существующий `rust:1.97-bookworm` переиспользован; listener-контейнер удалён,
  его отсутствие проверено; нового image нет. Build cache лежит в
  `./data/g18-mime-rfc2231-equals`. Это узкий parser/wire этап, не полная MIME
  совместимость и не проверка приложения Litterkeep; G18 остаётся ACTIVE.

### 2026-09-28: G17 RFC 9755 FETCH BODYSTRUCTURE string mode, PARTIAL T352

- Connection-local `StringMode` теперь проходит через BODY, BODYSTRUCTURE,
  рекурсивные MIME parts и nested message ENVELOPE. RFC 3501 `string` значения
  MIME description, Content-ID и RFC 2231 parameters используют literals до
  ENABLE, а quoted UTF-8 после ENABLE. ASCII поля сохраняют compact quoted form.
- Unit-проверка покрывает decoded Content-Description, Unicode Content-ID,
  Content-Type `name*`, disposition `filename*` и обе сериализации. Независимый
  STARTTLS клиент через APPEND сохранённого ASCII RFC 2047/RFC 2231 fixture
  подтвердил ENVELOPE и BODYSTRUCTURE до ENABLE и после `ENABLE UTF8=ACCEPT`.
  PASS: `python tests/blackbox/imap_utf8_envelope_mode.py --port 32143 --ca
  data/pki/ca.pem --credentials data/g22-imap-bodystructure-runtime/test-accounts.tsv`.
- Windows Rust 1.97.1: 965 library + 1 CLI, 4 ignored; all-target tests,
  fmt и strict Clippy прошли. Linux `rust:1.97-bookworm`, Rust 1.97.1:
  1076 library + 1 CLI, 4 ignored; fmt, strict Clippy, all-target tests и
  release build прошли. Runtime fixture `./data/g22-imap-bodystructure-runtime`,
  target `./data/g22-imap-bodystructure-linux-target`; existing image reused,
  disposable listener container удалён и его отсутствие проверено, новый image
  не создавался. Изменённые code/test файлы не превышают 300 строк.
- Остаются непокрытые IMAP response strings и исходный RFC822 header downgrade;
  capability `UTF8=ACCEPT` не рекламируется, Litterkeep не запускался. G17
  остаётся PARTIAL/ACTIVE.

### 2026-09-28: G17 RFC 9755 FETCH ENVELOPE string mode, PARTIAL T351

- FETCH ENVELOPE теперь получает режим quoting из владельца IMAP-сессии. В
  Legacy UTF-8 значения, в том числе decoded RFC 2047 subject/display name,
  сериализуются как RFC 3501 length-counted literals; после ENABLE UTF8=ACCEPT
  они сериализуются quoted UTF-8. Контрольные ASCII строки остаются quoted.
- Новый независимый STARTTLS `imaplib` acceptance APPEND-ит ASCII encoded-word
  message и читает ENVELOPE двумя соединениями: до ENABLE и после него. Проверены
  точный literal/quoted вариант и отсутствие UTF-8 внутри legacy quoted-string.
  PASS: `python tests/blackbox/imap_utf8_envelope_mode.py --port 32143 --ca
  data/pki/ca.pem --credentials data/g22-imap-envelope-runtime/test-accounts.tsv`.
- Windows Rust 1.97.1: 964 library + 1 CLI, 4 ignored; all-target tests,
  fmt и strict Clippy прошли. Linux Rust 1.97.1 в `rust:1.97-bookworm`:
  1075 library + 1 CLI, 4 ignored; fmt, strict Clippy, all-target tests и
  release build прошли. Runtime fixture `./data/g22-imap-envelope-runtime`,
  Linux target `./data/g22-imap-envelope-linux-target`; существующий image
  использован, созданный listener container остановлен и его отсутствие
  подтверждено, image не создавался. Изменённые файлы ниже 300 строк.
- Это derived ENVELOPE string-mode coverage, не downgrade исходных RFC822
  заголовков, не acceptance остальных response paths, не полный RFC 9755 gate
  и не проверка клиента Litterkeep. G17 остаётся PARTIAL/ACTIVE.

### 2026-09-28: G18 RFC 6533 UTF-8 ORCPT unitext parsing, PARTIAL T350

- Typed decoder для utf-8 ORCPT теперь принимает и 7-bit xtext, и unitext с
  буквальными Unicode scalar values и \x{HEXPOINT} escape-ами. Оба варианта
  проходят общую mailbox validation и сохраняются как семантические UTF-8
  octets; исходный способ кодирования намеренно не сохраняется в sidecar.
- Расширенный независимый SMTP socket peer против Linux release listener-а
  подтвердил для xtext и буквального unitext одинаковый отказ 555 5.5.4,
  отсутствие DSN capability и восстановление NOOP/RSET/QUIT. Это отказ
  unsupported extension, не RFC 6533 wire support. Отдельная native
  utf-8-address form policy всё ещё требует SMTPUTF8-specific handling.
- Windows Rust 1.97.1: 964 library + 1 CLI test, 4 ignored; all-target tests,
  strict Clippy и fmt прошли. Linux Rust 1.97.1: 1074 library + 1 CLI test,
  4 ignored; all-target tests, strict Clippy, fmt и release build прошли.
- Wire peer прошёл: python tests/blackbox/smtp_dsn_refusal.py 127.0.0.1 2526.
  Disposable listener container удалён и его отсутствие проверено; Docker
  image не создавался. Linux build оставлен в
  ./data/g18-rfc6533-unitext-linux-target, runtime fixture в
  ./data/g18-rfc6533-unitext-runtime. Изменённые source/test файлы ниже 300
  строк; пользовательский smtp_data_line_bound.py сохранён.
- T350 PARTIAL, G18 ACTIVE.

### 2026-09-28: G18 RFC 6533 ORCPT wire refusal, PARTIAL T349

- Независимый Python socket peer прошёл против Linux release listener-а:
  EHLO не содержит `DSN`, корректный `ORCPT=utf-8;...\x{4F8B}...` распознаётся
  и отвергается как неподдерживаемое расширение ровно `555 5.5.4`, после отказа
  та же SMTP-сессия принимает NOOP, RSET и QUIT.
- Проверены `python tests/blackbox/smtp_dsn_refusal.py 127.0.0.1 2525` и
  `PASS smtp-dsn-refusal`. Сервер собран в Linux Rust 1.97.1 release из
  `./data/g18-rfc6533-linux-target-verify`. Временный контейнер удалён и его
  отсутствие подтверждено; образы не создавались, runtime fixture оставлен в
  `./data/g18-rfc6533-wire-runtime`.
- Это проверка отказа wire-пути, не приём RFC 3461/RFC 6533 и не проверка
  приложения Litterkeep. T349 PARTIAL, G18 ACTIVE.

### 2026-09-28: G18 RFC 6533 UTF-8 ORCPT xtext, PARTIAL T348

- RCPT parser теперь распознаёт address-type `utf-8` и только безопасную
  7-bit форму `utf-8-addr-xtext`: QCHAR плюс `\x{HEXPOINT}`. Проверяются
  ограничения RFC 6533 для HEXPOINT, Unicode scalar и синтаксис UTF-8 mailbox.
  Sidecar version 2 хранит UTF-8 type/octet value; decoder сохраняет чтение
  version 1 RFC822 записей.
- Проверены malformed/native UTF-8 отказ, mailbox grammar, типизированный
  command parsing и Store reopen. Это внутренний parser/storage acceptance:
  сервер по-прежнему не рекламирует DSN и отказывает параметризованным MAIL/
  RCPT кодом `555 5.5.4`. Unitext/native wire support, SMTPUTF8 negotiation для
  них, отчёты и полная RFC 3461 семантика остаются GAP.
- Windows Rust 1.97.1: 962 library + 1 CLI test, 4 ignored; all-target tests,
  strict Clippy, fmt и diff check прошли. Linux Rust 1.97.1: 1072 library + 1
  CLI test, 4 ignored; all-target tests, strict Clippy и fmt прошли.
- Существующий `rust:1.97-bookworm` переиспользован; rustfmt/clippy установлены
  только в disposable `--rm` container, Docker image не создавался. Linux target
  оставлен в `./data/g18-rfc6533-linux-target-verify`. Первый повтор на старом
  bind-mounted incremental target получил missing-object ошибку Cargo; без
  удаления артефактов повтор на отдельном target прошёл полностью. Изменённые
  source/test файлы меньше 300 строк; user-owned blackbox diff сохранён.
- T348 PARTIAL, G18 ACTIVE.

### 2026-09-28: G18 SMTP DSN coverage reconciliation, PARTIAL T347

- Актуальная матрица `docs/protocol-coverage.md` теперь разделяет внутренний
  typed/durable handoff T345/T346 и неподдерживаемый RFC 3461 wire-профиль:
  EHLO не рекламирует DSN, команды остаются отказанными, генерация уведомлений
  не реализована. Исторические T339/T344 записи сохранены, добавлены строки
  приёмки T345/T346.
- RFC 6533 первоисточник проверен: `utf-8` ORCPT имеет xtext, unitext и native
  формы с разными требованиями SMTPUTF8. Не выдаём частичный парсер за
  поддержку; весь формат остаётся GAP до определения строгого typed контракта.
- Документационная сверка проверена `git diff --check`; исходники и тесты не
  менялись. Использован существующий Docker image; контейнеры и образы не
  создавались, пользовательский diff `smtp_data_line_bound.py` сохранён.
- T347 PARTIAL, G18 ACTIVE.

### 2026-09-28: G18 SMTP DSN envelope through transaction and Store owner, PARTIAL T346

- Typed MAIL `RET`/`ENVID` и recipient-scoped `NOTIFY`/`ORCPT` теперь входят в
  SMTP transaction envelope, проходят через DATA/BDAT state, bounded
  `AppendMany` request и single Store owner; Store связывает sidecar с commit ID.
  Обычные письма без DSN параметров не создают sidecar. Сверяются reverse-path,
  количество и порядок адресов до durable delivery.
- Linux integration test отправляет запрос реальному storage owner, получает
  append completion и после reopen находит точный DSN envelope. Отдельный Linux
  SMTP socket peer проверил EHLO без `DSN`, точный `555 5.5.4` отказ MAIL/RCPT и
  восстановление через NOOP/RSET/QUIT. Это независимый peer, не приложение
  Litterkeep.
- Windows Rust 1.97.1: 958 library + 1 CLI test, 4 ignored; strict Clippy,
  fmt и diff check прошли. Linux `rust:1.97-bookworm`: 1068 library + 1 CLI,
  4 ignored; strict Clippy, fmt и release build прошли. Wire refusal peer прошёл.
  Все затронутые исходные/test файлы не превышают 300 строк.
- DSN capability остаётся выключенной и параметризованные команды остаются
  отвергнутыми: сервер ещё не планирует обязательные success/failure/delay
  уведомления и не прошёл DSN report conformance. Новая цепочка проверяет
  внутренний typed handoff, но не является RFC 3461 wire acceptance.
- Linux build artifacts оставлены под `./data/g18-dsn-transport-linux`, runtime
  fixture под `./data/g18-dsn-wire-runtime`. Все созданные контейнеры удалены;
  существующий `rust:1.97-bookworm` образ переиспользован без создания image.
  T346 PARTIAL, G18 ACTIVE.

### 2026-09-28: G18 SMTP DSN durable Store boundary, PARTIAL T345

- Store теперь пишет bounded/versioned/checksummed SMTP DSN sidecar frame рядом
  с append transaction. Sidecar становится видимым только после глобального
  transaction commit; reopen отбрасывает незакоммиченные записи, recovery
  обрезает неполный хвост, старые Store без sidecar продолжают открываться.
- Это пока внутренний Store API: SMTP reactor и storage worker не передают туда
  DSN envelope. Wire команды с корректными параметрами всё ещё получают
  `555 5.5.4`, capability `DSN` не объявляется. Не заявляется выполнение
  success/failure/delay уведомлений.
- Windows Rust 1.97.1: 956 library tests и 1 CLI test прошли, 4 ignored;
  strict all-target Clippy, fmt и diff check прошли. Linux в существующем
  `rust:1.97-bookworm`: 1065 library + 1 CLI tests, 4 ignored, strict Clippy и
  fmt прошли. Все новые DSN-файлы меньше 300 строк.
- Linux артефакты и отдельный Cargo target оставлены в
  `./data/g18-dsn-store-linux`. Два временных контейнера автоматически удалены
  через `--rm`; Docker-образы не создавались. Приложение Litterkeep не проверено.
- Следующий шаг: провести typed envelope через реальный SMTP transaction и
  single-owner storage request, затем проверить фактическую durable доставку.
  T345 PARTIAL, G18 ACTIVE.

### 2026-09-28: G18 RFC 3461 typed SMTP parameter grammar, PARTIAL T344

- Добавлены typed-модели MAIL `RET`/`ENVID`, RCPT `NOTIFY`/`ORCPT` и отдельные
  enum-варианты SMTP команд. `NOTIFY` проверяет регистр, непустой список,
  уникальность условий и эксклюзивность `NEVER`; xtext проверяет uppercase hex,
  printable ASCII и границы RFC для ENVID/ORCPT.
- Синтаксически верные DSN-команды пока отклоняются сервером ответом
  `555 5.5.4`; EHLO capability `DSN` не объявляется. T345 добавил отдельный
  durable Store boundary, но SMTP transaction пока не вызывает его.
- Windows Rust 1.97.1: `cargo test --all-targets` прошёл (950 library tests,
  1 CLI test, 4 ignored), fmt и strict all-target Clippy прошли. Linux release
  сборка прошла в существующем `rust:1.97-bookworm`; независимый wire peer
  проверил отсутствие DSN capability, отказ обеих параметризованных команд и
  recovery обычной MAIL transaction. Новые исходные/test файлы ниже 300 строк.
- Временный runtime container остановлен и отсутствует; образы не создавались.
  Linux build/runtime артефакты оставлены под `./data/g18-dsn-grammar-linux`.
  Независимый peer не является проверкой приложения Litterkeep.
- RFC 3461 остаётся неполным: durable envelope policy, success/failure/delay
  DSN scheduling, MIME report conformance, RFC 6533 UTF-8 ORCPT и end-to-end
  crash recovery ещё не реализованы. G18 ACTIVE.

### 2026-09-28: G18 SMTP DSN refusal wire check, GAP T339

- RFC 3461 is intentionally not advertised because recipient DSN preferences,
  original-recipient identity, MAIL envelope identifiers, and return-content
  policy do not survive the SMTP transaction and durable delivery path. Merely
  accepting `NOTIFY`/`ORCPT` would silently discard a sender's request.
- Added an independent Python wire peer that confirms EHLO omits `DSN`, a
  syntactically valid `NOTIFY=SUCCESS` receives `555 5.5.4`, and the SMTP
  session remains usable for NOOP after refusal. This is behavior verification
  for the unsupported-extension boundary, not RFC 3461 implementation.
- Windows Rust 1.97.1 `cargo fmt --check` passes; `cargo test --all-targets`
  passes (944 library tests and 1 CLI test, 4 ignored). Linux release server
  build passes in the existing `rust:1.97-bookworm` image; the independent
  Linux wire peer passes and its Python syntax compiles.
- The temporary runtime container was stopped and verified absent. No image was
  created; test output remains isolated in `./data/g18-dsn-wire`. Other Docker
  containers and images were left untouched. No Litterkeep client interop is
  claimed.
- G18 remains ACTIVE; T339 remains GAP pending end-to-end durable RFC 3461
  semantics and acceptance.

### 2026-09-28: G18 POP3 SCRAM-SHA-1 TLS wire profile, scoped PASS T343

- POP3 AUTH теперь имеет отдельный SCRAM-SHA-1 state machine: mechanism
  доступен только под TLS, verifier читается через единственный Accounts owner,
  proof и server signature завершаются до maildrop transaction. Server-final
  отправляется отдельным RFC 5034 challenge; success reply не несёт payload.
- Независимый Linux STARTTLS peer подтвердил valid mutual proof, `e=invalid-proof`,
  initial и challenge flows, восстановление после отказа, конфликт двух
  maildrop владельцев (`IN-USE`) и повторный захват после QUIT. CAPA peer
  подтвердил отсутствие SASL до TLS и рекламу PLAIN/SCRAM после TLS по состояниям.
  Это не проверка приложения Litterkeep.
- Windows Rust 1.97.1: 944 library + 1 CLI tests, 4 ignored; fmt и strict
  all-target Clippy пройдены. Linux Docker Rust 1.97.1: 1053 library + 1 CLI
  tests, 4 ignored; fmt и strict all-target Clippy пройдены. Python acceptance
  успешно выполнен против Linux release сервера. Существующий образ
  `rust:1.97-bookworm` переиспользован, временный контейнер очищен; новых образов
  не создавалось. Артефакты этапа сохранены в `./data/g19-pop3-scram-linux`.
- RFC 5034 PLAIN остаётся обязательным базовым механизмом; этот этап расширяет
  современный профиль SCRAM-SHA-1. SCRAM-SHA-256/PLUS, прочие механизмы и
  Litterkeep interoperability остаются открытыми; G18 ACTIVE.

### 2026-09-28: G18 RFC 2231 duplicate-form rejection, scoped PASS T329

- RFC 2231 regular `filename` и extended `filename*`/`filename*0*` теперь
  считаются конфликтующими формами одного attribute и не сливаются при
  сборке MIME tree. Добавлены regression tests на оба порядка и допустимый
  пустой extended payload.
- Независимый Linux STARTTLS IMAP acceptance доставляет multipart письмо с
  конфликтующим attachment filename. BODYSTRUCTURE не публикует ни одно имя;
  RFC822 FETCH подтверждает сохранность исходных байтов. Это не проверка
  приложения Litterkeep.
- Windows Rust 1.97.1: 913 библиотечных и 1 CLI тест прошли, 4 ignored;
  fmt, strict all-target Clippy и `git diff --check` прошли. Тестовые
  контейнеры удалены, существующие образы повторно использованы.
- RFC 2231 ABNF whitespace у `=` остаётся открытой темой: попытка ужесточения
  затронула comment/quoted-value совместимость и не включена. G18 ACTIVE.

### 2026-09-28: G18 ManageSieve SCRAM-SHA-1 TLS wire exchange, scoped PASS T328

- Завершён ManageSieve SCRAM-SHA-1 command/state/reactor путь поверх TLS:
  challengeable AUTHENTICATE, bounded literal/quoted SASL строки, single-owner
  verifier lookup, проверка client proof и выдача server signature. По RFC 5804
  §2.1 механизм рекламируется только после TLS совместно с PLAIN.
- Независимый Python TLS peer подтвердил SCRAM mutual proof/signature,
  `e=invalid-proof` и восстановление соединения через NOOP. Это не проверка
  приложения Litterkeep. Попытка STARTTLS acceptance не состоялась: текущий
  disposable listener был собран только в implicit-TLS режиме; отдельный
  STARTTLS listener в этой среде не поднялся. Не заявляется как пройденный.
- Windows Rust 1.97.1: `cargo fmt --check`, strict all-target Clippy и
  `cargo test --locked --all-targets` прошли (908 библиотечных + 1 CLI,
  4 ignored). Совместимость приложения и STARTTLS wire acceptance не проверены.
  Оба созданных временных контейнера очищены; образы не создавались.
- Цель G18 остаётся ACTIVE: SCRAM-SHA-256/PLUS, полная совместимость RFC 5804,
  приложение Litterkeep и прочие открытые этапы не закрыты. Исполняемый `goal`
  CLI не найден; фиктивные вызовы не выполнялись.

### 2026-09-28: G18 single-owner SCRAM verifier lookup, scoped PASS T327

- Добавлена типизированная пара request/response для чтения SCRAM verifier
  через единственного владельца Accounts. Ошибки I/O и отсутствующий verifier
  различаются; Debug completion намеренно не раскрывает credential. Worker-тест
  создаёт аккаунт и подтверждает получение записи через bounded канал.
- Windows Rust 1.97.1: `cargo fmt --check`, strict all-target Clippy и
  `cargo test --locked --all-targets` прошли (904 library + 1 CLI, 4 ignored).
  Linux Rust 1.97.1 в существующем `rust:1.97-bookworm`: те же gates прошли
  (1013 library + 1 CLI, 4 ignored). `rustfmt`/`clippy` установлены лишь внутри
  одноразового контейнера; `--rm` удалил контейнер. Новый image не создавался.
- Подтверждён только путь безопасной загрузки verifier. SCRAM всё ещё не
  подключён к ManageSieve command/state machine, client/server exchange и
  wire-acceptance отсутствуют; capability не рекламируется. G18 ACTIVE.
- В этой среде не найден исполняемый `goal` CLI или repository-инструкция его
  вызовов; никаких фиктивных goal-вызовов не делалось. Фактический журнал
  `GOAL.md` обновлён вручную.

### 2026-09-28: G18 SCRAM server-final and mutual proof completion, scoped PASS T326

- `ServerFinal::complete` связывает client-final transcript с verifier:
  некорректный proof даёт `e=invalid-proof`, корректный выдаёт `v=` с
  Base64 ServerSignature. Все RFC 5802 §7 server-error tokens представлены
  закрытым enum, не открытыми строковыми исходами.
- RFC 5802 §5.1 published client proof завершается точной опубликованной
  server signature. Отрицательный тест заменяет proof и получает только
  typed invalid-proof outcome; десять прочих error-вариантов проверены на
  wire encoding.
- Windows Rust 1.97.1: 907 библиотечных и 1 CLI тест прошли, 4 игнорированы;
  fmt/strict Clippy прошли. Linux Docker Rust 1.97.1: 1012 библиотечных и
  1 CLI тест прошли, 4 игнорированы; fmt/strict Clippy прошли. Контейнер
  использовал `--rm`.
- Это формирует только SCRAM final SASL attribute, не ManageSieve outcome,
  session transitions или capability. G18 ACTIVE.

### 2026-09-28: G18 SCRAM client-final proof and transcript, scoped PASS T325

- Добавлен client-final validator fixed order `c,r,extensions,p`. Он проверяет
  exact `biws` GS2 channel binding для `n,,`, совпадение полного server nonce,
  optional extension область и canonical Base64 proof ровно 20 байт.
- `AuthMessage` строится из оригинальных client-first-bare, server-first и
  client-final-without-proof строк без нормализации. Published RFC 5802 §5.1
  transcript принят текущим production verifier; server signature совпала с
  опубликованной `rmF9...` величиной. Отказы nonce/cbind/order/encoding
  проверяются focused тестами.
- Windows Rust 1.97.1: 904 библиотечных и 1 CLI тест прошли, 4 игнорированы;
  fmt/strict Clippy прошли. Linux Docker Rust 1.97.1: 1009 библиотечных и
  1 CLI тест прошли, 4 игнорированы; fmt/strict Clippy прошли. Контейнер
  временный (`--rm`).
- Отдельный server-final wire outcome и SASL state machine всё ещё не
  подключены к ManageSieve. Capability остаётся выключенной. G18 ACTIVE.

### 2026-09-28: G18 cryptographic SCRAM server-first challenge, scoped PASS T324

- Добавлен builder RFC 5802 `r,s,i`: объединяет client nonce с 18 случайными
  октетами, полученными у secure random provider Rustls, и включает salt/
  iteration count из сохранённой SCRAM-записи. Base64 suffix printable ASCII,
  поэтому comma-разделитель nonce исключён.
- Три focused-теста проверяют формат/параметры, различие двух случайных
  challenge и отказ на пустом/непечатном suffix. Windows Rust 1.97.1: 900
  библиотечных и 1 CLI тест прошли, 4 игнорированы; fmt/strict Clippy прошли.
  Linux Docker Rust 1.97.1: 1005 библиотечных и 1 CLI тест прошли, 4
  игнорированы; fmt/strict Clippy прошли. Контейнер удалён через `--rm`.
- Challenge builder пока не подключён к ManageSieve SASL state machine;
  client-final проверки и capability advertisement отсутствуют. G18 ACTIVE.

### 2026-09-28: G18 SCRAM client-first message semantics, scoped PASS T323

- `scram_client_first` принимает профиль GS2 `n,,`, требует ordered `n` then
  `r`, проверяет RFC saslname `=2C`/`=3D`, применяет SASLprep query к username,
  проверяет printable nonce и сохраняет exact `client-first-bare` transcript.
  Unknown optional extensions игнорируются, как предписывает RFC; mandatory
  `m=` отклоняется нижележащим parser-ом.
- RFC 5802 §7 аудит выявил чрезмерное ASCII-only ограничение T322. Общий
  attribute `value-char` теперь принимает UTF-8 и пробелы/control за
  исключением NUL/comma delimiter; printable ASCII проверяется только для
  nonce. Это исправление зафиксировано в coverage, а не скрыто как новый PASS.
- Одиннадцать focused SCRAM parser тестов прошли. Windows Rust 1.97.1:
  897 библиотечных и 1 CLI тест прошли, 4 игнорированы; fmt и strict Clippy
  прошли. Linux Docker Rust 1.97.1: 1002 библиотечных и 1 CLI тест прошли,
  4 игнорированы; fmt и strict Clippy прошли. Контейнер удалён через `--rm`.
- Это не server-first/client-final/server-final parser, SASL state machine
  или ManageSieve wire exchange; capability не рекламируется. G18 ACTIVE.

### 2026-09-28: G18 bounded SCRAM attribute syntax, scoped PASS T322

- Добавлен типизированный парсер SCRAM attribute list: односимвольные ASCII
  имена, уникальность, opaque значения, явные ошибки и отказ на неподдержанное
  mandatory `m=`. Вход ограничен 16 KiB, а список - 26 элементами до создания
  полного вектора. Значения с padding `=` сохраняются без изменения.
- Пять focused-тестов проверяют известные поля, дубликаты, malformed/empty
  формы, mandatory extension, Unicode/whitespace, padding и обе границы.
  Windows Rust 1.97.1: 891 библиотечный и 1 CLI тест прошли, 4 игнорированы;
  fmt и strict Clippy прошли. Linux Docker Rust 1.97.1: 996 библиотечных и
  1 CLI тест прошли, 4 игнорированы; fmt и strict Clippy прошли. Контейнер
  удалён автоматически через `--rm`.
- Это только syntax parser, не проверка обязательных/взаимозависимых полей,
  transcript, server nonce или SASL state machine. Capability выключена, G18
  ACTIVE.

### 2026-09-28: G18 SCRAM verifier backfill after password login, scoped PASS T321

- Успешная проверка существующего `.phc2` теперь создаёт недостающий SCRAM
  sidecar; неверный пароль не изменяет каталог. Legacy `.phc` переход сперва
  записывает sidecar, затем публикует `.phc2`, сохраняя прежний hash.
- Sidecar создаётся через fsync временного файла и hard-link публикацию.
  Параллельный backfill допускает одного победителя, а остальные используют
  уже опубликованную запись. Тест проверяет backfill, неверный пароль,
  повторный вход и legacy migration.
- Windows Rust 1.97.1: 882 библиотечных и 1 CLI тест прошли, 4 игнорированы;
  fmt и strict Clippy прошли. Linux Docker Rust 1.97.1: 991 библиотечный и
  1 CLI тест прошли, 4 игнорированы; fmt и strict Clippy прошли. Контейнер
  временный (`--rm`), других контейнеров данного test image не осталось.
- SCRAM не рекламируется: SASL exchange и ManageSieve wire acceptance
  отсутствуют. G18 ACTIVE.

### 2026-09-28: G18 versioned SCRAM account sidecar, scoped PASS T320

- Новые аккаунты получают отдельную `litterkeep-scram-sha1-v1` запись с
  криптографически случайной солью, 4096 итерациями, StoredKey и ServerKey.
  Формат строго проверяет версию, границы, hex и длины ключей; тест после
  сериализации/reload проверяет SCRAM proof. Sidecar публикуется после durable
  Argon2 PHC. Если SCRAM публикация не удалась, новый PHC удаляется и ошибка
  возвращается вызывающему коду.
- Windows Rust 1.97.1: 881 библиотечный и 1 CLI тест прошли, 4 игнорированы;
  fmt, strict Clippy и diff check прошли. Linux Docker Rust 1.97.1: 990
  библиотечных и 1 CLI тест прошли, 4 игнорированы; fmt и strict Clippy прошли.
  Временный контейнер использовал `--rm`.
- Это не миграция существующих аккаунтов и не SASL exchange. Для уже
  сохранённых `.phc`/`.phc2` sidecar отсутствует до следующего этапа; SCRAM
  capability остаётся выключенной. G18 ACTIVE.

### 2026-09-28: G18 SCRAM-SHA-1 server proof primitive, scoped PASS T319

- `ScramSha1Verifier` теперь восстанавливает ClientKey из proof/AuthMessage,
  сверяет StoredKey циклом без раннего выхода по байтам и строит server
  signature. Можно создавать verifier из двух фиксированных 20-октетных ключей
  для следующего этапа хранения.
- RFC 5802 §5.1 тест применяет production server verifier к известному client
  proof и сравнивает server signature. Отдельные regressions отвергают
  изменённый и усечённый proof. На Windows Rust 1.97.1 прошли 878 библиотечных
  и 1 CLI-тест, 4 теста пропущены; fmt, strict Clippy и diff checks прошли.
  В Linux-контейнере на Rust 1.97.1 прошли 987 библиотечных и 1 CLI-тест,
  4 теста пропущены; fmt и strict Clippy прошли. Контейнер был временным
  (`--rm`) и удалился после команды.
- Это только proof primitive: SCRAM credential serialization/lookup, SASL
  attribute parser, client/server nonce lifecycle, ManageSieve exchange и wire
  acceptance отсутствуют. Никакие capability strings не изменялись.
- G18 остаётся активной; следующий технический шаг - версионированный verifier
  format с криптографической солью и контролируемой миграцией аккаунтов.

### 2026-09-28: G18 ManageSieve SCRAM gap inventory reconciliation, scoped PASS T318

- Сверила текущее состояние кода с `docs/dependencies.md`,
  `docs/sieve-coverage.md`, `docs/protocol-coverage.md`, `docs/unicode-data.md`
  и RFC 5804/5802/7677. Устаревшие формулировки, будто SASLprep и Argon2
  migration отсутствуют, заменены на точный статус T314-T317.
- Подтвердила по RFC 5804 §2.1 обязательность SCRAM-SHA-1 для ManageSieve.
  Существующие SHA-1/PBKDF2 verifier primitives не являются SCRAM protocol;
  credential persistence, ManageSieve state machine и wire acceptance ещё
  отсутствуют. RFC 7677 SCRAM-SHA-256 записан как более сильный, но тоже
  неподдерживаемый дополнительный механизм. Ни один SCRAM mechanism не объявлен.
- T263 в `docs/acceptance.tsv` помечен SUPERSEDED как исторический audit;
  актуальный пробел не закрыт. Это документационная сверка, код и тесты не
  менялись. Источники: [RFC 5804 §2.1](https://www.rfc-editor.org/rfc/rfc5804.html#section-2.1),
  [RFC 7677](https://www.rfc-editor.org/rfc/rfc7677.html).
- G18 остаётся активной; следующий этап требует verifier format и полного
  ManageSieve SASL exchange, не преждевременной рекламы capability.

### 2026-09-28: G18 independent SMTP wire acceptance for legacy PHC, scoped PASS T317

- Новый `tests/blackbox/smtp_legacy_phc_migration.py` использует Python
  `smtplib` как независимый клиент. На одноразовом Linux сервере стандартный
  bootstrap `.phc2` перенесён в legacy `.phc`; SMTP TLS AUTH PLAIN прошёл,
  новая запись опубликована, оригинальные байты `.phc` сохранились.
- Повторный wire-вход был выполнен с временно переименованным `.phc`, поэтому
  подтверждено чтение нового формата, а не fallback. Исходный legacy файл
  восстановлен тестом. Это закрывает wire-пробел T316 только для SMTP; другие
  протоколы и Litterkeep app не проверялись.
- Linux использовал уже существующий `rust:1.97-bookworm`; новый image не
  создавался. Одноразовый сервис остановлен, контейнер исчез,
  `./data/g18-legacy-phc-20260928` оставлен как acceptance fixture.
  `smtp_data_line_bound.py`
  пользователя не менялся.
- G18 остаётся активной. В следующем этапе продолжить RFC-gap аудит других
  протоколов; SCRAM-SHA-1 обмен ManageSieve пока отсутствует и не рекламируется.

### 2026-09-28: G18 безопасная миграция legacy PHC, scoped PASS T316

- Успешная аутентификация старого `.phc` теперь создаёт отдельную Argon2
  запись `.phc2` с SASLprep StoredValue во временный файл, затем публикует
  готовый inode атомарной жёсткой ссылкой. Старый файл не удаляется; новая
  запись получает приоритет. Конкурентная миграция проверяет уже готовую
  запись, а не читает частично записанный файл.
- Ошибки чтения/записи проходят отдельным `io::Result<bool>` до протокола:
  IMAP, SMTP, POP3 и ManageSieve возвращают временный отказ, а не маскируют
  проблему диска под неверный пароль. Юнит-тест подтверждает переход к
  нормализованному варианту после миграции. Это scoped PASS, не завершение G18.
- Windows Rust 1.97.1: `cargo fmt --check`, последовательные
  `cargo test --all-targets` (877 library + 1 CLI, 4 ignored) и strict Clippy
  прошли. Один параллельный общий прогон поймал несвязанный конфликт имён
  временного файла Windows; повторный последовательный suite прошёл.
- Linux Docker Rust 1.97.1: те же fmt, тесты (986 library + 1 CLI, 4 ignored)
  и strict Clippy прошли. Использован существующий `rust:1.97-bookworm`,
  одноразовые контейнеры запускались с `--rm`, новых образов не создавалось.
  Одновременная миграция проверена unit-тестом на обеих платформах.
- Оставленный пользователем `smtp_data_line_bound.py` не менялся. Остаток G18:
  независимая protocol wire-проверка legacy-перехода и SCRAM state machine.

### 2026-09-28: G18 SASLprep Argon2 credential storage, scoped PASS T315

- Подключила RFC 4013 к общей границе `Accounts`: новые Argon2 записи
  нормализуются в StoredValue-контексте и сохраняются под отдельным
  `SaslPrepCredentials` file kind (`.phc2`); query passwords подготавливаются
  перед проверкой. SMTP/IMAP/POP3/ManageSieve используют общий owner auth path.
- Старые `.phc` не объявляются SASLprep credentials: совместимость сохраняет
  проверку и подготовленного query, и прежних exact raw bytes. Автоматической
  перезаписи нет; raw fallback для legacy credentials остаётся явным пробелом.
- Windows Rust 1.97.1: fmt, strict Clippy `-D warnings`, all-targets:
  880 library + 1 CLI прошли. Linux Docker Rust 1.97.1: fmt, strict Clippy,
  985 library + 1 CLI прошли. Одноразовый container удалён через `--rm`,
  отдельный image не создавался. G18 ACTIVE; независимая protocol wire-проверка
  Unicode password login и безопасная миграция старых PHC ещё нужны.

### 2026-09-28: G18 SASLprep Unicode 3.2 and verifier integration, scoped PASS T314

- Добавлен собственный RFC 4013 SASLprep профиль: B.1/C.1.2 mapping,
  Unicode 3.2 NFKC, полный RFC prohibited output, bidi и контексты
  StoredValue/Query для A.1 unassigned. `ScramSha1Verifier::derive` нормализует
  UTF-8 secret до PBKDF2. Runtime-зависимости не добавлялись.
- Генератор закрепляет хеши RFC/UCD источников и проверяет 14 таблиц RFC 3454
  на всём диапазоне кодовых точек против опубликованных диапазонов. Rust NFKC
  прошёл полный `NormalizationTest-3.2.0.txt`; RFC 4013 примеры и эквивалентные
  verifier credentials прошли.
- Windows Rust 1.97.1: fmt, strict Clippy `-D warnings`, all-targets:
  871 library + 1 CLI, 4 ignored. Linux Docker Rust 1.97.1: те же gates,
  980 library + 1 CLI, 4 ignored. Полный Unicode 3.2 NFKC corpus отдельно
  прошёл на обеих платформах. Одноразовый container удалён автоматически,
  временный image tag очищен, существующий Rust image сохранён.
- G18 остаётся ACTIVE: account persistence/migration, SCRAM state machine и
  независимое wire acceptance всё ещё отсутствуют; SCRAM не рекламируется.

### 2026-09-28: G18 SCRAM-SHA-1 verifier derivation, scoped PASS T313

- Связала crypto-примитивы в `ScramSha1Verifier`, получающий StoredKey и
  ServerKey из уже SASLprep-normalized password. Параметры ограничены до KDF:
  salt 1..64 octets, iterations 4096..200000.
- Сквозной crypto test воспроизводит опубликованные client proof и server
  signature из RFC 5802 §5.1; пустой salt и iteration вне диапазона отвергаются.
  Windows: 864 library + 1 CLI, 3 ignored, fmt и strict Clippy; Linux Docker:
  973 library + 1 CLI, 3 ignored, doc-tests и strict Clippy. Test image удалён.
- Account storage, SASLprep, миграция Argon2-only записей и протокольный
  SCRAM exchange ещё отсутствуют; mechanism не рекламируется. G18 ACTIVE.

### 2026-09-28: G18 SCRAM-SHA-1 primitive foundation, scoped PASS T312

- Реализованы собственные safe-Rust SHA-1, HMAC-SHA-1 и PBKDF2-HMAC-SHA-1
  без добавления зависимости. RFC known-answer tests покрывают SHA-1, короткий
  и длинный HMAC key, а PBKDF2 итерации 1/2/4096.
- Windows Rust 1.97.1: fmt, strict Clippy, 862 library + 1 CLI тест прошли,
  3 external corpus ignored. Linux Docker: release build, strict Clippy,
  971 library + 1 CLI, 3 ignored, doc-tests прошли. Временный test image удалён.
- Это не SCRAM authentication: долговечный verifier, миграция Argon2-only
  аккаунтов, SASL state machine и wire acceptance отсутствуют. Capability
  остаётся скрытой. G18 остаётся ACTIVE.

### 2026-09-28: G18 SMTP DATA bounded incomplete-line wire acceptance, scoped PASS T311

- Existing DATA receiver bounds an unterminated line fragment before CRLF and
  drains to dot terminator; independent STARTTLS client sent more than 26 MiB,
  observed 552, then successful NOOP in the same session.
- This is bounded-memory/synchronization evidence for SMTP DATA only, not
  general SMTP conformance or Litterkeep app interoperability. Acceptance is
  recorded as T311; the pre-existing user-owned probe was run but not edited.
- G18 remains ACTIVE; continue the cross-protocol RFC gap audit.

### 2026-09-28: G18 registered IANA MIME aliases, scoped PASS T308

- Added explicitly enumerated aliases from the IANA registry for already
  supported ISO-8859-1..8 and ISO-8859-13 profiles, plus Windows-1251/1252/1255/1256
  `cs` aliases and registered Windows-1252 `cp1252`. Unregistered `x-cp1252`
  remains rejected; no decoder families were added.
- Unit tests cover every new alias. IANA's full registry is much broader, so
  this stage claims only names mapped to existing decoders, not registry-wide
  charset support. Linux network matrix coverage remains the 41 canonical labels
  from T307; this alias-only stage has no additional wire acceptance claim.
- G18 remains ACTIVE; broader RFC/registry audit and other protocol gaps remain.

### 2026-09-28: G18 UTF-8 canonical MIME label, scoped PASS T309

- Correction to T309: IANA registers `csUTF8` as the `cs` alias for preferred
  label `UTF-8`; accept that explicit alias and keep unregistered `utf8` rejected.
  Windows and Linux acceptance has now verified `csUTF8` through SMTP MIME body
  delivery and IMAP SEARCH. This supersedes T309's unsupported no-network claim.
- Independent charset matrix now has 42 labels. Rust 1.97.1 Windows and Linux
  Docker gates pass; details and created-resource cleanup are recorded in T310.
- G18 remains ACTIVE; full protocol and charset conformance remain outstanding.

### 2026-09-28: G18 RFC 1556 bidi MIME charset aliases, scoped PASS T307

- Добавлены четыре отдельные typed charset profiles: ISO-8859-6-E/I и
  ISO-8859-8-E/I, их IANA aliases и byte mappings на существующие строгие
  ISO-8859-6/8 таблицы. Неизвестные mappings по-прежнему дают ошибку.
- Unit tests проверяют distinct enum values, арабский/ивритский text,
  сохранение LRM/RLM и отказ undefined octets. Windows Rust 1.97.1 strict
  Clippy и full suite: 856 library + 1 CLI, 3 ignored.
- Linux Docker 1.97 strict Clippy и full suite: 965 library + 1 CLI,
  3 ignored; doctests прошли. Независимый Python SMTP STARTTLS -> IMAP UTF-8
  SEARCH подтвердил все 41 charset labels, включая четыре новые bidi profiles.
- MIME search mapping подтверждён; explicit control presentation и implicit
  bidi layout не реализуются и не заявляются. Acceptance в
  `./data/acceptance-mime-bidi-20260928`; созданный контейнер и оба images
  удалены с проверкой отсутствия. G18 остаётся ACTIVE.

### 2026-09-28: G18 EUC-KR MIME decoder and IMAP SEARCH, scoped PASS T306

- Добавлен собственный strict WHATWG EUC-KR decoder: ASCII, typed WHATWG/IANA
  labels и O(1) mapping через закреплённую таблицу в `charset_euc_kr_table.bin`.
  Invalid, truncated и unmapped последовательности отклоняются.
- Таблица строится `tools/generate_euckr_table.py` из WHATWG `index-euc-kr.txt`
  с SHA-256 validation. Rust unit tests проверяют asset shape; отдельный тест
  сравнивает все 23 940 wire pointer slots с decoder output.
- Windows Rust 1.97.1 strict Clippy и all-target tests прошли: 854 library +
  1 CLI, 3 ignored. Linux Docker Rust 1.97 gate прошёл strict Clippy,
  963 library + 1 CLI, 3 ignored и doc-tests.
- Независимый Python 3.14 `euc_kr` codec и SMTP STARTTLS -> IMAP UTF-8 SEARCH
  acceptance прошли для 37 charset labels, в том числе EUC-KR `안녕하세요`, и
  отдельного RFC 2047 Subject. Это не проверка приложения Litterkeep.
- Индекс сохранён в `./data/g18-euckr-table-20260928`, acceptance данные в
  `./data/acceptance-mime-euckr-20260928`; контейнер и оба созданных image
  удалены, отсутствие проверено. G18 остаётся ACTIVE.

### 2026-09-28: G18 Shift_JIS MIME decoder and IMAP SEARCH, scoped PASS T305

- Добавлен строгий typed Shift_JIS decoder для WHATWG labels и IANA alias:
  однобайтовый ASCII/U+0080, half-width kana, JIS X 0208 и Windows EUDC
  private-use mapping. Malformed, truncated и unmapped пары отвергаются.
- Unit tests сравнили все 11 280 двухбайтовых pointers и все 256 одиночных
  octets. Windows Rust 1.97.1: профильные 5 тестов, strict Clippy и полный
  набор: 848 library + 1 CLI, 3 ignored.
- Linux Docker Rust 1.97: strict Clippy, 957 library + 1 CLI, 3 ignored и
  doc-tests. Независимая SMTP STARTTLS -> IMAP UTF-8 SEARCH matrix прошла для
  всех 36 charset labels, включая Python `shift_jis` японский текст, и
  отдельного RFC 2047 Subject сценария. Это не проверка приложения Litterkeep.
- Acceptance данные находятся в `./data/acceptance-mime-shiftjis-20260928`.
  Созданный контейнер и test/runtime images удалены, отсутствие проверено;
  G18 остаётся ACTIVE, EUC-KR не реализован.

### 2026-09-28: G18 ISO-2022-JP MIME state machine and IMAP SEARCH, scoped PASS T304

- Добавлен typed `ISO-2022-JP` Charset и строгий конечный автомат WHATWG:
  ASCII, Roman, Katakana и JIS X 0208. JIS пары используют pinned таблицу
  EUC-JP; malformed escape/pair sequences, redundant shifts и отсутствие
  финального ASCII reset завершаются ошибкой.
- RFC 2047 unit test проверяет reset внутри encoded-word. Независимый Linux
  SMTP STARTTLS -> IMAP UTF-8 SEARCH wire test проверил 35 MIME charset labels,
  японский ISO-2022-JP body и encoded-word Subject `日本語件名` отдельным SEARCH
  SUBJECT. Это independent client, не проверка приложения Litterkeep.
- Windows Rust 1.97.1 strict Clippy/all-target tests: 843 library + 1 CLI,
  3 ignored. Linux Docker gate: 952 library + 1 CLI, 3 ignored, doc-tests.
  Профиль явно помечает Katakana как WHATWG compatibility ветку, которую
  RFC 1468 исключает из собственного mail profile.
- Acceptance данные сохранены в `./data/acceptance-mime-iso2022jp-20260928`
  и `./data/acceptance-mime-iso2022jp-final-20260928`. Созданные контейнеры и
  test/runtime images удалены и их отсутствие проверено. G18 остаётся ACTIVE.

### 2026-09-28: G18 EUC-JP MIME decoder and IMAP SEARCH, scoped PASS T303

- Добавлен строгий EUC-JP decoder с типизированными WHATWG labels и четырьмя
  wire-формами: ASCII, SS2 half-width kana, JIS X 0208 и SS3/JIS X 0212.
  Malformed, truncated и незаданные index mappings завершаются ошибкой.
- Две таблицы собраны Python stdlib генератором с pinned SHA-256. Исчерпывающая
  проверка сверяет все 8 836 допустимых wire pointers каждой JIS ветки,
  отдельные тесты сверяют японский текст, kana, JIS 0212 и отказные варианты.
- Windows Rust 1.97.1 fmt, strict Clippy и all-target tests прошли (839 library
  + 1 CLI, 3 ignored). Linux Rust 1.97 Docker gate прошёл strict Clippy, 948
  library + 1 CLI, 3 ignored, doc-tests. Независимый Python 3.14 SMTP STARTTLS
  -> IMAP UTF-8 SEARCH прошёл для всех 34 labels, включая EUC-JP `日本語かな`.
- Исходные индексы оставлены в `./data/g18-eucjp-table-20260928`, acceptance
  fixture в `./data/acceptance-mime-eucjp-20260928`. Созданный контейнер и оба
  Docker image удалены и их отсутствие проверено. Litterkeep app,
  ISO-2022-JP, Shift_JIS и EUC-KR не проверялись; G18 остаётся ACTIVE.

### 2026-09-28: G18 Big5/Big5-HKSCS MIME decoder and IMAP SEARCH, scoped PASS T302

- Добавлен строгий собственный Big5 decoder с типизированным Charset и WHATWG
  labels, O(1) generated index lookup и fail-closed ошибками malformed,
  truncated и unmapped sequences. Четыре normative pointer exceptions дают
  ровно две Unicode code points каждая.
- Rust тесты проверяют структуру index asset и сверяют все 19 782 pointer
  positions с результатом decoder-а. Отдельно проверяются четыре expansion
  mappings, aliases, традиционный китайский текст и некорректные последовательности.
- Windows Rust 1.97.1 targeted 6 tests, strict Clippy и all-target tests прошли
  (833 library + 1 CLI, 3 ignored). Linux Rust 1.97 Docker gate прошёл strict
  Clippy, 942 library + 1 CLI, 3 ignored, doc-tests. Независимый Python 3.14
  SMTP STARTTLS -> IMAP UTF-8 SEARCH прошёл для всех 33 MIME labels, включая
  Big5 `繁體中文` и Big5-HKSCS `Ê̄Ê̌ê̄ê̌`.
- Индекс оставлен в `./data/g18-big5-table-20260928`, acceptance fixture в
  `./data/acceptance-mime-big5-20260928`. Созданные контейнер и оба образа
  удалены и проверены. Litterkeep app, EUC-JP, ISO-2022-JP, Shift_JIS и EUC-KR
  не проверялись; G18 остаётся ACTIVE.

### 2026-09-28: G18 GBK/GB18030 MIME decoder and IMAP SEARCH, scoped PASS T301

- Добавлены типизированные labels GBK и GB18030. Собственный строгий decoder
  обрабатывает ASCII, WHATWG euro mapping, двухбайтовые CJK sequences и
  четырёхбайтовые GB18030 scalars; malformed, unmapped и truncated sequences
  возвращают ошибку. Runtime dependencies не изменились.
- Таблицы сгенерированы отдельным Python stdlib tool из WHATWG Encoding index
  файлов с проверкой SHA-256 источников. В тестах перебраны все 23 940
  двухбайтовых указателей и граничные точки four-byte ranges.
- Windows Rust 1.97.1 fmt, strict Clippy и all-target tests прошли (827 library
  + 1 CLI, 3 ignored). Linux Rust 1.97 Docker strict Clippy, 936 library + 1
  CLI, 3 ignored, и doc-tests прошли. Независимая Linux Python SMTP STARTTLS ->
  IMAP UTF-8 SEARCH acceptance прошла для всех 31 MIME charset labels, включая
  GBK `简体中文` и GB18030 `中文𠀀😀`.
- Источники индексов сохранены в `./data/g18-gbk-table-20260928`, acceptance
  fixtures в `./data/acceptance-mime-gbk-20260928` и чистом повторном прогоне
  `./data/acceptance-mime-gbk-final-20260928`. Оба созданных Docker container
  и пары образов удалены и проверены. Big5/EUC-JP/ISO-2022-JP/Shift_JIS/EUC-KR и приложение
  Litterkeep не проверялись; G18 остаётся ACTIVE.

### 2026-09-28: G18 MIME charset inventory reconciled with wire matrix, T300

- Сверила реализацию, `imap-profile.md`, wire matrix и актуальный IANA registry:
  ISO-8859-11 уже принимается как alias TIS-620, но отсутствовал в Search
  allowlist документа. Запись добавлена, число независимых single-byte wire
  кейсов уточнено с 28 до 29 по результату T299.
- Явно перечислены пока неподдерживаемые распространённые multibyte семейства:
  GBK/GB18030, Big5, EUC-JP, ISO-2022-JP, Shift_JIS и EUC-KR. Их упоминание
  фиксирует пробел, а не заявляет реализацию или acceptance.
- Только документационная корректировка; Rust source и runtime не менялись.
  G18 остаётся ACTIVE, полноценная поддержка multibyte charset отсутствует.

### 2026-09-28: G18 ISO-8859-11 MIME SEARCH wire coverage, scoped PASS T299

- Существующий `ISO-8859-11` alias декодера TIS-620 был покрыт unit-тестом,
  но отсутствовал как отдельный label в сетевой проверке MIME. В независимую
  Linux SMTP STARTTLS -> IMAP UTF-8 SEARCH матрицу добавлен codec `iso8859_11`.
- Все 29 однобайтовых charset labels теперь проходят сетевую матрицу, включая
  раздельные `TIS-620` и `ISO-8859-11`. Python 3.14 `py_compile` прошёл.
  Rust source не менялся, поэтому Rust suite повторно не запускался.
- Fixture `./data/acceptance-mime-iso8859-11-20260928` сохранена. Созданный
  Linux runtime image и контейнер удалены и их отсутствие проверено.
- Это расширение wire evidence только для MIME SEARCH: прочие MIME поля,
  charset labels и совместимость с Litterkeep app не подтверждены. G18 ACTIVE.

### 2026-09-28: G18 RFC 2319 KOI8-U MIME charset, scoped PASS T298

- MIME `Charset` поддерживает зарегистрированные `KOI8-U` и alias `csKOI8U`.
  Декодер за один проход переиспользует KOI8-R таблицу и применяет восемь
  украинских позиций RFC 2319; unit-тест сверяет все 256 octets.
- Независимый Linux Python SMTP STARTTLS -> IMAP UTF-8 SEARCH acceptance прошёл
  для 28 однобайтовых charset, включая украинский `Їжак` через встроенный
  `koi8_u` codec. Проверка не использовала Litterkeep app.
- Windows Rust 1.97.1 fmt, strict Clippy и all-target tests прошли (821 library
  + 1 CLI, 3 ignored). Linux Rust 1.97 Docker strict Clippy, 930 library + 1
  CLI, 3 ignored, и doc-tests прошли. Контейнер и оба созданных image удалены;
  fixture `./data/acceptance-mime-koi8-u-20260928` оставлена.
- Проверен этот MIME charset/Search путь, не полный набор charset и не остальные
  RFC profile gaps. Совместимость с приложением Litterkeep не проверялась; G18
  остаётся ACTIVE.

### 2026-09-28: G18 RFC 1489 KOI8-R MIME charset, scoped PASS T297

- MIME `Charset` принимает зарегистрированные IANA имя `KOI8-R` и alias
  `csKOI8R`; однопроходная таблица реализует все 256 octets по RFC 1489.
- Rust unit oracle сравнивает полную таблицу. Независимый Linux SMTP -> IMAP
  acceptance отправил сообщение с русским текстом `charset=KOI8-R`, и IMAP
  SEARCH по UTF-8 нашёл его через общий MIME decoder; проверены все 27
  single-byte body charset в матричном сценарии.
- Windows Rust 1.97.1 fmt, strict Clippy и all-target tests прошли (819 library
  + 1 CLI, 3 ignored). Linux Rust 1.97 Docker strict Clippy, 928 library + 1
  CLI, 3 ignored, и doc-tests прошли. Свежие test/runtime images и контейнер
  удалены; fixture `./data/acceptance-mime-koi8-r-20260928` оставлен.
- Сценарий проверяет MIME charset/IMAP SEARCH только для этого набора и не
  закрывает оставшиеся charset, остальные RFC profile gaps или Litterkeep app
  interoperability. G18 остаётся ACTIVE.

### 2026-09-28: G18 RFC 2045 quoted-printable illegal literal octets, scoped PASS T296

- `quoted-printable` decoder теперь отклоняет незакодированные C0 controls,
  кроме TAB, DEL и high-bit octets; допустимый printable ASCII, TAB, CRLF,
  `=HH` bytes и существующий tolerant malformed-escape recovery сохранены.
- IMAP BINARY projection сохраняет typed `UNKNOWN-CTE` при ошибке декодирования.
  Независимый TLS IMAP peer APPEND-ит письмо с raw NUL/VT/0xFF, проверяет
  `NO [UNKNOWN-CTE]`, успешный NOOP, точные неизменённые bytes через
  `BODY.PEEK[]` и отсутствие `\\Seen`.
- Windows Rust 1.97.1 fmt, strict Clippy, all-target tests и Linux Docker Rust
  1.97 strict Clippy/test/doc-tests пройдены; точные итоги внесены в T296.
  Это malformed MIME APPEND/FETCH acceptance независимым peer, не SMTP
  malformed delivery и не проверка совместимости Litterkeep приложения.
- Fixture хранится в `./data/acceptance-mime-qp-illegal-raw-20260928`.
  Созданные для этапа runtime container и Docker images удалены после проверки;
  G18 остаётся ACTIVE.

### 2026-09-28: G18 POP3 UIDL wire stability across server restart, scoped PASS T295

- Independent Python STARTTLS acceptance delivered two messages through SMTP,
  obtained their POP3 UIDLs and closed without `QUIT`. The Linux server
  container was restarted against the same `./data/acceptance-pop3-uidl-20260928`
  bind mount; the second phase verified byte-derived UIDL values were unchanged.
- The peer then committed `DELE`/`QUIT` for the first entry, delivered a third
  message over SMTP and verified POP3 retained the second message's ID while
  assigning a distinct ID to the new message. Both acceptance phases and Python
  3.14 syntax compilation passed. Runtime used the current Rust release layer
  previously validated by T294; this checkpoint does not claim a fresh Rust
  suite run or Litterkeep app interoperability.
- The created container and runtime image were removed and absence verified.
  The per-stage fixture and its generated UIDL state remain in `./data` for
  reproducibility; unrelated resources were untouched. G18 remains ACTIVE.

### 2026-09-28: G18 RFC 1939 durable UIDL stability, scoped PASS T294

- POP3 UIDL uses the Store mailbox-entry UID. Added a file-backed regression
  that reads the exact multiline UIDL bytes, closes a session without UPDATE,
  reopens the Store, and verifies that a new session still sees the same ID.
  After EXPUNGE, a later append receives a distinct UID and the remaining
  message keeps its previous ID.
- Windows Rust 1.97.1 `cargo fmt --check`, strict Clippy and all-target tests
  passed (815 library + 1 CLI, 3 ignored). Fresh Linux Docker test target passed
  strict Clippy, 924 library + 1 CLI tests (3 ignored), and doc-tests.
- The created image `litterkeep-linux-test:g18-pop3-uidl-20260928` was removed
  and its absence verified; no container was created. Existing images and
  unrelated containers were left untouched. This is file-backed unit evidence,
  not independent POP3 wire or Litterkeep app interoperability. G18 remains
  ACTIVE.

### 2026-09-28: G18 stale SMTPUTF8 acceptance reconciled, scoped PASS T293

- Current source advertises `8BITMIME` and `SMTPUTF8` in each ESMTP EHLO
  response variant. T195 already supplies independent STARTTLS acceptance for
  capability advertisement, the required MAIL flag, U-label routing to the
  allowlisted local A-label, RSET state reset, and byte-identical IMAP
  attachment retrieval.
- The older T84 row still incorrectly claimed SMTPUTF8 was absent and U-label
  domains unsupported despite T204 having documented the superseding evidence.
  T84 is now explicitly `SUPERSEDED`; protocol-coverage text points to T195.
- This is a status/documentation reconciliation based on current source and
  recorded T195 evidence, not a new wire run. External DNS/MX delivery, broad
  RFC 6532 structured-field grammar and Litterkeep app compatibility remain
  unverified; G18 stays ACTIVE.

### 2026-09-28: G18 RFC 5322 obsolete quoted-pair in IMAP ENVELOPE, scoped PASS T292

- Follow-up parser regression covers all representative `obs-NO-WS-CTL` byte
  ranges (1-8, 11-12, 14-31, 127), accepts NUL/CR/LF when protected by
  obsolete quoted-pair, and rejects those three octets unquoted. ENVELOPE
  projection checks exact literal framing and octet count for each of NUL, CR
  and LF. Focused unit test plus Windows 1.97.1 fmt, all-target tests (814 library + 1 CLI, 3
  ignored), and strict Clippy passed. A fresh Linux Docker test stage also
  passed strict Clippy, 923 library + 1 CLI tests (3 ignored), and doc-tests.

- Audit correction: the prior independent wire test covered an address without
  comments only. A parser regression is added for a trailing comment plus an
  escaped domain-literal closer. Wire interoperability for this combined form
  remains unverified, so T292 remains scoped to the isolated wire case.

- RFC 5322 §4.1 obs-qp includes NUL, CR, LF and ASCII control bytes, so
  domain-literal parsing now accepts those octets when protected by a quoted
  pair. Shared IMAP nstring encoding uses a synchronizing literal when data
  contains NUL/CR/LF instead of emitting an invalid quoted-string.
- An independent Python imaplib STARTTLS client APPENDed a message containing
  an escaped NUL in the From domain-literal and successfully fetched ENVELOPE;
  the response carried the 11-octet semantic host as a parsed literal, without
  the semantically invisible quoting backslash. Windows
  Rust 1.97.1 format, all-target tests and strict Clippy passed (814 library +
  1 CLI, 3 ignored). Fresh Linux Rust 1.97 Docker test stage passed strict
  Clippy, 923 library + 1 CLI tests (3 ignored), and doc-tests; the stage's
  runtime container and both created images were removed and absence verified.
- This verifies one obsolete quoted-pair and IMAP string path, not all
  obs-qp/control combinations or full RFC 5322/9051 interoperability; G18
  remains ACTIVE.
- Correction to the initial T292 evidence: its first assertion incorrectly
  counted the quoted-pair backslash as address data. RFC 5322 defines that
  backslash as semantically invisible; the regression now checks decoded data.

### 2026-09-28: G18 RFC 5322/6532 domain-literal dtext validation, scoped PASS T291

- Domain-literal parsing previously preserved arbitrary characters, including
  NUL, despite RFC 5322's `dtext` ranges. Validation now accepts RFC 5322
  visible dtext, whitespace and obsolete control/quoted-pair forms, accepts
  UTF-8 non-ASCII dtext from RFC 6532, and rejects invalid ASCII controls.
- Regression covers a UTF-8 literal (`nœde`) and rejects NUL. Windows Rust
  1.97.1 formatting, strict Clippy and all-target tests passed (812 library +
  1 CLI, 3 ignored). Fresh Linux Rust 1.97 Docker test stage passed strict
  Clippy, 921 library + 1 CLI tests (3 ignored), and doc-tests; image removed
  and absence verified.
- Evidence is parser/unit-only, not complete domain-literal conformance or
  independent IMAP wire/app interoperability; G18 remains ACTIVE.

### 2026-09-28: G18 RFC 5322 escaped domain-literal closer, scoped PASS T290

- `normalize_dot_cfws` tracked quoted-pairs inside quoted local-parts but not
  inside obsolete domain-literals. An escaped `]` therefore ended literal mode
  early and whitespace around a later dot could be rewritten as obsolete CFWS.
  The normalizer now preserves escape state in bracketed literals, then removes
  the quoting backslash during semantic projection.
- A regression verifies decoded `]` and exact following ` . ` dtext, without
  preserving the semantically invisible backslash.
- Follow-up audit found that the initial assertion exercised only the
  no-comment path. A new parser regression also covers a trailing comment;
  both forms assert that the escape protects `]` during scanning and the final
  semantic projection removes its quoting backslash. The new assertion still
  was verified by the focused unit test and full Linux/Windows test gates below.
  Windows Rust 1.97.1 formatting, strict Clippy and all-target tests passed
  (812 library + 1 CLI, 3 ignored). Fresh Linux Rust 1.97 Docker test stage
  passed strict Clippy, 921 library + 1 CLI tests (3 ignored), and doc-tests.
  The created image was removed and absence verified.
- This is focused parser/unit evidence for obsolete domain-literal escaping,
  not full domain-literal syntax or independent wire/app interoperability; G18
  remains ACTIVE.

### 2026-09-28: G18 RFC 5322 dtext inside domain-literal, scoped PASS T289

- The shared address scanner still treated parentheses inside square-bracket
  domain-literals as comment nesting. RFC 5322 §3.4.1 allows these bytes as
  `dtext`, so a lone parenthesis incorrectly made a valid literal appear
  unterminated. Scanner state now treats the complete bracketed body as data;
  it also keeps obsolete quoted-pair escapes from closing the literal early.
- Unit regressions verify balanced and unbalanced parentheses, an escaped `]`,
  and existing `@`/`>` cases through ENVELOPE projection. Windows Rust 1.97.1
  formatting, strict Clippy and all-target tests passed (812 library + 1 CLI,
  3 ignored). Fresh Linux Rust 1.97 Docker test stage passed strict Clippy,
  921 library + 1 CLI tests (3 ignored), and doc-tests. The image was removed
  and absence verified.
- This remains parser/unit evidence, not full dtext conformance or independent
  IMAP wire/app interoperability. G18 stays ACTIVE.

### 2026-09-28: G18 RFC 5322 domain-literal comment data, scoped PASS T288

- `strip_comments` previously treated parentheses inside `[...]` as comment
  syntax and removed their contents from domain-literal data. It now tracks the
  bracketed literal before comment delimiters, preserves its contents, handles
  quoted-pair boundaries, and rejects malformed unmatched/nested brackets.
- The domain-literal unit fixture verifies preservation of parentheses as well
  as `@` and `>`, and rejection of an unmatched `]`. Windows Rust 1.97.1
  formatting, strict Clippy and all-target tests passed (812 library + 1 CLI,
  3 ignored). Fresh Linux Rust 1.97 Docker test stage passed strict Clippy,
  921 library + 1 CLI tests (3 ignored), and doc-tests. The test image was
  removed and absence verified.
- This is parser/unit evidence only. It does not establish complete dtext,
  obsolete domain-literal, independent IMAP wire or app interoperability; G18
  remains ACTIVE.

### 2026-09-28: G18 RFC 5322 domain-literal separators in angle-addr, scoped PASS T287

- The address scanner recognized square-bracket domain literals only outside
  angle addresses. As a result, `@` within dtext could be mistaken for another
  addr-spec separator and `>` could be mistaken for the end of angle-addr.
  Scanner state now protects these delimiters while inside a domain literal.
- Unit coverage exercises both delimiters through address parsing and IMAP
  ENVELOPE projection. Windows Rust 1.97.1 format check, strict Clippy and
  all-target tests passed (812 library + 1 CLI, 3 ignored). Fresh Linux Rust
  1.97 Docker test stage passed strict Clippy, 921 library + 1 CLI tests
  (3 ignored), and doc-tests. The created image was removed and absence verified.
- Evidence is parser/unit-only, not an independent IMAP wire or app test. Other
  domain-literal syntax and broader RFC 5322 forms remain incomplete; G18 stays
  ACTIVE.

### 2026-09-28: G18 RFC 5322 quoted display-name validation, scoped PASS T286

- Quoted display-name decoding removed valid quoted-pairs but previously let an
  unescaped inner DQUOTE pass through as ordinary name data. The decoder now
  rejects an unescaped quote while preserving escaped quotes; unit regressions
  cover rejection and the existing escaped-quote acceptance.
- Windows Rust 1.97.1 formatting, strict Clippy and all-target tests passed
  (811 library + 1 CLI, 3 ignored). Fresh Linux Rust 1.97 Docker test stage
  passed strict Clippy, 920 library + 1 CLI tests (3 ignored), and doc-tests.
  The created test image was removed and absence verified.
- This is quoted-string parser/unit evidence only, not independent IMAP wire or
  app interoperability. Other RFC 5322 phrase forms remain incomplete and G18
  stays ACTIVE.

### 2026-09-28: G18 RFC 5322 obsolete route element validation, scoped PASS T285

- RFC 5322 §4.4 `obs-domain-list` permits empty comma positions after its
  required `@domain`, but each non-empty optional element must also be `@domain`.
  Route parsing previously ignored later non-empty elements without `@` and
  could accept malformed route text. Validation now checks each element while
  retaining the RFC's optional empty positions; route handling has its own
  module to keep source files within the 300-line limit.
- Windows Rust 1.97.1 format check, strict Clippy and all-target tests passed
  (810 library + 1 CLI, 3 ignored). Fresh Linux Rust 1.97 Docker test stage
  passed strict Clippy, 919 library + 1 CLI tests (3 ignored), and doc-tests.
  The created test image was removed and absence verified.
- Evidence is unit/parser-only, with no independent wire client or app
  interoperability claim. Broader RFC 5322 forms remain incomplete and G18
  stays ACTIVE.

### 2026-09-28: G18 RFC 5322 angle-addr suffix validation, scoped PASS T284

- `parse_mailbox` previously extracted the substring inside `<...>` and silently
  discarded all text after `>`. It now accepts only trailing CFWS/comments and
  rejects arbitrary text or a second angle address. Unit regression covers both
  valid trailing comment and malformed suffixes.
- Windows Rust 1.97.1 format check, strict Clippy, and all-target tests passed
  (810 library + 1 CLI, 3 ignored). Fresh Linux Rust 1.97 Docker test stage
  passed strict Clippy, 919 library + 1 CLI tests (3 ignored), and doc-tests.
  The created test image was removed and absence verified.
- Evidence is parser/unit-only for this suffix boundary; no independent IMAP
  wire client or Litterkeep app scenario was run. Broader RFC 5322 forms remain
  incomplete, and G18 stays ACTIVE.

### 2026-09-28: G18 Preserve RFC 2231 language on Content-Disposition, scoped PASS T283

- The shared RFC 2231 parameter parser retained decoded values and BCP 47
  language hints, but `Disposition::parse` copied only the decoded parameter
  map. A language-qualified attachment filename therefore lost its language
  metadata between header parsing and the MIME entity model.
- `Disposition` now owns a typed `parameter_languages` map alongside decoded
  parameters. Regression tests cover direct parsing and full MIME-tree
  construction. The IMAP BODYSTRUCTURE encoder deliberately keeps its existing
  RFC wire shape; the language hint remains parser metadata, not an extra
  unstandardized BODYSTRUCTURE field.
- Windows Rust 1.97.1 format check, strict Clippy and all-target tests passed
  (809 library + 1 CLI, 3 ignored). A fresh Linux Rust 1.97 Docker test stage
  passed strict Clippy and 918 library + 1 CLI tests, 3 ignored, plus doc-tests.
  The created test image was removed and absence verified. This closes the
  disposition metadata loss only; registry validation and wider MIME profiles
  remain incomplete, and G18 stays ACTIVE.

### 2026-09-28: G18 RFC 2231 language-tagged filename continuation, scoped PASS T282

- RFC 2231 extended-parameter parser tests already covered segmented filename
  decoding and BCP 47 language metadata, while prior SMTP-to-IMAP acceptance
  covered other filename encodings. The language-tagged continuation had no
  end-to-end SMTP delivery and IMAP BODYSTRUCTURE evidence.
- A new independent STARTTLS peer delivered a MIME attachment from Alice to Bob
  with `filename*0*` carrying `UTF-8'en-US'` and `filename*1*` carrying the
  remaining percent-encoded name. IMAP SEARCH found the unique message;
  BODYSTRUCTURE returned the joined Unicode filename while RFC822.HEADER kept
  both original extended parameter segments and the language tag byte-exact.
- Python compilation and the Linux wire acceptance passed. The Linux Rust
  1.97 release runtime image reused the existing Rust base and cached source
  build; Rust sources were unchanged, so no new Linux Rust suite result is
  claimed. Windows Rust 1.97.1 `cargo fmt --check`, strict Clippy and all-target
  tests passed (807 library + 1 CLI, 3 ignored). TLS was negotiated with peer
  certificate verification disabled.
  Fixture `./data/acceptance-rfc2231-filename-20260928` is retained. The
  created container and runtime image were removed and absence verified. This
  is an independent peer check, not Litterkeep-app interoperability or full
  RFC 2231 charset/language registry coverage; G18 remains ACTIVE.

### 2026-09-28: G18 RFC 2047 UTF-16/UTF-32 IMAP SEARCH wire acceptance, scoped PASS T281

- MIME decoder unit coverage already included UTF-16/UTF-32 encoded-words, but
  the SMTP -> persistent message -> IMAP SEARCH path had no independent wire
  evidence for these encodings.
- A new independent STARTTLS peer sent six unique Subject fixtures through
  SMTP: generic, BE and LE variants for UTF-16 and UTF-32. Bob then searched
  each decoded Unicode value using RFC 9051 UTF-8 synchronizing literals.
  Acceptance requires exactly one matching message for every charset. The
  generic UTF-32 vector includes its registered big-endian BOM.
- Python compilation and the Linux wire acceptance passed. The Linux Rust
  1.97 release runtime image used the existing Docker Rust image and cached
  source build. TLS was negotiated, but the independent client deliberately
  disabled certificate validation; this does not verify client trust setup.
  Fixture ./data/acceptance-rfc2047-utf16-utf32-20260928 is retained.
  The created server container and runtime image are removed and absence
  verified. This is not Litterkeep-app interoperability or complete charset
  coverage; G18 remains ACTIVE.

### 2026-09-28: G18 RFC 3516 BINARY Seen/FLAGS response, scoped PASS T280

- RFC 3516 §4.2 distinguishes BINARY from BINARY.PEEK: the former implicitly
  sets Seen, while PEEK does not. The T279 completion path already shares
  durable Seen and updated-FLAGS behavior with BINARY. This checkpoint adds
  independent wire evidence for that path rather than changing Rust logic.
- Linux STARTTLS acceptance used a fresh mailbox and Base64 MIME leaf. It
  verified BINARY.PEEK omits FLAGS, BINARY returns decoded octets `01 02 62`
  and FLAGS containing `\Seen`, and repeated BINARY without a new transition
  omits the implicit FLAGS item. Python `py_compile` passed. Rust sources are
  unchanged from the T279 Linux gate: strict Clippy and 916 library + 1 CLI
  tests passed, 3 ignored. Runtime image reused cached release compilation.
- Fixture `./data/acceptance-imap-fetch-seen-flags-20260928` is retained.
  The created container and image are removed and absence verified. Acceptance
  used an independent peer, not Litterkeep app; G18 remains ACTIVE.

### 2026-09-28: G18 RFC 9051 FETCH FLAGS after Seen transition, scoped PASS T279

- RFC 9051 §6.4.5 says a FETCH response SHOULD include FLAGS if reading the
  body changes message flags. The server durably transitioned `\Seen` but
  omitted the updated FLAGS unless the client had asked for them. Completion
  now includes the updated item only for messages changed to Seen and avoids
  duplicates when FLAGS was explicitly requested.
- Two Rust unit tests verify implicit insertion and duplicate suppression.
  Independent Linux STARTTLS acceptance confirms BODY.PEEK does not receive
  implicit FLAGS, the first BODY returns FLAGS with `\Seen`, and repeated BODY
  without a new transition omits them. Fresh Linux Rust 1.97.1 Docker strict
  Clippy and tests passed (916 library + 1 CLI, 3 ignored). Windows fmt,
  strict Clippy and all-target tests passed (807 library + 1 CLI, 3 ignored).
- Fixture `./data/acceptance-imap-fetch-seen-flags-20260928` is retained.
  Created runtime container and test image are removed and absence verified.
  Acceptance used an independent peer, not Litterkeep app; G18 remains ACTIVE.

### 2026-09-28: G18 RFC 9051 UID STORE implicit UID item, scoped PASS T278

- RFC 9051 requires UID in FETCH updates caused by UID STORE. The existing
  completion path already included UID, but previous black-box coverage only
  checked the persisted flags with a later FETCH. A new independent Linux
  STARTTLS scenario APPENDs two messages, removes the first by UID, then
  applies UID STORE to the second. It verifies the immediate response carries
  UID 2 exactly once while the response number remains sequence number 1.
- Python `py_compile` and the wire acceptance passed. Rust 1.97.1 Windows
  `cargo fmt --check`, strict Clippy and all-target tests passed (807 library
  + 1 CLI, 3 ignored). The Linux runtime image reused cached release
  compilation because no Rust source changed. This is independent protocol
  peer evidence, not Litterkeep app interoperability or full IMAP4rev2 proof.
- Fixture `./data/acceptance-imap-uid-store-implicit-20260928` is retained.
  The created container and runtime image are removed and absence verified;
  G18 remains ACTIVE.

### 2026-09-28: G18 RFC 9051 UID FETCH implicit UID item, scoped PASS T277

- RFC 9051 §6.4.8 requires UID in every FETCH response caused by UID FETCH,
  regardless of the requested data items. The production serializer already
  complied; this checkpoint adds regression evidence rather than changing
  protocol behavior: UID FETCH inserts a missing UID, suppresses duplicates,
  and ordinary FETCH does not inject an unrequested UID.
- An independent Linux STARTTLS peer appended one message, compared ordinary
  FETCH with `UID FETCH (FLAGS RFC822.SIZE)`, and verified exactly one matching
  UID in the latter response. Rust 1.97.1 Windows fmt, strict Clippy and all
  targets passed (807 library + 1 CLI, 3 ignored). A fresh Linux Docker test
  build after the test-source change passed strict Clippy and 914 library + 1
  CLI tests, 3 ignored. Python `py_compile` and `git diff --check` passed.
- Fixture `./data/acceptance-imap-uid-fetch-implicit-20260928` is retained.
  Created runtime container and both test images were removed and absence
  verified. Acceptance used an independent protocol peer, not Litterkeep app;
  broader IMAP4rev2 remains PARTIAL and G18 remains ACTIVE.

### 2026-09-27: G18 RFC 9051 SELECT FLAGS existing keywords, scoped PASS T270

- The selected-mailbox `FLAGS` response now unions system flags with the
  persistent keyword atoms present in the same Store snapshot. Unit coverage
  verifies normalization and deduplication. An independent Linux STARTTLS peer
  APPENDs a unique keyword and confirms it after both SELECT and EXAMINE.
- Rust 1.97.1 Windows tests passed (807 library + 1 CLI, 3 ignored), together
  with format and strict Clippy. The Linux release wire scenario passed. This
  closes only existing keyword reporting for SELECT/EXAMINE, not full IMAP4rev2
  flag semantics. Temporary Docker image and container are removed.

### 2026-09-27: G18 RFC 9051 EXAMINE PERMANENTFLAGS, scoped PASS T269

- EXAMINE uses the shared selection response path but is read-only. It now
  returns `PERMANENTFLAGS ()` instead of advertising permanent system flags and
  keyword creation. SELECT remains read-write and retains its existing allowed
  flags. An independent Linux STARTTLS peer checks both branches, including
  `[READ-ONLY]`, LIST attributes and the existing reselect/failure behavior.
- Rust 1.97.1 Windows tests passed (807 library + 1 CLI, 3 ignored), along with
  fmt and strict Clippy. A Linux release runtime wire acceptance passed. This
  is a scoped EXAMINE permission response fix, not full IMAP4rev2 conformance.
  Its temporary container and image are removed.

### 2026-09-27: G18 RFC 9051 SELECT LIST special-use attribute, scoped PASS T268

- The mandatory SELECT/EXAMINE LIST response now serializes the folder's typed
  `SpecialUse` metadata, using the existing wire attribute mapping. It does not
  infer special use from localized or user-controlled names. Independent Linux
  STARTTLS acceptance verifies `\\Archive` on SELECT Archive, in addition to
  the required LIST name, ordinary SELECT, EXAMINE READ-ONLY and failed-select
  boundaries.
- Rust 1.97.1 Windows full tests passed (807 library + 1 CLI, 3 ignored), as did
  fmt and strict Clippy. The current Linux release runtime wire scenario passed.
  This is scoped attribute coverage; other base LIST attributes and full
  IMAP4rev2 remain unclaimed. The temporary image and container are removed.

### 2026-09-27: G18 RFC 9051 successful SELECT LIST response, scoped PASS T267

- RFC 9051 §6.3.2 requires successful SELECT/EXAMINE to include an untagged
  LIST response naming the selected mailbox. The completion now carries the
  canonical mailbox display name from the Store-owned catalog and emits that
  response before FLAGS/EXISTS. Catalog lookup avoids cloning the entire folder
  list. A Store test checks renamed Unicode names; independent Linux STARTTLS
  acceptance checks SELECT INBOX, SELECT for a unique user-created mailbox,
  and EXAMINE INBOX including its READ-ONLY result.
- Windows Rust 1.97.1 full tests passed (807 library + 1 CLI, 3 ignored), as did
  format, strict Clippy, and the Linux release runtime wire scenario. This is
  scoped RFC response coverage only, not full IMAP4rev2 or Litterkeep-app
  interoperability. The temporary stage image/container are removed after
  verification; fixture data remains under `./data`.

### 2026-09-27: G18 IMAP implicit mailbox close on SELECT, scoped PASS T266

- SELECT/EXAMINE now close any previous mailbox before starting the new Store
  selection. The session clears its selected projection and emits untagged
  `OK [CLOSED]` before new-mailbox responses. A failed selection therefore
  leaves authenticated state with no mailbox selected and does not expunge
  `\\Deleted` messages.
- Independent Linux STARTTLS acceptance verifies CLOSED ordering for repeated
  SELECT, CLOSED before a nonexistent-mailbox NO response, rejection of FETCH
  after that failure, and NOOP recovery. Windows Rust 1.97.1 fmt, strict
  Clippy and full tests passed (807 library, 1 CLI, 3 ignored). Linux Docker
  Rust 1.97.1 tests passed (906 library, 1 CLI, 3 ignored); fmt, strict Clippy
  and wire acceptance passed. Linux test artifacts used container-local
  `/tmp` after the shared Windows target produced an incremental-copy error.
  Stage runtime/gate containers were removed, the existing Rust image retained,
  and no image created. Fixture remains in
  `./data/acceptance-imap-select-reselection-20260927`.

### 2026-09-27: G18 IMAP ENABLE atom validation, scoped PASS T265

- ENABLE now validates each unquoted capability token as an IMAP atom before
  mapping it to the capability enum. Invalid atom-special characters produce
  a syntax error; valid but unknown capability atoms remain typed as
  `Unsupported` and are ignored as required. ENABLE and UTF8=ACCEPT coverage
  remain partial; this is not an IMAP4rev2 claim.
- Unit tests distinguish malformed atoms from valid unknown names.
  Independent Linux STARTTLS grammar acceptance verifies tagged BAD for
  `ENABLE X%`, an empty ENABLED response and tagged OK for `ENABLE X-UNKNOWN`,
  then completes the broader grammar scenario.
- Windows Rust 1.97.1 fmt, strict Clippy and full tests passed (807 library,
  1 CLI, 3 ignored). Linux Docker Rust 1.97.1 tests passed (906 library,
  1 CLI, 3 ignored); rustfmt, strict Clippy and the STARTTLS wire scenario
  passed. Stage containers were removed; the existing Rust image was retained
  and no image was created. Fixture remains under
  `./data/acceptance-imap-enable-atom-20260927`.

### 2026-09-27: G18 IMAP FETCH sequence-set framing, scoped PASS T264

- FETCH now consumes exactly the next token for its mandatory sequence-set;
  it no longer searches past list delimiters. This rejects the malformed wire
  form `FETCH (1 FLAGS` instead of accepting `1` after discarding the opening
  parenthesis. The change is limited to this FETCH grammar boundary and does not
  claim IMAP4rev2 conformance.
- The parser regression rejects the malformed form while existing FETCH/UID
  FETCH cases remain covered. Independent Linux STARTTLS grammar acceptance
  passed and verified NOOP recovery. Its two stale APPEND expectations were
  corrected to account for the legal literal continuation and the established
  `NO APPEND cancelled` result for an empty literal.
- Windows Rust 1.97.1 fmt, strict Clippy, and full tests passed (806 library,
  1 CLI, 3 ignored). Linux Docker Rust 1.97.1 tests passed (905 library,
  1 CLI, 3 ignored); strict Clippy and fmt passed after installing those
  toolchain components in a disposable container. The STARTTLS wire test and
  Python compile passed. Stage containers were removed; the existing Rust
  image was retained and no image was created. Fixture remains under
  `./data/acceptance-imap-fetch-grammar-20260927`.

### 2026-09-27: G18 ManageSieve SCRAM-SHA-1 dependency audit, GAP T263

- RFC 5804 §2.1 makes SCRAM-SHA-1 mandatory, and current capability lists omit
  it. Local evidence confirms production allowlist has only rustls/argon2;
  pinned rustls keeps ring hash/HMAC adapters crate-private, while account
  persistence stores only one-way Argon2id PHC verifiers. A compliant SCRAM
  exchange therefore needs a cryptographic API and per-account SCRAM verifier
  format not available in the approved implementation surface.
- No mechanism was falsely advertised and no custom cryptography or dependency
  exception was introduced. The open gap and evidence are recorded in
  `docs/dependencies.md` and `docs/sieve-coverage.md`; work continues on other
  RFC gaps under the existing constraints.

### 2026-09-27: G18 ManageSieve PLAIN SASL challenge exchange, scoped PASS T262

- `AUTHENTICATE "PLAIN"` without initial data now emits the SASL empty-string
  challenge and owns a one-response session phase. Quoted and bounded literal
  responses are parsed; `*` cancels and returns to unauthenticated command
  mode. Valid PLAIN data continues through the existing asynchronous account
  owner and TLS policy. This does not implement SCRAM-SHA-1.
- Unit tests cover no-initial-response parsing, challenge phase, response,
  cancellation/retry, malformed response strings and bounded literal
  conversion. Independent Linux STARTTLS acceptance authenticates Bob using a
  literal continuation, checks refreshed OWNER capabilities, cancels a second
  challenge, and then reauthenticates using PLAIN initial-response.
- Windows Rust 1.97.1 fmt, strict Clippy and all-target tests passed (805
  library + 1 CLI, 3 ignored). Linux Docker Rust 1.97.1 fmt, strict Clippy,
  904 library + 1 CLI passed, 3 ignored, release build and TLS wire scenario
  passed. Stage containers removed; pre-existing Rust image retained.

### 2026-09-27: G18 ManageSieve AUTHENTICATE PLAIN literal initial response, scoped PASS T261

- RFC 5804 AUTHENTICATE initial responses may use a string literal. The
  ManageSieve reactor now bounds and collects a PLAIN literal response, checks
  its UTF-8 form and final CRLF, and routes the typed command through the
  existing TLS policy and credential owner. This does not add SCRAM-SHA-1.
- Linux receiver unit coverage verifies exact initial-response bytes. The
  independent STARTTLS listener test submits a non-synchronizing PLAIN literal
  before TLS and verifies `NO "encryption required"`, demonstrating command
  parsing without attempting credential validation over cleartext.
- Windows Rust 1.97.1 fmt, strict Clippy and all-target tests passed (802
  library + 1 CLI, 3 ignored). Linux Docker Rust 1.97.1 fmt, strict Clippy,
  900 library + 1 CLI passed, 3 ignored, release build and independent STARTTLS
  wire scenario passed. Stage containers removed; pre-existing Rust image kept.

### 2026-09-27: G18 ManageSieve final-literal line framing, scoped PASS T260

- Final ManageSieve string literals now require the RFC 5804 command-line CRLF
  after exactly the declared octets; a following pipelined command remains
  buffered until that terminator arrives. PUTSCRIPT's literal name is excluded
  from final-argument handling because its body argument follows; RENAMESCRIPT
  validates the terminator after its second name, while HAVESPACE parses its
  numeric tail and line terminator in its dedicated receiver.
- Unit regressions cover split terminator arrival, pipeline preservation,
  final argument forms and literal-name/body combinations. Independent Linux
  STARTTLS socket acceptance passed against the release binary. This is a
  protocol-framing result only, not Litterkeep interoperability.
- Windows Rust 1.97.1 fmt, strict Clippy and all-target tests passed (802
  library + 1 CLI, 3 ignored). Linux Docker Rust 1.97.1 fmt, strict Clippy,
  899 library + 1 CLI passed, 3 ignored, release build and independent wire
  scenario passed. Stage containers removed; existing Rust image preserved.

### 2026-09-27: G18 ManageSieve HAVESPACE literal-name framing, scoped PASS T259

### 2026-09-27: G18 ManageSieve HAVESPACE literal-name framing, scoped PASS T259

- HAVESPACE accepts the RFC 5804 string name as a bounded synchronizing or
  non-synchronizing literal, then requires one decimal script-size number.
  UTF-8 name validation, strict numeric syntax, tail bounds, and following
  pipeline preservation are enforced by the Linux reactor receiver.
- Linux tests cover split reads, numeric conversion, malformed/extra numeric
  arguments, and a pipelined command. Independent STARTTLS wire acceptance
  confirms literal-name parsing, the expected unauthenticated state refusal,
  and following NOOP. It does not prove successful authenticated quota logic.
- Windows Rust 1.97.1 fmt, strict Clippy and all-target tests passed (802
  library + 1 CLI, 3 ignored; reactor tests are excluded on Windows). Linux
  Docker Rust 1.97.1 fmt, strict Clippy, 898 library + 1 CLI passed, 3 ignored,
  release build and independent wire scenario passed. Temporary container
  removed; existing Rust image retained.

### 2026-09-27: G18 ManageSieve RENAMESCRIPT literal argument framing, scoped PASS T258

- RENAMESCRIPT now accepts either script name as a bounded synchronizing or
  non-synchronizing literal, independently of the other name's quoted/literal
  form. Per-argument size and UTF-8 validation are retained, and bytes after
  the final literal stay available to the next pipelined command.
- Linux receiver tests verify literal old + quoted new, two sync literals,
  quoted old + non-sync literal new, empty quoted old name, and pipeline
  preservation. Independent STARTTLS socket acceptance sends two non-sync
  literals followed by NOOP and verifies the command reaches the expected
  unauthenticated state refusal, then verifies NOOP is preserved. It does not
  prove a successful Store rename using literal names.
- Windows Rust 1.97.1 fmt, strict Clippy and all-target tests passed (802
  library + 1 CLI, 3 ignored; Linux reactor tests are excluded). Linux Docker
  Rust 1.97.1 fmt, strict Clippy, 895 library + 1 CLI passed, 3 ignored,
  release build and independent wire scenario passed. Rust image retained and
  disposable container removed after acceptance.

### 2026-09-27: G18 ManageSieve PUTSCRIPT literal-name framing, scoped PASS T257

- PUTSCRIPT now accepts its script name as a client literal. After consuming
  exactly the name octets, the same session-owned receiver parses the remaining
  source string as quoted or literal; literal source can independently use a
  synchronizing continuation or non-synchronizing marker. A pipelined command
  remains in the input buffer after the complete script source.
- Linux reactor tests verify name literal + quoted source, name literal +
  non-sync source, two sequential sync continuations, and exact source/name
  command conversion. Independent Linux wire acceptance checks a sync literal
  name with quoted body, non-sync name+body followed by NOOP, and confirms the
  parsed PUTSCRIPT is denied in the pre-auth FSM state. It does not prove a
  successful Store write for literal PUTSCRIPT. Windows Rust 1.97.1 fmt,
  strict Clippy and all-target tests passed (802 library + 1 CLI, 3 external
  fixtures ignored; reactor tests Linux-only). Linux Docker Rust 1.97.1 fmt,
  strict Clippy, 891 library + 1 CLI passed, 3 ignored, release build and wire
  scenario passed. Container removed, image retained. Literal RENAMESCRIPT
  names and remaining command string positions are still unsupported.

### 2026-09-27: G18 ManageSieve script-name literal arguments, scoped PASS T256

- The bounded RFC 5804 literal receiver now also handles final string values
  for GETSCRIPT, DELETESCRIPT and SETACTIVE. Literal bytes are converted to the
  typed command only after complete UTF-8 validation; an empty SETACTIVE literal
  maps to the protocol's disable-active-script value. Existing PUTSCRIPT and
  CHECKSCRIPT body handling is unchanged.
- Linux unit tests confirm typed command conversion. The independent
  `tests/blackbox/managesieve_noop_tag.py` checks synchronizing GETSCRIPT,
  non-synchronizing GETSCRIPT/DELETESCRIPT/SETACTIVE, zero-length deactivation,
  invalid UTF-8 handling, continuation bytes and the server's unauthenticated
  state rejection. Windows Rust 1.97.1 fmt, strict Clippy and all-target tests
  passed: 802 library + 1 CLI, 3 fixture-dependent ignored; reactor literal
  tests are Linux-only. Linux Docker Rust 1.97.1 fmt, strict Clippy, 889 library
  + 1 CLI passed, 3 Unicode fixtures ignored, release build passed. Container
  removed; Rust image preserved. Literal arguments in PUTSCRIPT/RENAMESCRIPT
  name positions and remaining string positions are still uncovered.

### 2026-09-27: G18 ManageSieve NOOP literal tag forms, scoped PASS T255

- RFC 5804 §1.2 and §4 allow command strings as quoted or literal values. The
  NOOP optional tag now accepts synchronizing and non-synchronizing client
  literals using the bounded session-owned ManageSieve receiver; script literal
  handling remains unchanged. The receiver consumes exactly the declared
  octets and leaves following pipelined commands untouched.
- Linux reactor regression verifies sync mode and pipeline boundary.
  `tests/blackbox/managesieve_noop_tag.py` verified continuation, non-sync
  payload, exact echoed TAG for both forms and invalid-UTF-8 rejection against
  the release listener.
  Windows Rust 1.97.1 fmt, strict Clippy and all-target tests passed (802 library
  + 1 CLI, 3 fixture-dependent ignored); the Linux-only receiver test is not
  compiled on Windows. Linux Docker Rust 1.97.1 fmt, strict Clippy, 888 library
  + 1 CLI passed, 3 external Unicode fixtures ignored, release build passed.
  Container removed; existing Rust image retained. Literal forms for the other
  ManageSieve string arguments remain unverified.

### 2026-09-27: G18 ManageSieve malformed quoted-string rejection, scoped PASS T254

- The RFC 5804 token reader previously returned a truncated token list after
  an unterminated quoted string or invalid escape, allowing a command prefix
  to be parsed as valid. Tokenization now returns an explicit error and the
  command is rejected as syntax-invalid; focused parser regression covers
  both failure forms.
- Independent `tests/blackbox/managesieve_noop_tag.py` verified exact syntax
  errors over the Linux release listener while preserving valid NOOP/TAG
  responses. Windows Rust 1.97.1 fmt, strict Clippy and all-target tests passed:
  802 library + 1 CLI, 3 external Unicode fixture tests ignored. Linux Docker
  Rust 1.97.1 fmt, strict Clippy, 887 library + 1 CLI passed, 3 ignored, and
  release build passed. Container removed and existing Rust image retained.
  SCRAM-SHA-1 remains an explicit gap because the project production dependency
  allowlist excludes required SHA-1/HMAC primitives.

### 2026-09-27: G18 ManageSieve RENAMESCRIPT response and active rename, scoped PASS T253

- RFC 5804 §2.11 recommends `ALREADYEXISTS` for a target-name collision and
  allows renaming an active script while it remains active. Typed Store errors
  now map only actual name collisions to `NO (ALREADYEXISTS)`; rename preserves
  the active script identity and source version.
- Store regression verifies collision atomicity, active rename and persistence
  after reopen. Independent Linux TLS acceptance
  `tests/blackbox/managesieve_rename.py` confirms exact response bytes, both
  scripts survive collision, and active designation plus source bytes survive
  rename. Windows Rust 1.97.1 fmt, strict Clippy, 801 library + 1 CLI tests
  passed, 3 fixture-dependent ignored. Linux Docker Rust 1.97.1 strict Clippy,
  886 library + 1 CLI passed, 3 ignored, release build and fmt passed; TLS
  black-box passed. No Litterkeep client interoperability claim. Temporary
  container removed; preexisting Rust image retained.

### 2026-09-27: G18 ManageSieve active-delete response code, scoped PASS T252

- RFC 5804 §2.10 requires rejecting DELETESCRIPT on the active script and
  recommends response code `ACTIVE`. Store errors now carry a typed
  active-script marker and return `NO (ACTIVE)` without changing the catalogue.
- The independent Linux TLS ManageSieve acceptance tries deletion while active,
  verifies exact `NO (ACTIVE)` and the unchanged ACTIVE listing, then disables
  and deletes the script. Windows Rust 1.97.1 strict Clippy and 800 library +
  1 CLI pass, 3 fixture-dependent ignored. Linux Docker Rust 1.97.1 strict
  Clippy, 885 library + 1 CLI pass, 3 ignored, release build and fmt pass.
  Litterkeep client interoperability is not claimed. Container removed; the
  existing Rust image is retained.

### 2026-09-27: G18 SETACTIVE missing-script code and active disable, scoped PASS T251

- RFC 5804 §2.8 recommends `NONEXISTENT` for an unknown active script and
  requires empty-name deactivation to succeed, including when none is active.
  Store missing-script I/O retains a typed source marker and the ManageSieve
  completion maps it to `NO (NONEXISTENT)` while preserving `NotFound`.
- Independent `tests/blackbox/managesieve_setactive.py` over Linux TLS verifies
  no-active empty SETACTIVE, missing-name rejection, successful activation,
  ACTIVE projection, empty-name deactivation, idempotent repeat and deletion.
  Windows Rust 1.97.1 strict Clippy and 799 library + 1 CLI pass, 3
  fixture-dependent ignored. Linux Docker Rust 1.97.1 strict Clippy, 884
  library + 1 CLI pass, 3 ignored, release build and fmt pass. No Litterkeep
  client interoperability claim; disposable container removed, existing Rust
  image retained.

### 2026-09-27: G18 ManageSieve quota response code, scoped PASS T250

- RFC 5804 §1.5 requires an imposed quota rejection to carry response code
  `QUOTA`. Store size, script-count and account-catalog quota errors now carry
  a typed source marker; mutation formatting maps only that marker to
  `NO (QUOTA)`, leaving filesystem `StorageFull` failures distinct.
- Store test reaches the seventeenth script boundary; Linux reactor formatter
  tests cover success, quota and ordinary I/O failure. Windows Rust 1.97.1:
  798 library + 1 CLI passed, 3 fixture-dependent ignored; formatting passed.
  Linux Docker Rust 1.97.1: strict Clippy, 883 library + 1 CLI passed,
  3 ignored, release build passed. Independent Linux TLS black-box uploads 16
  scripts, gets exact `NO (QUOTA) "quota exceeded"` on the 17th, and verifies
  the catalog remains unchanged. Container removed; preexisting Rust image
  retained. Litterkeep client interoperability remains unverified.

### 2026-09-27: G18 ManageSieve UNAUTHENTICATE state reset, scoped PASS T249

- Реализована рекомендованная RFC 5804 `UNAUTHENTICATE` extension: capability
  объявляется в обоих состояниях, команда разрешена только после auth и
  возвращает capabilities без `OWNER`. Сессия и отдельный account context
  reactor-а очищаются, TLS сохраняется; после reset принимается новый AUTH.
- Unit-тест проверяет FSM, command denial без owner, сохранение SASL PLAIN/TLS
  контекста и повторную authentication. `tests/blackbox/managesieve_starttls_capabilities.py`
  прошёл против Linux STARTTLS server: auth, UNAUTHENTICATE, capability refresh,
  отказ LISTSCRIPTS, explicit CAPABILITY без owner, повторный auth.
  Windows Rust 1.97.1: fmt, strict Clippy, 797 library + 1 CLI прошли;
  3 fixture-dependent tests ignored. Linux Docker Rust 1.97.1: 881 library
  + 1 CLI прошли, 3 ignored; независимый TLS black-box сценарий прошёл.
  Контейнер с `--rm` остановлен, созданный Python bytecode удалён; базовый
  образ Rust сохранён.

### 2026-09-27: G18 ManageSieve VERSION 1.0 mandatory capability, scoped PASS T248

- Общий capability builder теперь всегда рекламирует `VERSION "1.0"` вместе с
  `IMPLEMENTATION` и `SIEVE`, включая начальный и post-auth capability block.
  Исправлена также ожидаемая в существующем независимом TLS тесте строка SIEVE,
  чтобы она совпадала с фактически рекламируемой `vacation-seconds` extension.
- Unit cases проверяют VERSION после greeting и в implicit-TLS greeting.
  Независимые Linux socket tests проверяют VERSION в открытом greeting; общий
  TLS-transition peer проверяет VERSION в greeting, после STARTTLS, после
  AUTHENTICATE и при явном CAPABILITY. Windows Rust 1.97.1: fmt, strict Clippy,
  796 library + 1 CLI прошли; 3 fixture-dependent tests ignored. Linux Docker
  Rust 1.97.1: 880 library + 1 CLI прошли, 3 ignored; оба независимых
  ManageSieve socket acceptance прошли. Контейнеры одноразовые и удалены.

### 2026-09-27: G18 ManageSieve SETACTIVE required argument and deactivation, scoped PASS T247

- `SETACTIVE` теперь требует аргумент по RFC 5804 §2.8. Переданная пустая
  строка `""` преобразуется в `None`, то есть в команду отключения активного
  скрипта; отсутствующий параметр и лишние аргументы parser отвергает.
- Unit cases проверяют отсутствующее, пустое, обычное имя и лишний параметр.
  Независимый black-box сценарий также проверяет wire-отказ для команды без
  параметра; проверка отключения реально активного script через
  аутентифицированный Store path пока не выполнена. Windows Rust 1.97.1:
  fmt, strict Clippy, 796 library + 1 CLI прошли; 3 fixture-dependent tests
  ignored. Linux Docker Rust 1.97.1: 880 library + 1 CLI прошли, 3 ignored;
  independent NOOP/SETACTIVE parser wire scenario прошёл. Оба контейнера
  одноразовые и удалены после проверок.

### 2026-09-27: G18 ManageSieve NOOP pre-auth and TAG response, scoped PASS T246

- ManageSieve теперь принимает `NOOP` в неаутентифицированном состоянии,
  принимает необязательный quoted-string tag и возвращает его в response code
  `(TAG "...")`, экранируя кавычки и обратную косую черту. Без tag прежняя
  строка ответа сохранена. Неизвестные дополнительные аргументы отвергаются.
- Unit tests проверяют синтаксис, escaping и оба состояния протокола.
  Независимый `tests/blackbox/managesieve_noop_tag.py` прошёл на Linux Rust
  1.97.1 against живой STARTTLS listener: pre-auth NOOP без tag, обычный tag,
  escaping, LOGOUT. Контейнер был `--rm`; данные теста оставлены в отдельной
  `./data/g43-managesieve-noop`. SCRAM-SHA-1 по RFC 5804 §2.1 не реализован и
  остаётся явным обязательным пробелом; эта работа его не закрывает. Windows:
  fmt, strict Clippy, 796 library + 1 CLI прошли; 3 fixture-dependent tests
  ignored. Linux Docker: 880 library + 1 CLI прошли; 3 ignored.

### 2026-09-27: G18 graceful SMTP pool shutdown, scoped PASS T245

- Исходящий SMTP worker теперь явно закрывает свои idle plaintext/implicit-TLS
  sessions командой `QUIT` при shutdown; штатный `Drop` не выполняет сетевой I/O.
  Завершение пробует закрыть остальные sessions даже после ошибки одной сессии,
  освобождает учётные слоты и возвращает ошибку вызывающему owner. Drain очереди,
  service и reactor сохраняют ошибки shutdown, не пропуская освобождение задач
  и storage.
- Peer-тесты проверяют QUIT/221 при shutdown пулов, worker, outbound service и
  plaintext/TLS scheduler; повторное использование между транзакциями остаётся
  без промежуточного QUIT. Windows Rust 1.97.1: fmt, strict Clippy, 795 library
  + 1 CLI прошли; 3 fixture-dependent теста ignored. Linux Rust 1.97:
  879 library + 1 CLI прошли, 3 ignored. Контейнер запущен с `--rm`, образ не
  собирался, отдельные образы/контейнеры не оставлены.
- Проверен путь штатного shutdown с успешными ответами peer. Ошибки QUIT на
  нескольких idle connections и внешний MTA interoperability остаются отдельно
  непроверенными.

### 2026-09-27: G18 outbound SMTP QUIT semantics for plaintext and implicit TLS, scoped PASS T244

- Исправлен standalone implicit-TLS `deliver`: после завершённого SMTP результата
  отправляется `QUIT` и ожидается ответ с command timeout, кроме канала, который
  peer уже закрыл через `421`. Plaintext one-shot путь также теперь отправляет
  QUIT после прочитанного SMTP-level 4xx/5xx; pooled TLS соединения не получают
  QUIT между повторно используемыми транзакциями.
- TLS peer-тесты проверяют `QUIT/221`, timeout на реальном TLS socket после
  100-ms тестового command timeout при сохранении предыдущего `Delivered`, а
  также отсутствие лишнего QUIT при reuse pool. Plaintext wire-тесты проверяют
  QUIT после 450/550 ответов. Windows Rust 1.97.1: fmt, strict Clippy, 795
  library + 1 CLI прошли; 3 fixture-dependent теста ignored. Linux Rust 1.97:
  879 library + 1 CLI прошли, 3 ignored; контейнер удалён через `--rm`.
- Это локальные peers, не внешний MTA. Нормативные 5-минутные задержки на
  wall clock не запускались; явное graceful завершение пула добавлено в T245.

### 2026-09-27: G18 outbound SMTP QUIT response timeout, scoped PASS T243

- Добавлен локальный plaintext TCP peer: он принимает DATA, отвечает `250`,
  читает `QUIT` и не отправляет `221`. Клиент завершает ожидание по короткому
  100-ms command timeout; подтверждённый ранее результат доставки остаётся
  `Delivered`. Peer проверяет закрытие соединения после возврата клиента.
- Windows Rust 1.97.1: fmt, strict Clippy, 794 library + 1 CLI прошли;
  3 fixture-dependent теста ignored. Linux Rust 1.97: новый socket-тест прошёл
  отдельно в одноразовом контейнере с `--rm`.
- T244 добавил implicit-TLS QUIT peer и применяет graceful завершение к
  plaintext SMTP-level rejection; нормативный wall-clock timeout не проверялся.

### 2026-09-27: G18 outbound BDAT socket timeout phase verification, scoped PASS T242

- К предыдущему изменению добавлены два локальных TCP peer-сценария с короткими
  overrides: промежуточный BDAT без ACK обрывается через 100 ms, а delayed
  BDAT LAST успешно получает ACK через 250 ms при 700-ms termination timeout.
  Тем самым проверено socket-level применение разных timeout в plaintext
  transport; TLS transport проходит общие сборку и тесты, но отдельный TLS
  peer-сценарий здесь не запускался.
- Windows Rust 1.97.1: fmt, strict Clippy, 793 library + 1 CLI прошли;
  3 fixture-dependent теста ignored. Linux Rust 1.97: оба socket-теста прошли
  в контейнере с `--rm`; полные Linux тесты для предыдущей версии production
  изменения прошли как 875 library + 1 CLI, 3 ignored.
- Это не проверка нормативных минут wall-clock задержки, QUIT timeout или
  интеграции с внешним MTA.

### 2026-09-27: G18 outbound SMTP per-chunk BDAT response timers, scoped PASS T241

- Разделены ожидания ответов на промежуточный `BDAT` и `BDAT LAST`: первый
  использует 300-секундный command timeout, последний 600-секундный
  DATA-termination timeout. Лимит записи каждого чанка остался 180 секунд.
  Plain TCP и implicit TLS устанавливают read timeout отдельно перед каждым
  чанком; маркер LAST передаётся из chunk iterator в транспорт.
- Добавлены unit checks выбора фазового интервала и последовательности маркеров.
  Windows Rust 1.97.1: fmt, strict Clippy, 791 library + 1 CLI прошли;
  3 fixture-dependent теста ignored. Linux Rust 1.97: 875 library + 1 CLI
  прошли, 3 ignored. Контейнер удалён через `--rm`.
- Отдельный короткий socket-level peer-тест добавлен в T242. Нормативные
  минуты wall-clock ожидания и внешний MTA по-прежнему не проверялись.

### 2026-09-27: G17 RFC 2231 duplicate and continuation rejection vectors, scoped PASS T240

- Добавлен corpus для повторных MIME параметров, повторных continuation
  индексов, разрывов и продолжения без начального сегмента. Он фиксирует
  строгую политику отказа парсера; production behavior не менялся.
- Windows Rust 1.97.1: fmt, strict Clippy, 790 library + 1 CLI прошли;
  3 fixture-dependent теста ignored. Linux Rust 1.97: все 21 тест параметров
  прошли. Контейнер запускался с `--rm`, постоянный образ не создавался.
- Это unit coverage MIME parameter parsing, не независимая клиентская
  совместимость и не полное подтверждение RFC 2231.

### 2026-09-27: G17 MIME quoted-printable encoder escape-path completion, scoped PASS T239

- Закрыт оставшийся одиночный CR/LF путь: он теперь использует тот же
  фиксированный трёхбайтовый стековый `=HH` token, без временного `String`.
- Проверены fmt, строгий Clippy и полный набор Windows Rust 1.97.1: 789
  library + 1 CLI тест прошли, 3 fixture-dependent теста ignored. Целевые
  encoder-тесты: 4 прошли. Linux Rust 1.97 в одноразовом Docker-контейнере:
  873 library + 1 CLI прошли, 3 ignored; контейнер удалён через `--rm`.
- Этот узкий результат не подтверждает полную RFC 2045 conformance или
  внешнюю совместимость.

### 2026-09-27: G17 MIME quoted-printable encoder octet matrix and allocation path, scoped PASS T238

- Vacation QP encoder no longer builds a temporary formatted heap string for
  each escaped octet; it constructs `=HH` using a fixed 3-byte stack token.
- New tests verify the independently expected wire form for each of 256 octets,
  byte-exact decoder round-trip, hard CRLF versus isolated CR/LF, combined
  full-range folding and the 76-octet line bound. Production dependencies do
  not change.
- Windows Rust 1.97.1: fmt, strict Clippy, 789 library + 1 CLI passed; 3
  fixture-dependent tests ignored. Linux Rust 1.97 container: 873 library + 1
  CLI passed, 3 ignored. The targeted encoder tests passed all four cases.
  Container used `--rm`; no persistent image was built.
- This does not prove external MTA or client interoperability, nor full RFC
  2045 conformance.

### 2026-09-27: G17 MIME quoted-printable complete octet mapping, scoped PASS T237

- RFC 2045 §6.7 unit coverage now iterates all 256 octet values encoded as
  uppercase `=HH` and robust lowercase `=hh`; all 93 printable safe literal
  ASCII octets from ranges 33..60 and 62..126 are also checked.
- Only decoder mapping coverage was expanded; production code is unchanged.
  Targeted QP suite passed 6 tests. Windows Rust 1.97.1 fmt, strict Clippy,
  786 library + 1 CLI passed with 3 fixture-dependent tests ignored. Linux
  Rust 1.97 container: 870 library + 1 CLI passed, 3 ignored. Container ran
  with `--rm`; no persistent image was built.
- This is not complete QP line-length, transport-padding, malformed-input,
  encoder conformance, MIME interoperability, or RFC 2045 conformance.

### 2026-09-27: G17 IDNA2008 strict overlap against Unicode 17 IdnaTestV2, scoped PASS T236

- Сохранён официальный Unicode 17 `IdnaTestV2.txt` в
  `./data/g41-idna-v2/IdnaTestV2.txt`; SHA-256
  `BEB5D0BE20E896189B03209A82FDC34F06351502BBD4B8E2523583FC2954D9CF`.
- Ignored oracle test сверяет только строгий cross-profile overlap: NFC,
  RFC 5892 PVALID, пустые nontransitional ToUnicode/ToASCII статусы и ASCII
  результат. Для входных A-label свойства проверяются по decoded U-label.
  Пройдено 172 записи; две строки с surrogate code units явно пропущены.
- Это не полная UTS #46 проверка и не полный IDNA2008 corpus: mapping,
  transitional processing, CONTEXT rules и оставшиеся строки не засчитываются.
  Windows Rust 1.97.1: fmt, strict Clippy, 784 library + 1 CLI прошли, 3
  ignored; targeted ignored oracle прошёл. Linux Rust 1.97 container:
  868 library + 1 CLI прошли, 3 ignored; тот же targeted oracle сверил 172
  строки и пропустил 2 non-scalar строки. Контейнер автоматически удалён через
  `--rm`; persistent image для этого этапа не создавался.

### 2026-09-27: G17 RFC 3492 §7.1 complete sample set, scoped PASS T235

- Punycode unit test now verifies all 19 official RFC 3492 §7.1 sample strings
  in both encode and decode directions. Russian sample I uses the corrected
  spelling from RFC Editor Erratum 3026; pure-ASCII sample S checks Bootstring,
  not IDNA label acceptance. Production codec required no changes.
- Fixed contradictory Unicode documentation: SMTP U-label and SMTPUTF8 are
  connected in the incoming local route path, while the complete IDNA label /
  domain conformance corpus and remote MX delivery remain unverified.
- Rust 1.97.1 Windows: `cargo fmt --check`, strict Clippy, 784 library + 1 CLI
  tests passed; 2 fixture-dependent corpus tests ignored. Linux Docker Rust
  1.97: release build passed; Clippy component download stalled, so that build
  was stopped and its temporary image/container absence verified. A separate
  disposable `rust:1.97-bookworm` container ran `cargo test --locked`: 868
  library + 1 CLI passed, 2 fixture-dependent tests ignored; `--rm` removed
  the container. Linux Clippy is not verified for this stage.
- This is RFC sample-vector coverage, not a complete Punycode or IDNA
  conformance suite, no DNS MX test, and no Litterkeep application test.

### 2026-09-27: G17 RFC 5892 Unicode 17 Derived Property corpus, scoped PASS T234

- Добавлен отдельный ignored Rust oracle test, который читает официальный
  Unicode 17 `Idna2008.txt`, проверяет непрерывное покрытие U+0000..U+10FFFF и
  сравнивает классификацию каждого Unicode scalar с production lookup.
- Все 1,112,064 scalar values совпали, включая PVALID, CONTEXTJ, CONTEXTO и
  общий disallowed/unassigned результат. Источник `./data/g40-idna17/Idna2008.txt`,
  SHA-256 `E4A7526A8A37539C0DEFA4DA25F5DBF77D0212A14D4762D455CADEA608A8921C`.
- Windows Rust 1.97.1: fmt, strict Clippy, 784 library + 1 CLI passed, 2
  ignored; corpus oracle прошёл отдельно. Linux Docker Rust 1.97: release,
  strict Clippy, 868 library + 1 CLI passed, 2 ignored; тот же oracle прошёл.
  Временный container автоматически удалён и test image удалён с проверкой.
- Это exhaustive Derived Property comparison, но не полная IDNA2008 label /
  domain conformance, Punycode corpus, DNS MX и удалённая доставка. Цель G17 и
  корневая цель остаются ACTIVE.

### 2026-09-27: G18 IMAP SEARCH single-byte MIME charset matrix, scoped PASS T233

- Независимый Python-клиент отправил Alice одно multipart письмо с 26
  однобайтовыми MIME charset через SMTP STARTTLS и 8BITMIME; Bob выполнил
  `SEARCH CHARSET UTF-8 BODY {n}` с UTF-8 synchronizing literal для каждой
  уникальной строки после MIME decoding. Все 26 поисков вернули сообщение.
- Проверены ISO-8859-1..10 и 13..16, TIS-620, IBM866, Windows-874 и
  Windows-1250..1258. Тестовые байты независимо созданы стандартными codec-ами
  Python в strict режиме, fixture representability проверена для каждой метки.
- Linux release runtime собран в Docker на Rust 1.97; независимая wire-проверка
  завершилась `PASS single-byte MIME charset matrix: 26 labels`. Контейнер и
  временный image удалены и проверяются отдельно. Локальные данные теста
  сохранены в `./data/g19-imap-singlebyte-matrix`. Это проверка Python клиента,
  не приложения Litterkeep; UTF-16/32, RFC 2047 headers и другие IMAP SEARCH
  charset в этот сценарий не входят.

### 2026-09-27: G18 IMAP SEARCH Windows-874 body decoding, scoped PASS T232

- Профиль SEARCH MIME projection использует charset decoder, но независимая
  wire проверка конкретно для charset текста ещё отсутствовала.
- Независимый клиент передал Alice Thai 8BITMIME body с charset Windows-874,
  затем Bob выполнил `SEARCH CHARSET UTF-8 BODY {n}` со synchronizing UTF-8
  literal и получил совпадение. Это подтверждает MIME charset -> Unicode
  projection -> IMAP SEARCH на реальном Linux listener, а не decoder unit test.
- Сценарий: `python tests/blackbox/smtp_imap_charset_search.py --credentials
  ./data/g17-search-windows874/test-accounts.tsv`; release runtime собран в
  Docker Rust 1.97. Временные контейнер и image удалены. Приложение Litterkeep
  и остальные SEARCH charset не проверялись.

### 2026-09-27: G17 UTF-32 MIME charset support, scoped PASS T231

- Добавляются только зарегистрированные UTF-32, UTF-32BE и UTF-32LE labels;
  aliases не угадываются. Generic UTF-32 соблюдает IANA default big-endian и
  распознаёт зарегистрированный big-endian BOM.
- Scalar validation отвергает surrogate units и значения выше U+10FFFF.
  Регистрация IANA помечает UTF-32 как unsuitable для MIME text, поэтому эти
  labels исключены из `text/*` projection, но используются в RFC 2047/2231.
- Windows Rust 1.97.1: fmt, strict Clippy, 784 library + 1 CLI passed;
  один внешний Unicode corpus ignored без fixture.
- Linux Docker Rust 1.97: release build, strict Clippy, 868 library + 1 CLI
  passed; один внешний Unicode corpus ignored без fixture. Временный image
  после wire acceptance удалён, отсутствие проверено.
- Независимый Python-клиент выполнил SMTP STARTTLS login Alice, передачу и
  доставку письма, затем IMAP STARTTLS login Bob и FETCH. Сверены неизменённый
  RFC 2047 Subject и UTF-32BE RFC 2231 filename `😀` в BODYSTRUCTURE.
  Запускался `python tests/blackbox/smtp_imap_utf32_charset.py --credentials
  ./data/g17-utf32-wire/test-accounts.tsv`; это не проверка приложения
  Litterkeep, POP3 или Sieve.

### 2026-09-27: G17 IBM866 MIME integration, scoped PASS T230

- В дополнение к таблице charset проверена реальная интеграция в RFC 2047
  Base64 encoded-word заголовка и RFC 2231 encoded filename parameter.
- CP866 encoded-word декодируется в «Привет»; RFC 2231 filename с IBM866
  octets декодируется в «Привет.txt».
- Проверки являются MIME parser unit tests; сетевой клиент и Litterkeep не
  запускались, совместимость с ними не заявляется.

### 2026-09-27: G17 IBM866 MIME charset, scoped PASS T229

- Добавлен отдельный typed decoder IBM866/CP866 без внешнего codec dependency;
  ASCII, Cyrillic, украинские буквы, знаки и CP866 псевдографика сохранены.
- Unicode CP866 table покрыта ожиданиями для всех 256 октетов; unit-тесты
  проверяют основные кириллические и хвостовые назначения. IANA labels
  IBM866, CP866, csIBM866 и 866 распознаются через enum Charset.
- Сетевой MIME/client acceptance не запускался и не заявляется. Следующие
  многобайтовые charset из IANA (например GB18030, GBK, Shift_JIS) остаются
  отдельными крупными задачами G17.

### 2026-09-27: G17 Windows-874 Thai MIME charset, scoped PASS T228

- По Microsoft CP874 table и IANA registration добавлен отдельный typed
  decoder, не смешанный с TIS-620/ISO-8859-11. Oracle проверяет каждый octet;
  31 undefined octet отвергается, Thai word и baht назначение подтверждены.
- IANA charset-registry alias `cswindows874` принимается; `cp874` отвергается,
  поскольку registration явно говорит, что это не alias.
- Windows Rust 1.97.1: 772 library tests и 1 CLI test прошли; один внешний
  Unicode corpus ignored без fixture path. fmt, strict Clippy и diff check
  прошли. Первый полный запуск имел неповторившийся `AlreadyExists` в
  несвязанном temp-file test; targeted повтор и полный прогон затем прошли.
- Linux Docker Rust 1.97: release build, strict Clippy, 856 library tests и
  1 CLI test прошли; один внешний Unicode corpus ignored без fixture path.
  Временный image удалён, отсутствие проверено; wire/client acceptance не
  заявляются. G17 остаётся active.

### 2026-09-27: G17 Windows-1258 MIME charset, scoped PASS T227

- По Microsoft CP1258 mapping и IANA registry добавлен typed decoder:
  exhaustive oracle сверяет все 256 octets; девять undefined C1 positions
  отклоняются, а Vietnamese combining marks сохраняются без normalization.
- `windows-1258` и registered alias `cswindows1258` принимаются;
  `cp1258` намеренно отклоняется, поскольку регистрация IANA говорит,
  что это сокращённое название, но не MIME alias.
- Windows Rust 1.97.1: 769 library tests и 1 CLI test прошли; один внешний
  Unicode corpus ignored без fixture path. fmt, strict Clippy и diff check
  прошли.
- Linux Docker Rust 1.97: release build, strict Clippy, 853 library tests и
  1 CLI test прошли; один внешний Unicode corpus ignored без fixture path.
  Временный image удалён, отсутствие проверено; wire/client acceptance не
  заявляются. G17 остаётся active.

### 2026-09-27: G17 Windows-1257 MIME charset, scoped PASS T226

- По Microsoft CP1257 mapping и IANA registry добавлен typed decoder для
  Baltic профиля. Все 256 octets сверяются с отдельной таблицей ожиданий;
  12 undefined positions отвергаются.
- Проверены литовские/латышские diacritics; принимаются `windows-1257` и
  registered alias `cswindows1257`, но не `cp1257`.
- Windows Rust 1.97.1: 766 library tests и 1 CLI test прошли; один внешний
  Unicode corpus ignored без fixture path. fmt, strict Clippy и diff check
  прошли.
- Linux Docker Rust 1.97: release build, strict Clippy, 850 library tests и
  1 CLI test прошли; один внешний Unicode corpus ignored без fixture path.
  Временный image удалён, отсутствие проверено; wire acceptance и Litterkeep
  client compatibility не заявляются. G17 остаётся active.

### 2026-09-27: G17 Windows-1256 MIME charset, scoped PASS T225

- По Microsoft CP1256 mapping и IANA registry добавлен typed decoder для
  арабского профиля: independent oracle проверяет все 256 octets, включая
  Persian letters, Arabic punctuation/marks, ZWNJ/ZWJ и LRM/RLM.
- Принимаются `windows-1256` и IANA alias `cswindows1256`, но не `cp1256`;
  прогнаны Arabic и Persian joining samples.
- Windows Rust 1.97.1: 763 library tests и 1 CLI test прошли; один внешний
  Unicode corpus ignored без fixture path. fmt, strict Clippy и diff check
  прошли.
- Linux Docker Rust 1.97: release build, strict Clippy, 847 library tests и
  1 CLI test прошли; один внешний Unicode corpus ignored без fixture path.
  Временный image удалён, отсутствие проверено; wire acceptance и Litterkeep
  client compatibility не заявляются. G17 остаётся active.

### 2026-09-27: G17 Windows-1255 MIME charset, scoped PASS T223

- По Microsoft CP1255 mapping и IANA registry добавлен typed decoder для
  Hebrew: независимый oracle сверяет все 256 octets, 23 undefined positions
  отвергаются, включая bidi marks как явно назначенные U+200E/U+200F.
- Принимаются `windows-1255` и IANA alias `cswindows1255`, но не `cp1255`;
  тестируются Hebrew word, новый shekel sign, Hebrew points и direction marks.
- Windows Rust 1.97.1: 760 library tests и 1 CLI test прошли; один внешний
  Unicode corpus ignored без fixture path. fmt, strict Clippy и diff check
  прошли.
- Linux Docker Rust 1.97: release build, strict Clippy, 844 library tests и
  1 CLI test прошли; один внешний Unicode corpus ignored без fixture path.
  Временный image удалён, отсутствие проверено; wire acceptance и Litterkeep
  client compatibility не заявляются. G17 остаётся active.

### 2026-09-27: G17 Windows-1254 MIME charset, scoped PASS T222

- По Microsoft CP1254 mapping и IANA registry добавлен typed decoder: 256
  octets проверены отдельным oracle, семь undefined позиций отвергаются;
  принимаются `windows-1254` и `cswindows1254`, но не `cp1254`.
- Windows Rust 1.97.1: 757 library tests и 1 CLI test прошли; один внешний
  Unicode corpus ignored без fixture path. fmt, strict Clippy и diff check
  прошли.
- Linux Docker Rust 1.97: release build, strict Clippy, 841 library tests и
  1 CLI test прошли; один внешний Unicode corpus ignored без fixture path.
  Временный image удалён, отсутствие проверено; network acceptance и
  Litterkeep client compatibility не заявляются. G17 остаётся active.

### 2026-09-27: G17 Windows-1253 MIME charset, scoped PASS T221

- По Microsoft CP1253 Unicode table и IANA registry добавлен отдельный typed
  decoder; implementation отвергает undefined bytes и распознаёт только
  canonical `windows-1253` и registered alias `cswindows1253`.
- Полный пооктетный oracle и Greek sample проверены.
- Windows Rust 1.97.1: 754 library tests и 1 CLI test прошли; 1 внешний
  Unicode corpus ignored без fixture path. fmt, strict Clippy и diff check
  прошли.
- Linux Docker Rust 1.97: release build, strict Clippy, 838 library tests и
  1 CLI test прошли; 1 внешний Unicode corpus ignored без fixture path.
  Временный image удалён, отсутствие проверено; wire acceptance и Litterkeep
  client compatibility не заявляются. G17 остаётся active.

### 2026-09-27: G17 Windows-1250 MIME charset, scoped PASS T220

- По Microsoft Unicode mapping и IANA registry добавлен typed Windows-1250
  decoder: каждый octet проверен отдельным oracle, пять undefined позиций
  отвергаются; принимается canonical `windows-1250` и alias `cswindows1250`,
  а `cp1250` не считается зарегистрированным alias.
- Windows Rust 1.97.1: 751 library tests и 1 CLI test прошли; 1 внешний
  Unicode corpus ignored без fixture path. fmt, strict Clippy и diff check
  прошли.
- Linux Docker Rust 1.97: release build, strict Clippy, 835 library tests и
  1 CLI test прошли; 1 внешний Unicode corpus ignored без fixture path.
  Временный image удалён, отсутствие проверено; network acceptance и
  Litterkeep client compatibility не заявляются.
- G17 остаётся active по другим MIME/protocol gaps.

### 2026-09-27: G17 full ISO-8859-16 MIME charset, scoped PASS T219

- По Unicode Consortium ISO/IEC 8859-16:2001 mapping и IANA registry
  реализуется typed Latin-10 decoder с полным byte oracle и всеми aliases.
- Full-byte oracle покрывает все 256 октетов, отдельный regression сохраняет
  C0/C1, проверяются Romanian comma-below text и все IANA aliases. Windows
  Rust 1.97.1: 748 library tests и 1 CLI test прошли; 1 внешний Unicode corpus
  ignored без fixture path. fmt, strict Clippy и diff check прошли.
- Linux Docker Rust 1.97: release build, strict Clippy, 832 library tests и
  1 CLI test прошли; 1 внешний Unicode corpus ignored без fixture path.
  Временный image удалён, отсутствие проверено; wire acceptance и Litterkeep
  client compatibility не заявляются.
- Все ISO-8859-1..16 предпочитаемые профили теперь имеют отдельные high-bit
  mapping, кроме несуществующего ISO-8859-12; G17 остаётся active по другим
  MIME/protocol gaps.

### 2026-09-27: G17 full ISO-8859-15 MIME charset, scoped PASS T218

- По Unicode Consortium mapping и IANA registry реализована типизированная
  Latin-9 таблица со всеми 256 октетами, euro и восьмью заменами Latin-1;
  добавлены aliases `ISO_8859-15`, `Latin-9`, `csISO885915`.
- Проверены все 256 octets, euro и все восемь replacement positions. Windows
  Rust 1.97: 745 library tests и 1 CLI test прошли; 1 внешний Unicode corpus
  ignored без fixture path. fmt, strict Clippy и diff check прошли.
- Linux Docker Rust 1.97: release build, strict Clippy, 829 library tests и
  1 CLI test прошли; 1 внешний Unicode corpus ignored без fixture path.
  Временный image удалён, отсутствие проверено; wire acceptance и Litterkeep
  client compatibility не заявляются.
- ISO-8859-16 и G17 остаются active.

### 2026-09-27: G17 full ISO-8859-14 MIME charset, scoped PASS T217

- Реализована typed ISO-8859-14 Celtic mapping с IANA aliases latin8/l8,
  iso-celtic и csISO885914. Full-byte oracle проверяет все 256 октетов,
  включая Celtic letters, обычные Latin-1 позиции и сохранение C1 controls.
- Windows Rust 1.97: 742 library tests и 1 CLI test прошли; 1 внешний Unicode
  corpus ignored без fixture path. fmt, strict Clippy и diff check прошли.
- Linux Docker Rust 1.97: release build, strict Clippy, 826 library tests и
  1 CLI test прошли; 1 внешний Unicode corpus ignored без fixture path.
  Временный image удалён, отсутствие проверено; wire acceptance и Litterkeep
  client compatibility не заявляются.
- G17 остаётся active: ISO-8859-15/-16 и другие protocol gaps остаются.

### 2026-09-27: G17 full ISO-8859-13 MIME charset, scoped PASS T216

- Реализована typed таблица ISO-8859-13 с IANA alias `csISO885913`.
  Проверен полный диапазон 256 octets, включая Baltic letters, punctuation,
  changed quote positions и C1 controls.
- Windows Rust 1.97: 738 library tests и 1 CLI test прошли; 1 внешний Unicode
  corpus ignored без fixture path. fmt, strict Clippy, diff check прошли.
- Linux Docker Rust 1.97: release build, strict Clippy, 822 library tests и
  1 CLI test прошли; 1 внешний Unicode corpus ignored без fixture path.
  Временный image удалён, отсутствие проверено; wire acceptance и Litterkeep
  client compatibility не заявляются.
- G17 остаётся active: ISO-8859-14..-16 и другие protocol gaps остаются.

### 2026-09-27: G17 TIS-620 / ISO-8859-11 MIME mapping, scoped PASS T215

- Реализован typed TIS-620 profile с alias ISO-8859-11, NBSP, Thai characters
  и строгим отказом для undefined DB..DE и FC..FF. Проверены все assigned
  старшие позиции и C1 controls.
- Windows Rust 1.97: 734 library tests и 1 CLI test прошли; 1 внешний Unicode
  corpus ignored без fixture. fmt, strict Clippy, diff check прошли.
- Linux Docker Rust 1.97: release build, strict Clippy, 818 library tests и
  1 CLI test прошли; 1 внешний Unicode corpus ignored без fixture. Временный
  image удалён, отсутствие проверено; wire acceptance и Litterkeep client
  compatibility не заявляются.
- G17 остаётся active: ISO-8859-13..-16 и другие protocol gaps остаются.

### 2026-09-27: G17 full ISO-8859-10 MIME charset, scoped PASS T214

- Добавлена отдельная таблица ISO-8859-10 и typed `Charset`; распознаются IANA
  aliases `latin6`/`l6`. Проверены все 256 octets, Nordic text и C1 controls.
- Windows Rust 1.97: 730 library tests и 1 CLI test прошли; 1 внешний Unicode
  corpus test ignored без fixture path. fmt, strict Clippy и diff check прошли.
- Linux Docker Rust 1.97: release build, strict Clippy, 814 library tests и
  1 CLI test прошли; 1 внешний Unicode corpus test ignored без fixture path.
  Временный image удалён и отсутствие проверено; wire acceptance и
  совместимость с приложением Litterkeep не заявляются.
- G17 остаётся active: ISO-8859-11, -13..-16 и другие protocol gaps остаются.

### 2026-09-27: G17 full ISO-8859-9 MIME charset, scoped PASS T213

- Добавлена отдельная таблица ISO-8859-9 и typed `Charset`; IANA aliases
  `latin5`/`l5` распознаются. Проверены все 256 octets, шесть турецких
  замен и сохранение C1 controls.
- Windows Rust 1.97: 726 library tests и 1 CLI test прошли; 1 внешний Unicode
  corpus test ignored без fixture path. fmt, strict Clippy и diff check прошли.
- Linux Docker Rust 1.97: release build, strict Clippy, 810 library tests и
  1 CLI test прошли; 1 внешний Unicode corpus test ignored без fixture path.
  Временный образ удалён, отсутствие проверено; wire acceptance и
  совместимость с приложением Litterkeep не заявляются.
- G17 остаётся active: ISO-8859-10, -11, -13..-16 и другие protocol gaps
  не реализованы.

### 2026-09-27: G17 full ISO-8859-8 MIME charset, scoped PASS T212

- Реализован отдельный typed MIME decoder ISO-8859-8 для базового visual-order
  профиля. Проверены Hebrew text, LRM/RLM, ASCII/C1, все назначенные старшие
  байты и строгий отказ на undefined positions. RFC 1556 -i/-e не aliases:
  эти отдельные directionality-профили остаются вне реализации.
- Windows Rust 1.97: 723 library tests и 1 CLI test прошли; 1 внешний Unicode
  corpus test пропущен, потому что fixture path не задан. fmt, strict Clippy,
  diff checks прошли.
- Linux Docker Rust 1.97: release build, strict Clippy, 807 library tests и
  1 CLI test прошли; 1 внешний Unicode corpus test ignored без fixture path.
  Временный image `litterkeep-iso8859-8-20260927:gate` удалён и его отсутствию
  выполнена проверка. Протокольный wire acceptance и подключение приложения
  Litterkeep этим изменением не проверялись.
- G17 остаётся активной: следующие зарегистрированные таблицы ISO-8859 и
  прочие оставшиеся пункты протокольной полноты ещё не реализованы.

### 2026-09-27: G17 full ISO-8859-7 MIME charset, scoped PASS T211

- Добавлена отдельная таблица modern ISO-8859-7 и typed вариант `Charset`.
  Greek, Euro и final sigma декодируются; undefined AE, D2 и FF отклоняются.
  Проверены все назначенные байты, ASCII/C1 range и registered MIME label.
- Windows Rust 1.97: 719 library passed, 1 внешний Unicode fixture ignored,
  1 CLI passed и doctests; 29 charset tests, strict Clippy, fmt и diff check
  прошли. Linux Docker Rust 1.97: release build, strict Clippy, 803 library
  passed, 1 ignored, 1 CLI passed и doctests.
- Linux test image удалён, отсутствие тега проверено. Нового network acceptance
  и Litterkeep app compatibility нет; ISO-8859-8 и последующие профили остаются
  неполными. T211 scoped PASS; G17/root ACTIVE.

### 2026-09-27: G17 full ISO-8859-6 MIME charset, scoped PASS T210

- Добавлена самостоятельная разреженная таблица ISO-8859-6 и typed вариант
  `Charset`. Арабские буквы и punctuation декодируются, ASCII/C1 сохраняются,
  undefined high-bit octets отклоняются. Проверены строка «الاسلام» и все
  undefined позиции таблицы.
- Windows Rust 1.97: 715 library passed, 1 внешний Unicode fixture ignored,
  1 CLI passed и doctests; 25 MIME charset tests, strict Clippy, fmt и diff
  check прошли. Linux Docker Rust 1.97: release build, strict Clippy,
  799 library passed, 1 ignored, 1 CLI passed и doctests.
- Test image удалён, отсутствие image tag проверено. Сетевое acceptance и
  Litterkeep app не проверялись; ISO-8859-7 и более поздние таблицы остаются
  неполными. T210 scoped PASS; G17/root ACTIVE.

### 2026-09-27: G17 full ISO-8859-5 MIME charset, scoped PASS T209

- Добавлена отдельная таблица ISO-8859-5 и typed вариант `Charset`. Проверены
  кириллические байты, отличия от Windows-1251, C1 control mapping и все 256
  октетов без undefined positions.
- Windows Rust 1.97: 711 library passed, 1 внешний Unicode fixture ignored,
  1 CLI passed и doctests; focused charset tests, strict Clippy, fmt и diff check
  прошли. Linux Docker Rust 1.97: release build, strict Clippy, 795 library
  passed, 1 ignored, 1 CLI passed и doctests.
- Созданный Linux test image удалён, отсутствие тега подтверждено. Нового socket
  acceptance и проверки приложения Litterkeep не проводилось; ISO-8859-6 и
  последующие таблицы ещё неполны. T209 scoped PASS; G17/root ACTIVE.

### 2026-09-27: G17 full ISO-8859-4 MIME charset, scoped PASS T208

- Добавлена самостоятельная таблица ISO-8859-4 и typed вариант `Charset`.
  Все назначенные позиции A0..FF декодируются; undefined A5, AE, BE, C3, D0,
  E3 и F0 возвращают ошибку. Проверены Latin-4 символы, C1 prefix, полный набор
  89 назначенных байтов и выбор charset по registered label.
- Windows Rust 1.97: 707 library passed, 1 внешний Unicode fixture ignored,
  1 CLI passed, doctests; strict Clippy, форматирование и diff check прошли.
  Linux Docker Rust 1.97: release build, strict Clippy, 791 library passed,
  1 ignored, 1 CLI passed и doctests.
- Временный Linux test image удалён, отсутствие проверено. Нет нового socket
  acceptance или проверки Litterkeep app; прочие ISO-8859 high-bit таблицы
  остаются неполными. T208 scoped PASS; G17/root ACTIVE.

### 2026-09-27: G17 full ISO-8859-3 MIME charset, scoped PASS T207

- Добавлена самостоятельная таблица ISO-8859-3 и typed вариант `Charset`.
  Все назначенные позиции A0..FF декодируются; undefined A5, AE, BE, C3, D0,
  E3 и F0 возвращают ошибку вместо replacement character. Проверены контрольные
  символы, определённые байты и выбор charset по registered label.
- Windows Rust 1.97: 703 library passed, 1 внешний Unicode fixture ignored,
  1 CLI passed и doctests; charset tests, strict Clippy, fmt и diff check прошли.
  Linux Docker Rust 1.97: release build, strict Clippy, 787 library passed,
  1 ignored, 1 CLI passed и doctests.
- Созданный Linux test image удалён, отсутствие тега проверено. Нового socket
  acceptance или проверки Litterkeep app не было; остальные ISO-8859 high-bit
  таблицы остаются неподдержанными. T207 scoped PASS; G17/root ACTIVE.

### 2026-09-27: G17 full ISO-8859-2 MIME charset, scoped PASS T206

- Добавлена отдельная полная таблица ISO-8859-2 и typed вариант `Charset`;
  тестируется весь диапазон A0..FF и польская строка. RFC 2047 Sieve `header`
  также сравнивает ASCII key с декодированным high-bit байтом `ł`.
- Windows Rust 1.97: 699 library passed, 1 внешний Unicode fixture ignored,
  1 CLI passed, doctests; targeted charset/Sieve tests, strict Clippy, fmt и
  diff check прошли. Linux Docker Rust 1.97: release build, strict Clippy,
  783 library passed, 1 ignored, 1 CLI passed и doctests.
- Созданный test image удалён, отсутствие image tag проверено. Нет нового
  socket acceptance или проверки с приложением Litterkeep; другие ISO-8859
  high-bit таблицы всё ещё отсутствуют. T206 scoped PASS; G17/root ACTIVE.

### 2026-09-27: G17 Sieve RFC 5228 ISO-8859 ASCII subset, scoped PASS T205

- RFC 5228 §2.7.2 требует conversion capability для US-ASCII subset ISO-8859-*.
  `Charset` теперь распознаёт зарегистрированные preferred MIME labels
  ISO-8859-2..11 и 13..16, декодируя только ASCII bytes; high-bit bytes остаются
  ошибкой, без предположения о неизвестной code table. RFC 2047 `header` test
  подтверждает ASCII Q-word сравнение с обычным Sieve ключом.
- Windows: 695 library passed, 1 отсутствующий внешний Unicode fixture ignored,
  1 CLI passed, doc-tests, targeted charset/Sieve tests, strict Clippy и fmt.
  Linux Docker Rust 1.97: 779 library passed, 1 external Unicode fixture ignored,
  1 CLI passed, doc-tests, strict Clippy и release build. Временный test image
  удалён, отсутствие проверено; runtime container не создавался.
- High-bit части большинства ISO-8859 charset, полная конверсия, wire-клиент и
  Litterkeep acceptance этим не подтверждены. T205 scoped PASS; G17/root ACTIVE.

### 2026-09-27: G17 RFC 6532 SMTPUTF8 coverage wording reconciled, scoped PASS T204

- Уточнена текущая матрица: старый T84 forced-extension delivery был до T195 и
  больше не описывает advertisement; T195 проверил conforming EHLO SMTPUTF8,
  STARTTLS delivery U-label адреса и побайтное извлечение MIME-вложения.
- Удалено ошибочное описание UTF-8 field names как возможной части RFC 6532:
  §3 явно сохраняет ASCII-only syntax для имён заголовков. Оставшийся scope
  сформулирован как неполная грамматика UTF-8 в structured field values и
  непроверенная совместимость с Litterkeep app. Проверено по первичному RFC,
  семи MIME header parser tests и ранее записанному T195 wire acceptance.
- Документы прошли `cargo fmt --check` и `git diff --check`; source/tests не
  менялись. Это исправление инвентаризации, не новый network acceptance и не
  закрытие RFC 6532 целиком. G17/root остаются ACTIVE.

### 2026-09-27: G17 Sieve RFC 6131 vacation-seconds, scoped PASS T203

- Добавлена typed capability `vacation-seconds`, typed `VacationInterval`, разбор
  RFC-диапазона `0..2^31-1` и запрет совместного `:days`/`:seconds`. Capability
  рекламируется в ManageSieve; она может использоваться без явного `vacation`.
- Store suppression принимает унифицированный интервал секунд. Локальная политика
  ограничивает значения 60 секундами..365 днями и сохраняет default семь дней;
  значение `:seconds 0` проходит синтаксис, но clamp-ится до 60 секунд. Сквозной
  Store test переводит активный script на `:seconds 60` и проверяет durable
  suppression после reopen.
- Финальный Linux Docker Rust 1.97 gate прошёл: strict Clippy, 777 library tests,
  1 CLI test и doc-tests; один внешний Unicode fixture-test ignored, поскольку
  UCD-файл не предоставлен. Windows полный набор до последних parser guards
  дал 692 pass + 1 ignored, после guards прошли все 30 vacation tests, strict
  Clippy и fmt. Docker image удалён, проверка отсутствия тега успешна.
- Доказательства ограничены unit/Store/CAPABILITY coverage. Независимый сетевой
  ManageSieve client и Litterkeep app не запускались; T203 scoped PASS, G17/root
  остаются ACTIVE.

### 2026-09-27: G17 EHLO timeout defect discovered, checkpoint T201 FAIL

- Расширенный независимый peer первоначально прошёл greeting, затем завис на
  EHLO дольше 15 секунд, хотя `LITTERKEEP_SMTP_COMMAND_TIMEOUT_SECONDS=1`.
  Проверка остановилась по внешнему watchdog до MAIL/RCPT/DATA. Причина была в
  том, что оба outbound client сохраняли greeting read timeout при чтении EHLO.
- T201 намеренно оставлен FAIL в матрице как диагностический результат, а не
  переименован в PASS после исправления. Production fix и повторный полный прогон
  зарегистрированы отдельно в T202.

### 2026-09-27: G17 outbound SMTP EHLO command deadline, scoped PASS T202

- Plaintext и implicit-TLS transports переключают socket read timeout на обычный
  command deadline после greeting и до отправки EHLO. Создание plaintext channel
  вынесено в отдельный `src/outbound_connect.rs`; `src/outbound.rs` теперь 290
  строк. Добавлен отдельный implicit-TLS rustls peer, который при greeting limit
  2 s и command limit 100 ms удерживает TLS канал открытым: тест получил timeout
  после 0,11 s.
- Linux Rust 1.97 project Docker test stage прошёл strict Clippy, 772 library +
  1 CLI test, один внешний Unicode corpus ignored без файла. Runtime wire suite
  прошёл greeting, EHLO, MAIL, RCPT, DATA 354 и final DATA wait, каждый peer
  удерживал соединение открытым до выбранного timeout. Python `py_compile` и
  `cargo fmt --check` прошли. Уникальные Docker images удалены, containers
  запускались с `--rm`; fixture `./data/acceptance-outbound-timeout-20260927`.
- Независимый peer не является Litterkeep app; QUIT, BDAT и реальные минуты RFC
  на wall clock не проверялись. T202 scoped PASS; G17/root остаются ACTIVE.

### 2026-09-27: G17 outbound SMTP phase timeout wire checks, scoped PASS T200

- Добавлен отдельный независимый peer `tests/blackbox/smtp_outbound_phase_timeout.py`.
  С короткими environment overrides он удерживает TCP соединение открытым и
  проверяет завершение клиента на greeting, ожидании `354` и финальном ответе
  после DATA terminator. Для последней фазы peer отдельно подтвердил получение
  terminator до начала молчания; ожидание FIN/ошибки вместо timeout не засчитывается.
- Python `py_compile` и три wire cases прошли. Runtime собран из текущего source
  существующим `docker/Dockerfile.linux-test`, контекст 24.94 KiB содержит только
  заявленные Dockerfile пути; уникальный image tag удалён после проверки, все
  контейнеры использовали `--rm`. Fixture: `./data/acceptance-outbound-timeout-20260927`.
- Rust source не менялся в этом checkpoint, Rust gates не перезапускались. Проверен
  только outbound runtime на независимом TCP peer, не Litterkeep app и не все SMTP
  timeout фазы. T200 scoped PASS; G17 и root остаются ACTIVE.

### 2026-09-27: G17 RFC 5321 DATA line boundary for 7BIT and 8BITMIME, scoped PASS T199

- Расширен independent STARTTLS wire acceptance точной границы RFC 5321
  §4.5.3.1.6 для обоих BODY-профилей. После снятия transparency сервер
  принимает 1000 октетов вместе с CRLF и сохраняет точные байты; 8BITMIME
  fixture содержит high-bit octet. 1001 октет отклоняется кодом 552.
- IMAP read-back подтвердил точные unstuffed bytes обоих принятых граничных
  сообщений и отсутствие обоих overlong сообщений. `NOOP` вернул 250 после
  каждого отказа. Скрипт прошёл `py_compile` и содержит 220 строк.
- Rust source не менялся после T198: полный Rust suite и Clippy в этом
  wire-only checkpoint не перезапускались. Использованы ранее gate-verified
  Rust 1.97 и существующий `rust:1.97-bookworm` image. Fixture
  `./data/acceptance-rfc5321-line-bound-20260927`; runtime container удалён,
  новый image не создавался. T199 scoped PASS; G17/root ACTIVE.

### 2026-09-27: G17 RFC 2045 canonical CRLF in SMTP DATA, scoped PASS T198

- DATA validator теперь отвергает NUL и непарные CR/LF отдельной причиной
  invalid octets; RFC 2045 7bit/8bit требует CRLF-пары. SMTP завершает такое
  письмо ответом `554 5.6.0`, сохраняя `552 5.3.4` для size limit. Правило
  действует на headers и body, не нормализует полученные октеты. BINARYMIME
  остаётся opaque и использует отдельный BDAT путь.
- Linux Rust 1.97 полный набор: 769 library + 1 CLI прошли, один внешний
  Unicode corpus-тест ignored без fixture; strict Clippy и rustfmt прошли.
  Независимый Python STARTTLS peer проверил одиночные CR и LF в DATA для
  профилей без BODY и `BODY=8BITMIME`: четыре 554 5.6.0, четыре NOOP 250,
  затем корректная доставка. IMAP подтвердил, что принято только валидное
  письмо. Отдельный BINARYMIME regression сохранил arbitrary octets.
- Fixture `./data/acceptance-rfc2045-crlf-20260927`; build cache переиспользован
  из локального `./data/acceptance-rfc2045-nul-20260927`. Runtime container
  остановлен и удалён, образ не создавался. T198 scoped PASS; G17 и root ACTIVE.

### 2026-09-27: G17 RFC 2045 rejects NUL in SMTP DATA, scoped PASS T197

- SMTP DATA отдельно помечает NUL как запрещённый content octet в режимах
  7BIT и 8BITMIME. После dot terminator сервер отвечает `554 5.6.0`; превышение
  размера остаётся самостоятельным `552 5.3.4`. Причины отказа разделены enum.
- Linux Rust 1.97 проверки: целевой parser unit test прошёл; полный набор
  `cargo test --locked`: 765 library + 1 CLI прошли, 1 внешний Unicode corpus
  ignored без fixture; строгий Clippy и `cargo fmt --check` прошли. Независимый
  Python STARTTLS peer отправил DATA с NUL и без BODY/с `BODY=8BITMIME`; обе
  транзакции получили `554 5.6.0`, после каждой `NOOP` получил 250. Следующее
  корректное письмо доставлено, IMAP подтвердил его наличие и отсутствие обеих
  отвергнутых копий. Litterkeep app не проверялось.
- Кэш сборки использовал `./data/acceptance-rfc2045-nul-20260927`. Временные
  Docker containers удалены и отсутствие проверено; новый образ не создавался,
  использован существующий `rust:1.97-bookworm`. Скрипт
  `tests/blackbox/smtp_data_nul_rejection.py` прошёл. T197 scoped PASS;
  G17 и root остаются ACTIVE.

### 2026-09-27: G17 BINARYMIME acceptance aligned with SMTPUTF8 capability, scoped PASS T196

- Исправлен устаревший independent `smtp_binarymime_delivery.py`: он ошибочно
  падал при наличии EHLO SMTPUTF8, хотя текущий сервер обязан объявлять эту
  capability вместе с 8BITMIME. Проверка теперь требует CHUNKING, BINARYMIME,
  8BITMIME и SMTPUTF8 и всё ещё отдельно доказывает, что UTF-8 headers требуют
  MAIL-параметр SMTPUTF8, а не одного лишь наличия capability.
- Скрипт запускается на свежем Rust 1.97 Linux runtime image с implicit TLS:
  DATA отказ для BODY=BINARYMIME, noncanonical text rejection с RSET recovery,
  точные binary payload octets через BDAT/IMAP, отказ UTF-8 headers без MAIL
  SMTPUTF8 и успешная доставка с ним. Результат: `Inbound SMTP BINARYMIME ->
  IMAP exact payload and SMTPUTF8 guard passed`. Это независимый Python peer,
  не Litterkeep app.
- Fixture `./data/acceptance-g17-rfc3030-smtputf8-regression-20260927`
  сохранена; созданный runtime container и image tag удалены и их отсутствие
  проверено. Исправлена только acceptance harness, Rust source не менялся.
  T196 scoped PASS; полный RFC 3030, приложение Litterkeep и G17 остаются
  непроверенными/ACTIVE.

### 2026-09-27: G17 inbound SMTPUTF8 with IDNA U-label routing, scoped PASS T195

- SMTP EHLO теперь объявляет `SMTPUTF8` вместе с `8BITMIME`. Входящий parser
  принимает U-label домен только при корректном MAIL параметре SMTPUTF8,
  преобразует его в A-label до local-route lookup и сбрасывает SMTPUTF8 marker
  при RSET. Domain aliases ограничены текущим локальным тестовым route policy.
- Независимые Rust проверки Windows прошли: SMTP address 14, command 15,
  локальная маршрутизация 7; полный набор: 685 library passed, 1 ignored
  внешний Unicode corpus, 1 CLI passed, strict Clippy и fmt. Linux Docker
  Rust 1.97 образ прошёл strict Clippy, 764 library tests, 1 CLI test и
  doctests; один внешний Unicode corpus test требует отсутствующий fixture.
- Conforming Python STARTTLS клиент подтвердил EHLO SMTPUTF8/8BITMIME,
  SMTP policy/RSET, а затем отправил MIME письмо от Alice на
  `пользователь@bücher.test`; независимый IMAP STARTTLS клиент прочитал
  attachment. Исходные и извлечённые байты совпали, SHA-256
  `7f4e114fb71bb6cf784bda9f0d77d06e3ccb15cfdc667bbad322dcef81da20f8`.
  Проверялась независимая Python harness, не Litterkeep app.
- Docker test/runtime images и запущенный контейнер удалены; отсутствие
  созданных тегов проверено. Данные acceptance сохранены в
  `./data/acceptance-g17-smtputf8-idna-20260927`. DNS/MX discovery, внешняя
  доставка и RFC 6532 UTF-8 field names не проверялись. T195 scoped PASS;
  G17 и root остаются ACTIVE.

Root Goal ID: `01a0ba65-3922-7d21-ac75-0963ef79819a`
Root status: ACTIVE  
Current subgoal: G17

### 2026-09-27: G17 IDNA domain validation and A-label round-trip, scoped PASS T194

- Входящие SMTP ASCII домены теперь валидируют `xn--` как A-label, а не как
  непрозрачный LDH-текст: метка lower-case нормализуется, декодируется,
  проверяется как U-label и повторно кодируется с обязательным совпадением.
  `to_alabel_domain` преобразует смешанные Unicode/ASCII домены в DNS A-label
  form и ограничивает encoded имя 253 октетами.
- RFC 5893 activation учитывает R/AL/AN и, когда одна метка делает домен Bidi,
  проверяет каждую соседнюю метку, включая ASCII LTR labels. Исправлен прежний
  acceptance ошибочного standalone U-label `١٢`: AN сам делает label RTL.
  RFC 5891 leading combining-mark запрет проверяется по Unicode 17 Mn/Mc/Me;
  закреплённый UnicodeData.txt генератор создаёт 327 ranges в двух shards.
- Windows: 27 IDNA и 13 SMTP address тестов, full suite 684 library + 1 CLI
  passed, 1 внешний NFC corpus ignored без fixture, strict Clippy и fmt прошли.
  Linux Docker Rust 1.97 release + strict Clippy + 762 library + 1 CLI passed,
  1 внешний NFC corpus ignored, doctests прошли. Созданный image удалён и tag
  отсутствует. SMTP parser принимает только ASCII wire-домены; U-label mailbox
  parsing/SMTPUTF8 и DNS resolution не входят в эту проверку. T194 scoped PASS;
  G17 и root остаются ACTIVE.

### 2026-09-27: G17 Unicode 17 IDNA Bidi Rule, scoped PASS T193

- U-label validation применяет RFC 5893 §2 к меткам, содержащим классы R или
  AL: начальный класс, допустимое множество, финальный класс с завершающими
  NSM и запрет смешения EN/AN в RTL label. Таблица всех Unicode 17
  Bidi_Class ranges сгенерирована из официального DerivedBidiClass.txt; SHA-256
  `4867B4B7F0731ED1BFCD34CC6251211FF1542541FCE0734B6FBDA139EE80B3A4`.
- Искусственные class-векторы покрывают RTL/LTR правила, допустимые окончания,
  недопустимые классы и смешение цифр; U-label тесты проверяют арабский пример,
  смешение европейских и арабских цифр и LTR/RTL классов. Windows: 18 IDNA
  тестов, strict Clippy, fmt и полный набор (674 library passed, 1 внешний NFC
  corpus ignored без fixture, 1 CLI и doctests). Linux Docker Rust 1.97 release,
  strict Clippy и полный набор: 752 library passed, 1 такой corpus ignored,
  1 CLI и doctests. Файлы не превышают 300 строк. Образ удалён, tag отсутствует.
- Проверка вызывается на отдельной U-label: конвертер пока не получает весь
  domain, поэтому не активирует RFC 5893 для ASCII/LTR sibling labels в домене,
  где другая метка RTL. SMTP parser и SMTPUTF8 по-прежнему не интегрированы;
  входящая A-label validation отсутствует. T193 scoped PASS, G17/root ACTIVE.

### 2026-09-27: G17 Unicode 17 IDNA CONTEXTJ rules

- Завершены RFC 5892 Appendix A.1/A.2 правила Join Controls: U+200D требует
  непосредственно предшествующий Canonical_Combining_Class=Virama; U+200C
  допускает тот же virama либо `(L|D) T* U+200C T* (R|D)`. Unicode 17
  `DerivedJoiningType.txt` и `UnicodeData.txt` закреплены SHA-256; генератор
  создаёт 542 joining ranges в трёх shards по 255/255/47 строк и 58 virama
  ranges в 61-строчном source table.
- Семь CONTEXTJ U-label vectors покрывают virama-based ZWNJ/ZWJ, Arabic
  joining context, пропуск Transparent mark и неверные контексты. Отдельные
  тесты проверяют joining lookup и virama классы. Windows Rust 1.97 targeted
  IDNA tests (12), strict Clippy и fmt прошли. Linux Docker Rust 1.97 release
  build, strict Clippy, full suite (746 library pass, 1 внешний NFC corpus
  ignored без fixture, 1 CLI и doctests) и отдельные 12 IDNA tests прошли.
  `git diff --check` пройден, созданный image удалён и tag отсутствует.
- Bidi, входящие A-label, whole-domain conversion и SMTP parser integration
  остаются незавершёнными. SMTPUTF8 не рекламируется.
  T192 является scoped PASS; G17 и корневая цель остаются ACTIVE.

### 2026-09-27: G17 Unicode 17 IDNA CONTEXTO rules

- U-label converter теперь применяет все шесть RFC 5891 §4.2.3.1 CONTEXTO
  условий: Catalan middle dot, Greek lower numeral sign, Hebrew punctuation,
  Japanese middle dot и запрет смешения Arabic-Indic наборов цифр. Script
  ranges для Greek/Hebrew/Hiragana/Katakana/Han собраны из официального
  Unicode 17 Scripts.txt (SHA-256 закреплён генератором), отсортированы для
  binary search и занимают 129 строк.
- Пять тестов U-label и два теста Script проверяют допустимые/запрещённые
  контексты и распознавание скриптов. Windows Rust 1.97 targeted IDNA tests
  (10), strict Clippy и fmt прошли. Linux Docker Rust 1.97 release build,
  strict Clippy, полный набор (744 library pass, 1 внешний NFC corpus ignored
  без fixture, 1 CLI и doctests) и отдельные 10 IDNA tests прошли.
  `git diff --check` пройден, созданный image удалён и его tag отсутствует.
- CONTEXTJ, bidi, входные A-label validation, whole-domain conversion и
  SMTP parser integration остаются открытыми; SMTPUTF8 не рекламируется.
  T191 зарегистрирован как scoped PASS, G17 и корневая цель остаются ACTIVE.

### 2026-09-27: G17 scoped PVALID-only U-label to A-label conversion

- Реализована отдельная `smtp::idna17::to_alabel`: проверяет NFC, свойства
  Unicode 17, leading/trailing/reserved hyphens, RFC 3492 conversion и длину
  готового `xn--` label не более 63 ASCII октетов. Четыре новых теста включают
  multilingual vectors, non-NFC/disallowed/hyphen cases, отказ на CONTEXTJ/O и
  расширение за DNS label limit.
- Windows Rust 1.97.1 targeted SMTP IDNA tests (7) и strict all-target Clippy
  прошли. Linux Docker Rust 1.97 release build, strict Clippy, 741 library
  tests, 1 CLI test и doctests прошли; один внешний normalization corpus test
  пропущен без fixture. Семь targeted IDNA tests прошли и на Linux. `cargo fmt
  --check` и `git diff --check` прошли; созданный Docker image удалён и его tag
  отсутствует. Ошибка первой версии одного test vector была исправлена: его
  Punycode output не пересекал 63-октетную границу.
- Функция намеренно не принимает контекстные code points до реализации всех
  RFC 5891 rules; bidi и A-label validation отсутствуют. SMTP address parser
  её не использует, capability SMTPUTF8 остаётся снятой. Это промежуточный
  conversion building block, не IDNA conformance и не клиентская acceptance.
  T190 записан как scoped PASS; G17 и корневая цель остаются ACTIVE.

### 2026-09-27: G17 Unicode 17 RFC 5892 derived-property lookup

- Добавлен собственный классификатор Unicode 17 IDNA2008 Derived Property с
  типами PVALID, CONTEXTJ, CONTEXTO и DisallowedOrUnassigned. Диапазоны
  сгенерированы из закреплённого официального Idna2008.txt; generator проверяет
  SHA-256, shards не превышают 300 строк. Три теста покрывают все типы на
  представительных code points. Таблица содержит 1 244 включительных диапазона.
- Windows Rust 1.97.1: targeted tests и strict all-target Clippy прошли;
  Linux Docker Rust 1.97 release build, strict Clippy, 737 library tests,
  1 CLI test и doctests прошли; один внешний normalization corpus test
  пропущен без fixture. Linux targeted property tests также прошли. `cargo fmt
  --check` и `git diff --check` прошли. Созданный image удалён, его tag
  отсутствует; fixture `./data/g40-idna17/Idna2008.txt` сохранена для
  воспроизводимой генерации. Источник и лицензия описаны в
  `docs/unicode-data.md`.
- Это property lookup, не IDNA label validator. Не реализованы NFC/hyphen/
  contextual/bidi правила, A-label round-trip, DNS conversion и подключение к
  SMTP parser; SMTPUTF8 не рекламируется. T189 является scoped PASS; G17 и
  корневая цель остаются ACTIVE.

### 2026-09-27: G17 RFC 3492 bounded Punycode codec foundation

- Добавлен собственный encoder/decoder Punycode Bootstring с checked arithmetic,
  входным лимитом 1024 bytes, декодированием ASCII digits без учёта регистра,
  правильным delimiter после basic code points и отдельным типом ошибок.
  RFC 3492 §7.1 Chinese/Japanese vectors, round-trip multilingual strings,
  malformed input и size boundary проверяются unit-тестами.
- Windows Rust 1.97.1: strict Clippy, fmt, 656 library + 1 CLI test pass,
  1 официальный Unicode normalization corpus test ignored без fixture. Linux
  Docker Rust 1.97: release build, strict Clippy, 734 library + 1 CLI tests,
  doctests и exact Punycode test group pass. Test image удалён, отсутствие
  проверено; runtime containers не создавались.
- Codec реализует RFC 3492 encoding, а не IDNA2008 profile. Он пока не соединён
  с SMTP address parser: U-label/A-label checks, RFC 5890/5891/5893 validity,
  DNS conversion, SMTPUTF8 EHLO и Litterkeep acceptance остаются открытыми.
  T188 зарегистрирован как scoped PASS; G17 и корневая цель остаются ACTIVE.

### 2026-09-27: G17 reconcile Sieve redirect coverage matrix with T119

- Исправлено устаревшее утверждение в `docs/sieve-coverage.md`, будто redirect
  SMTP delivery не проверялась. Текущий `tests/blackbox/sieve_redirect_copy_peer.py`
  и зарегистрированный T119 покрывают ManageSieve activation, local implicit
  keep, остановку source Store, durable queue drain, independent SMTP peer,
  сравнение envelope/RFC822 и decoded attachment bytes.
- Текущий acceptance harness не запускался повторно: Docker fixture и image
  T119 уже удалены. Запись ограничена аудитом согласованности по проверяемому
  исходнику и сохранённому acceptance evidence; Litterkeep и произвольный
  внешний MTA остаются непроверенными. Корневая цель и G17 остаются ACTIVE.

### 2026-09-27: G17 Sieve ASCII-casemap contains linear-time matching

- `:contains` с `i;ascii-casemap` заменён с перебора всех окон на KMP:
  поиск работает за O(n + m), хранит O(m) байтов ключа и prefix table и не
  копирует всё проверяемое поле. Octet comparator и glob matcher не менялись.
- Adversarial unit test: 131072 ASCII `a` и промах на ключе длиной 2049.
  В Windows Rust debug до исправления тест занимал 1.29s, после 0.01s в той
  же среде. Отдельный тест подтверждает ASCII-only folding и сохранение точных
  не-ASCII октетов. Оба targeted теста прошли в Linux Rust 1.97 Docker image.
- Полный Linux gate прошёл strict Clippy, 731 library tests, 1 внешний Unicode
  corpus test ignored без fixture, 1 CLI test и doctests. `cargo fmt --check`
  и `git diff --check` прошли. Test image удалён и отсутствие проверено.
  Это ограниченный matcher/performance checkpoint, не полный RFC 5228 профиль,
  production benchmark или acceptance Litterkeep. T187 записан как scoped PASS;
  G17 и корневая цель остаются ACTIVE.

### 2026-09-27: G17 RFC 2047 encoded-word bounded lookahead

- `decode_header` теперь ищет `?=` только в пределах 75 символов от каждого
  `=?`, как ограничивает RFC 2047 §2. Oversized malformed token больше не
  заставляет parser многократно просматривать весь остаток header; поиск
  продолжает находить последующее самостоятельное корректное encoded-word.
- Regression на 32 768 malformed starters замерена в том же Windows Rust debug
  профиле: до изменения 5.21s, после 0.04s. Linux Rust 1.97 targeted tests для
  большого malformed header и последующего валидного слова прошли (0.04s и
  0.00s). Полный Linux Docker gate прошёл strict Clippy, 729 library tests,
  1 внешний Unicode corpus test ignored без fixture, 1 CLI test и doctests.
- `cargo fmt --check` и `git diff --check` прошли. Временный test image удалён,
  отсутствие проверено; тестовые контейнеры запускались с `--rm`. Это
  unit-level decoder coverage и одна локальная performance regression, а не
  production throughput или acceptance Litterkeep. T186 внесён как scoped PASS;
  G17 и корневая цель остаются ACTIVE.

### 2026-09-27: G17 RFC 2045 quoted-printable linear whitespace scan

- Устранён квадратичный lookahead при декодировании длинных SP/HTAB runs:
  decoder теперь классифицирует непрерывный участок за один проход. Transport
  padding у hard CRLF/end-of-object удаляется; пробелы перед `=` soft break
  сохраняются как encoded content по RFC 2045 §6.7.
- Добавлен unit-вектор с 8 KiB WSP для обеих позиций. Windows Rust 1.97 targeted
  test прошёл. Linux Docker Rust 1.97 strict Clippy и полный gate прошли:
  727 library tests, 1 внешний Unicode corpus test ignored без fixture, 1 CLI
  test и doctests; новый targeted Linux test также прошёл.
- `cargo fmt --check` и `git diff --check` пройдены. Test image удалён,
  отсутствие подтверждено; контейнеры acceptance запускались с `--rm`. Это
  targeted decoder coverage, не полный RFC-векторный корпус и не проверка
  Litterkeep app. T185 записан как scoped PASS; G17 и корневая цель остаются
  ACTIVE.

### 2026-09-27: G17 SMTP HELP command inventory

- Исправлен неполный ответ HELP: в многострочный список добавлены уже
  реализованные `AUTH`, `STARTTLS` и `BDAT`. Новый отдельный Linux reactor unit
  test проверяет эти команды в сформированном ответе.
- Docker test stage Rust 1.97 прошёл strict Clippy, 726 library tests,
  1 внешний Unicode corpus test ignored из-за отсутствующей fixture, 1 CLI test
  и doctests. Новый точечный тест повторно прошёл в готовом test image.
- `cargo fmt --check` и `git diff --check` прошли; test image удалён и отсутствие
  подтверждено. Это локальное HELP покрытие, не широкая SMTP interoperability
  проверка; Litterkeep app не тестировался. T184 зарегистрирован как scoped PASS;
  G17 и корневая цель остаются ACTIVE.

### 2026-09-27: G17 RFC 5321 DATA block timeout refresh, wall-clock verification

- T183 проверил фактический reset срока на новом body read. Независимый Linux
  STARTTLS peer оставил DATA незавершённым, через 120 секунд отправил новый
  частичный блок и наблюдал TCP EOF через 180.1 секунды после него. Acceptance
  требует 165-205 секунд; таким образом результат отличает refreshed timeout от
  первоначального срока, который истёк бы примерно через минуту.
- `tests/blackbox/smtp_data_block_timeout_refresh.py` прошёл `py_compile`,
  104 строки. Docker test-stage gates и runtime image использовали закэшированные
  Rust source layers; Rust suite повторно не запускался, так как Rust source не
  менялся.
- Fixture `./data/acceptance-smtp-data-timeout-refresh-20260927` сохранена.
  Временные containers и оба созданных image удалены, отсутствие проверено.
  Клиент Litterkeep не тестировался. T183 записан как scoped PASS; G17 и
  корневая цель остаются ACTIVE.

### 2026-09-27: G17 RFC 5321 SMTP command idle timeout, wall-clock verification

- Закрыт wall-clock пробел T96. Независимый Linux STARTTLS peer прошёл EHLO,
  TLS upgrade и AUTH Alice, затем не отправлял SMTP commands. Сервер завершил
  TCP session через 300.1 секунды; acceptance требует диапазон 285-330 секунд.
- `tests/blackbox/smtp_command_idle_timeout.py` прошёл `py_compile`, 111 строк.
  Docker test-stage image собран от текущего source с кэшированными Rust gate
  layers; Rust suite для этого wire-only acceptance не rerun-ился.
- Fixture `./data/acceptance-smtp-command-timeout-wallclock-20260927` сохранена.
  Bootstrap был `--rm`; runtime container и image удалены, отсутствие проверено.
  Клиент Litterkeep не тестировался. T182 внесён как scoped PASS; вместе с T181
  это подтверждает реальные пятиминутный command и трёхминутный DATA deadlines.
  G17 и корневая цель остаются ACTIVE.

### 2026-09-27: G17 RFC 5321 inbound DATA block timeout, wall-clock verification

- Закрыт оставшийся в T97 пробел реального elapsed-time acceptance. Независимый
  Linux STARTTLS peer прошёл AUTH, MAIL, RCPT и DATA, получил 354, передал
  неполный body block и зафиксировал TCP EOF через 180.1 секунды. Новый скрипт
  требует закрытия в диапазоне 165-205 секунд; принудительный Rust timeout не
  использовался.
- `tests/blackbox/smtp_data_block_timeout.py` прошёл `py_compile`, 123 строки.
  Docker test-stage image был собран из текущего source с кэшированными Rust
  gate layers; Rust suite для wire-only acceptance не запускался повторно.
- Fixture `./data/acceptance-smtp-data-timeout-wallclock-20260927` сохранена.
  Bootstrap был `--rm`; runtime container и image удалены с подтверждением
  отсутствия. Litterkeep app не тестировался. T181 зарегистрирован как scoped
  PASS, G17 и корневая цель остаются ACTIVE.

### 2026-09-27: G17 RFC 5321 SMTP DATA line boundary, independent wire acceptance

- Новый независимый STARTTLS acceptance отправил dot-stuffed DATA строку,
  которая после снятия одного transparency dot имеет ровно 1000 октетов вместе
  с CRLF, и получил 250. Строка в 1001 октет получила 552; последующий NOOP
  получил 250, подтверждая восстановление SMTP command synchronization.
- Скрипт `tests/blackbox/smtp_data_line_exact_boundary.py` прошёл
  `py_compile`, 94 строки. Linux test-stage image Rust 1.97 использован для
  runtime; слои Rust test gate были кэшированы и свежий Rust suite не запускался,
  поскольку Rust source не менялся. `cargo fmt --check` и `git diff --check`
  прошли.
- Fixture `./data/acceptance-smtp-data-line-exact-20260927` сохранена.
  Bootstrap контейнер был `--rm`; runtime-контейнер и созданный image удалены,
  отсутствие проверено. Это независимый SMTP peer, не Litterkeep app.
  Результат зарегистрирован как T180 PASS в данном scope; G17 и корневая цель
  остаются ACTIVE.

### 2026-09-27: G17 RFC 6531 SMTPUTF8 guard in inbound BINARYMIME, wire acceptance

- T179 расширен с targeted validator test до независимого implicit-TLS
  SMTP/IMAP acceptance на Linux. EHLO подтверждает CHUNKING/BINARYMIME и
  отсутствие SMTPUTF8; UTF-8 field values без флага отклоняются при BDAT LAST,
  с принудительно отправленным MAIL SMTPUTF8 сообщение доставляется Bob, exact
  payload читается через IMAP и остаётся после рестарта того же контейнера.
- Свежий Docker Rust 1.97 gate: strict Clippy, 727 library tests passed,
  1 внешний Unicode UCD test ignored, 1 CLI test passed, doctests без тестов.
  Python `py_compile`, `cargo fmt --check` и `git diff --check` прошли.
- Это проверка внутренней guard семантики с намеренно forced extension, не
  совместимость клиента по SMTPUTF8 и не полная EAI conformance: сервер не
  рекламирует SMTPUTF8, IDNA и Litterkeep app не тестировались. Fixture
  `./data/acceptance-smtputf8-binarymime-20260927` сохранена; контейнер и образ
  удалены с подтверждением отсутствия. T179 теперь PASS в узком указанном scope;
  G17 и корневая цель остаются ACTIVE.

### 2026-09-27: G17 RFC 6531 SMTPUTF8 required for BINARYMIME headers

- Исправлен входящий BINARYMIME путь: RFC 6532 UTF-8 field values теперь
  отклоняются, если MAIL не содержал valueless `SMTPUTF8`; с флагом они проходят
  ту же MIME-проверку. Это следует из требования RFC 6531 §3.1/§3.4.
- Targeted Linux unit test проверяет оба исхода. Docker Rust 1.97 full gate прошёл:
  strict Clippy, 727 library tests passed, 1 внешний Unicode UCD test ignored,
  1 CLI test passed, doctests без тестов. `cargo fmt --check` и
  `git diff --check` также проверены.
- Определение UTF-8 заголовков выделено в отдельный модульный файл.
  `src/outbound.rs` содержит 305 строк и уже превышал лимит 300 до этого этапа;
  нарушение остаётся открытым и не считается выполненным критерием.
- Это не отдельная wire acceptance, не полная inbound SMTPUTF8/EAI реализация,
  не проверка внешнего MTA или Litterkeep app. G17 и корневая цель остаются
  ACTIVE. Созданный для gate Docker image удалён, отсутствие проверено.

### 2026-09-27: G17 RFC 3030 inbound BINARYMIME durable delivery

- SMTP теперь объявляет `BINARYMIME` вместе с `CHUNKING`, принимает typed
  `BODY=BINARYMIME` и валидирует его по RFC profile: `text/*` и multipart
  preamble/epilogue сохраняют canonical CRLF без DATA limit в 1000 октетов,
  non-text body octets остаются opaque. `DATA` после BINARYMIME MAIL получает
  503 и требует RSET.
- Linux Docker Rust 1.97 full gate прошёл: strict Clippy, 726 library tests
  passed, 1 внешний Unicode UCD test ignored, 1 CLI test passed, doctests без
  тестов. Независимый implicit-TLS peer прошёл SMTP AUTH, MAIL/RCPT/BDAT,
  noncanonical-text отказ, DATA refusal/RSET и чтение exact raw payload Bob
  через IMAP. После restart того же контейнера/Store прежние octets повторно
  прочитаны через IMAP. Проверялся собственный peer-клиент, не Litterkeep app.
- `cargo fmt --check`, `git diff --check`, Python source compilation прошли;
  Rust файлы остаются <=300 строк. Fixture `./data/acceptance-smtp-binarymime-20260927`
  сохранена. Тестовый контейнер и image удаляются и проверяются на отсутствие.
  G17 и корневая цель остаются ACTIVE.

### 2026-09-27: G17 RFC 6531 outbound Unicode envelope requires SMTPUTF8

- Исходящий builder ранее ставил `SMTPUTF8` только для UTF-8 header fields,
  хотя RFC 6531 §3.4 требует его также при Unicode mailbox в SMTP envelope.
  Plain SMTP учитывает sender и recipient; implicit-TLS transport учитывает
  sender и все recipients, включая multi-recipient delivery. Если хотя бы один
  путь требует SMTPUTF8, builder требует peer capabilities `SMTPUTF8` и
  `8BITMIME`, добавляет MAIL flag и не переносит envelope необходимость в
  `BODY=8BITMIME` при ASCII body.
- Targeted Rust 1.97 tests прошли для UTF-8 header path, Unicode envelope и
  отказа peer без SMTPUTF8; strict Clippy и полный Linux Docker/Rust 1.97 gate
  прошли: 723 library tests passed, 1 внешний Unicode UCD test ignored, 1 CLI
  test passed, doctests без тестов. `cargo fmt --check` и `git diff --check`
  прошли.
- Пока проверены builder unit tests, не wire acceptance внешнего MTA и не
  Litterkeep app. Полная SMTPUTF8/EAI поддержка и Unicode домены не заявляются;
  G17 и корневая цель остаются ACTIVE.
- Временный Docker image `litterkeep-linux-test:g17-smtputf8-envelope-20260927`
  будет удалён после проверки.

### 2026-09-27: G17 RFC 3030 BINARYMIME canonical text correction

- RFC 3030 требует canonical CRLF в `text/*`, но это BDAT transfer profile,
  поэтому SMTP DATA line cap в 1000 октетов не должен отбрасывать длинную,
  корректно оформленную текстовую строку. Валидатор теперь отдельно проверяет
  наличие только CRLF в text body и проверяет multipart preamble/epilogue,
  оставляя non-text MIME bodies октетно прозрачными.
- Добавлены регрессии на длинную text line и bare LF в multipart preamble;
  Linux Docker Rust 1.97 gate прошёл: strict Clippy, 722 library tests passed,
  1 внешний Unicode UCD test ignored, 1 CLI test passed, doctests без тестов.
  Windows шесть targeted outbound BDAT тестов и strict Clippy прошли.
- Только independent unit peers/локальные тесты; interoperability с внешним
  MTA или Litterkeep app не проверялась. Это узкая коррекция outbound profile,
  inbound BINARYMIME и сквозная внешняя доставка остаются не реализованными /
  непроверенными. Цель и G17 остаются ACTIVE.
- Временный Docker image `litterkeep-linux-test:g17-binarymime-crlf-20260927`
  будет удалён перед завершением этапа.

### 2026-09-27: G17 outbound RFC 3030 BINARYMIME over BDAT

- Исходящий plain TCP и implicit-TLS SMTP transport теперь передаёт MIME
  `Content-Transfer-Encoding: binary` только если peer объявил одновременно
  `BINARYMIME` и `CHUNKING`. Валидатор требует корректное MIME-дерево и
  canonical CRLF для текстовых частей; нетекстовый binary payload остаётся
  непрозрачным набором октетов. MAIL объявляет точный размер исходных BDAT
  данных, отправка использует bounded chunks, ждёт отдельный 250 на каждый и
  немедленно останавливается после отказного ответа.
- Ограничение длины MAIL включает расширения RFC 3030 и суммирует их с
  allowance SMTPUTF8 из RFC 6531. Отдельные boundary тесты проверили 512, 522,
  528 и 538 октетов.
- Linux Docker Rust 1.97 release build и strict Clippy прошли; полный gate:
  720 library tests passed, 1 внешний Unicode UCD corpus ignored, 1 CLI test
  passed, doctests прошли без тестов. Windows targeted BDAT и BINARYMIME тесты
  прошли (9 и 8), Windows strict Clippy прошёл. Файлы исходников не превышают
  300 строк; `cargo fmt --check` и `git diff --check` прошли.
- Проверены независимые plain SMTP и implicit-TLS unit peers; это не внешний
  MTA и не клиент Litterkeep. Inbound BINARYMIME, durable inbound хранение
  произвольных octets и end-to-end outbound доставка остаются непроверенными.
  Корневая цель и G17 остаются ACTIVE.
- Временный образ `litterkeep-linux-test:g17-outbound-binarymime-20260927`
  удалён; `docker image inspect` подтвердил его отсутствие, связанных
  контейнеров не было.

### 2026-09-27: G17 outbound MIME binary transfer is not sent via DATA

- Исходящая ESMTP-команда теперь проверяет валидное MIME-дерево перед MAIL.
  Если дерево содержит `Content-Transfer-Encoding: binary`, transport завершает
  попытку permanent failure вместо неверной передачи этих octets через DATA.
- Windows Rust 1.97 проверка целевого outbound builder прошла: все 7 его тестов,
  включая новый binary CTE case; strict Clippy прошёл. Независимый plain SMTP
  peer принял EHLO с `CHUNKING` и `8BITMIME` без `BINARYMIME`, затем подтвердил
  EOF без MAIL. Этот checkpoint проверяет только запрет некорректного DATA
  relay; outbound BINARYMIME+BDAT не реализован и не проверен.
- `cargo fmt --check` и `git diff --check` прошли. Тесты выполнены через
  независимый peer, не через приложение Litterkeep. Корневая цель и G17
  остаются ACTIVE.

### 2026-09-27: G17 RFC 3030 typed BODY=BINARYMIME refusal boundary

- RFC 3030 допускает `BINARYMIME` как значение параметра `BODY`; парсер больше
  не классифицирует его как ошибку синтаксиса. Пока сервер не реализовал
  BINARYMIME и не объявляет capability, EHLO оставляет её скрытой, а корректный
  `MAIL ... BODY=BINARYMIME` отклоняется кодом `555` без открытия envelope.
- Linux Docker Rust 1.97 stage прошёл strict Clippy, 708 library tests passed
  (плюс 1 внешний UCD corpus ignored), 1 CLI test и doc-tests. Отдельно
  в Linux контейнере прошли три targeted теста parser/reactor; Windows `cargo
  test --lib` прошёл 633 теста, 1 внешний corpus ignored. `cargo fmt --check`
  прошёл. BINARYMIME payload, durable byte preservation и outbound relay не
  реализованы и не проверялись; это только типизированная отказная граница.
- Временный image `litterkeep-linux-test:g17-binarymime-20260927` удалён;
  отсутствие image и контейнеров подтверждено. Litterkeep app не запускалось.
  G17 и корневая цель остаются ACTIVE.

### 2026-09-27: G17 RFC 5321 MAIL ordering precedes sender policy

- `MAIL` внутри открытой транзакции теперь проверяет командный порядок раньше
  sender authorization и возвращает `503 5.5.1`, сохраняя состояние. Раньше
  чужой sender ошибочно получал `550` из-за policy-проверки, которая должна
  применяться только после проверки допустимой последовательности команд.
- Независимый implicit-TLS тест отправил MAIL Alice, RCPT Bob, затем чужой MAIL
  и получил 503. В той же сессии DATA успешно завершился; Bob прочитал письмо
  через IMAP и подтвердил `Return-Path: <alice@litterkeep.test>`, то есть исходный
  envelope пережил отказ. Linux Docker Rust 1.97 gate: strict Clippy, 698
  library pass, 1 внешний UCD ignored, 1 CLI pass и doctests pass. `cargo fmt
  --check`, Python `py_compile`, `git diff --check` прошли.
- Fixture `./data/acceptance-smtp-mail-order-20260927` сохранена. Это независимый
  wire peer, не клиент Litterkeep. Runtime/test images и container удалены,
  отсутствие будет проверено перед коммитом. G17 остаётся ACTIVE.

### 2026-09-27: G17 RFC 6409 sender-rights rejection code

- Sender mismatch was returning `553 5.7.1`. RFC 6409 §6.1 specifies `550`
  with an appropriate enhanced status for authorization failures. The MSA now
  returns `550 5.7.1`; the negative black-box oracle and null-sender regression
  both enforce that exact response. The null-path acceptance behavior is kept.
- Linux Docker Rust 1.97 test stage passed strict Clippy, 698 library tests
  (1 external Unicode corpus ignored), 1 CLI test, and doctests. The fresh
  implicit-TLS runtime wire test confirmed foreign MAIL rejection with 550,
  null MAIL acceptance, Bob delivery, and `Return-Path: <>`. `cargo fmt --check`,
  Python `py_compile`, and `git diff --check` passed.
- Fixture `./data/acceptance-smtp-null-reverse-20260927` remains. No Litterkeep
  app interoperability is claimed. The created container and images are
  removed, absence confirmed. G17 remains ACTIVE.

### 2026-09-27: G17 RFC 6409 null reverse-path in authenticated SMTP submission

- RFC 6409 §3.2 требует не отклонять submission только из-за `MAIL FROM:<>`,
  если MSA может определить return path из authenticated identity. Политика
  sender ownership теперь допускает только этот пустой reverse-path в TLS
  authenticated session; проверка чужого ненулевого sender остаётся строгой.
- Linux Docker Rust 1.97 test stage прошёл strict Clippy, 698 library tests
  (697 passed, 1 внешний Unicode corpus ignored), 1 CLI test и doctests.
  Отдельно прошли обе новые unit tests. Независимый implicit-TLS SMTP peer
  проверил 553 для чужого sender, acceptance `MAIL FROM:<>`, DATA delivery
  Bob и сохранённый `Return-Path: <>` через IMAP. Python `py_compile`,
  `cargo fmt --check` и `git diff --check` прошли.
- Fixture `./data/acceptance-smtp-null-reverse-20260927` сохранена. Проверен
  локальный AUTH submission и mailbox, не MTA relay наружу и не клиент
  Litterkeep. G17 и корневая цель остаются ACTIVE.
- Созданные для проверки Docker runtime/test images и runtime container
  удаляются после acceptance; отсутствие будет подтверждено перед коммитом.

### 2026-09-27: G17 implicit-TLS SMTP delivery followed by peer close

- Независимый SMTP-клиент подключился к implicit-TLS listener, прошёл AUTH
  PLAIN, отправил DATA и получил `250`; затем инициировал TLS `close_notify`
  без SMTP QUIT. После закрытия Bob вошёл по implicit-TLS IMAP и нашёл письмо
  с уникальным Subject в INBOX. Это подтверждает сохранённую локальную доставку.
- `tests/blackbox/rfc8314_smtp_data_peer_close.py` прошёл на Linux Docker
  runtime. Runtime build использовал cached Rust 1.97 release layer, исходники
  Rust не менялись, поэтому новый Rust compilation/test gate не заявляется.
  `py_compile`, `cargo fmt --check` и `git diff --check` прошли.
- Fixture `./data/acceptance-rfc8314-peer-close-20260927` сохранена. Независимый
  peer доказывает SMTP-to-IMAP доставку и последующий TLS close, не клиент
  Litterkeep. Закрытие до получения SMTP DATA acknowledgement отдельно не
  проверено. G17 и корневая цель остаются ACTIVE.
- Созданные для acceptance runtime container и image удалены, отсутствие
  подтверждено.

### 2026-09-27: G17 RFC 8314 implicit TLS peer closure, SMTP and IMAP wire evidence

- На том же runtime бинарнике независимые TLS peers проверили implicit SMTP
  и IMAP: каждый прочитал greeting, выполнил NOOP, получил обычный protocol
  reply, инициировал `close_notify` и завершил `unwrap()` после ответного alert.
- Runtime использует release build с источниками этапа `85fd9bd`; Docker build
  переиспользовал готовый release layer, поэтому это новый wire acceptance, но
  не новый Rust compilation/test gate. `py_compile`, `cargo fmt --check` и
  `git diff --check` прошли.
- Fixture `./data/acceptance-rfc8314-peer-close-20260927` сохранена. Проверены
  только closure сценарии implicit-TLS SMTP/IMAP, не полный protocol profile и
  не клиент Litterkeep. G17 и корневая цель остаются ACTIVE.
- Созданные runtime container/image удалены, отсутствие подтверждено.

### 2026-09-27: G17 RFC 8314 peer-initiated POP3 TLS closure, scoped PASS

- `ServerTls` сохраняет `peer_has_closed()` от rustls. Reactor обрабатывает
  уже полученный protocol input, ожидает завершения pending Store request,
  дренирует обычные ответы и отправляет TLS `close_notify` перед освобождением
  соединения. POP3 QUIT сохраняет прежний порядок `+OK`, alert, cleanup.
- Linux Docker Rust 1.97 gate прошёл: strict Clippy, 696 library tests,
  1 CLI test и doc-tests; 1 внешний Unicode corpus test ignored из-за
  отсутствующего файла. Независимый POP3 implicit-TLS peer захватил maildrop
  через AUTH PLAIN, отправил alert без QUIT, завершил `unwrap()` и повторно
  аутентифицировал тот же аккаунт, проверив освобождение maildrop lease.
  Отдельное соединение проверило QUIT reply и alert через `unwrap()`.
- Fixture `./data/acceptance-pop3-peer-close-20260927` сохранена. Проверен
  POP3 runtime, не клиент Litterkeep; другие TLS-протоколы и peer close при
  незавершённых Store-командах не являются частью wire acceptance. G17 и
  корневая цель остаются ACTIVE.
- Созданные test/runtime images и runtime container очищены после проверки;
  отсутствие будет сверено перед фиксацией этапа.

### 2026-09-27: G17 RFC 2034 verifier enforces status-class agreement

- Уточнён independent SMTP peer: теперь каждая строка ответа проверяется не
  только на синтаксис enhanced status, но и на совпадение класса `2/4/5` с
  основным SMTP-кодом и ожидаемым ответом; multiline проверяется построчно.
- Regression проверки принимают согласованные ответы и отвергают отсутствующий
  код, несовпадение классов и несоответствие ожидаемому классу. Это усиление
  oracle теста, не новое подтверждение полного SMTP RFC профиля и не повторный
  Docker/Linux runtime прогон. G17 и корневая цель остаются ACTIVE.

### 2026-09-27: G17 SMTP RFC 2034 enhanced status codes, scoped PASS

- EHLO теперь публикует `ENHANCEDSTATUSCODES` в plaintext, pre-TLS и TLS
  вариантах. Обычные SMTP 2xx/4xx/5xx ответы, включая ошибки получателя,
  очередь, HELP и QUIT, содержат согласованный enhanced status code; greeting,
  HELO/EHLO и 3xx continuation остаются исключениями RFC.
- Linux Docker Rust 1.97 test gate: strict Clippy, 696 library tests, 1 CLI test
  и doc-tests прошли; 1 внешний Unicode corpus test ignored из-за отсутствия
  UCD файла. Независимый STARTTLS peer проверил capability до/после TLS,
  AUTH, multiline HELP, ошибки/успех envelope, DATA принятие, NOOP и QUIT.
- Отдельный сценарий `tests/blackbox/smtp_enhanced_status_codes.py` сохраняет
  границу ответственности теста DATA line. Fixture
  `./data/acceptance-smtp-enhanced-status-20260927` сохранена; проверялись
  независимый wire peer и Linux runtime, не Litterkeep app. RFC 2034 coverage
  ограничена описанными ответами; G17 и корневая цель остаются ACTIVE.
- Созданные для проверки Docker image и runtime container удалены, их отсутствие
  подтверждено; fixture сохранена. Этап закоммичен отдельно.

### 2026-09-27: G17 bounded SMTP DATA line buffering, PASS checkpoint

- DATA сессия больше не удерживает неполную строку без CRLF до общего лимита
  размера письма. При получении 1001-го октета без CRLF fragment сбрасывается,
  DATA помечается oversized, последующие octets потоково отбрасываются до
  завершающей точки; затем сервер отвечает 552 и принимает NOOP.
- Linux Docker 1.97 test gate: strict Clippy, 696 library tests, 1 CLI test,
  doc-tests прошли; 1 внешний Unicode corpus test ignored из-за отсутствия
  UCD файла. Independent STARTTLS peer отправил DATA fragment размером больше
  прежнего общего session input cap, проверил 552 и последующий NOOP 250.
  Проверка доказывает wire-синхронизацию и отсутствие старого общего-cap
  disconnect, не является измерением RSS.
- Python acceptance `tests/blackbox/smtp_data_line_bound.py` проходит
  `py_compile` и Linux runtime сценарий. Fixture
  `./data/acceptance-smtp-data-line-bound-20260927` сохранена локально. Это
  ограниченная защита DATA line buffering, не полный SMTP RFC PASS и не тест
  Litterkeep app. Временные image/container удалены после прогона; G17 и корневая
  цель остаются ACTIVE.

### 2026-09-27: G17 инкрементальная классификация SMTP DATA, PASS checkpoint

- Linux strict Clippy и `cargo test --locked` внутри `docker/Dockerfile.linux-test`
  прошли: 695 library tests, 1 CLI test и doc-tests; 1 внешний Unicode corpus
  test ignored из-за отсутствия UCD файла. Linux reactor включён в этот прогон.
- DATA state теперь один раз переключается из заголовков в тело на пустой CRLF
  строке. Удалён повторный просмотр накопленного сообщения на каждой строке,
  который давал квадратичный профиль работы на сообщениях с короткими строками.
  Unit regression проверяет границу и независимые SMTPUTF8/8BITMIME политики.
- Независимый Linux STARTTLS PIPELINING -> DATA -> Store -> IMAP smoke повторно
  прошёл на изменённом бинарнике. Это не замер производительности и не проверка
  Litterkeep app. Fixture `./data/acceptance-smtp-data-streaming-20260927`
  сохранена; тестовые учётные данные лежат только в локальной папке.
- Созданный для прогона Docker container и image подлежат удалению после проверки
  отсутствия. Это ограниченный parser/performance checkpoint, не полный SMTP RFC
  acceptance; G17 и корневая цель остаются ACTIVE.

### 2026-09-27: G17 SMTP PIPELINING all-rejected recipients, PASS checkpoint

- RFC 2920 §3.2 network coverage дополнена противоположным сценарием: MAIL,
  несуществующий RCPT и DATA переданы одним socket write. Сервер по порядку
  ответил 250, 550 и 503, не входя в DATA; последующий NOOP/QUIT подтвердил
  синхронизацию. Существующий успешный pipeline по-прежнему доставил письмо,
  найденное независимым IMAP peer.
- Python `py_compile` и оба Linux STARTTLS SMTP/IMAP wire-сценария прошли.
  Rust исходники не менялись; Docker Rust test-stage был cache hit, поэтому
  свежий полный unit suite не заявляется. Fixture
  `./data/acceptance-smtp-pipelining-all-rejected-20260927` сохранена; точный
  временный контейнер и образ удалены, отсутствие проверено. Проверялся
  независимый peer, не Litterkeep app; это ограниченное RFC 2920 acceptance,
  G17 остаётся ACTIVE.

### 2026-09-27: G17 POP3 CAPA state matrix, PASS evidence checkpoint

- Добавлен независимый STARTTLS wire-клиент, который полностью считывает CAPA
  multiline response в AUTHORIZATION до TLS, AUTHORIZATION после TLS, после
  USER и в TRANSACTION. Все четыре сетевых состояния прошли: общие capability
  теги сохраняются, `STLS` доступен только до TLS, `SASL PLAIN` объявляется
  после TLS.
- Python `py_compile` и сеточная проверка прошли на Linux test image с Rust
  1.97. Docker test-stage был cache hit для неизменённых Rust-исходников, поэтому
  новый полный Rust test run этим этапом не заявляется. Fixture
  `./data/acceptance-pop3-capa-states-20260927` сохранена; точные Docker
  container/image удалены, отсутствие проверено. Клиент независимый, не
  приложение Litterkeep; это CAPA-only coverage и не полный RFC PASS. G17 остаётся
  ACTIVE.

### 2026-09-27: G17 POP3 and Sieve source responsibility split

- Свойства POP3 сообщения вынесены в отдельный `message_profile.rs`; maildrop
  теперь отвечает за snapshot, команды и отложенные удаления. UTF8/maildrop
  проверки вынесены из общего тестового файла в `message_tests.rs`, а Sieve
  grammar regressions отделены в `parser_tests.rs`.
- Поведение не менялось. Windows Rust 1.97.1: 629 library tests + 1 CLI test
  прошли, 1 внешний UCD test ignored; strict Clippy, rustfmt и diff-check прошли.
  Linux Docker Rust 1.97 test-stage: strict Clippy, 695 library tests + 1 CLI
  test прошли, 1 UCD test ignored, doc-tests прошли. Все Rust-файлы в `src`
  не превышают 300 строк; `mod.rs` отсутствует. Точный тестовый Docker image
  удалён, контейнер для этапа не создавался. Это структурная работа, она не
  добавляет RFC-покрытие; G17 и корневая цель остаются ACTIVE.

### 2026-09-27: G17 RFC 6856 USERAccepted is still AUTHORIZATION, correction

- Повторная сверка RFC 1939 §§3-4 и RFC 6856 §§2.1, 3.1 показала, что успешный
  `USER` не переводит POP3-сессию в новое протокольное состояние: до успешного
  PASS/AUTH она остаётся в AUTHORIZATION. Внутренняя фаза `UserAccepted` не
  является дополнительным RFC state. Прежние T160/T161 ошибочно исключали в ней
  `UTF8` и `LANG`; код и их acceptance были скорректированы.
- `UTF8` теперь разрешена в AUTHORIZATION до/после USER, а `LANG` в
  AUTHORIZATION до/после USER и в TRANSACTION. `NOOP` не меняется, поскольку
  RFC 1939 §5 ограничивает именно эту команду состоянием TRANSACTION.
- Unit regressions и независимые Linux STARTTLS peers проверяют обе команды
  после USER, credential-backend отказ от синтаксически корректного PASS,
  разрешённый SASL retry после неудачного PASS и завершение POP3-сеанса.
- Windows Rust 1.97.1: 629 library tests + 1 CLI test прошли, 1 внешний UCD
  test ignored без корпуса; strict Clippy, rustfmt, Python `py_compile` и
  `git diff --check` прошли. Linux Docker Rust 1.97 test-stage: strict Clippy,
  695 library tests + 1 CLI test прошли, 1 UCD test ignored, doc-tests прошли.
  Два независимых Linux STARTTLS peer acceptance-сценария прошли. Fixture
  `./data/acceptance-pop3-rfc6856-useraccepted-20260927` сохранена; точные
  временные Docker container/image удалены и их отсутствие проверено. Только
  независимый peer, не приложение Litterkeep. G17 остаётся ACTIVE.

### 2026-09-27: G17 RFC 1939 POP3 NOOP transaction-only state, PASS checkpoint

- RFC 1939 §5 разрешает `NOOP` только в TRANSACTION. POP3 FSM ошибочно
  возвращала `+OK` также в AUTHORIZATION и USERAccepted; теперь обе ранние
  попытки получают state error, а в TRANSACTION команда продолжает отвечать
  положительно.
- Unit regression и независимый Linux STARTTLS peer проверяют NOOP до USER,
  после USER, ответ credential backend на корректный PASS после второго NOOP,
  затем SASL PLAIN retry после отказа PASS, успешный NOOP после входа и QUIT.
- Windows Rust 1.97.1: 630 library tests (629 passed, 1 внешний UCD test
  ignored без корпуса) и 1 CLI test прошли; strict Clippy, rustfmt и
  `git diff --check` и Python `py_compile` прошли. Linux Docker test stage: 696 library tests
  (695 passed, 1 UCD test ignored), 1 CLI test, doc-tests и strict Clippy
  прошли. Независимый TLS acceptance прошёл. Fixture
  `./data/acceptance-pop3-noop-state-20260927` сохранена; временные Docker
  container и image удалены, отсутствие проверено. Litterkeep app не проверялось; полный
  POP3 профиль и G17 остаются ACTIVE.

### 2026-09-27: G17 RFC 1939 POP3 USER/PASS argument bounds, PASS checkpoint

- RFC 1939 §3 ограничивает каждый аргумент 40 символами, тогда как сервер до
  исправления ограничивал только общую строку в 255 октетов по RFC 2449 и
  принимал username длиной 248. Parser теперь отклоняет USER и PASS длиннее
  40 ASCII octетов, включая PASS с пробелами; предел 255 для прочих команд
  сохранён.
- Unit tests проверяют значения 40/41 и общий размер команды 255/256. Новый
  независимый Linux POP3 STARTTLS acceptance дополнительно проверяет отказ
  больших USER/PASS, успешную аутентификацию следующим корректным обменом и
  STAT/QUIT.
- Windows Rust 1.97.1: 629 library tests (628 passed, 1 внешний UCD test
  ignored без корпуса) и 1 CLI test прошли; strict Clippy, rustfmt, Python
  `py_compile` и `git diff --check` прошли. Linux Docker test stage: 695
  library tests (694 passed, 1 UCD test ignored), 1 CLI test, doc-tests и strict
  Clippy прошли. Независимый TLS acceptance прошёл. Fixture
  `./data/acceptance-pop3-credential-limit-20260927` сохранена; точные runtime
  container и test image удалены, отсутствие проверено. Проверялся независимый
  peer, не Litterkeep app; общий POP3 профиль и G17 остаются ACTIVE.

### 2026-09-27: G17 RFC 6856 LANG authorization and transaction states, SUPERSEDED BY T164

- Первоначальный вывод считал USERAccepted отдельным wire-state и ошибочно
  применял RFC6856 таблицу LANG без учёта состояний RFC1939. T164 исправляет
  разбор: USERAccepted остаётся частью AUTHORIZATION.
- Ранее выполненные тесты точно фиксируют прежнее поведение, но не являются
  подтверждением актуального RFC-поведения; исправленный unit/wire acceptance
  приведён в T164.
- Windows Rust 1.97.1: 626 library tests и 1 CLI test прошли, 1 внешний UCD
  test ignored без корпуса; strict Clippy, rustfmt, Python `py_compile` и
  `git diff --check` прошли. Linux Docker test stage: strict Clippy, 692 library
  tests и 1 CLI test прошли, 1 UCD test ignored, doc-tests прошли. Fixture
  `./data/acceptance-pop3-lang-state-20260927` сохранён. Точный временный Docker
  container/image удалён, отсутствие проверено. Независимый peer, не приложение
  Litterkeep; полный POP3 профиль и G17 остаются ACTIVE.

### 2026-09-27: G17 RFC 6856 UTF8 command authorization state, SUPERSEDED BY T164

- Первоначальный вывод ошибочно трактовал внутреннюю фазу USERAccepted как
  отдельное состояние протокола. RFC 1939 оставляет сессию в AUTHORIZATION до
  завершения authentication; T164 разрешает UTF8 во всей этой фазе.
- Ранее собранное доказательство отражает старое поведение и не подтверждает
  RFC-конформность; исправленные state/unit/wire проверки перечислены в T164.
- Windows Rust 1.97.1: 625 library tests и 1 CLI test прошли, 1 внешний UCD
  test ignored без корпуса; strict Clippy, rustfmt, Python `py_compile` и
  `git diff --check` прошли. Linux Docker test stage: strict Clippy, 691 library
  tests и 1 CLI test прошли, 1 UCD test ignored, doc-tests прошли. Fixture
  `./data/acceptance-pop3-utf8-auth-state-20260927` сохранён. Проверочные Docker
  container/image удалены, отсутствие проверено.

### 2026-09-27: G17 RFC 5321 RSET before SMTP authentication, PASS checkpoint

- RFC 5321 §4.1.4 разрешает RSET в любой момент и рекомендует отвечать на
  session-control commands до EHLO. После STARTTLS `RSET` ошибочно проходил
  через guard, который требует authentication для DATA, и получал `530`.
  Guard теперь защищает только DATA; RSET сбрасывает транзакцию и до AUTH.
- Независимый Linux TCP/TLS acceptance проверил STARTTLS, затем RSET до нового
  EHLO и AUTH с ответом `250`; последующие EHLO, NOOP и QUIT подтвердили
  корректное состояние session. Сценарий не подменяет проверку клиента
  Litterkeep.
- Windows Rust 1.97.1: 624 library tests и 1 CLI test прошли, 1 внешний UCD
  test ignored без корпуса; strict Clippy, rustfmt, Python `py_compile` и
  `git diff --check` прошли. Linux Docker test stage: strict Clippy, 690 library
  tests и 1 CLI test прошли, 1 UCD test ignored, doc-tests прошли. Fixture
  `./data/acceptance-smtp-rset-before-auth-20260927` сохранён. Временный Docker
  container/image удалён, отсутствие проверено.

### 2026-09-27: G17 RFC 6531 VRFY/EXPN request syntax, PARTIAL

- RFC 6531 §3.7.4.2 задаёт optional valueless `SMTPUTF8` для VRFY/EXPN и
  разрешает UTF-8 query; marker относится к формату возможного ответа, а не к
  параметрам MAIL. Parser теперь сохраняет query и enum `ReplyEncoding`,
  проверяет controls и отвергает `SMTPUTF8=value`.
- SMTP capability остаётся не объявленной. Независимый Linux TCP peer проверил
  ASCII/UTF-8 query с marker и без него, неправильное значение marker, коды
  ответов и восстановление через NOOP. Ответы не раскрывают mailbox: VRFY
  остаётся `252`, EXPN `502`, весь ответ ASCII. Первоначальная проверка ошибочно
  связывала UTF-8 query с marker-ом; это исправлено в T158. Это parser/fallback
  checkpoint, не SMTPUTF8 interoperability.
- Windows Rust 1.97.1: 623 library и 1 CLI test прошли, один внешний UCD test
  ignored из-за отсутствующего корпуса; strict Clippy и rustfmt прошли.
  Linux Docker: strict Clippy, 689 library и 1 CLI test прошли, один UCD test
  ignored; doc-tests прошли. Python `py_compile` и независимый wire acceptance
  прошли. Fixture `./data/acceptance-smtp-smtputf8-verify-query-20260927`
  сохранён; временные Docker image и контейнер удалены.

### 2026-09-27: G17 RFC 6531 VRFY/EXPN query semantics correction, PASS

- Повторная сверка RFC 6531 §3.7.4.2 выявила ошибочную связь между UTF-8
  символами в query и valueless `SMTPUTF8`: marker сообщает о способности
  клиента читать UTF-8 в ответе, но не является разрешением UTF-8 в запросе.
  Парсер теперь независимо валидирует query и сохраняет reply capability.
- Независимый Linux raw-TCP сценарий прошёл для ASCII VRFY, Unicode VRFY без
  marker-а, Unicode VRFY/EXPN с marker-ом, ASCII-only ответов, отсутствующей
  SMTPUTF8 advertisement, отказа от `SMTPUTF8=YES` и восстановления через
  NOOP. При этом сервер намеренно не возвращает UTF-8 identity в ответе.
- Windows Rust 1.97.1: 624 library tests и 1 CLI test прошли, 1 внешний UCD
  test ignored без корпуса; strict Clippy, rustfmt и diff-check прошли. Linux
  Docker test stage: strict Clippy, 690 library tests и 1 CLI test прошли,
  1 UCD test ignored, doc-tests прошли. Python wire acceptance прошёл.
  Fixture `./data/acceptance-smtp-smtputf8-vrfy-query-semantics-20260927`
  сохранён. Созданный контейнер и image удалены, отсутствие проверено.
- Это независимый parser/wire test; capability остаётся не объявленной и
  подключение клиента Litterkeep не проверялось.

### 2026-09-27: G17 RFC 6531 sender/recipient enhanced-status wire evidence, PASS

- RFC 6531 §3.5 требует `550` для отказа по MAIL reverse-path и `553` для
  отказа по RCPT forward-path; при поддержке enhanced status оба используют
  `5.6.7`. Независимый TCP acceptance теперь проверяет не только трехзначные
  ответы, но и контекстные typed значения enhanced status для обеих команд.
- Linux raw-TCP сценарий прошёл на server image, собранном из текущего source:
  RCPT без активного SMTPUTF8 получил `553 5.6.7`, MAIL без флага получил
  `550 5.6.7`, после отказов та же сессия приняла RSET и корректный помеченный
  MAIL/RCPT flow. Capability по-прежнему не объявляется; это не SMTPUTF8 full
  conformance и не проверка клиента Litterkeep.
- Rust source не менялся после полного Windows/Linux suites из T155; в этом
  checkpoint повторно проверены только Python `py_compile`, формат diff и
  независимый Linux wire path. Fixture
  `./data/acceptance-smtp-smtputf8-enhanced-status-20260927` сохранён; точные
  временные image и контейнер удалены, отсутствие проверено.

### 2026-09-27: G17 RFC 6531 MAIL SMTPUTF8 refusal status, PASS checkpoint

- Повторная сверка RFC 6531 §3.5 обнаружила, что UTF-8 reverse-path без
  `SMTPUTF8` классифицировался как синтаксическая ошибка `501`, хотя отказ
  отправителю из-за ASCII-only ограничений должен иметь SMTP код `550` и
  enhanced status `5.6.7`.
- Ошибка разбора теперь типизирована как `SmtpUtf8Required`; SMTP reactor
  сериализует `550 5.6.7`. Parser regression и независимый raw-TCP клиент
  проверяют код ответа, последующее восстановление сессии и успешный MAIL с
  явным флагом. EHLO capability по-прежнему скрыта: IDNA и остальные условия
  полной SMTPUTF8 реализации не выполнены.
- Windows Rust 1.97.1: 620 library tests прошли, 1 внешний Unicode corpus
  ignored из-за отсутствующего файла; 1 CLI test, doc-tests, strict Clippy,
  rustfmt и `git diff --check` прошли. Linux Docker: strict Clippy, 686 library
  tests и 1 CLI test прошли, 1 такой же corpus test ignored, doc-tests прошли.
  Independent Linux raw-TCP policy test прошёл; это не проверка Litterkeep.
- Fixture `./data/acceptance-smtp-smtputf8-mail-reply-20260927` сохранён.
  Проверочный контейнер и image удалены, отсутствие обоих проверено.

### 2026-09-27: G17 IMAP LIST extension capability advertisement, PASS checkpoint

- Аудит RFC 5258/5819 обнаружил, что сервер исполнял extended LIST и
  `LIST RETURN STATUS`, но не объявлял `LIST-EXTENDED` и `LIST-STATUS`.
  Capability builder теперь объявляет обе поддерживаемые возможности во всех
  TLS/STARTTLS/plaintext ответах.
- Независимый Python IMAP client проверяет оба capability до TLS и после
  STARTTLS, затем включает `UTF8=ACCEPT`, создаёт mailbox с `é` и проверяет
  соседние LIST/STATUS responses с UTF-8 именем и запрошенными полями.
  `RECENT` разрешён в typed `LIST RETURN STATUS`: неполученные сообщения
  учитываются до SELECT, а после SELECT используется session-owned набор
  recent UID. Отдельный STATUS и LIST RETURN STATUS согласованы с этим набором.
  Существующая recursive SUBSCRIBED/CHILDINFO acceptance тоже проходит.
- Windows Rust 1.97.1 fmt, strict Clippy, 605 library tests, 1 CLI test и
  doc-tests pass. Linux Docker Rust 1.97 release, strict Clippy, 668 library
  tests, 1 CLI test и doc-tests pass; один внешний Unicode corpus test ignored
  из-за отсутствующего файла. Python syntax check pass; black-box script 297 строк.
- Fixture остаётся в `./data/acceptance-imap-utf8-list-status-20260927`.
  Временные Docker container/image удалены, имена проверены; контейнеры других
  проектов не изменялись. Это закрывает только capability advertisement,
  RECENT в LIST STATUS и проверенные extended-LIST wire paths, не весь RFC
  9755/IMAP профиль.

### 2026-09-27: G17 POP3 UTF-8 mode rejects unconvertible high-bit content, PASS checkpoint

- Второй аудит RFC 6856 §2.1 выявил, что предыдущий ASCII guard исправил
  утечку high-bit octets до UTF8, но пропускал opaque `0xFF` после команды
  UTF8. Режим должен передавать корректное UTF-8 содержимое; неподдержанную
  legacy-кодировку следует преобразовать либо отклонить, а не объявлять UTF-8.
- Maildrop profile теперь классифицируется типизированными вариантами ASCII,
  UTF-8, opaque и mixed. UTF8 mode передаёт ASCII/валидный UTF-8; opaque и
  смешанные повреждённые последовательности получают `-ERR`. Валидный UTF-8
  сохраняется побайтно. Обнаружение и проверка профиля выполняются при создании
  snapshot и используют кэш на LIST/TOP/RETR.
- Windows Rust 1.97.1 fmt, strict Clippy, 605 library tests, 1 CLI test и
  doc-tests pass. Linux Docker Rust 1.97 release, strict Clippy, 668 library
  tests, 1 CLI test и doc-tests pass; один внешний Unicode corpus test ignored,
  потому что файл корпуса отсутствует.
- Independent Linux STARTTLS acceptance доставляет валидное UTF-8 письмо через
  SMTP и добавляет opaque-octet сообщение через IMAP APPEND. Проверены `[UTF8]`
  в ASCII mode, отказ opaque bytes до и после UTF8 и byte-exact retrieval
  валидного UTF-8 сообщения. Litterkeep клиент не входит в сценарий.
- Stage data остаётся в `./data/acceptance-pop3-utf8-validation-20260927`.
  Временные контейнер и image удалены; точные имена проверены и отсутствуют.
  Остальные контейнеры других проектов не изменялись.

### 2026-09-27: G17 POP3 ASCII-mode high-bit guard, PASS checkpoint

- RFC 6856 §2.1 forbids sending non-ASCII character strings unchanged when
  UTF8 mode is disabled. Maildrop detection previously searched only for valid
  non-ASCII UTF-8 scalars, so an opaque `0xFF` with no other UTF-8 sequence could
  pass the ASCII-mode retrieval guard.
- Maildrop now caches both high-bit presence and valid UTF-8 presence. LIST n,
  TOP and RETR fail closed for opaque high-bit content in ASCII mode; `[UTF8]`
  is reserved for content containing a valid non-ASCII UTF-8 scalar. Explicit
  UTF8 mode still permits the exact opaque octets when the MIME binary-CTE
  policy allows retrieval.
- Windows Rust 1.97.1 fmt, strict Clippy, 605 library tests, 1 CLI test and
  doc-tests pass. Linux Docker Rust 1.97 release build, strict Clippy, 668
  library tests, 1 CLI test and doc-tests pass; one external Unicode corpus test
  is ignored because its data file is absent.
- Independent Linux POP3 STARTTLS wire acceptance delivered a normal 8BITMIME
  UTF-8 message via SMTP and appended a separate `application/octet-stream`
  fixture containing only `0xFF` via IMAP. LIST/TOP/RETR reject the first with
  `[UTF8]` and the second with generic `-ERR`; prior capability, STLS ordering,
  and binary-CTE cases also pass. No Litterkeep client was tested.
- Fixture `./data/acceptance-pop3-opaque-octets-20260927` is retained. The
  disposable Docker container and test image are removed and checked after
  acceptance; unrelated containers are not part of this test.
- Этот checkpoint фиксировал промежуточное поведение: последующая сверка
  RFC 6856 уточнила, что opaque octets нельзя выдавать и после UTF8 без
  charset conversion. Исправление и отдельная проверка записаны выше.

### 2026-09-27: G17 preserve opaque octets in IMAP LOGIN password literals, PASS checkpoint

- RFC synchronizing literal framing is octet-based. After CRLF framing was
  corrected, a second loss point remained: the receiver and tokenizer forced
  every literal through `String::from_utf8`, despite the auth owner already
  accepting password `Vec<u8>`.
- IMAP lexer now represents received literals as `Token::Literal(Vec<u8>)`;
  LOGIN password parsing preserves those octets and routes them to the existing
  credential owner without UTF-8 conversion. Username, mailbox, and SEARCH
  literals remain text-valued and are validated by their command parser.
  Embedded NUL is still rejected as binary string data.
- Windows Rust 1.97.1 formatting, strict Clippy, 604 library tests, 1 CLI test
  and doc-tests pass. Linux Rust 1.97 Docker test stage passes release build,
  strict Clippy, 667 library tests, 1 CLI test and doc-tests; one external
  Unicode corpus test is ignored because the file is absent.
- Independent raw STARTTLS client sent password literal byte `0xFF` and observed
  ordinary authentication failure, sent a separate CRLF-containing bad password
  and recovered, then authenticated with the correct literal password. It also
  created, listed and selected a literal mailbox before successful NOOP. This
  verifies byte preservation through lexer, typed LOGIN command and auth owner.
- Fixture `./data/acceptance-imap-login-octets-20260927` is retained. The
  disposable runtime container and image were removed and absence verified.
  Invalid UTF-8 remains unsupported for text-valued username/mailbox/search
  arguments, so the checkpoint does not claim arbitrary-octet support throughout
  IMAP or complete IMAP4rev2 conformance.

### 2026-09-27: G17 IMAP synchronizing literal CRLF payload, PASS checkpoint

- RFC 9051 §4.3 explicitly includes CR and LF among synchronizing literal
  octets. The receiver previously rejected either byte after consuming the
  advertised count, so valid payload bytes were treated as a framing error.
- Receiver now frames by octet count and preserves embedded CRLF in the typed
  literal value. Raw NUL and invalid UTF-8 remain rejected by the current text
  token model; this checkpoint does not claim arbitrary-octet string support.
- Windows Rust 1.97.1 strict Clippy, fmt, 604 library tests, 1 CLI test and
  doc-tests pass. The black-box LOGIN scenario first sends a password literal
  containing CRLF, expects authentication failure, then retries valid literal
  LOGIN and exercises mailbox CREATE/LIST/SELECT.
- Linux Docker Rust 1.97 test stage passes strict Clippy, 666 library tests,
  1 CLI test and doc-tests; one external Unicode corpus test is ignored because
  its data file is absent. Independent STARTTLS client received a normal NO for
  a password literal containing CRLF, then authenticated with the right literal
  password and completed CREATE/LIST/SELECT. This confirms CRLF octets did not
  desynchronize the command or session.
- Fixture `./data/acceptance-imap-literal-crlf-20260927` is retained; its
  disposable runtime container and image were removed, and absence was verified.
  Invalid UTF-8 literal octets remain unsupported; this is not a full arbitrary
  octet or complete IMAP4rev2 claim.

### 2026-09-27: G17 broader IMAP mailbox synchronizing literals, PASS checkpoint

- RFC 9051 mailbox command arguments are `astring`, and literals are a general
  string form. Reactor admission now recognizes mailbox arguments for CREATE,
  DELETE, RENAME, SUBSCRIBE, UNSUBSCRIBE, LIST, LSUB, STATUS, COPY, MOVE,
  SELECT and EXAMINE, plus UID COPY/MOVE. Existing LOGIN and SEARCH paths remain.
- Added independent STARTTLS raw client coverage for CREATE, LIST and SELECT
  using literal mailbox names. The first fixture name collided with bootstrap's
  pre-existing Invoices folder and correctly received ALREADYEXISTS; it was not
  counted as acceptance, and the scenario was changed to a unique LiteralStage.
- Windows Rust 1.97.1 fmt, strict Clippy and full suite pass: 604 library tests,
  1 CLI test and doc-tests; Python compile passed. Linux Docker Rust 1.97 test
  stage passes strict Clippy, 666 library tests, 1 CLI test and doc-tests; one
  external Unicode corpus test remains ignored without its corpus file.
- Independent raw STARTTLS client created LiteralStage, found it with LIST using
  a literal pattern, selected it by literal and continued with NOOP. Fixture
  `./data/acceptance-imap-mailbox-literal-20260927` is retained. Its disposable
  runtime container and image were removed and absence verified. Other user
  containers were left running. This does not establish literals for every
  IMAP argument or compatibility with the Litterkeep app.

### 2026-09-27: G17 IMAP LOGIN synchronizing literals, scoped PASS

- RFC 9051 §4.3 допускает synchronizing literal как общую форму IMAP string;
  LOGIN передаёт username/password как `astring`. Ранее reactor принимал
  command literals только для SEARCH, поэтому корректный literal LOGIN
  отклонялся до типизированного LOGIN parser-а.
- Существующий bounded receiver теперь распознаёт LOGIN и сохраняет общую
  проверку размера (64 KiB на literal, общий лимит команды), CRLF framing,
  последовательные continuation и синхронизацию pipelined ввода. APPEND остаётся
  отдельным receiver-ом. Parser/tokenizer entry points переименованы в общие
  literal terms. LITERAL+ не рекламируется и поддержка literal другими IMAP
  командами не подразумевается.
- Windows Rust 1.97.1: fmt, strict Clippy и полный тестовый набор (604 library,
  1 CLI, doc-tests) прошли; Python `py_compile` прошёл. Первый фильтр reactor
  тестов на Windows не выбрал platform-gated Linux tests, что не засчитывалось
  как receiver-проверка.
- Linux Docker Rust 1.97: release build, strict Clippy, 667 library tests,
  1 CLI test и doc-tests прошли; один Unicode corpus test остаётся ignored без
  внешнего файла. Независимый raw STARTTLS acceptance выполнил два
  synchronizing literals с ожиданием обоих `+`, получил успешный LOGIN и
  authenticated NOOP. Это не проверка приложения Litterkeep.
- Fixture сохранён в `./data/acceptance-imap-login-literal-20260927`; созданные
  container и test image удалены, отсутствие проверено. Остальные контейнеры не
  затронуты. Полный IMAP4rev2 профиль и общий G17 остаются ACTIVE.

### 2026-09-27: G17 POP3 PASS password spaces, PASS

- POP3 command parser теперь трактует весь остаток `PASS` после первого SP как
  единственный password, сохраняя внутренние, начальные и конечные пробелы.
  `USER` остаётся ровно одним непустым токеном; пустой USER/PASS отвергается.
- RFC 1939 §7.3 разрешает включать пробелы в парольный аргумент. Unit проверяет
  точные password bytes на выходе session к authentication owner. Независимый
  Linux STARTTLS peer получил ожидаемый backend `authentication failed` на
  неверный пароль с пробелами, затем успешно повторил USER.
- Windows Rust 1.97.1: полный library suite 604 passed, 1 ignored; strict
  Clippy, fmt и Python `py_compile` прошли. Linux Docker Rust 1.97: release,
  strict Clippy, 665 library + 1 binary test, doc-tests; 1 внешний Unicode
  corpus test ignored.
- Runtime container и временный test image удалены; тестовые credentials
  сохранены в `./data/acceptance-pop3-pass-spaces-20260927` без вывода секрета.
  Acceptance не утверждает успешную проверку действующего пароля с пробелами.

### 2026-09-26: G17 POP3 сохранение CR вне CRLF, PASS

- POP3 ранее удалял любой `CR` из размера `LIST` и тела `RETR/TOP`. Теперь
  нормализация удаляет только `CR`, непосредственно составляющий пару с `LF`;
  отдельный CR сохраняется как октет и входит в размер CRLF-представления.
- Регрессия проверяет CR перед обычным символом и в конце последней строки:
  проверены точные `RETR` и `TOP` bytes, а также `LIST` size.
- Windows Rust 1.97.1: полный library suite 603 passed, 1 ignored; strict
  Clippy и fmt прошли. Linux Docker test-stage Rust 1.97: release build,
  strict Clippy, 664 library + 1 binary test, doc-tests; 1 внешний Unicode
  corpus test ignored.
- Временный test image `litterkeep-pop3-cr-preservation-test:work` удалён;
  созданный для acceptance контейнер не требовался. Заодно выявленный после
  прошлого acceptance временный `litterkeep-nfc-legacy-compat` остановлен и
  удалён вместе с его двумя проверенными test/runtime images; каталог
  `./data/g23-nfc-legacy-folder-list-wire` сохранён. Посторонний `devana-v2`
  контейнер не затронут. Изменение не заявляет поддержку malformed bare-CR
  RFC822 клиентами.

### 2026-09-26: G17 RFC 5322 WSP in MIME structured values, PASS

- MIME media type, параметр и Content-Disposition больше не используют
  Unicode-aware `trim()`: внешняя обрезка принимает только RFC WSP (SP/HTAB).
  NBSP остаётся допустимым значением внутри quoted-string и не исчезает.
- Новая regression сначала упала на NBSP перед `text/plain`; после изменения
  MIME parameter tests (19), disposition tests (5) и полный Windows library
  suite (569 + 1 binary test) прошли. `cargo fmt --all -- --check`, strict
  Clippy и `git diff --check` прошли. Все изменённые Rust-файлы меньше 300
  строк.
- Linux Docker PASS: Rust 1.97 release build, strict Clippy, 620 library + 1
  binary test и doc-tests. Созданный test image удалён; отдельная проверка
  подтвердила отсутствие image и container с именем этапа. Данные этапа
  сохранены в `./data/g17-mime-ascii-wsp`.
- PASS: новый независимый `tests/blackbox/mime_structured_wsp.py` выполнил
  IMAP STARTTLS LOGIN/CREATE/APPEND/FETCH через Python `imaplib`. Проверены
  fallback malformed Content-Type, literal parameter с quoted NBSP,
  отбрасывание malformed Content-Disposition и точное совпадение исходных
  RFC822 bytes. Временные runtime container и image удалены и отсутствие
  повторно подтверждено. Litterkeep client не проверялся.

### 2026-09-26: G17 RFC 2045 Content-Transfer-Encoding token checkpoint

- Устранено принятие невалидного CTE `base64` с Unicode NBSP: parser теперь
  ограничивает value одним ASCII token, допускает внешние SP/HTAB, отвергает
  tspecials и не ломает typed UNKNOWN-CTE для будущих корректных ASCII tokens.
  Sieve vacation `:mime` удаляет RFC comments перед общим token parser-ом.
- До исправления новая регрессия падала именно на NBSP. После исправления
  Windows Rust 1.97.1: 566 библиотечных и 1 бинарный тест, strict Clippy,
  `cargo fmt --check`; Linux Docker Rust 1.97: 617 библиотечных и 1 бинарный
  тест, doc-tests и strict Clippy. Независимый ManageSieve TLS CHECKSCRIPT
  принял comments и отверг NBSP. Временные test/runtime images и container
  удалены; данные этапа сохранены в `./data/g18-mime-cte-token`. G17 остаётся
  ACTIVE.

### 2026-09-26: G17 RFC 6532 message/global independent IMAP wire checkpoint

- Добавлен самостоятельный `tests/blackbox/imap_message_global.py`: через
  независимый `imaplib` клиент он загружает Base64 `message/global` с UTF-8
  вложенными заголовками и проверяет BODYSTRUCTURE, сохранение encoded
  `BODY[1]`, decoded `BODY[1.1]` и `BODY[1.HEADER]`, decoded `BINARY[1.1]` и
  точное сохранение исходного RFC822 literal.
- PASS: Python `py_compile`; Linux Rust 1.97 release runtime image собран;
  независимый STARTTLS socket acceptance прошёл в отдельном
  `./data/g18-message-global-wire`. Временные контейнер и образ после теста
  удалены и отсутствие проверено; acceptance data сохранена. Litterkeep app,
  полный MIME профиль и IMAP4rev2 не проверялись. G17 остаётся ACTIVE.

### 2026-09-26: G17 RFC 6532 UTF-8 quoted MIME parameters, PARTIAL

- RFC 6532 quoted-string extension подтверждён для MIME parameter values:
  общий parser сохраняет прямой UTF-8 в quoted `name`, отдельно от RFC 2231
  `filename*`. Добавлены unit и независимая IMAP `imaplib` wire-проверки;
  BODYSTRUCTURE вернул оба Unicode значения и оригинальный RFC822 literal
  совпал побайтно. Это не проверяет SMTPUTF8 transfer.
- PASS: Windows Rust 1.97.1 fmt, 564 библиотечных и 1 бинарный тест, strict
  Clippy; Linux Docker Rust 1.97 release build, strict Clippy, 615 библиотечных
  и 1 бинарный тест, doc-tests. Независимый implicit-TLS IMAP acceptance
  прошёл. Временные test/runtime images и container удалены, отсутствие
  проверено; существующий `./data/g17-rfc2231-lowercase-wire` сохранён. G17
  остаётся ACTIVE.

### 2026-09-26: G17 RFC 2231 Sieve coverage consistency, PARTIAL

- `vacation_mime` использует общий MIME parameter decoder, поэтому lowercase
  `%HH` escapes уже поддерживались после предыдущего исправления. Устаревшее
  утверждение в Sieve-матрице о требовании uppercase удалено; добавлен unit
  regression для Sieve vacation `:mime` с lowercase и mixed-case UTF-8 filename.
- PASS: Windows Rust 1.97.1 fmt, 563 библиотечных и 1 бинарный тест, strict
  Clippy; Linux Docker Rust 1.97 release build, strict Clippy, 614 библиотечных
  и 1 бинарный тест, doc-tests. Временный image удалён и проверен на
  отсутствие; контейнер не создавался. Sieve-specific wire acceptance не
  выполнялся. G17 остаётся ACTIVE.

### 2026-09-26: G17 RFC 2231 case-insensitive percent escapes, PARTIAL

- MIME extended parameters теперь принимают lowercase и mixed-case
  hexadecimal digits в `%HH`. RFC 2231 использует ABNF HEXDIG, где буквенные
  terminals нечувствительны к регистру. Удалено прежнее неверное ожидание,
  запрещавшее `%2a`; malformed non-hex escapes всё ещё отвергаются.
- PASS: Windows Rust 1.97.1 fmt, 562 библиотечных и 1 бинарный тест, strict
  Clippy; Linux Docker Rust 1.97 release build, strict Clippy, 613 библиотечных
  и 1 бинарный тест, doc-tests. Временный test image удалён и проверен на
  отсутствие. Отдельный Linux runtime-контейнер принял IMAP implicit TLS,
  LOGIN, APPEND и BODYSTRUCTURE FETCH от независимого `imaplib`; смешанные
  lowercase/uppercase escapes декодированы в filename, исходные RFC822 октеты
  совпали. Runtime-контейнер и оба временных образа удалены и проверены на
  отсутствие; `./data/g17-rfc2231-lowercase-wire` с acceptance состоянием
  сохранён. Litterkeep app interoperability не проверялась. G17 остаётся ACTIVE.

### 2026-09-26: G17 RFC 6532 encoded message/global ranges, PARTIAL

- `message/global` теперь декодирует Base64 и quoted-printable до разбора
  вложенного сообщения. Decoded bytes принадлежат отдельному `Arc` source;
  offsets узлов разрешаются относительно владельца и не смешиваются с
  исходными encoded offsets. Unknown CTE остаётся opaque
  `application/octet-stream` согласно MIME fallback.
- IMAP part resolver различает сам part и embedded message: `BODY[1]`
  возвращает сохранённое encoded body, `BODY[1.1]` выбирает decoded nested
  part, `BODY[1.HEADER]` читает заголовки embedded message. Проверены также
  nested BINARY и part numbering для multipart-вложения. Это unit evidence;
  независимый network black-box и Litterkeep acceptance ещё не выполнялись.
- Целевые IMAP/MIME тесты и strict Clippy прошли на Windows Rust 1.97.1.
  Полный Windows набор: 561 библиотечный и 1 бинарный тест, fmt и strict
  Clippy прошли. Linux Docker Rust 1.97: release build, strict Clippy,
  612 библиотечных и 1 бинарный тест и doc-tests прошли. Временный test image
  удалён; контейнер не создавался. Независимый сетевой black-box и Litterkeep
  acceptance не выполнялись. Goal остаётся ACTIVE.

### 2026-09-26: G17 RFC 6532 message/global MIME и IMAP BODYSTRUCTURE, PARTIAL

- MIME media type теперь различает `message/global` enum-вариантом. Для
  identity CTE (`7bit`, `8bit`, `binary`) сохраняется вложенное сообщение с
  UTF-8 полями заголовка; IMAP `BODYSTRUCTURE` выдаёт тип `MESSAGE/GLOBAL`, а
  BODY HEADER selection принимает путь к его внутреннему сообщению.
- Изначальный срез поддерживал только identity CTE и не имел модели ranges
  для отдельно декодированного сообщения. Реализация расширена в checkpoint-е
  выше: Base64 и quoted-printable теперь разбираются через отдельный Arc
  source, а неизвестный CTE остаётся непрозрачным fallback. IMAP4rev2 всё ещё
  не рекламируется. Сетевой клиент и Litterkeep acceptance не выполнялись.
- Предыдущая проверка identity-only superseded тестами и gate-ами checkpoint-а
  выше. G17 остаётся активной.

### 2026-09-26: G17 RFC 2045 quoted-printable soft-break whitespace, PASS

- Исправлена ошибочная потеря буквальных SP/TAB перед `=CRLF`: по RFC 2045
  §6.7 эти octets входят в `qp-segment` и являются содержимым, не transport
  padding. Padding после последнего символа hard line продолжает удаляться.
  Тот же корректный случай теперь проходит строгий Sieve `:mime` validator.
  Этим исправлены неверные ожидания двух checkpoint-ов от 2026-09-25 ниже.
- Unit-векторы проверяют пример RFC, safe/hex octets, hard-line padding и
  77-октетный malformed input без усечения. Независимые SMTP STARTTLS -> IMAP
  STARTTLS BINARY.PEEK сравнил весь decoded body; ManageSieve CHECKSCRIPT
  подтвердил принятие soft break с предшествующим SP. Клиент Litterkeep не
  проверялся.
- PASS: Windows Rust 1.97.1 `cargo fmt --check`, 552 библиотечных и 1
  бинарный тест, strict Clippy. Linux Docker Rust 1.97 strict Clippy, release
  build, 603 библиотечных и 1 бинарный тест, doc-tests. Временные container и
  test image удалены; acceptance данные сохранены в
  `./data/g17-mime-qp-soft-break-wire`. G17 остаётся активной.

### 2026-09-26: G17 RFC 5321 SMTP reply без текста, PASS

- Исходящий SMTP-клиент принимает финальные ответы из трёх цифр с CRLF и
  ответы с пустым текстом после пробела; multiline separator и strict CRLF
  сохранены. Он также сохраняет обработку будущих числовых кодов по первому
  разряду, не ограничивая диапазон только известной таблицей ответов.
- Приёмка: `outbound_bare_reply_tests::delivers_when_peer_omits_reply_text`
  использует независимый TCP peer и проверяет коды 220, 250, 354 и 221 по
  всему пути доставки. Unit-тесты покрывают bare code, пустой текст,
  continuation, нецифровой code и LF-only framing, а отдельная регрессия
  сохраняет поддержку будущего положительного кода 299 по первому разряду.
  PASS: Windows Rust 1.97.1 fmt, strict Clippy, 548 библиотечных и 1 бинарный
  тест; Linux Docker Rust 1.97 strict Clippy, 599 библиотечных и 1 бинарный
  тест, doc-tests. Тестовый образ удалён; Litterkeep interoperability не
  проверялась. G17 остаётся активной.

### 2026-09-26: G17 RFC 5321 неизвестная SMTP-команда и разделение 500/501, PASS

- Парсер отличает неизвестный verb от неверного синтаксиса аргумента
  распознанной команды. Неизвестная команда получает 500, а неверные
  аргументы DATA и некорректное разделение TAB получают 501. Обработка
  продолжается в том же соединении.
- PASS: Windows Rust 1.97.1: 545 библиотечных и 1 бинарный тест. Linux Docker
  Rust 1.97: strict Clippy, 596 библиотечных и 1 бинарный тест и doc-tests.
  Повторяемый `tests/blackbox/smtp_unknown_command.py` на Linux raw-TCP проверил
  приветствие, WUT/500, DATA с аргументом и TAB/501, NOOP/250 и QUIT/221 в
  одной сессии. Litterkeep-клиент не проверялся.
  Тестовый контейнер и оба уникальных образа удалены; данные и TLS fixture
  оставлены в `./data/g18-smtp-unknown-command`. G17 остаётся активной.

### 2026-09-26: G17 RFC 5321 server command and DATA block timeouts, PASS

- RFC 5321 §4.5.3.2.7 command idle keeps its five-minute interval; §4.5.3.2.5
  DATA inactivity now uses a separate three-minute deadline. The DATA clock is
  tied to socket reads, not parsing of buffered lines, preventing stale buffered
  content from refreshing an incomplete block. Worker ownership and pending
  replies remain outside these timeouts.
- PASS: Windows Rust 1.97.1 fmt, 544 library + 1 binary tests and strict
  Clippy. Linux Docker Rust 1.97 strict Clippy, 595 library + 1 binary tests,
  and doc-tests. New Linux lifecycle tests verify command and DATA boundaries,
  cancellation of the lease at deadline, pending-worker retention, and that
  parsing stale complete lines does not refresh the timer for a partial block.
  Independent STARTTLS runtime authenticated Alice and verified 235 AUTH, MAIL
  250, RCPT 250, DATA 354, then disconnected with an unfinished DATA body.
  Wall-clock 180-second expiry was not observed; litterkeep interoperability
  was not tested. Test container and both image tags removed; PKI/accounts stay
  under `./data/g17-smtp-data-timeouts`. G17 remains active.

### 2026-09-26: G15 RFC 4954 AUTH continuation semantics correction, PASS

- Superseded the preceding checkpoint after RFC review: §4 defines Base64 client
  responses and `*` cancellation, not arbitrary SMTP command interruption.
  During AUTH continuation, `NOOP` and nested `AUTH` are therefore rejected as
  invalid Base64 responses with 501; the following separate NOOP is processed
  normally. Earlier T94 is marked SUPERSEDED; T95 replaces it.
- PASS: Windows Rust 1.97.1 fmt, 544 library + 1 binary tests and strict
  Clippy. Linux Docker Rust 1.97 strict Clippy, 595 library + 1 binary tests
  and doc-tests. Independent STARTTLS black-box verifies both invalid response
  lines, 501 replies, command recovery, explicit cancellation, oversized line,
  and a later successful continuation login. Test container and unique image
  tags removed; generated PKI and accounts remain under `./data/g15-auth-interrupt`.
  Litterkeep app interoperability was not tested; G15 remains active.

### 2026-09-26: G15 RFC 4954 continuation line limit, PASS

- SMTP SASL PLAIN continuation now shares the 512-octet RFC 5321 command-line
  bound. Overlong response returns 500 and terminates only the pending auth
  exchange; the following NOOP is parsed normally.
- PASS: Windows Rust 1.97.1 fmt, 544 library + 1 binary tests and strict Clippy;
  Linux Docker Rust 1.97 strict Clippy, 591 library + 1 binary tests and
  doc-tests. Independent Python smtplib over STARTTLS sent a 513-octet
  continuation, received 500, and confirmed NOOP 250 plus successful subsequent
  AUTH flows. Temporary container and unique image tags removed; shared stage
  fixture under `./data/g15-auth-plain-continuation` retained. G15 remains active.

### 2026-09-26: G15 RFC 4954 AUTH PLAIN continuation, PASS

- AUTH PLAIN now accepts optional initial response. Without it the connection
  owner records a typed continuation state and sends the required empty
  `334 ` challenge. A single client response continues SASL; `*` cancellation
  and invalid Base64 receive 501 and return the session to command mode.
  Existing async storage-owner credential verification remains unchanged.
- PASS: Windows Rust 1.97.1 fmt, 544 library + 1 binary tests, strict Clippy;
  Linux Docker Rust 1.97 strict Clippy, 591 library + 1 binary tests and
  doc-tests. Independent Python `smtplib` over STARTTLS verified empty
  challenge, cancel, invalid response and NOOP recovery, successful continuation
  login, repeat-auth 503, and successful initial-response login. Stage container
  and unique image tags are removed; generated PKI and randomized credential
  fixture remain under `./data/g15-auth-plain-continuation`. App/Litterkeep
  interoperability was not tested. G15 remains active.

### 2026-09-26: G15 RFC 5321 EHLO and HELO argument grammar, PASS

- Greeting argument validation is separated into `smtp/greeting.rs`:
  HELO requires one DNS Domain; EHLO accepts one DNS Domain or address-literal.
  Multiple words, malformed DNS labels, missing values, repeated/trailing SP,
  and HTAB are rejected. Unit tests cover IPv4 and IPv6 literal distinction.
- PASS: Windows Rust 1.97.1 fmt, 542 library + 1 binary tests, strict Clippy;
  Linux Docker Rust 1.97 strict Clippy, 589 library + 1 binary tests and
  doc-tests; independent raw-TCP acceptance rejected seven malformed greetings
  and accepted DNS HELO/EHLO plus IPv6 EHLO. Temporary container removed and
  `./data/g15-smtp-greeting` retained. A pre-existing image-tag collision was
  detected after the test build: `litterkeep-server:g15-smtp-greeting` now points
  to the verified runtime from this source; its previous image digest was not
  available for restoration. The separate `litterkeep-linux-test:g15-smtp-greeting`
  image remains untouched. Litterkeep app interoperability was not tested.
  G15 remains active.

### 2026-09-26: G15 RFC 5321 ESMTP parameter separators, PASS

- После reverse/forward path парсер принимает только RFC separator `SP`;
  отклоняет HTAB, повторный SP и завершающий SP. Корректные несколько MAIL
  параметров с одиночным SP сохранены. Регрессии разделены по parser- и
  raw-TCP уровням.
- PASS: Windows Rust 1.97.1 fmt, 539 library + 1 binary tests и strict Clippy;
  Linux Docker Rust 1.97 strict Clippy, 586 library + 1 binary tests и
  doc-tests. Независимый Linux raw-TCP клиент получил 501 для пяти неверных
  вариантов и 250 для валидного MAIL/RCPT, затем завершил сессию. Временные
  контейнер и образы удалены; fixture `./data/g15-smtp-path-colon-whitespace`
  сохранён. Совместимость приложения Litterkeep не проверялась. G15 активна.

### 2026-09-26: G15 RFC 5321 path delimiter whitespace, PASS

- SMTP parser no longer trims whitespace after `MAIL FROM:` or `RCPT TO:`;
  path must immediately follow the command colon. Unit coverage checks SP and
  HTAB for both commands and retains valid paths with MAIL parameters.
- PASS: Windows Rust 1.97.1 fmt, 538 library + 1 binary tests, strict Clippy;
  Linux Docker Rust 1.97 release build, strict Clippy, 585 library + 1 binary
  tests and doc-tests. Independent raw-TCP blackbox rejected all four malformed
  commands with 501, then accepted valid MAIL/RCPT and completed RSET/QUIT in
  the same session. Temporary container and stage image tags are removed;
  `./data/g15-smtp-path-colon-whitespace` is retained. Litterkeep client
  interoperability was not tested. G15 remains active.

### 2026-09-26: G17 STORE and APPEND flag-list boundaries, PASS

- STORE no longer discards parentheses while collecting flags; it validates the
  exact parenthesized or unparenthesized form, requires atom flags, and rejects
  unbalanced or nested lists. APPEND reuses the same strict parenthesized parser
  and rejects quoted flag atoms.
- PASS: Windows Rust 1.97.1 strict Clippy, 530 library + 1 binary tests, fmt,
  Python syntax and diff checks. Linux Docker Rust 1.97 strict Clippy, 577
  library + 1 binary tests and doc-tests; independent STARTTLS acceptance
  received tagged BAD for malformed STORE forms and quoted APPEND flag before
  literal continuation. Temporary image and container removed; existing
  containers untouched and `./data/g17-imap-malformed-utf8` preserved. G17
  remains ACTIVE.

### 2026-09-26: G17 LIST pattern production, PASS

- Basic LIST validates reference as astring and pattern as RFC list-mailbox:
  bare `%`, `*`, and `]` remain valid; other atom-specials need quoted form.
- PASS: Windows Rust 1.97.1 strict Clippy, 527 library + 1 binary tests, fmt,
  Python syntax and diff checks. Linux Docker Rust 1.97 strict Clippy, 574
  library + 1 binary tests and doc-tests; independent STARTTLS acceptance
  rejected bare `{` and accepted a bare `*` pattern with normal completion.
  Temporary container and image removed; existing containers untouched, and
  `./data/g17-imap-malformed-utf8` preserved. G17 remains ACTIVE.

### 2026-09-26: G17 LOGIN and mailbox astring validation, PASS

- LOGIN username/password and SELECT/EXAMINE mailbox require either a valid
  `ASTRING-CHAR` atom or a lexer-validated quoted string. LOGIN rejects extra
  arguments. `]` remains valid; `%`, `*`, braces, quote, and backslash require
  quoted form in these astring contexts.
- PASS: Windows Rust 1.97.1 strict Clippy, 525 library + 1 binary tests, fmt,
  Python syntax and diff checks. Linux Docker Rust 1.97 strict Clippy, 572
  library + 1 binary tests and doc-tests; independent STARTTLS acceptance
  rejected an unquoted `%` in LOGIN, then completed the remaining grammar suite
  and authentication. Temporary image and container removed, pre-existing
  containers untouched, fixture `./data/g17-imap-malformed-utf8` preserved.
  G17 remains ACTIVE; not every astring argument production is migrated yet.

### 2026-09-26: G17 command verb token boundaries, PASS

- Top-level IMAP argument extraction no longer skips list delimiters; command
  verbs and UID subcommands require atom tokens instead of quoted strings.
  Added parser regressions and independent wire cases for both invalid forms.
- PASS: Windows Rust 1.97.1 strict Clippy, 523 library + 1 binary tests, fmt,
  Python syntax and diff checks. Linux Docker Rust 1.97 strict Clippy, 570
  library + 1 binary tests and doc-tests; independent STARTTLS acceptance
  rejected parenthesized and quoted NOOP verbs with tagged BAD and continued
  the session successfully. Temporary image and container removed; fixture
  `./data/g17-imap-malformed-utf8` preserved. G17 remains ACTIVE.

### 2026-09-26: G17 корректная грамматика command tag, PASS

- Предыдущее исключение `]` из command tag было ошибочным: RFC 9051 tag
  использует `ASTRING-CHAR`, где `]` разрешён, и исключает `+`. Parser и
  malformed-UTF-8 sanitizer теперь используют ту же production.
- PASS: Windows Rust 1.97.1 strict Clippy, 520 library + 1 binary tests,
  fmt, Python syntax и diff checks. Linux Docker Rust 1.97 strict Clippy, 567
  library + 1 binary tests и doc-tests; independent STARTTLS wire acceptance
  прошёл valid `A]8` NOOP, tagged malformed-UTF8 error для `A]7`, и untagged
  BAD для запрещённого `A+8`. Временный image и контейнер удалены, остальные
  контейнеры не затронуты, fixture `./data/g17-imap-malformed-utf8` сохранена.
  G17 остаётся ACTIVE.

### 2026-09-26: G17 embedded quote in atom, PASS

- Tokenizer rejects a double quote embedded in an unquoted atom. Quoted strings
  remain a separate token form. Unit test and independent STARTTLS wire test
  require tagged BAD and a usable session afterwards. This is a narrow RFC 9051
  grammar correction, not full atom-special handling.
- PASS: Windows Rust 1.97.1 strict Clippy, 517 library + 1 binary tests,
  format check, and Python syntax compilation. Linux Rust 1.97 Docker strict
  Clippy and 564 library + 1 binary tests plus doc-tests pass. Independent
  STARTTLS black-box grammar acceptance passed, including BAD for the embedded
  quote followed by a successful NOOP in the same session. Both stage image
  tags and the acceptance container were removed; inventory contains only
  pre-existing containers. Fixture `./data/g17-imap-malformed-utf8` remains.
  G17 remains ACTIVE; this does not implement full IMAP4rev2 atom contexts.

### 2026-09-26: G17 IMAP command SP and CTL grammar, PASS

- IMAP command lexer больше не нормализует HTAB и другие whitespace octets в
  разделитель: между command components принимается только SP. Управляющие
  octets отвергаются в atom и quoted-string, кроме разрешённого HTAB внутри
  quoted-string; CR/LF/NUL там недопустимы. UTF-8 literals остаются отдельным
  сохранённым путём.
- PASS: Windows Rust 1.97.1 strict Clippy, 516 library + 1 binary tests,
  fmt, Python compile и diff check. Linux Docker Rust 1.97 strict Clippy,
  563 library + 1 binary tests и doc-tests; independent STARTTLS wire test
  подтвердил BAD для TAB между tag/NOOP и успешную следующую команду.
  Контейнер и образы удалены; `./data/g17-imap-malformed-utf8` сохранена.

### 2026-09-26: G17 IMAP4rev1 high-bit atom rejection, PASS

- IMAP4rev1 lexer теперь отвергает UTF-8 high-bit characters внутри atom;
  прежнее принятие незакавыченного username противоречило заявленному профилю.
  UTF-8 synchronizing SEARCH literals остаются отдельной разрешённой формой.
  Unit и независимый STARTTLS acceptance подтвердили tagged BAD и сохранение
  последующей сессии.
- PASS: Windows Rust 1.97.1 strict Clippy, 514 library + 1 binary tests,
  fmt, Python compile и diff check. Linux Docker Rust 1.97 strict Clippy,
  561 library + 1 binary tests и doc-tests; independent runtime black-box
  PASS. Этапные образы и контейнер удалены; `./data/g17-imap-malformed-utf8`
  сохранена. UTF8=ACCEPT и IMAP4rev2 всё ещё не заявлены.

### 2026-09-25: G17 IMAP resp-special tag sanitization, SUPERSEDED

- Исторический тест проверял неверное утверждение, будто `]` запрещён в tag.
  Последующая RFC-сверка показала отличие `tag` от `atom`; production исправлена
  отдельным checkpoint-ом от 2026-09-26 выше.
- PASS: Windows Rust 1.97.1 strict Clippy, 513 library + 1 binary tests,
  fmt, Python compile и diff check. Linux Docker Rust 1.97 strict Clippy,
  560 library + 1 binary tests и doc-tests; независимый runtime black-box
  PASS. Временные образы и контейнер удалены, `./data/g17-imap-malformed-utf8`
  сохранена. Это узкая корректировка error path, не реализация UTF8=ACCEPT.

### 2026-09-25: G17 IMAP malformed UTF-8 tagged error checkpoint, PASS

- IMAP сохраняет безопасный ASCII command tag, если строка содержит
  некорректную UTF-8 последовательность; invalid tag сам не отражается в ответе
  и использует звездочный untagged fallback. Независимый STARTTLS клиент
  отправил malformed UTF-8 в quoted LOGIN, получил tagged BAD, затем передал
  повреждённый tag и получил untagged BAD без отражения байтов; NOOP после
  обоих случаев подтвердил сохранение сессии.
- PASS: Windows Rust 1.97.1 cargo test --all-targets --locked (512 library +
  1 binary), strict Clippy, fmt, Python compile и diff check. Linux Docker Rust
  1.97 test target: strict Clippy, 559 library + 1 binary tests и doc-tests;
  независимый Linux runtime black-box прошёл. Созданные контейнер и оба image
  tags удалены, fixture ./data/g17-imap-malformed-utf8 сохранена. G17 остаётся
  ACTIVE; UTF8=ACCEPT и IMAP4rev2 не объявлены.

### 2026-09-25: G17 SMTP PIPELINING RFC 2920 checkpoint, PASS

- EHLO теперь рекламирует PIPELINING в plaintext, STARTTLS и TLS профилях.
  Независимая проверка в tests/blackbox/smtp_pipelining.py отправляет одним
  write MAIL, несуществующий RCPT, корректный RCPT и DATA, затем сверяет ответы
  по порядку (250/550/250/354), передаёт тело после 354 и находит сохранённое
  сообщение Bob независимым IMAP STARTTLS клиентом.
- PASS: Windows Rust 1.97.1 cargo test --all-targets --locked (510 library +
  1 binary), strict Clippy, fmt, py_compile и diff check. Linux Rust 1.97
  Docker test target прошёл strict Clippy, 557 library + 1 binary tests и
  doc-tests. Независимый runtime black-box прошёл на Docker Linux через
  STARTTLS/AUTH PLAIN. Временный контейнер и оба этапных image tags удалены;
  fixture data/g17-smtp-pipelining сохранена. Остальной G17 остаётся ACTIVE.

### 2026-09-25: G17 IMAP APPENDLIMIT checkpoint, PASS

- IMAP CAPABILITY now publishes the server-wide upload maximum as
  `APPENDLIMIT=26214400` octets. STATUS and LIST RETURN STATUS expose the same
  fixed limit; APPEND above it receives tagged `NO [TOOBIG]` before body bytes
  are accepted.
- PASS: Windows Rust 1.97.1 full suite (510 library + 1 binary), strict Clippy
  and formatting; Linux Rust 1.97 Docker target strict Clippy and 557 library +
  1 binary tests plus doc-tests. Independent STARTTLS checks verified CAPABILITY,
  over-limit APPEND rejection without uploading payload, STATUS APPENDLIMIT and
  LIST STATUS APPENDLIMIT. Test image/container removed; fixture
  `./data/g17-managesieve-capabilities` preserved. G17 remains ACTIVE.

### 2026-09-25: G17 IMAP quoted-pair syntax and tagged lexer errors, PARTIAL

- IMAP quoted strings now unescape only `\"` and `\\` as permitted by RFC 3501;
  other escaped characters are syntax errors. On tokenizer failure the parser
  preserves the command tag so BAD remains correlated rather than becoming
  `* BAD`.
- Independent STARTTLS `imaplib` acceptance sent an invalid quoted-pair in
  LOGIN, received tagged BAD, then completed NOOP and valid LOGIN on the same
  connection. Fixture: `./data/g17-managesieve-capabilities`. UTF8=ACCEPT and
  IMAP4rev2 remain unsupported and are not advertised.
- Follow-up closes that tokenizer defect: legacy IMAP4rev1 quoted strings reject
  raw high-bit characters, while a separately received UTF-8 SEARCH literal is
  injected as a typed value and does not pass through the quoted-string grammar.
  UTF8=ACCEPT and IMAP4rev2 remain unsupported and are not advertised.
- PASS: Windows Rust 1.97 full suite (510 library + 1 binary), strict Clippy and
  fmt. Linux Rust 1.97 Docker target passed strict Clippy and 557 library + 1
  binary tests plus doc-tests. Independent STARTTLS tests verified tagged BAD
  and session recovery for raw UTF-8 quoted LOGIN, ASCII BODY/UID literals,
  UTF-8 BODY literal and empty literal. Follow-up also parses multiple literals
  in one SEARCH command and preserves the subsequent pipelined command. Created
  container and image are removed after acceptance; fixture
  `./data/g17-managesieve-capabilities` is preserved. G17 remains ACTIVE.

### 2026-09-25: G17 Sieve vacation MIME quoted-printable network checks, SUPERSEDED BY T100

- Этот исторический прогон ошибочно считал SP перед soft break нарушением
  RFC 2045. Проверка исправлена и повторена в T100.
- PASS: Linux Rust 1.97 test-stage image использовал закэшированные успешные
  release build, strict Clippy и 552 library + 1 binary tests; независимый
  CHECKSCRIPT прошёл с credentials fixture `./data/g17-managesieve-capabilities`.
  Image tag и созданный контейнер удалены, данные оставлены. Это проверяет
  собственный сервер, не клиент Litterkeep и не полный MIME interoperability.
  G17 остаётся ACTIVE.

### 2026-09-25: G17 ManageSieve RFC 5804 TLS capability transitions, PARTIAL

- ManageSieve capabilities now reflect the security state: before STARTTLS
  the SASL list is empty; after TLS the server advertises PLAIN and removes
  STARTTLS. It reissues capabilities after TLS and successful AUTHENTICATE,
  adds escaped OWNER for the authenticated account, and rejects repeat
  STARTTLS with an explicit response in authenticated and unauthenticated
  sessions.
- Independent Linux socket acceptance passed plaintext greeting/CAPABILITY,
  STARTTLS, post-TLS greeting/CAPABILITY, PLAIN AUTHENTICATE, OWNER, post-auth
  CAPABILITY and repeated STARTTLS rejection. Fixture:
  `./data/g17-managesieve-capabilities`. This does not establish implicit-TLS
  network interoperability or compatibility with Litterkeep.
- PASS: Windows Rust 1.97.1 full suite and strict Clippy, fmt, Python syntax and
  diff checks; Linux Rust 1.97 Docker test target and independent ManageSieve
  socket acceptance. The test container and temporary image tag are removed
  after verification; fixture data is preserved. G17 remains ACTIVE.

### 2026-09-25: G17 IMAP SASL-IR grammar and TLS acceptance, PARTIAL

- IMAP `AUTHENTICATE PLAIN` now parses mechanism and initial response as
  `astring` forms, accepts atom and quoted values, and distinguishes absent
  response from SASL-IR `=` (present but empty). Empty PLAIN credentials return
  authentication failure without wedging the session. No new SASL mechanism or
  capability was added.
- Independent `imaplib` STARTTLS acceptance confirmed pre/post-TLS capability,
  continuation, atom SASL-IR, quoted mechanism/response, empty marker rejection,
  and continued NOOP after denial. Fixture: `./data/g17-imap-sasl-ir`.
- PASS: Windows Rust 1.97.1 full suite (507 library + 1 binary), strict Clippy,
  fmt and diff checks. Docker Linux Rust 1.97 test target passed strict Clippy,
  552 library + 1 binary tests and doc-tests. Docker wire acceptance passed.
  Test container and temporary image were removed; fixture data is retained.
  Other SASL mechanisms/channel binding and Litterkeep compatibility
  remain unverified. G17 remains ACTIVE.

### 2026-09-25: G17 forced SMTPUTF8-to-IMAP durable delivery, PARTIAL

- The opt-in `init-smtputf8-test` command creates a separate random-password
  acceptance account named `пользователь`; ordinary `init`, existing four
  accounts and their `test-accounts.tsv` bytes remain unchanged. The independent
  `smtp_smtputf8_delivery.py` client authenticated Alice over SMTP STARTTLS,
  forced a raw `MAIL ... SMTPUTF8` and UTF-8 RCPT despite SMTPUTF8 being absent
  from EHLO, then sent a base64 MIME attachment. IMAP STARTTLS authenticated
  the Unicode authcid using SASL PLAIN and fetched the committed message.
- The attachment octets matched exactly, SHA-256
  `7f4e114fb71bb6cf784bda9f0d77d06e3ccb15cfdc667bbad322dcef81da20f8`.
  After stopping and restarting the server against the same
  `./data/g17-smtputf8-delivery-optin`, a read-only IMAP pass recovered the same
  bytes.
- This proves the current internal Unicode local-part routing, durable Store
  key, SASL authcid, and MIME octet path together. It is deliberately not called
  standards-conforming SMTPUTF8 client interoperability: the test forces an
  extension that EHLO does not offer. SMTPUTF8 must remain unadvertised until
  complete RFC 6531 support, including IDNA domain handling. IMAP4rev2/UTF8
  capability conformance, Litterkeep compatibility, and other EAI cases remain
  unverified.
- PASS: Windows Rust 1.97.1 full suite (506 library + 1 binary), strict Clippy,
  fmt and diff checks; Python 3.14 byte-client syntax check. Linux Rust 1.97
  runtime built and passed initial plus post-restart SMTP-to-IMAP acceptance.
  Linux test target passed release build, strict Clippy, 551 library + 1 binary
  test and doc-tests. Python 3.14 syntax check and `git diff --check` passed.
  Temporary container and image tags were removed; `litterkeep-g12-phone` is
  preserved. G17 remains ACTIVE.

### 2026-09-25: G17 SMTPUTF8 independent recipient-policy socket checkpoint, PARTIAL

- Added `tests/blackbox/smtp_smtputf8_recipient_policy.py` as an independent
  byte-oriented TCP client. On a Linux runtime initialized under
  `./data/g17-smtputf8-recipient-policy`, it verified SMTPUTF8 is absent from
  EHLO, an unmarked UTF-8 RCPT receives 553, an unmarked UTF-8 reverse-path
  receives 501, and marked UTF-8 MAIL/RCPT reaches account lookup (550 for the
  deliberately nonexistent Unicode recipient). The expected response sequence
  confirms the MAIL/RCPT gates and UTF-8 path parsing on the actual listener.
- This is not a delivery proof: there is no Unicode test account in bootstrap,
  no accepted Unicode recipient, and no SMTP-to-IMAP message-byte assertion.
  IDNA and SMTPUTF8 EHLO remain unsupported; no Litterkeep client test.
- PASS: Python 3.14 `py_compile`; the independent socket test passed against a
  freshly initialized Linux Rust 1.97 runtime image. The exact test container was
  stopped and auto-removed, its temporary image tag was removed, and
  `litterkeep-g12-phone` was preserved. G17 remains ACTIVE.

### 2026-09-25: G17 SMTP RFC 6531 UTF-8 local-part groundwork, PARTIAL

- SMTP mailbox grammar now accepts non-ASCII UTF-8 characters in dot-atom and
  quoted local-parts while measuring RFC 5321's 64-octet limit on the wire form.
  Unicode DNS domains remain rejected because IDNA U-label validation and DNS
  conversion are not implemented. MAIL with a UTF-8 reverse-path requires the
  valueless `SMTPUTF8` parameter; the reactor guard refuses a UTF-8 RCPT unless
  the active MAIL set it. `SMTPUTF8` no longer incorrectly requires `BODY=8BITMIME`; high-bit
  body octets still require 8BITMIME, independently of UTF-8 headers.
- PASS: Windows Rust 1.97.1 `cargo test --all-targets --locked` (505 library +
  1 binary tests), strict Clippy, fmt and diff checks. The first focused run
  exposed an obsolete ASCII-only quoted local-part expectation, which was
  corrected. Linux Docker Rust 1.97 target passed release build, strict
  Clippy, 550 library + 1 binary test and doc-tests. The separate runtime image
  returned `litterkeep-server 0.1.0` for `version`.
- Not claimed: SMTPUTF8 EHLO capability (it is still not advertised), IDNA,
  Unicode-domain routing, or end-to-end network delivery to a UTF-8 account.
  No Litterkeep client test. Temporary test/runtime image tags were removed;
  `litterkeep-g12-phone` is preserved. G17 remains ACTIVE.

### 2026-09-25: G17 Unicode account persistence checkpoint, PARTIAL

- Обратимое namespace-кодирование теперь используется не только для mailbox
  journal/flags/expunges, но и для password credentials, folder catalog,
  подписок и Sieve state/temp. Существующие безопасные ASCII имена оставляют
  прежние filenames. Unicode ключи сохраняют исходные UTF-8 байты, без
  Unicode normalization; длина account ограничена 64 октетами, controls
  отвергаются. Новые кодированные каталоги синхронизируются до Store root.
- Regression создаёт Unicode credential, каталог папок, подписку и active Sieve
  script, закрывает владельца и восстанавливает все четыре области после reopen.
  Это storage/auth API доказательство, не SMTPUTF8 network acceptance: SMTP
  mailbox parser и EAI delivery path остаются ASCII-only, capability не
  рекламируется. Litterkeep app не тестировалось.
- PASS: Windows Rust 1.97.1 `cargo test --all-targets --locked` (503 library
  + 1 binary), strict Clippy, fmt и diff checks. Linux Docker Rust 1.97 test
  target: 548 library + 1 binary, doc-tests, strict Clippy; release runtime
  image built and `version` command returned `litterkeep-server 0.1.0`. Temporary
  tags removed, and `docker ps -a` retained only `litterkeep-g12-phone`.
  G17 remains ACTIVE.

### 2026-09-25: G17 Unicode-safe mailbox storage key groundwork, PARTIAL

- Store mailbox keys that are valid UTF-8 and contain no controls can now be
  persisted without using protocol text as a path component. Existing ASCII
  keys retain their legacy filenames; other values use a reversible lowercase
  UTF-8 hex namespace under `mailboxes/utf8/`, split into bounded path
  components. Recovery, flags, expunge and MOVE use the same mapping and sync
  encoded directories at durable boundaries.
- Independent Store regressions verify an adversarial `用户/../почта` logical
  key, append/flags/expunge/reopen, Unicode MOVE, and unchanged legacy paths.
  This is storage groundwork only: SMTP local-part parsing, account credential
  keys, folder/subscription/Sieve journals, and inbound SMTPUTF8 network delivery
  remain incomplete. The server does not advertise SMTPUTF8.
- PASS: Windows Rust 1.97.1 `cargo test --all-targets --locked` (502 library
  + 1 binary), strict Clippy, fmt and diff checks. Linux Docker Rust 1.97 test
  target: 547 library + 1 binary test, doc-tests and strict Clippy. Linux runtime
  image built and `version` command ran successfully. No SMTP network acceptance
  was claimed because SMTPUTF8 envelope parsing and account-key wiring remain
  incomplete. Temporary image tags were removed; `docker ps -a` shows only the
  pre-existing `litterkeep-g12-phone`. G17 remains ACTIVE.

### 2026-09-25: G17 SMTPUTF8 header octet transport checkpoint, PARTIAL

- Parser распознаёт MAIL-параметр `SMTPUTF8` без значения, отвергает повтор и
  значение, требует вместе с ним `BODY=8BITMIME` и разрешает предусмотренные
  RFC 6531 дополнительные 10 октетов длины MAIL-команды. Входящий DATA разделяет
  UTF-8 field values заголовков и 8-bit MIME body: первое разрешает только флаг
  SMTPUTF8, второе только BODY=8BITMIME; UTF-8 field names остаются запрещены.
- Исходящий SMTP обнаруживает валидные UTF-8 заголовки и перед MAIL требует от
  peer обе capability, `8BITMIME` и `SMTPUTF8`; передаёт `BODY=8BITMIME`
  и `SMTPUTF8`, сохраняя message octets. Независимый Linux SMTP peer подтвердил
  команду MAIL и точные DATA bytes; Linux Rust 1.97 test image: strict Clippy,
  541 library + 1 binary test и doc-tests. Windows Rust 1.97.1: 497 library
  + 1 binary test, strict Clippy, fmt и diff check.
- Локальный EHLO намеренно не рекламирует SMTPUTF8: Unicode envelope local-part,
  IDNA U-label validation и безопасные Unicode account/storage keys ещё не
  реализованы. Входящий wire сценарий полного SMTP listener не проверялся;
  Litterkeep app не тестировалось. Это ограниченный transport checkpoint, не
  реализация EAI/SMTPUTF8 целиком. Fixture: `./data/g17-smtputf8`.

### 2026-09-25: G17 SMTP RFC 5321 address-literal syntax

- SMTP mailbox grammar now accepts IPv4 address-literals and registered tagged
  IPv6 address-literals (including compressed and IPv4-embedded IPv6). IPv4
  octets are decimal 0..255 with 1..3 digits; malformed literals and unregistered
  tags are syntax errors. Correct literals retain their domain form through
  parsing but do not match a configured local domain or enable relay.
- PASS: Windows Rust 1.97.1 `cargo test --locked` (494 library + 1 binary),
  strict Clippy, fmt, Python compile and diff check. Linux Docker Rust 1.97:
  release, strict Clippy, 537 library + 1 binary tests and doc-tests. Independent
  STARTTLS SMTP/IMAP test confirms valid IPv4/IPv6 paths reach sender/recipient
  policy (553/550), malformed IPv4 gets 501, and quoted/source-routed messages
  still reach Bob's INBOX.
- Temporary runtime container and test image were removed; `litterkeep-g12-phone`
  remains. Fixture: `./data/g17-smtp-address-literals`. Litterkeep app was not
  tested; SMTPUTF8 and general unregistered literal tags remain unsupported.
  G17 remains active; the full SMTP profile is not complete.

### 2026-09-25: G17 SMTP RFC 5321 source-route compatibility

- SMTP MAIL FROM и RCPT TO распознают устаревший `@domain,@domain:mailbox`
  согласно RFC 5321 §4.1.2, проверяют каждый route element и удаляют route до
  локальной policy. Route не используется для relay; downstream получает только
  конечный mailbox. Некорректные пустые, частично заданные и DNS-invalid routes
  отвергаются как синтаксическая ошибка.
- PASS: Windows Rust 1.97.1 `cargo test --locked` (490 библиотечных + 1 бинарный),
  strict Clippy, `cargo fmt --check`, Python compile и `git diff --check`.
  Linux Docker Rust 1.97: release, strict Clippy, 533 библиотечных + 1 бинарный
  тест и doc-tests. Независимый STARTTLS SMTP-клиент передал quoted MAIL/RCPT с
  source routes; отдельный IMAP STARTTLS-клиент нашёл оба письма в INBOX Bob.
- Временные runtime container и test image удалены; `litterkeep-g12-phone`
  сохранён. Fixture: `./data/g17-smtp-source-route`. Прямое подключение
  Litterkeep app не проверялось; address literals и SMTPUTF8 остаются вне этого
  checkpoint. Общий RFC 5321 профиль не завершён, G17 остаётся активной.

### 2026-09-25: G17 SMTP RFC 5321 quoted local-part support

- Исправлена ранее задокументированная несовместимость: MAIL FROM и RCPT TO
  теперь принимают dot-atom и ASCII quoted-string local-part по RFC 5321
  §4.1.2, включая quoted-pair и `@`/`>` внутри кавычек. Ограничение 64
  октета считается по исходной wire-форме local-part.
- Для локальной policy экранированный local-part канонизируется; wire reverse
  path при этом сохраняется без изменения для Received/DSN. Независимый
  STARTTLS SMTP peer отправил письмо с `"alice"@litterkeep.test` к
  `"bob"@litterkeep.test`; отдельный IMAP STARTTLS peer нашёл письмо в INBOX Bob.
- PASS: Windows Rust 1.97.1 полный suite (488 библиотечных + 1 бинарный тест),
  strict Clippy, fmt, Python compile и diff check. Linux Docker Rust 1.97:
  531 библиотечных + 1 бинарный тест, strict Clippy и doc-tests; quoted
  SMTP-to-IMAP acceptance прошёл на Linux runtime.
- Address literals, source routes и SMTPUTF8 остаются неподдержанными; это не
  объявляется полной реализацией mailbox grammar. Litterkeep app не проверялось.
  Временные container/image очищены; `litterkeep-g12-phone` сохранён. Fixture:
  `./data/g17-smtp-quoted-localpart`; G17 остаётся активной.

### 2026-09-25: G17 POP3 RFC 6856 LANG state acceptance

- PASS: независимый TLS peer подтвердил `LANG` capability в AUTHORIZATION и
  TRANSACTION, RFC language listing `en`, case-insensitive `EN`, preferred `*`
  и корректное отклонение неподдерживаемого `de` без потери сессии. После
  ошибки TRANSACTION продолжил успешно выполнять STAT.
- PASS: Windows Rust 1.97.1 полный suite (484 библиотечных + 1 бинарный тест),
  strict Clippy, fmt, Python compile и diff check. Linux Docker Rust 1.97 image
  подтверждён: 527 библиотечных + 1 бинарный тест, strict Clippy и doc-tests;
  новый независимый socket acceptance прошёл на Linux runtime.
- Litterkeep app не тестировалось. Временные runtime container и test image
  удалены, существующий `litterkeep-g12-phone` сохранён. Fixture:
  `./data/g17-pop3-lang`; G17 остаётся активной.

### 2026-09-25: G17 SMTP DATA transparency and POP3 TOP wire regression

- Независимый SMTP → Store → POP3 TOP тест выявил, что входящий SMTP DATA
  удалял две точки вместо одной для dot-leading строк. Исправлено снятие
  ровно одного transparency dot; RFC 5321 предел строки проверяется после
  unstuffing, включая CRLF и исключая дополнительную wire-точку.
- POP3 `TOP` зафиксирован независимым STLS peer по RFC 1939 §7: ноль строк
  сохраняет headers и пустой разделитель, один возвращает ровно одну строку,
  запрос больше тела возвращает всё тело. `.first` и `..second` совпали после
  SMTP DATA и POP3 multiline dot transparency.
- PASS: Windows Rust 1.97.1 `cargo fmt --check`, 484 библиотечных + 1 бинарный
  тест, strict Clippy, Python compile и `git diff --check`. Linux Docker Rust
  1.97: 527 библиотечных + 1 бинарный тест, strict Clippy и doc-tests.
- PASS: независимый SMTP → Store → POP3 STARTTLS wire acceptance с валидными
  локальными учётными данными; TOP 0/1/9 и dot-transparency прошли. Litterkeep
  app не тестировалось. Runtime container и два созданных для этапа image tags
  очищены; `litterkeep-g12-phone` сохранён. Fixture: `./data/g17-pop3-top-dotfix`.

### 2026-09-25: G17 RFC 1870 длинный SMTP SIZE advertisement

- EHLO `SIZE` принимает произвольную последовательность десятичных цифр, как
  задано RFC 1870; ограничение в 20 цифр относится к MAIL `SIZE` value, а не к
  объявлению peer-а. Переполнение `u64` сохраняется отдельным typed состоянием
  конечного лимита выше диапазона локального message size. Исходящий клиент
  продолжает посылать точный локальный размер, не отбрасывая корректное
  объявление peer-а.
- PASS: целевые Rust тесты проверили SIZE выше `u64::MAX`, длинную запись с
  ведущими нулями и построение MAIL. Windows Rust 1.97.1: 484 библиотечных +
  1 бинарный тест, strict Clippy, fmt, Python compile и diff check. Linux Docker
  Rust 1.97: strict Clippy, 525 библиотечных + 1 бинарный тест и doc-tests.
  Независимый Linux SMTP peer подтвердил 21-значное EHLO SIZE и точный MAIL
  SIZE, вместе с существующими 8BITMIME, reply-line и phase-reply cases.
  Временные образы удалены; `litterkeep-g12-phone` сохранён. Litterkeep app не
  проверялось; G17 остаётся RUNNING.

### 2026-09-25: G17 потоковая запись исходящего SMTP DATA

- Убран временный `Vec` полного размера письма, который удваивал память
  активной доставки до 25 MiB payload. DATA теперь проходит строками: каждая
  строка dot-stuffing-уется и передаётся `write_all`; временный фрагмент
  ограничен SMTP-пределом 1000 октетов. Завершающий CRLF и terminator
  сохраняют прежнюю wire-семантику для plaintext и implicit TLS.
- PASS: Windows Rust 1.97.1: 481 библиотечный + 1 бинарный тест, strict
  Clippy, fmt, Python compile и diff check. Новый unit regression формирует
  2 MiB dotted payload, ограничивает каждый записываемый фрагмент и заставляет
  writer возвращать частичные записи. Linux Docker Rust 1.97: strict Clippy,
  522 библиотечных + 1 бинарный тест и doc-tests; release image собран.
  Независимый SMTP peer принял около 2 MiB и сверил все DATA octets после
  dot-unstuffing. Временные образы удалены; `litterkeep-g12-phone` сохранён.
  Fixture: `./data/g17-smtp-outbound-stream`. Litterkeep app не тестировалось;
  G17 остаётся RUNNING.

### 2026-09-25: G17 outbound SMTP per-phase timeouts

- Replaced the shared 10-second socket I/O timeout with RFC 5321 §4.5.3.2
  per-phase defaults for plaintext and implicit TLS: 300 seconds for greeting,
  MAIL, RCPT and ordinary commands; 120 seconds awaiting DATA acceptance;
  180 seconds per DATA send; 600 seconds for the post-terminator completion.
  Tests can retain short uniform deadlines for deterministic stalled-peer checks.
- PASS: Windows Rust 1.97.1: 478 library + 1 binary test, strict Clippy, fmt
  and diff check. Linux Docker Rust 1.97: 519 library + 1 binary test, strict
  Clippy and doc-tests. Timeout table unit test and stalled-greeting wire test
  pass. Temporary image removed; only `litterkeep-g12-phone` remains. Fixture:
  `./data/g17-smtp-phase-timeouts`. Runtime timeout reconfiguration and slow
  peer behavior for each phase remain unverified; G17 stays RUNNING.

### 2026-09-25: G17 configurable outbound SMTP phase timeouts

- Added five positive-seconds environment overrides for greeting, ordinary
  command, DATA acceptance, DATA writes and final DATA completion. Missing values
  retain the RFC defaults; invalid values fail outbound-worker configuration.
  Plaintext and implicit-TLS transports share the same phase policy. README now
  documents the actual defaults and explicitly notes that cancelling active
  socket work waits for its current phase timeout, up to 600 seconds.
- PASS: Windows Rust 1.97.1 `cargo test --locked` (480 library + 1 binary
  test), strict Clippy, format and diff check. Linux Docker Rust 1.97 test target
  passed strict Clippy, 521 library + 1 binary test and doc-tests. Unit tests
  cover defaults, partial overrides and zero rejection. Slow-peer behavior for
  every phase and interruption of active socket I/O on cancel remain unverified.
  Temporary Docker image removed; only `litterkeep-g12-phone` remains. Fixture:
  `./data/g17-smtp-timeout-env`. Litterkeep client was not tested; G17 remains
  RUNNING.

### 2026-09-25: G17 outbound SMTP EHLO-to-HELO fallback

- Outbound plaintext and implicit-TLS transports now fall back to `HELO` when
  `EHLO` receives the RFC-defined single-line `500 command not recognized`.
  The fallback session has no ESMTP capability state and therefore sends no
  `SIZE` or `BODY=8BITMIME` MAIL parameters. Other negative EHLO replies, such
  as policy rejection `550`, remain errors and do not trigger downgrade.
- PASS: Windows Rust 1.97.1: 477 library + 1 binary test, strict Clippy, fmt,
  diff check. Linux Docker Rust 1.97: 518 library + 1 binary test, strict
  Clippy and doc-tests. Independent plaintext and implicit-TLS peers verified
  EHLO -> 500 -> HELO -> 250 -> MAIL and full delivery. Temporary image removed;
  only `litterkeep-g12-phone` remains. Fixture directory:
  `./data/g17-smtp-helo-fallback`. Litterkeep client not tested; G17 remains
  RUNNING.

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

### 2026-09-25: G17 quoted-printable soft-break padding checkpoint, SUPERSEDED BY T100

- Предыдущая реализация ошибочно удаляла буквальные SP/TAB перед `=CRLF`;
  T100 исправляет это поведение по RFC 2045 §6.7 и обновляет регрессии.
- Unit tests исходного checkpoint-а проверяли прежнее ошибочное поведение.
  Windows Rust 1.97.1: 395
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
- На момент G15 quoted local-parts, domain literals, source routes и SMTPUTF8
  не принимались; следующий G17 checkpoint закрыл только quoted local-parts.
  Остальные ограничения сохраняются, поэтому RFC 5321 профиль не закрыт.

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
| G14 | G13 | IMAP4rev2 RFC 9051 baseline plus tested IMAP4rev1 client compatibility, including RFC 9394 PARTIAL SEARCH/FETCH | RUNNING |
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

### 2026-09-26: G17 RFC 9051 APPEND date-time checkpoint

- Parser сохраняет синтаксическую форму необязательного INTERNALDATE: RFC
  `date-time` принимается только как quoted-string. Дата-подобный atom
  отклоняется до literal continuation; корректная quoted-дата с literal
  разбирается типизированно.
- Добавлены parser regressions на некавыченную дату, корректную quoted-дату и
  лишний аргумент после literal marker. Независимый STARTTLS acceptance
  проверяет отказ до continuation и успешное завершение с пустым APPEND.
- G17/G14 остаются RUNNING: остальные пробелы APPEND, IMAP4rev2 и другие
  протоколы не закрыты.

### 2026-09-26: G17 RFC 5322 obsolete Content-ID quoted-word checkpoint

- Разбор `Content-ID` вынесен в отдельную предметную область. Obsolete
  `local-part = word *("." word)` теперь принимает atom и quoted-string
  segments, не считая точки внутри кавычек разделителями; CFWS нормализуется
  только на границах words. Управляющие octets и некорректные quoted-pairs
  отвергаются.
- Unit cases проверяют quoted words с точками и по обе стороны от atom.
  Независимый STARTTLS acceptance проверяет их wire-проекции в `BODY` и
  `BODYSTRUCTURE`. Реализация остаётся ограниченной корректными printable
  формами, не заявляет obsolete control-character compatibility.
- Обновлены RFC coverage и acceptance T86. Остальная MIME, IMAP4rev2 и
  межпротокольная совместимость продолжают оставаться открытыми.

### 2026-09-26: G17 RFC 5322 quoted-word CFWS wire checkpoint

- Проверены комментарии RFC 5322 CFWS вокруг quoted/atom words и dot
  separators внутри obsolete Content-ID local-part. Удаление comments не
  меняет quoted-string; wire projection оставляет только семантические слова.
- Unit regression и независимый IMAP STARTTLS APPEND/FETCH проверяют
  нормализованный Content-ID в BODY и BODYSTRUCTURE. Запись acceptance T87;
  полная MIME и IMAP4rev2 грамматика остаются открытыми.

### 2026-09-26: G17 RFC 5322 bare-CR header checkpoint

- MIME header-block parser теперь отвергает одиночный CR внутри обычной строки
  поля и folded continuation; корректное CRLF + WSP unfolding сохранено.
- Unit regressions подтверждают оба отказа и прежнюю проверку CRLF framing.
  Socket acceptance не запускался, поэтому отмечен только parser-level
  результат. Полный RFC 5322 и MIME профили остаются открытыми.

### 2026-09-26: G17 RFC 5322 extension field-name checkpoint

- MIME header parser расширен до `ftext = %d33-57 / %d59-126`, включая
  пунктуацию допустимых extension names кроме `:`. Сохранена ранее проверенная
  нормализация завершающего WSP перед двоеточием; внутренний SP, CTL и high-bit
  octets в field-name отвергаются.
- Unit regressions покрывают punctuation и отрицательные octets. Socket
  acceptance не запускался; подтверждён только parser-level результат.

### 2026-09-26: G17 RFC 5322 extension field-name wire checkpoint

- Независимый IMAP STARTTLS клиент выполнил APPEND с легальным punctuation
  extension field, дождался BODYSTRUCTURE parsing и сравнил RFC822 FETCH с
  исходным буфером. Все байты совпали. Acceptance T88; Litterkeep app не
  проверялось.

### 2026-09-26: G17 SMTPUTF8 coverage correction

- T84 уточнён по сохранённым forced-extension Linux runtime evidence: Unicode
  local-part, account persistence и MIME bytes были проверены end-to-end, но
  этот клиент намеренно использовал SMTPUTF8, отсутствовавший в EHLO. Статус
  остаётся PARTIAL: сервер не заявляет полную RFC 6531 interoperability, в
  частности не поддерживает IDNA U-label domains.

### 2026-09-26: G17 RFC 5322 obsolete Content-ID CFWS checkpoint

- MIME `Content-ID` теперь принимает RFC 5322 `obs-id-left`/`obs-id-right`
  CFWS вокруг dot-separated atoms, удаляет comments и схлопывает whitespace
  вокруг точек, не меняя семантический идентификатор. Другие obsolete формы
  по-прежнему не заявляются. Норма: [RFC 5322 §4.5.4](https://www.rfc-editor.org/rfc/rfc5322.html#section-4.5.4).
- Unit regressions проверяют comments и SP/HTAB между dot-separated atoms.
  Независимый STARTTLS `imaplib` acceptance передаёт obsolete форму через
  APPEND и сравнивает нормализованный ID в обоих `BODY` и `BODYSTRUCTURE`.
- Обновлены protocol coverage и запись acceptance T85. Ранее оставшаяся в G14
  заметка об отсутствующих Content-ID/Description была устаревшей: basic fields
  уже подтверждены checkpoint-ом T61; теперь закрыта ещё одна форма RFC 5322.
  Полная MIME и IMAP4rev2 грамматика остаются незакрытыми.

### 2026-09-26: G17 MIME structured WSP metadata checkpoint

- В `Content-Language` и `Content-Location` внешнее удаление пробелов теперь
  ограничено RFC WSP (SP/HTAB); Unicode NBSP не принимается как CFWS. В
  unstructured `Content-Description` Unicode-пробелы остаются данными. Норма:
  RFC 5322 WSP, RFC 3282 CFWS и RFC 6532 UTF-8 `text`.
- Сначала regressions воспроизвели ошибочное принятие/удаление NBSP, затем
  прошли 13 тестов metadata. Полный Windows `cargo test --locked`: 572 library
  и 1 binary test. Linux Docker release build, strict Clippy и test gate прошли:
  623 library, 1 binary и doc-tests. Независимый STARTTLS Python `imaplib`
  проверил шесть APPEND/FETCH сообщений, BODYSTRUCTURE projection и точное
  совпадение RFC822 literal bytes. Это не проверка Litterkeep.
- Контейнер `litterkeep-mime-wsp-runtime` и образы
  `litterkeep-mime-wsp-runtime:20260926`, `litterkeep-mime-wsp-check:20260926`
  удалены и их отсутствие подтверждено; аккаунты и PKI сохранены в
  `./data/g17-mime-ascii-wsp`. G17 остаётся RUNNING: полная MIME-грамматика и
  остальные протоколы ещё не закрыты.

### 2026-09-26: G17 RFC 6532 UTF-8 phrase whitespace checkpoint

- RFC 2047 phrase tokenization и RFC 5322 address display-name normalization
  больше не классифицируют Unicode NBSP как CFWS: разделителями являются
  только SP/HTAB, а NBSP сохраняется как UTF-8 phrase data.
- Две регрессии сначала воспроизвели изменение `Alice North` на `Alice North`.
  После исправления Windows полный `cargo test --locked` прошёл: 574 library,
  1 binary и doc-tests; fmt и strict Clippy прошли. Linux Docker release build,
  strict Clippy и test gate прошли: 625 library, 1 binary и doc-tests.
  Независимый STARTTLS `imaplib` APPEND/FETCH ENVELOPE подтвердил точное UTF-8
  NBSP display-name. Acceptance T101; Litterkeep app не проверялось.
- Контейнер `litterkeep-rfc6532-phrase-wsp` и образы
  `litterkeep-rfc6532-phrase-wsp-runtime:20260926`,
  `litterkeep-rfc6532-phrase-wsp-check:20260926` удалены и их отсутствие
  проверено. Аккаунты и PKI сохранены в `./data/g17-rfc6532-phrase-wsp`.
  G17 остаётся RUNNING: остальные internationalized address forms, MIME
  grammar и протокольные пробелы требуют отдельных проверенных этапов.

### 2026-09-26: G17 RFC 5321 SMTP ASCII command-string checkpoint

- `HELP`/`NOOP` optional строки больше не очищают Unicode whitespace через
  `str::trim`. После удаления только ASCII SP аргументы проверяются на
  printable ASCII; Unicode bytes не превращают ошибочный аргумент в
  отсутствующий optional value. VRFY/EXPN используют отдельную RFC 6531 String
  grammar и допускают UTF-8 query, как подтверждено позже в T158.
- Регрессия до изменения воспроизвела принятие `VRFY NBSP alice`. Windows
  `cargo test --locked`: 575 library + 1 binary; fmt и strict Clippy прошли.
  Linux Docker release build, strict Clippy и test gate: 626 library + 1 binary
  и doc-tests. Независимый raw-TCP клиент проверил HELP/NOOP с NBSP
  (501), затем NOOP (250) и QUIT (221) в той же сессии. Acceptance T102.
- Контейнер `litterkeep-smtp-ascii-string-wsp` и образы
  `litterkeep-smtp-ascii-string-wsp-runtime:20260926`,
  `litterkeep-smtp-ascii-string-wsp-check:20260926` удалены; отсутствие
  проверено после acceptance. TLS/accounts сохранены в
  `./data/g17-smtp-ascii-string-wsp`. Сервер не рекламирует SMTPUTF8 и не
  возвращает UTF-8 VRFY identity; G17 остаётся RUNNING.

### 2026-09-26: G17 RFC 5322 Date header WSP in IMAP SENT search

- `SENTBEFORE`, `SENTON` и `SENTSINCE` читают RFC 5322 `Date` header. Внешние
  и weekday whitespace сокращения ограничены RFC WSP (SP/HTAB); NBSP больше не
  превращает невалидное значение в поисково-валидную дату.
- Новая регрессия до изменения приняла leading NBSP. После фикса Windows
  `cargo test --locked` прошёл: 576 library + 1 binary и doc-tests; fmt и strict
  Clippy прошли. Linux Docker release build, strict Clippy и тесты: 627 library
  + 1 binary и doc-tests. Независимый IMAP STARTTLS `imaplib` APPEND/SEARCH
  проверил пять Date-вариантов; `SENTON 2-Feb-2026` вернул только сообщение с
  валидной датой. Acceptance T103.
- Runtime `litterkeep-imap-sent-date-wsp` и образы
  `litterkeep-imap-sent-date-wsp-runtime:20260926`,
  `litterkeep-imap-sent-date-wsp-check:20260926` удалены после теста, отсутствие
  проверено. Аккаунты и PKI хранятся в
  `./data/g17-imap-sent-date-wsp`. Поддержка obsolete Date syntax остаётся
  частичной, G17 остаётся RUNNING.

### 2026-09-26: G17 RFC 3834 Auto-Submitted whitespace guard

- `Auto-Submitted` больше не использует Unicode-aware trim при сравнении
  keyword `no`: только RFC WSP SP/HTAB может быть внешним whitespace. Malformed
  NBSP не отключает suppression и не открывает путь для vacation loop.
- Новая регрессия до изменения разрешала ответ на leading NBSP `no`; после
  фикса SP/HTAB `no` остаётся разрешённым значением. Windows suite: 577 library
  + 1 binary, doc-tests, fmt и strict Clippy. Linux Docker release build,
  strict Clippy и test suite: 628 library + 1 binary и doc-tests. Проверка
  unit-only; Sieve runtime acceptance не выполнялся. Acceptance T104.
- Тестовый образ `litterkeep-sieve-autosubmitted-wsp-check:20260926` удалён
  после gate, его отсутствие проверено; отдельный runtime контейнер не запускался.
  Stage path: `./data/g17-sieve-autosubmitted-wsp`. RFC 3834 comments и optional
  parameter-list parsing остаются частично поддержанными, G17 RUNNING.

### 2026-09-26: G17 RFC 3834 Auto-Submitted CFWS and parameters

- `Auto-Submitted` выделен в отдельный typed parser. Он удаляет RFC 5322 CFWS
  comments вне quoted-string, разбирает RFC 2045 token/quoted-string параметры,
  распознаёт `no`, `auto-generated`, `auto-replied` и extension keyword. Только
  ровно одно синтаксически корректное поле со значением `no` разрешает vacation;
  invalid значение и повторы подавляют ответ.
- Unit-регрессии до исправления показали подавление валидного `no` с comments
  и parameters, а также разрешение ответа при двух повторных `no`. После фикса
  тесты проходят. Windows полный suite: 579 library + 1 binary, doc-tests,
  fmt и strict Clippy; Linux Docker Rust 1.97 release build, strict Clippy и
  suite: 630 library + 1 binary и doc-tests. Проверка parser unit-only,
  отдельный vacation runtime acceptance не выполнялся. Acceptance T105.
- Disposable test image удалён после gate, отсутствие проверено; runtime-контейнер
  не запускался. Data stage `./data/g17-sieve-autosubmitted-wsp`.
  G17 остаётся RUNNING.

### 2026-09-26: G17 current IMAP UTF-8 standard gap audit

- Перепроверена граница IMAP internationalization по актуальному RFC 9755
  (2025), который заменил RFC 6855 и расширяет IMAP4rev1, но не IMAP4rev2.
  `imap-profile.md` теперь перечисляет фактическое состояние и отдельные
  acceptance gates для capability/ENABLE state, UTF-8 quoted strings, mailbox
  names и responses, APPEND, SEARCH charset, authentication и message/global.
- CAPABILITY не рекламирует `UTF8=ACCEPT`; наличие capability в parser enum и
  пустой ENABLE response не являются реализацией extension. Runtime acceptance
  не запускался и PASS не выставлялся. Следующий IMAP этап должен реализовать
  состояние и wire семантику до объявления capability. G17 остаётся RUNNING.

### 2026-09-26: G17 Auto-Submitted wire acceptance

- `tests/blackbox/sieve_autosubmitted_wire.py` использует отдельную data stage
  `./data/g17-sieve-autosubmitted-wire-final` и независимые ManageSieve TLS,
  SMTP, IMAP и SMTP peer реализации. Восемь SMTP доставок прошли через активный
  vacation script; IMAP прочитал все восемь исходников, peer принял ровно два
  ответа на RFC 3834 `no` с корректными comments/parameters и сверил
  `MAIL FROM:<>` и destinations. Automated, extension, malformed, duplicate и
  NBSP значения подавлены. Acceptance T106.
- Клиент принудительно передал `SMTPUTF8` для всех входных писем, хотя сервер
  capability не рекламирует. Acceptance доказывает Sieve policy и queue runtime
  behavior, но не объявленную SMTPUTF8 interoperability. Первая harness-попытка
  ошибочно запускала queue-drain со вторым Store writer-ом и получила writer
  lock; её stage `./data/g17-sieve-autosubmitted-wire` сохранена и не засчитана.
  Финальный сценарий прошёл через штатный outbound scheduler. Disposable Linux
  image и runtime container удаляются после gate; data/PKI stage сохраняется.
  G17 остаётся RUNNING.

### 2026-09-26: G17 RFC 9755 quoted-string lexer foundation

- Добавлен типизированный `StringMode`: legacy режим остаётся default и
  отвергает high-bit quoted-string; отдельный RFC 9755 режим допускает валидные
  UTF-8 scalars только внутри quoted-string, не расширяя ASCII atom. Unicode
  controls отвергаются. Helper пока не подключён к session ENABLE state,
  capability не рекламируется, runtime acceptance отсутствует; это PARTIAL,
  не реализация `UTF8=ACCEPT`. Acceptance T107.
- Windows `cargo test --locked`: 581 library + 1 binary, doc-tests, fmt и strict
  Clippy прошли. Linux Docker Rust 1.97 release build, strict Clippy и полный
  test target: 632 library + 1 binary и doc-tests. Disposable test image удалён
  после gate; runtime container не запускался. Следующие обязательные связки: ENABLE/session, mailbox
  input/output, APPEND header policy и SEARCH charset. G17 остаётся RUNNING.

### 2026-09-26: G17 RFC 9755 conformance wording audit

- По первичному RFC 9755 уточнены нормативные силы рекомендаций для UTF-8
  mailbox names и SEARCH CHARSET, обязательства APPEND при реализованном
  extension, а также исключённое множество Unicode mailbox controls.
- `UTF8=ONLY` выделен в самостоятельную неподдержанную строку acceptance:
  он подразумевает `UTF8=ACCEPT`, но добавляет обязательный legacy rejection
  profile. CAPABILITY не изменена; сеточной реализации или PASS нет.
- Проверено `git diff --check`; это аудит документации, не основание менять
  тестовые счётчики или статус G17.

### 2026-09-26: G17 UTF-8 mailbox validation foundation

- Добавлен типизированный validator mailbox name для точных исключений RFC
  9755: C0, DEL, C1, U+2028 и U+2029; начальный BOM отклоняется по RFC 5198.
  Три unit-теста подтверждают отказ и сохранение разрешённой исходной строки.
- Validator не подключён к IMAP wire/session, не выполняет NFC normalization
  и не проверяет Unicode assigned-version; `UTF8=ACCEPT` остаётся скрытым,
  acceptance T108 имеет PARTIAL, runtime acceptance не заявляется.
- Windows: 584 library + 1 binary tests, doc-tests, fmt и strict Clippy прошли.
  Linux Docker Rust 1.97 release build, strict Clippy, 635 library + 1 binary
  tests и doc-tests прошли. Временный image удалён; matching containers не было.

### 2026-09-26: G17 RFC 9755 SELECT parser foundation

- Добавлен явный parser entry point с `StringMode`; SELECT/EXAMINE декодируют
  quoted UTF-8 mailbox names через RFC 9755 validator только в `Utf8Accept`.
  Обычный parser остаётся legacy modified UTF-7, что проверяют два новых теста.
- Reactor/session ENABLE не переключены, остальные mailbox commands и mailbox
  response encoding не подключены. Capability остаётся скрытой; T109 PARTIAL,
  runtime acceptance отсутствует. Следующая связка должна охватить весь mailbox
  command set, LIST/NAMESPACE output и только затем session state.
- Windows: 586 library + 1 binary tests, doc-tests, fmt и strict Clippy прошли.
  Linux Docker Rust 1.97 release build, strict Clippy, 637 library + 1 binary
  tests и doc-tests прошли. Временный image удалён; matching containers не было.

### 2026-09-26: G17 RFC 9755 mailbox command parser foundation

- Явный `StringMode` передаётся во все parser paths с mailbox input: SELECT,
  EXAMINE, CREATE, DELETE, RENAME, SUBSCRIBE, UNSUBSCRIBE, APPEND, STATUS,
  COPY/MOVE, UID COPY/MOVE, LIST и LSUB reference/pattern. Legacy production
  parser сохраняет modified UTF-7 default. Три focused tests проверяют выбранные
  команды, включая wildcard hierarchy и запрещённый scalar.
- Session ENABLE, reactor mode transition и UTF-8 response encoding не включены;
  APPEND header policy и SEARCH charset requirements не закрыты. T110 PARTIAL,
  `UTF8=ACCEPT` остаётся скрытым, wire acceptance отсутствует.
- Windows: 587 library + 1 binary tests, doc-tests, fmt и strict Clippy прошли.
  Linux Docker Rust 1.97 release build, strict Clippy, 638 library + 1 binary
  tests и doc-tests прошли. Временный image удалён; matching containers не было.

### 2026-09-26: G17 RFC 9755 session mailbox codec foundation

- `Session` хранит typed `StringMode` с default Legacy. Reactor передаёт его
  обычному parser и SEARCH-literal parser; LIST, LSUB, STATUS и root LIST output
  выбирают serializer: modified UTF-7 для Legacy или UTF-8 quoted-string для
  Utf8Accept. ENABLE перехода не делает, поэтому фактический wire mode остаётся
  Legacy и CAPABILITY не меняется.
- Два Linux-only теста проверяют кодирование Unicode и quoted escapes. Первая
  Linux попытка выявила dead SEARCH wrappers; они удалены, тестовый caller
  переведён на typed mode API. Итоговые gates: Windows 587 library + 1 binary,
  fmt, tests, doc-tests и strict Clippy; Linux Docker Rust 1.97 release build,
  strict Clippy, 640 library + 1 binary и doc-tests. Временный image удалён,
  matching containers не было. T111 остаётся PARTIAL: wire acceptance отсутствует.

### 2026-09-26: G17 RFC 9755 APPEND UTF-8 header guard

- После RFC-проверки UTF8=ACCEPT не активировался отдельно, поскольку сервер
  должен отклонять APPEND с 8-bit header octets до ENABLE и принимать его после.
  Добавлен guard на завершении literal до отправки storage request. Проверяется
  только header region; тело с произвольными octets и пустой header block
  пропускаются. При отсутствующем разделителе проверка консервативно считает
  весь literal заголовками.
- Четыре Linux-only unit tests покрывают UTF-8 заголовок, бинарное тело,
  пустой header block и отсутствие separator. Linux Docker Rust 1.97: release
  build, strict Clippy, 644 library + 1 binary tests и doc-tests прошли. T112
  PARTIAL: независимый wire тест ещё не доказывает ответ NO и отсутствие записи;
  ENABLE, SEARCH charset behavior и capability остаются неподключёнными. На
  Windows platform-specific reactor tests не компилируются, поэтому gate там
  служит только проверкой portable library.

### 2026-09-26: G17 RFC 9755 ENABLE and SEARCH charset transition

- `ENABLE UTF8=ACCEPT` теперь переключает `Session.imap_string_mode` только
  после аутентификации и включает extension в соответствующий `ENABLED`
  response; ENABLE без этой capability сохраняет прежнее поведение. Строка
  режима сбрасывается при TLS lifecycle reset.
- SEARCH parser различает отсутствующий и явно переданный CHARSET. В UTF8
  session явный CHARSET приводит к BAD до Store lookup; если mailbox не выбран,
  сохраняется первичный NO `select mailbox first`. Новые проверки закрепляют
  parser distinction и успешный/неуспешный ENABLE state transition.
- CAPABILITY намеренно не рекламирует UTF8=ACCEPT: независимое wire acceptance,
  проверка durable APPEND отказа до ENABLE и полного приёма после него ещё нужны.
  T113 PARTIAL; поведение текущего клиента не проверялось. После добавления
  reactor-проверки BAD при `RETURN (SAVE)` Windows gate дал 588 library + 1
  binary test, fmt и strict Clippy; Linux Docker Rust 1.97 дал release build,
  strict Clippy, 648 library + 1 binary tests и doc-tests. Временный image
  удалён, matching containers отсутствовали.

### 2026-09-26: G17 RFC 9755 independent APPEND wire acceptance

- Добавлен `tests/blackbox/imap_utf8_accept_append.py`, независимый Python
  `imaplib` STARTTLS клиент. На живом Rust 1.97 Linux runtime он подтвердил:
  UTF-8 header APPEND до ENABLE получает NO; SELECT в отдельной сессии видит
  пустой INBOX; ENABLE в AUTH state возвращает `* ENABLED UTF8=ACCEPT`; затем
  APPEND получает APPENDUID, а FETCH RFC822 возвращает исходные 55 octets,
  включая NUL в body. Пароль Bob скрипт прочитал из локального
  `test-accounts.tsv`, не печатая его. Для актуальной RFC 9755 framing тест
  намеренно не включает legacy imaplib `_mode_utf8()`, который добавляет старый
  APPEND `UTF8` data item из RFC 6855.
- T112 теперь PASS только для этого проверенного APPEND transition, не для
  полного RFC 9755. CAPABILITY остаётся скрытым; проверка generic клиента не
  является проверкой Android/Litterkeep клиента. Данные сохранены в
  `./data/g19-rfc9755-append-wire`. Одноразовый runtime container и image
  остановлены/удалены, отсутствие проверено; ранее существовавшие контейнеры
  не изменялись. Скрипт `--help` проверен, wire acceptance прошёл.

### 2026-09-26: G17 RFC 9755 mailbox command wire acceptance

- Тот же независимый STARTTLS `imaplib` сценарий расширен session matrix:
  legacy UTF-8 CREATE получает BAD, затем после ENABLE проходят UTF-8 CREATE,
  SUBSCRIBE, LIST, LSUB, STATUS и SELECT. APPEND UTF-8 headers в отдельной
  Unicode mailbox сохраняет точные исходные bytes при FETCH.
- T114 PASS ограничен перечисленными командами. RFC 5198 NFC/assigned-version
  проверки, malformed UTF-8 wire cases, остальные response paths и CAPABILITY
  readiness остаются незакрытыми; T111/T113 остаются PARTIAL. Данные сохранены
  в `./data/g19-rfc9755-mailbox-wire`. Созданные runtime container/image
  удалены и отсутствие проверено.
- При финальной перепроверке выяснилось, что реализация этого чекпойнта уже
  была в HEAD `d75fc71`; краткий статус показал лишь отдельное незакоммиченное
  разбиение `Session`. TLS lifecycle методы перенесены в
  `session_transport_methods.rs`, чтобы оба файла соблюдали лимит 300 строк.
  После переноса повторно прошли fmt, Windows 587 + 1 тест, strict Clippy и
  Linux Docker Rust 1.97 release build, strict Clippy, 640 + 1 тест и
  doc-tests. Временный image удалён, matching containers отсутствовали.

### 2026-09-26: G17 RFC 9755 invalid UTF-8 mailbox wire rejection, PASS

- Независимый TLS acceptance расширен отрицательными CREATE-командами в
  включённом `UTF8=ACCEPT` session: malformed UTF-8 внутри quoted-string,
  запрещённые RFC 9755 C1 и C0 scalars, а также UTF-8 atom. Каждый случай
  получает tagged BAD, после чего тот же клиент успешно выполняет NOOP.
- Linux Rust 1.97 image собран отдельным временным тегом. `init` создал свежие
  данные в `./data/g19-rfc9755-invalid-quoted-wire`; полный независимый
  STARTTLS сценарий прошёл, включая прежние positive CREATE/LIST/LSUB/STATUS/
  SELECT и байтовую APPEND/FETCH проверку. Container остановлен и удалён,
  image удалён, отсутствие обоих подтверждено; этапные данные сохранены.
- Обзор `docs/imap-profile.md` исправлен: ENABLE уже включает локальный режим,
  хотя capability остаётся скрытой до полного RFC 9755 профиля. Таблица теперь
  отражает live-проверки malformed UTF-8, controls и UTF-8 atom. RFC 5198
  assigned-version/NFC policy, оставшиеся команды/ответы и клиент Litterkeep
  не проверялись; полный RFC профиль по-прежнему PARTIAL. Скрипт имеет 121
  строку, `--help` и `git diff --check` прошли.

### 2026-09-26: G17 RFC 9755 explicit SEARCH CHARSET wire acceptance, PASS

- Независимый STARTTLS сценарий после ENABLE и SELECT подтвердил tagged BAD
  как для `SEARCH CHARSET UTF-8 ALL`, так и для смешанного регистра
  `SEARCH cHaRsEt uTf-8 ALL`. После обеих ошибок NOOP проходит, затем обычный
  implicit UTF-8 `SEARCH ALL` успешно возвращает пустой результат. Существующий
  unit test проверяет, что extension отказ происходит до store routing.
- Linux Docker runtime использовал только новый каталог
  `./data/g19-rfc9755-search-charset-wire`; полный APPEND/mailbox/invalid-input
  сценарий вместе с новыми SEARCH checks прошёл. Временные container и image
  удалены, отсутствие подтверждено; данные сохранены. `py_compile` и
  `git diff --check` прошли.
- T113 закрывает только эту RFC 9755 SHOULD границу. Остальные SEARCH criteria,
  Unicode assigned-version policy, response paths, capability readiness и
  Litterkeep interoperability остаются непроверенными; G17 остаётся ACTIVE.

### 2026-09-26: G17 RFC 9755 Unicode mailbox mutation and transfer acceptance, PASS

- Независимый STARTTLS `imaplib` acceptance после ENABLE провёл письмо с
  исходными UTF-8 headers через Unicode mailbox: CREATE, SUBSCRIBE, COPY,
  UNSUBSCRIBE, MOVE, RENAME и DELETE. STATUS/LSUB проверили mailbox и
  subscription state; после COPY и MOVE FETCH RFC822 совпал с исходными 55
  octets, включая NUL в body. `UNSELECT` завершил выбор до RENAME/DELETE.
- Проверка воспроизводилась командой
  `python tests/blackbox/imap_utf8_accept_append.py --port 32143 --ca data/pki/ca.pem --credentials data/g19-rfc9755-mailbox-ops-retry/test-accounts.tsv`.
  Первый запуск теста обнаружил только harness mistake: исходный mailbox был
  выбран readonly, поэтому сервер правильно ответил `[READ-ONLY] COPY denied`.
  Тест исправлен явным writable SELECT и полностью повторён на новом каталоге.
- Linux Docker runtime image `litterkeep-rfc9755-ops:work` успешно прошёл; все
  созданные для этапа container/image остановлены/удалены и отсутствие
  подтверждено. Данные сохранены в `./data/g19-rfc9755-mailbox-ops-retry`.
  `python -m py_compile`, `git diff --check` прошли; acceptance script 182
  строк, Rust production code не менялся.
- Обновлены `docs/imap-profile.md` и `docs/protocol-coverage.md`, включая
  исправление устаревшего утверждения, что reactor ещё не включает ENABLE
  session mode. RFC 5198 assigned-version/NFC policy, остальные UTF8 response
  paths, Litterkeep interoperability и полная RFC 9755 CAPABILITY readiness
  не проверены; T114 расширяет только перечисленные команды, профиль остаётся
  PARTIAL, G17 ACTIVE.

### 2026-09-26: G17 RFC 9755 message/global interoperability, PASS

- Независимый STARTTLS `imaplib` test теперь выполняет ENABLE UTF8=ACCEPT до
  создания mailbox, APPEND `message/global` и MIME FETCH. BODYSTRUCTURE сохранил
  допустимое IMAP4rev1 `MESSAGE/GLOBAL`; encoded BODY[1], decoded BODY[1.1],
  вложенные UTF-8 headers, BINARY[1.1] и полный RFC822 literal совпали с
  ожидаемыми octets.
- PASS в Linux Docker Rust 1.97 runtime, данные этапа в
  `./data/g19-rfc9755-message-global-enabled`. Команда клиента:
  `python tests/blackbox/imap_message_global.py --port 32143 --ca data/pki/ca.pem`;
  пароль Bob передан через ожидаемую переменную окружения из локального fixture,
  не выведен. Временные image/container удалены и отсутствие подтверждено.
  `python -m py_compile` и `git diff --check` прошли; скрипт 134 строки.
- Матрицы теперь подтверждают сохранение `message/global` при UTF8 mode.
  RFC 9755 §6 разрешает оба BODYSTRUCTURE представления для IMAP4rev1; это
  acceptance одного media type, не полного MIME/IMAP профиля. Litterkeep,
  оставшиеся RFC 5198 Unicode constraints, CAPABILITY readiness и другие
  response paths не проверены; G17 остаётся ACTIVE.

### 2026-09-26: G17 RFC 9755 UTF-8 SEARCH subject acceptance, PASS

- После ENABLE и APPEND независимый `imaplib` выполняет implicit
  `SEARCH SUBJECT "café"` и получает ровно UID/sequence `1`; это проверяет
  передачу UTF-8 quoted-string через parser и поиск по реальному Subject, а не
  только успешный разбор либо пустой `SEARCH ALL`.
- PASS в Linux Rust 1.97 Docker runtime с новым хранилищем
  `./data/g19-rfc9755-search-subject-wire`. Повторённый общий RFC9755 сценарий
  также прошёл explicit CHARSET BAD, case-insensitive keyword, invalid UTF-8/
  C0/C1/atom rejection, mailbox mutations, COPY/MOVE exact bytes и APPEND.
  Одноразовые image/container удалены; stage data сохранены.
- `python -m py_compile tests/blackbox/imap_utf8_accept_append.py` и
  `python -m py_compile tests/blackbox/imap_message_global.py`,
  `cargo fmt --all -- --check`, `cargo test --locked imap::mailbox_utf8::tests`
  (3 passed), `cargo test --locked imap::mailbox_mode_tests` (3 passed) и
  `git diff --check` прошли. `docs/acceptance.tsv` синхронизирована с живым
  состоянием UTF8 negotiation: больше не утверждает, что ENABLE и validator
  не подключены; добавлены scoped wire rows T115-T117. Комментарий в
  `src/imap/mailbox_utf8.rs` исправлен: validator подключён, но assigned-version
  и NFC handling по-прежнему отсутствуют. T108 и полный RFC9755 профиль остаются
  PARTIAL; Litterkeep не проверялся.

### 2026-09-26: G17 Unicode 17.0 assigned-code-point enforcement, scoped PASS

- Добавлены встроенные таблицы unassigned scalar values из официального UCD
  Unicode 17.0.0. Генератор закрепляет SHA-256 исходного UnicodeData.txt,
  исчерпывающе сверяет все 1,114,112 code points и делит результат на Rust
  shards не более 300 строк. Compile-time проверка сопоставляет UCD с
  `char::UNICODE_VERSION` установленного Rust 1.97.1. Production crate не
  добавлялись; источник, лицензия и процедура повторной генерации описаны в
  `docs/unicode-data.md`.
- Validator enabled-mode mailbox commands теперь отклоняет unassigned scalars.
  Windows `cargo test --locked`: 592 библиотечных и 1 CLI тест прошли;
  `cargo fmt --all -- --check` и `cargo clippy --all-targets --locked -- -D
  warnings` прошли. Linux Docker Rust 1.97 release build, строгий Clippy и
  652 библиотечных плюс 1 CLI тест прошли.
- Независимый Python imaplib STARTTLS acceptance на Linux Docker runtime
  прошёл полный сохранённый mailbox/SEARCH/APPEND сценарий. После ENABLE
  CREATE имени с unassigned U+0378 получил tagged BAD, тот же клиент успешно
  продолжил команды; COPY/MOVE RFC822 octets совпали. Данные сохранены в
  `./data/g19-unicode17-wire`. Проверен независимый тестовый клиент, не
  приложение Litterkeep.
- Образа `litterkeep-unicode17-test:work`,
  `litterkeep-unicode17-runtime:work` и контейнер
  `litterkeep-unicode17-wire` остановлены/удалены после прогона; проверено их
  отсутствие. Этапные данные сохранены, несвязанный `lower-layer-v1` остался
  запущен и не затрагивался.
  NFC output normalization остаётся нереализованным, UTF8=ACCEPT capability
  остаётся скрытой, полный RFC 9755 профиль и G17 остаются PARTIAL/ACTIVE.

### 2026-09-26: G17 Unicode mailbox UID COPY/MOVE wire acceptance, PASS

- Новый независимый сценарий
  `tests/blackbox/imap_utf8_uid_transfer.py` через IMAP STARTTLS включает
  UTF8=ACCEPT, создаёт source/copy/move mailbox names с UTF-8 и проверяет
  `UID COPY`/`UID MOVE` по протоколу. Tagged ответы содержат `COPYUID`; после
  COPY source остаётся, после MOVE source пуст после нового SELECT, обе
  destination возвращают точные исходные RFC822 octets включая NUL body.
- Чистый Linux Rust 1.97 runtime прошёл сценарий на данных
  `./data/g19-rfc9755-uid-transfer-final`. Два первых прогона нашли дефекты
  acceptance harness, а не сервера: Python imaplib UID COPY возвращает
  FETCH-oriented untagged data, а EXISTS до повторного SELECT был прежним
  состоянием. Третий прогон после исправлений прошёл полностью. Эти ранние
  fixtures оставлены отдельно в `./data/g19-rfc9755-uid-transfer` и
  `./data/g19-rfc9755-uid-transfer-retry`.
- Скрипт компилируется через `python -m py_compile`, содержит 122 строки.
  Docker runtime собран для неизменённого Rust source; контейнер и image
  остановлены/удалены, отсутствие проверено. T110 остаётся PARTIAL из-за оставшихся
  Net-Unicode/NFC и response-path требований. `UTF8=ACCEPT` остаётся скрытой,
  Litterkeep не проверялся, G17 ACTIVE.

### 2026-09-26: G17 RFC 3894 redirect :copy external SMTP acceptance, PASS

- Новый independent acceptance
  `tests/blackbox/sieve_redirect_copy_peer.py` подключается к ManageSieve
  STARTTLS, проверяет объявленный `copy`, активирует `redirect :copy`, подаёт
  MIME письмо с binary attachment через SMTP STARTTLS и читает локальный
  implicit-keep результат через IMAP STARTTLS.
- После остановки единственного source Store owner bounded `queue-drain`
  доставил durable redirect job независимому SMTP capture peer. Проверены
  sender/recipient envelope, совпадение целых RFC822 bytes внешней доставки и
  Bob inbox, а также decoded attachment bytes. Source body получает ожидаемые
  Return-Path/Received trace fields; исходное submitted RFC822 содержимое и
  вложение сохранены после trace prefix.
- PASS на Linux Rust 1.97 image, fixture
  `./data/g19-sieve-redirect-copy-final`. Проверен независимый SMTP peer, не
  Litterkeep и не произвольный сторонний MTA. Container/image удалены после
  проверки, `lower-layer-v1` не затрагивался. `docs/sieve-coverage.md` и T119
  обновлены; Windows gates и Python compile прошли. Общая цель и G17 остаются
  ACTIVE, поскольку остальные RFC/Sieve gaps ещё открыты.

### 2026-09-26: G17 RFC 5198 Unicode 17 NFC mailbox normalization, scoped PASS

- Добавлена собственная NFC реализация без runtime-зависимостей: Unicode 17
  canonical decomposition, combining-class reordering, composition blocking,
  composition exclusions и алгоритмический Hangul. Генератор закрепляет SHA-256
  официальных Unicode 17.0.0 UCD файлов; generated Rust shards не превышают
  лимит 300 строк. Данные и corpus хранятся только в локальном `./data` и не
  попадают в Docker build context.
- Enabled-mode IMAP mailbox inputs нормализуются до доступа к Store, а
  соответствующие mailbox response names сериализуются в NFC. Полный официальный
  `NormalizationTest.txt` прошёл: Windows `cargo test --locked` с corpus-test
  (1 passed), основной прогон оставляет его ignored. Windows gates: fmt, strict
  Clippy, 595 library + 1 binary tests, Python compile и diff-check прошли.
- Linux Docker Rust 1.97 release image собран; strict Clippy и 655 library +
  1 binary tests прошли (1 ignored corpus test). Независимый Python imaplib
  STARTTLS сценарий на Linux проверил decomposed CREATE, composed LIST/SELECT и
  DELETE через decomposed имя; также прошли RFC 9755 mailbox/SEARCH/COPY/MOVE/
  APPEND проверки. Fixture сохранён в
  `./data/g20-rfc9755-nfc-wire`.
- Временные `litterkeep-nfc-wire`, `litterkeep-nfc-test:work` и
  `litterkeep-nfc-runtime:work` удалены, отсутствие подтверждено;
  `lower-layer-v1` не затрагивался. Проверялся независимый клиент, не
  приложение Litterkeep. Старые persisted decomposed mailbox names и их
  миграция/доступность не проверялись. Другие Net-Unicode response paths ещё
  открыты, поэтому RFC 9755 capability остаётся скрытой, T108 и G17 остаются
  PARTIAL/ACTIVE.

### 2026-09-26: G17 RFC 9755 NFC LSUB and STATUS response acceptance, scoped PASS

- Расширен независимый `imaplib` STARTTLS acceptance: папка создаётся с
  decomposed `cafe + combining acute`, затем SUBSCRIBE/LSUB и STATUS вызываются
  с decomposed именем. LSUB и STATUS wire responses обязаны содержать composed
  NFC `café`; проверка завершилась успешно на Linux Rust 1.97.
- Полный сценарий также повторно проверил ENABLE, malformed/unassigned mailbox
  rejection, LIST/SELECT/DELETE alias, SEARCH, APPEND exact octets и COPY/MOVE.
  Fixture сохранён в `./data/g21-rfc9755-nfc-lsub-status`; disposable container
  и оба tagged images удалены и отсутствие проверено. `lower-layer-v1` оставлен
  запущенным.
- `docs/acceptance.tsv` содержит scoped T120 PASS. Это не миграция исторических
  decomposed folder records и не аудит всех RFC 9755 quoted-string response
  paths; UTF8=ACCEPT остаётся скрытым, G17 остаётся ACTIVE.

### 2026-09-26: G17 NFC aliases для исторических IMAP mailbox, scoped PASS

- Store сохраняет исторические journal records и физические mailbox keys без
  переписывания. NFC-equivalent lookup, rename и delete находят прежний stable
  folder ID; listing выдаёт NFC-представление, subscription journal выдаёт и
  удаляет canonical alias. Неоднозначные aliases и rename-коллизии отклоняются.
- Linux Rust 1.97 Docker gates: release/runtime сборка, strict Clippy,
  661 library tests и 1 CLI test прошли; один официальный Unicode corpus test
  оставлен ignored в обычном прогоне. Windows gates: fmt, strict Clippy,
  600 library tests и 1 CLI test прошли. Полный Unicode 17 NormalizationTest
  corpus запускался отдельно и прошёл.
- Store close/reopen test подтверждает стабильный folder ID и прежние message
  bytes. Независимый Linux STARTTLS acceptance создал историческую папку в
  modified UTF-7 с decomposed именем, проверил NFC результат LIST в legacy
  режиме, затем после reconnect и ENABLE открыл composed UTF-8 alias и прочитал
  точные исходные message bytes. Полный RFC 9755 сценарий также прошёл.
- Acceptance fixture сохранён в
  `./data/g23-nfc-legacy-folder-list-wire`. Контейнер и образы этого прогона
  удалены; ранее существовавшие `litterkeep-nfc-legacy-compat` и `lower-layer-v1`
  не затрагивались. Litterkeep-приложение не тестировалось. Другие Net-Unicode
  response paths ещё открыты, поэтому UTF8=ACCEPT скрыт, T108 и G17 остаются
  PARTIAL/ACTIVE.

### 2026-09-26: G17 Sieve address для RFC 8098 MDN recipients, scoped PASS

- RFC 5228 §5.1 требует ограничивать `address` полями с адресной структурой,
  включает обязательные адресные поля и рекомендует другие address-list поля.
  RFC 8098 §2.1 задаёт `Disposition-Notification-To` как `mailbox-list`; это
  стандартизованный случай за пределами начального списка RFC 5322.
- Типизированный `HeaderName` теперь распознаёт это поле как адресный список.
  Sieve unit test сравнивает содержащийся mailbox и убеждается, что обычный
  `Subject` с теми же октетами не является адресным кандидатом. Произвольные
  пользовательские поля не открыты: allowlist остаётся явным и конечным.
- Independent Linux Rust 1.97 Docker gates: strict Clippy, 662 library tests,
  один CLI test и doc-tests прошли; один отдельный Unicode corpus test остаётся
  ignored по конструкции. Windows Rust 1.97.1 gates: fmt, strict Clippy,
  601 library tests и один CLI test прошли; Python acceptance compile прошёл.
- Независимая проверка ManageSieve -> SMTP -> IMAP активировала script с
  `address :is "disposition-notification-to"`, отправила письмо Alice и
  проверила одинаковые RFC822 octets в Bob INBOX и Invoices (`fileinto :copy`
  вместе с implicit keep). Это не проверка клиента Litterkeep.
- Fixture сохранён в `./data/g24-sieve-mdn-address`; созданные container и
  images удалены и отсутствие проверено. Несвязанный `lower-layer-v1` оставлен
  запущенным. RFC 5228 full profile и общий G17 остаются ACTIVE.

### 2026-09-26: G17 POP3 fail-closed MIME binary CTE policy, scoped PASS

- RFC 6856 §2.1 запрещает MIME `binary` CTE в POP3 независимо от включённого
  UTF8 mode. До изменения ошибка bounded MIME parser-а отображалась как
  `binary=false`, поэтому непроверяемая структура могла быть отправлена как
  обычное письмо.
- Maildrop теперь кэширует типизированное состояние MIME policy: дерево без
  binary CTE, найденный binary CTE или непроверяемая структура. Для последнего
  состояния POP3 `RETR` и `TOP` отвечают `-ERR`; письмо не удаляется, а IMAP и
  durable Store сохраняют прежнюю семантику. Это намеренно fail-closed для
  malformed/unsupported MIME и может ограничить только POP3 доступность.
- Windows Rust 1.97.1: targeted MIME policy tests (2), полный Cargo test suite
  (602 library + 1 CLI), strict Clippy, fmt и Python compile прошли. Linux Rust
  1.97 Docker gates: strict Clippy, 663 library
  tests, 1 CLI test и doc-tests прошли; один Unicode corpus test остаётся
  ignored в обычном прогоне.
- Independent Linux Rust 1.97 STLS acceptance проверила валидный multipart с
  binary CTE и malformed multipart с тем же CTE. Обе структуры дают ожидаемый
  `-ERR` на `RETR` и `TOP`, после отказов команда QUIT успешно завершается.
  Независимый SMTP/IMAP peer использовался только для создания fixtures, не
  приложение Litterkeep.
- Fixture сохранён в `./data/g25-pop3-mime-unverifiable`; созданные container и
  образы удалены, отсутствие проверено. Несвязанный `lower-layer-v1` оставлен
  запущенным. Полный RFC 6856 профиль и общий G17 остаются ACTIVE.

### 2026-09-26: G17 ManageSieve MIME media-type comment wire checkpoint, scoped PASS

- Ранее вложенные RFC comments в `Content-Type` и punctuation в MIME `type/subtype`
  были покрыты только unit tests для общего structured-value parser и Sieve
  `:mime` validator.
- Независимый STARTTLS ManageSieve `CHECKSCRIPT` теперь принимает корректный
  `multipart (outer(inner))/related` с quoted boundary `x(y)` и multipart body,
  а malformed subtype с запрещённым `?` отклоняет. Остальные structured-value
  vectors не объявляются проверенными по wire.
- Обнаруженная ошибка была только в acceptance helper: MIME quotes вставлялись
  внутрь Sieve quoted-string без escaping. Helper теперь экранирует backslash и
  DQUOTE, сохраняя исходные CRLF. После фикса весь
  `managesieve_vacation_mime.py` прошёл на Linux Rust 1.97 runtime; Python
  compile и ограничение 300 строк подтверждены.
- Fixture сохранён в `./data/g26-sieve-mime-type-comments`; созданные runtime
  container и image удалены и отсутствие проверено. Несвязанный `lower-layer-v1`
  не затрагивался. Litterkeep client и полный Sieve/MIME profile не проверялись;
  G17 остаётся ACTIVE.

### 2026-09-27: G17 IMAP RFC 7888 LITERAL-, scoped PASS

- CAPABILITY объявляет `LITERAL-`, поддерживаются bounded non-synchronizing
  literals до 4096 octets в LOGIN, SEARCH и APPEND. Unit tests проверяют отказ
  до приёма превышающего лимит non-sync payload; сервер закрывает сессию с
  `BYE [TOOBIG]`, чтобы остаток in-flight payload не разобрался как следующая
  команда. `LITERAL+` и MULTIAPPEND не объявляются.
- Linux Rust 1.97 Docker gate: release сборка, strict Clippy, 673 library tests,
  1 CLI test и doc-tests прошли; один внешний Unicode corpus test ignored.
  Независимый STARTTLS acceptance проверил capability до/после TLS, LOGIN,
  SEARCH и APPEND без ожидания continuation; последующий FETCH совпал с точными
  исходными APPEND octets. Python syntax также проверен.
- Fixture `./data/acceptance-imap-literal-minus-20260927` сохранён. Созданный
  контейнер и test image удалены, отсутствие проверено. Litterkeep-приложение не тестировалось; это не
  подтверждает поддержку literals в остальных IMAP production или готовность
  полного RFC 9051 профиля. G17 остаётся ACTIVE.

### 2026-09-27: G17 MULTIAPPEND atomic Store groundwork, PARTIAL

- RFC 3502 требует передавать одно или более сообщений как части APPEND;
  неуспешная пакетная вставка не должна оставлять частичный набор видимым.
  Реализован внутренний Store primitive: сообщения получают упорядоченные UID
  и отдельные immutable blob IDs, а mailbox frames используют один durable
  commit transaction. После reopen либо виден весь batch, либо ни одного его
  сообщения. Регрессионный тест также подтверждает независимые system flags,
  keywords и INTERNALDATE каждого сообщения после recovery.
- Windows Rust 1.97.1: fmt, strict Clippy, 612 library tests, 1 CLI test и
  doc-tests прошли. Linux Rust 1.97 Docker: release build, strict Clippy,
  677 library tests, 1 CLI test и doc-tests прошли; один внешний Unicode corpus
  test ignored. Unit failure injection после первой mailbox frame подтверждает
  пустую видимую папку после recovery. Пустое сообщение отклоняется как APPEND
  cancellation. Созданный test image удалён и отсутствие тега проверено; runtime
  container не создавался.
- Это только storage groundwork: parser, IMAP session framing, APPENDUID UID-set
  serialization и independent wire acceptance ещё не сделаны. MULTIAPPEND не
  объявляется в CAPABILITY; общий G17 остаётся ACTIVE.

### 2026-09-27: G17 RFC 3502 empty APPEND cancellation, scoped PASS

- IMAP APPEND с literal длины 0 теперь завершается tagged `NO APPEND cancelled`
  до передачи записи Store owner-у. Это сохраняет состояние mailbox и позволяет
  использовать пустой literal как cancellation, как определяет RFC 3502.
- Независимый STARTTLS acceptance проверяет `NO`, последующий `NOOP`, пустой
  mailbox после `SELECT`, а затем успешные APPEND/SEARCH/FETCH в той же сессии.
  Проверка расширяет существующий `imap_literal_minus.py`, который остаётся
  меньше 300 строк.
- Linux Rust 1.97 Docker: release build, strict Clippy, 677 library tests,
  1 CLI test и doc-tests прошли; один внешний Unicode corpus test ignored.
  Windows Rust 1.97.1: fmt, strict Clippy, 612 library tests, 1 CLI test и
  doc-tests прошли. Fixture сохранён в
  `./data/acceptance-imap-append-cancel-20260927`; disposable server container и
  оба созданных image удалены, отсутствие проверено.
- Этот узкий результат не завершает APPEND/MULTIAPPEND профиль. Пакетный parser,
  reactor, APPENDUID UID-set и MULTIAPPEND wire acceptance остаются открытыми;
  G17 остаётся ACTIVE.

### 2026-09-27: G17 RFC 3502 MULTIAPPEND, scoped PASS

- Завершён сквозной APPEND batch: типизированный parser сохраняет порядок и
  отдельные flags/keywords/INTERNALDATE; единый ограниченный receiver принимает
  несколько sync literals и оставляет pipelined следующую команду; reactor
  передаёт набор единому Store owner-у, который фиксирует весь batch одной
  транзакцией. Успех возвращает APPENDUID с UID-set в порядке вставки.
  Capability MULTIAPPEND добавлена после появления реализации и wire-проверки.
- Независимый Python `imaplib` STARTTLS peer проверил capability до и после TLS,
  два continuation для двух sync literals, ответы `APPENDUID 1 3,4`, отдельные
  `\\Seen`/`\\Flagged` и точные RFC822 bytes обоих сообщений; первое тело
  включало NUL и `0xFF`. Проверен независимый wire-клиент, а не Litterkeep app.
- Linux Docker на установленном Rust 1.97: release build, strict Clippy,
  678 library tests, 1 CLI test и doc-tests прошли; один внешний Unicode corpus
  test ignored из-за отсутствующего UCD-файла. Windows Rust 1.97.1: fmt,
  `cargo test --all-targets` (614 library tests, 1 CLI test), strict Clippy и
  diff-check прошли.
  Python compile прошёл; acceptance script 182 строки. Fixture остаётся в
  `./data/g27-multiappend-wire`. Тестовый контейнер и оба созданных image tags
  удалены; точечная проверка Docker подтвердила их отсутствие.
- Это не закрывает полный APPEND или RFC 9051 профиль: APPENDLIMIT максимум,
  другие literal production, LITERAL+ и client interoperability остаются
  отдельными границами. G17 остаётся ACTIVE.

### 2026-09-27: G17 MIME duplicate Content-Type fallback, scoped PASS

- Устранён конкретный MIME read-path разрыв: два поля `Content-Type` раньше
  отвергали всё MIME-дерево, хотя RFC 2045 §5.2 рекомендует считать
  синтаксически неверное значение `text/plain`/US-ASCII. Теперь неоднозначный
  повтор использует этот fallback, не выбирает произвольное поле и не меняет
  исходный RFC822.
- Unit-тест проверяет тип MIME-проекции, сохранённые два заголовка и точный body.
  Независимый STARTTLS SMTP-клиент доставляет headerless, malformed и duplicate
  fixtures; IMAP BODYSTRUCTURE для всех показывает `TEXT/PLAIN`, а RFC822 FETCH
  совпадает с исходными bytes. Это независимый протокольный клиент, не Litterkeep.
- Linux Rust 1.97 Docker: release build, strict Clippy, 678 library tests,
  1 CLI test и doc-tests прошли; один внешний Unicode corpus test ignored из-за
  отсутствующего UCD-файла. Windows Rust 1.97.1: `cargo test --all-targets`
  прошёл (613 library tests, 1 CLI test), strict Clippy и fmt прошли. Python
  acceptance script 121 строка. Fixture сохранён в
  `./data/g28-mime-duplicate-content-type`; test container и оба созданных
  image tags удалены, точечная проверка Docker подтвердила их отсутствие.
- Ограничение относится к MIME projection, а не к отправке/валидации всей
  RFC 2045 грамматики; другие RFC 2045/2046 gaps остаются в матрице, цель G17
  остаётся ACTIVE.

### 2026-09-27: G17 RFC 2046 multipart epilogue wire acceptance, scoped PASS

- Усилена независимая IMAP acceptance для RFC 2046 §5.1.1. Multipart содержит
  transport-padding у delimiter-ов, preamble, две полезные части и epilogue,
  в котором после close-delimiter стоит строка с boundary и ложным третьим
  `Content-Type`. Проверено, что эта строка остаётся discarded epilogue, не
  добавляет MIME part, а исходный RFC822 с preamble/epilogue сохраняется точно.
- Независимый STARTTLS `imaplib` клиент проверил BODYSTRUCTURE с ровно двумя
  частями, точное тело `BODY.PEEK[1]`, побайтно декодированное вложение
  `BINARY.PEEK[2]` и полный RFC822 FETCH. Клиент Litterkeep не запускался.
- Linux Rust 1.97 Docker test stage прошёл strict Clippy, 678 library tests,
  1 CLI test и doc-tests; один внешний Unicode corpus test ignored без UCD-файла.
  Сборочный test target использовал закэшированные неизменённые Rust-слои;
  runtime wire acceptance выполнен на Linux контейнере. Python compile прошёл,
  скрипт 125 строк. Fixture сохранён в `./data/g29-mime-multipart-epilogue`;
  созданные контейнер и два image tags удалены и отсутствие проверено.
- Это закрывает только сетевую границу preamble/epilogue и multipart delimiter
  handling; общий MIME и протокольные профили остаются неполными, G17 ACTIVE.

### 2026-09-27: G17 RFC 2046 multipart/digest default wire acceptance, scoped PASS

- Проверен RFC 2046 §5.1.5 default для дочерней части multipart/digest без MIME-заголовков. Независимый STARTTLS imaplib peer выполнил IMAP APPEND, затем BODYSTRUCTURE подтвердил DIGEST с вложенными MESSAGE/RFC822 и TEXT/PLAIN; RFC822 FETCH совпал с исходными октетами.
- Это независимый IMAP wire-клиент. Сквозной SMTP transport и приложение Litterkeep в этом сценарии не проверялись. Python acceptance script прошёл py_compile и содержит 139 строк.
- Linux Rust 1.97 Docker test image был собран с повторным использованием cached неизменённых Rust layers; нового запуска Rust test suite этим checkpoint не подтверждается. Runtime wire acceptance выполнен в Linux контейнере. Fixture ./data/g30-mime-multipart-digest сохранён; созданный runtime container и оба image tags удалены, отсутствие проверено.
- Закрыта только wire-проверка default multipart/digest; полные RFC 2045/2046/MIME и остальные протокольные профили остаются неполными, G17 остаётся ACTIVE.

### 2026-09-27: G17 IMAP embedded RFC822 body sections, scoped PASS

- Продолжена wire-проверка headerless multipart/digest child с default MESSAGE/RFC822. Независимый STARTTLS IMAP peer проверил RFC 3501/RFC 9051 body section selectors: BODY.PEEK[1] возвращает вложенное RFC822-содержимое целиком, BODY.PEEK[1.HEADER] заголовки, BODY.PEEK[1.TEXT] текст сообщения, а BODY.PEEK[1.1] дочернюю MIME body-part. Все четыре literal совпали с точными ожидаемыми octets.
- python -m py_compile tests/blackbox/imap_multipart_digest.py прошёл; скрипт 159 строк и проверяет также предыдущие BODYSTRUCTURE/RFC822 invariants. Это независимый клиент; Litterkeep app и SMTP delivery не проверялись.
- Linux Docker runtime acceptance прошёл на Rust 1.97 image; build и test layers были cached, поэтому свежий запуск Rust suite не заявляется. Fixture ./data/g30-mime-multipart-digest оставлен. Созданный runtime container и оба stage image tags удалены, отсутствие перепроверено.
- Закрыты только перечисленные embedded-message FETCH selectors; полный RFC 3501/9051, MIME, и цель G17 остаются ACTIVE.

### 2026-09-27: G17 IMAP BINARY path through multipart/digest, scoped PASS

- Исправлен подтверждённый отказ BINARY для вложенного MIME leaf в multipart/digest: resolver теперь traverses digest multipart children и для выбранного MESSAGE/RFC822 продолжает путь от embedded MIME entity. BINARY и BINARY.SIZE сохраняют общую проверку leaf-only.
- Новый unit test проверяет decoded body и точный размер. Независимый STARTTLS IMAP wire acceptance проверяет BINARY.PEEK[1.1], BINARY.SIZE[1.1], предыдущие BODY sections/BODYSTRUCTURE и точные исходные RFC822 bytes.
- Windows Rust 1.97.1: fmt, strict Clippy, cargo test --all-targets прошли (614 library + 1 CLI; 1 UCD-dependent normalization test ignored). Свежий Linux Rust 1.97 Docker --no-cache test stage: strict Clippy, 679 library + 1 CLI tests и doc-tests прошли; один UCD-dependent normalization test ignored. Независимый Linux STARTTLS wire test прошёл. Fixture ./data/g30-mime-multipart-digest сохранён; runtime container и два созданных image tags удалены, отсутствие проверено.
- Это закрывает только BINARY traversal для вложения в multipart/digest; полный MIME/IMAP профиль и G17 остаются ACTIVE. Приложение Litterkeep не запускалось.

### 2026-09-27: G17 IMAP BINARY nested multipart profile unit matrix, scoped PASS

- Добавлена unit matrix проверки numeric path [1.1] через embedded MESSAGE/RFC822 для multipart/mixed, multipart/alternative, multipart/related, multipart/digest и MESSAGE/GLOBAL child. Во всех случаях decoded message body и размер BINARY/SIZE совпадают.
- Таргетированный Rust test прошёл на Rust 1.97.1; strict Clippy, fmt и diff-check прошли. Это расширение unit-покрытия, не повторный Linux wire acceptance. multipart/digest имеет независимую wire-проверку в T142; остальные перечисленные subtype варианты здесь unit-only.
- Полные MIME и IMAP профили остаются неполными, G17 остаётся ACTIVE.

### 2026-09-27: G17 IMAP BINARY nested multipart profiles wire matrix, scoped PASS

- Независимый Linux STARTTLS imaplib peer проверил BINARY numeric path [1.1] для multipart/mixed, multipart/alternative, multipart/related, multipart/digest и multipart/mixed с вложенным message/global. Для каждого APPEND проверены фактический BODYSTRUCTURE root subtype и тип embedded message, exact quoted-printable decode через BINARY.PEEK и совпадающий BINARY.SIZE (12 octets).
- Acceptance script прошёл python -m py_compile, 218 строк, Rust production source не менялся. Linux runtime image Rust 1.97 собран из текущего source; отдельный Rust suite в этом black-box-only checkpoint не запускался.
- Fixture сохранён в ./data/g31-binary-multipart-profile-wire. Bootstrap init контейнер завершился с --rm; временные runtime container и image удалены и отсутствие проверено. Проверялся независимый wire-клиент, не приложение Litterkeep.
- Это закрывает перечисленные IMAP BINARY nested multipart paths, не полный MIME/IMAP4rev2 профиль; G17 остаётся ACTIVE.

### 2026-09-27: G17 IMAP PEEK Seen и zero-count partial rejection wire, scoped PASS

- Независимый Linux STARTTLS imaplib peer выбрал writable INBOX и на пяти nested MIME fixtures проверил, что `BINARY.PEEK[1.1]` и все partial BINARY.PEEK запросы оставили `\Seen` unset.
- Partial count `0` для `BINARY.PEEK[1.1]<0.0>` и `BODY.PEEK[1.1]<0.0>` получил tagged BAD; NOOP после каждого отказа подтвердил сохранение рабочей IMAP-сессии. Требование non-zero count описано в [RFC 9051 §6.4.5](https://www.rfc-editor.org/rfc/rfc9051.html#section-6.4.5).
- Python acceptance прошёл `py_compile`, скрипт ровно 300 строк; Rust production source не менялся, Rust suite в этом wire-only checkpoint не запускался. Fixture `./data/g34-imap-fetch-peek-wire` сохранён. Проверялся независимый клиент, приложение Litterkeep не запускалось.
- Указанные FETCH-семантики подтверждены; полный IMAP/MIME профиль и G17 остаются ACTIVE.

### 2026-09-27: G17 IMAP nested BINARY partial decoded offsets, PARTIAL checkpoint

- Независимый Linux STARTTLS peer отправил BINARY partial request для пяти multipart fixtures из T144. Команда вернула literal ssag, origin offset <2> и полный BINARY.SIZE 12, но выбранный offset не различал slicing decoded octets и encoded source.
- Wire probe подтвердил literal `ssag`, origin label и полный размер. Его offset начинался до quoted-printable escape и не отличал decoded slicing от raw source slicing; это не считается доказательством порядка decode/slice. Дискриминирующая проверка и EOF-границы добавлены в T146. Скрипт прошёл `python -m py_compile` и остаётся меньше 300 строк; Rust suite заново не запускался.
- Отдельный fixture сохранён в ./data/g32-binary-partial-wire. Bootstrap init container с --rm, runtime container и image удалены, отсутствие проверено. Тестировал независимый IMAP peer, не приложение Litterkeep.
- Частичный FETCH проверен только для nested BINARY leaf и указанных профилей; полный IMAP/MIME профиль и G17 остаются ACTIVE.

### 2026-09-27: G17 IMAP BINARY partial boundaries and decoded offsets, scoped PASS

- Усилен independent Linux STARTTLS acceptance пяти nested MIME fixtures. Offset 8 попадает внутрь encoded `=20`; `BINARY.PEEK[1.1]<8.2>` вернул decoded `bo`, в отличие от raw encoded slice, что различает порядок decode-before-slice. Также проверены offset 0, усечение у конца (`dy`), ровно EOF, offset за EOF и count больше остатка (`ssage body`), плюс FETCH origin label.
- Full BINARY.SIZE[1.1] для section остаётся 12. Python acceptance script прошёл py_compile, 264 строки. Rust source не менялся; Rust suite в этом black-box checkpoint не запускался. Cargo fmt и diff-check прошли.
- Исправлена оценка T145 в acceptance.tsv: <2.4> сам по себе не отличал decoded octets от одинакового начального QP source prefix, поэтому checkpoint теперь PARTIAL; его ограничение явно отражено в protocol matrix.
- Fixture ./data/g33-binary-ranges-wire оставлен; bootstrap init с --rm, runtime container и image удалены и отсутствие проверено. Клиент Litterkeep не проверялся. Полный IMAP/MIME профиль и G17 остаются ACTIVE.

### 2026-09-27: G17 IMAP failed multi-item FETCH Seen isolation, scoped PASS

- Подтверждён дефект порядка побочных эффектов: reactor durable-ставил `\Seen` до того, как сериализовал все items одного FETCH. Для `FETCH (BODY[] BINARY[1])` с неизвестным Content-Transfer-Encoding успешный BODY мог сопровождаться ошибкой `NO [UNKNOWN-CTE]`, оставляя сообщение отмеченным как прочитанное.
- Reactor теперь сначала подготавливает полный ответ для каждого выбранного сообщения и только после успешной проверки всех items планирует durable mutation `\Seen`. Добавлен regression test с обычным BODY и BINARY item с неизвестным CTE; тест доказывает, что UpdateFlags не ставится в очередь при ответной ошибке.
- Linux Rust 1.97 Docker test stage: strict Clippy прошёл; 681 library + 1 CLI tests прошли, 1 UCD normalization corpus test ignored, doc-tests прошли. Windows `cargo test --lib`: 615 passed, 1 ignored; Windows target не включает Linux reactor. `cargo fmt` и `git diff --check` прошли.
- Независимый Linux STARTTLS acceptance APPEND-ит сообщение с неизвестным CTE, получает NO [UNKNOWN-CTE] на FETCH (BODY[] BINARY[1]), после NOOP читает FLAGS и подтверждает отсутствие \Seen. Acceptance script прошёл py_compile и содержит 114 строк; fixture ./data/g35-imap-fetch-error-seen оставлен. Проверялся независимый peer, не приложение Litterkeep.
- Временные runtime container и runtime/test image удалены после acceptance, отсутствие проверено. Полные IMAP/MIME профили и G17 остаются ACTIVE.

### 2026-09-27: G17 IMAP partial HEADER.FIELDS after subset, scoped PASS

- По RFC 9051 §6.4.5 partial octet ranges для HEADER.FIELDS/HEADER.FIELDS.NOT вычисляются после subsetting. Добавлен unit test: невыбранное поле стоит до Subject в inclusion fixture, а исключённый Subject стоит до From в NOT fixture; offset 0 должен начинаться с первого октета уже отфильтрованного header.
- Независимый Linux STARTTLS peer проверил оба варианта по wire, включая exact literal и origin label <0>. Acceptance script прошёл py_compile и содержит 174 строки.
- Linux Rust 1.97 Docker test stage: strict Clippy, 682 library + 1 CLI test прошли, 1 UCD normalization corpus test ignored, doc-tests прошли. Адресный Windows unit test, cargo fmt --check и git diff --check прошли.
- Fixture ./data/g36-imap-header-partial сохранён; init/runtime containers и созданный test image удалены, отсутствие проверено. Это независимый IMAP-клиент, не Litterkeep app; остальные FETCH cases и полный IMAP4rev2 профиль остаются неполными, G17 ACTIVE.

### 2026-09-27: G17 RFC 3516 BINARY capability advertisement, scoped PASS

- Реализованный RFC 3516 BINARY/BINARY.PEEK/BINARY.SIZE отсутствовал в трёх вариантах IMAP CAPABILITY. Добавлен точный atom BINARY для plaintext, STARTTLS и защищённого состояния; RFC 3516 §4.1 требует его объявления при поддержке расширения.
- Новый независимый wire acceptance до STARTTLS, после переключения и после AUTHENTICATE PLAIN подтверждает BINARY во всех состояниях. Bob успешно вошёл по отдельному Linux runtime серверу; проверялись только протокол и независимый клиент, приложение Litterkeep не проверялось.
- Docker strict Clippy прошёл; 682 library + 1 CLI tests прошли, 1 UCD normalization corpus test ignored, doc-tests прошли. Windows cargo test --lib: 616 прошли, 1 ignored. Python acceptance py_compile, cargo fmt --check и git diff --check прошли; файл клиента 122 строки. Добавлен именованный runtime stage в docker/Dockerfile.linux-test, чтобы runtime acceptance действительно запускал production binary.
- Fixture ./data/g37-imap-binary-capability сохранена. Временный init/runtime container и все три созданных image tags удалены, отсутствие проверено. Исправлена только публикация уже поддерживаемой возможности; IMAP4rev2 и G17 остаются ACTIVE.

### 2026-09-27: G17 RFC 2971 IMAP ID extension, scoped PASS

- Добавлена типизированная RFC 2971 ID команда. Parser принимает NIL или список string/nstring пар, отвергает malformed list, дубликаты без учёта ASCII-регистра, более 30 пар, поле длиннее 30 октетов и значение длиннее 1024 октетов.
- Сервер объявляет ID во всех вариантах CAPABILITY и на запрос всегда отвечает `* ID NIL`, не сохраняя и не раскрывая client/build/host данные. Unit tests проверили форму/лимиты; независимый Linux STARTTLS peer проверил NIL, list, duplicate rejection, отсутствие untagged ID при BAD, NOOP recovery и capability transition.
- Rust 1.97 Docker strict Clippy прошёл; 686 library + 1 CLI tests прошли, один UCD normalization corpus test ignored, doc-tests прошли. Четыре целевых Windows tests, cargo fmt --check, git diff --check и Python py_compile прошли; acceptance client 127 строк.
- Fixture ./data/g38-imap-id-extension оставлена; временный init/runtime containers и оба созданных Docker images удалены, отсутствие проверено. Независимое wire acceptance не проверяло Litterkeep app; IMAP4rev2 и цель G17 остаются ACTIVE.

### 2026-09-27: G17 RFC 2971 ID synchronizing literal acceptance, scoped PASS

- RFC 2971 `string`/`nstring` допускает literal. Независимый wire-test выявил отказ общего receiver-а: он не допускал ID и считал marker только после пробела, хотя первый элемент ID-списка может начинаться сразу после `(`.
- Добавлен типизированный ID literal admission с теми же bounded limits; receiver выделяет открывающую скобку до marker и затем передаёт собранные octets typed parser-у. Unit regression проверяет marker у `(`. Независимый Linux STARTTLS acceptance теперь отправляет synchronizing literal отдельно в позиции поля и значения, проверяя `* ID NIL`.
- Rust 1.97 Docker test target: strict Clippy, 686 library + 1 CLI tests passed, 1 UCD corpus test ignored, doc-tests passed. `cargo fmt --check`, Python `py_compile` и `git diff --check` passed; acceptance client 161 строк.
- Fixture ./data/g38-imap-id-extension сохранена. Неудачная проверка до фикса оставила чистыми свои ресурсы; итоговые runtime/test containers и images удалены, отсутствие подтверждено. Linux-only reactor wire-path не тестировался на Windows; Litterkeep app не запускалось. Полный IMAP4rev2 и G17 остаются ACTIVE.

### 2026-09-27: G17 RFC 7888 LITERAL- for IMAP ID strings, scoped PASS

- RFC 7888 §3 разрешает non-synchronizing literal во всех IMAP командах, а LITERAL- ограничивает его 4096 октетами. Независимый peer теперь отправляет ID field и ID value в `{n+}` форме, передавая marker вместе с payload без ожидания continuation; проверяются capability ID/LITERAL- и приватный ответ `* ID NIL`.
- Python acceptance py_compile и git diff-check прошли; файл сценария 192 строки. Production release runtime образ собран на Rust 1.97 из текущего source, wire acceptance прошёл, контейнер и image удалены. Rust tests не повторялись: с T152 изменился только независимый acceptance-клиент; полный Rust suite T152 остаётся актуальным для неизменившегося server source.
- Fixture ./data/g38-imap-id-extension сохранена. Проверялся независимый peer, не Litterkeep app. Малые payload прошли; граничные 4096/4097 octets этим тестом не проверялись. Полный IMAP4rev2 и G17 остаются ACTIVE.

### 2026-09-27: G17 RFC 2971 ID wire size boundaries, scoped PASS

- Расширен независимый STARTTLS сценарий ID с `LITERAL-`: проверены допустимые границы field=30 и value=1024 октета, затем отказы field=31/value=1025 без untagged ID response. Две следующие NOOP команды подтвердили восстановление session state после обоих отказов.
- Production runtime image собран из неизменённого Rust source и удалён после PASS wire acceptance. Python `py_compile`, 210-строчный лимит acceptance и `git diff --check` проверены. Rust suite не перезапускался, так как в этом checkpoint менялся только независимый peer и документы.
- Fixture ./data/g38-imap-id-extension сохранена; временный runtime container и image удалены. Это не клиент Litterkeep и не полное IMAP4rev2 acceptance; G17 остаётся ACTIVE.

### 2026-09-27: G17 RFC 3030 typed BDAT syntax and unadvertised drain, PARTIAL

- Добавлена typed grammar `BDAT n [LAST]` с decimal size, zero-length chunk,
  case-insensitive LAST и overflow/malformed rejection. Bounded raw receiver
  тестирует partial TCP reads, произвольные binary octets, zero-size framing,
  over-budget discard и сохранение следующей pipelined command.
- Полноценные CHUNKING transaction, per-chunk acknowledgement, error recovery
  и взаимодействие с PIPELINING ещё не реализованы, поэтому capability в EHLO
  не менялась. Peer, приславший не объявленный BDAT, получает после exact-size
  drain ответ 502, а следующий NOOP получает 250. Независимый Linux wire
  acceptance прошёл на STARTTLS listener до TLS negotiation.
- Linux Docker Rust 1.97 strict Clippy прошёл; 705 library tests passed, один
  внешний UCD corpus test ignored, один CLI test и doc-tests passed. Python
  acceptance прошёл py_compile; `cargo fmt --check` и `git diff --check` прошли.
  Fixture `./data/acceptance-rfc3030-unadvertised-20260927` сохранена.
- Созданные для прогона container и test image удалены, отсутствие проверено.
  Это проверка typed parser и безопасного отказа, не реализация RFC 3030
  CHUNKING/BDAT, не BINARYMIME и не проверка клиента Litterkeep; G17 остаётся
  ACTIVE.

### 2026-09-27: G17 RFC 3030 CHUNKING message transaction, PARTIAL

- Reactor теперь владеет BDAT message transaction между chunks, принимает
  zero-length и промежуточные chunks с отдельным 250, сохраняет точную octet
  границу и передаёт message в существующий durable AppendMany путь только на
  LAST. Неуспешная команда или превышение размера дренирует объявленный payload,
  очищает накопленные message bytes и требует RSET до новой транзакции.
- EHLO теперь объявляет RFC 3030 CHUNKING. Независимый TLS SMTP клиент
  отправил нулевой chunk, два MIME message chunks и прочитал вложение через
  независимый TLS IMAP клиент. Содержимое attachment совпало побайтно;
  SHA-256: `34eb0bb669ce15c7df8cb3769c0a0d6ca91494fc041ab9de4e65cbf4ce696596`.
  Отдельный raw TLS peer подтвердил capability, exact drain при BDAT без
  MAIL/RCPT и сохранение границы после payload. Клиент Litterkeep не проверялся.
- Acceptance fixture `./data/acceptance-rfc3030-bdat-final-20260927` использует
  локальный bind mount; тестовые аккаунты созданы штатной командой init.
  Runtime и test containers/images удалены после acceptance. Docker Rust 1.97
  strict Clippy прошёл; 705 library tests и 1 CLI test прошли, один официальный
  UCD corpus test ignored из-за отсутствующего файла, doc-tests прошли. Также
  остаётся отдельный interoperability audit для прочих RFC 3030 failure codes;
  `BINARYMIME` не реализован. G17 остаётся ACTIVE, этот этап не является
  полным RFC 3030 acceptance.

### 2026-09-27: G17 RFC 3030 rejected chunk drain and RSET recovery, PARTIAL

- Независимый TLS acceptance отправил malformed message body с bare LF как
  `BDAT ... LAST`, получил 552 только после payload drain, затем успешно выполнил
  RSET и NOOP. Отдельная передача 26 214 401 октета подтвердила точный drain
  oversized chunk, 552 и такое же восстановление состояния.
- На одном свежем Linux runtime build прошли оба acceptance: positive
  Alice-SMTP-BDAT -> Bob-IMAP attachment byte comparison и negative
  rejected-content/oversize recovery. Python стандартные TLS SMTP/IMAP библиотеки
  и raw socket peer независимы от серверного Rust parser. Litterkeep клиент не
  проверялся.
- Runtime container и image удалены; fixture `./data/acceptance-rfc3030-bdat-negative-20260927` сохранена. Этот дополнительный прогон не менял Rust source и не заменяет полный RFC 3030 conformance suite.

### 2026-09-27: G17 RFC 3030 mandatory RSET after failed BDAT, PARTIAL

- RFC 3030 §2 audit выявил неверную семантику в предыдущем negative acceptance:
  оно считало немедленный NOOP после отказа BDAT допустимым. Этот ожидаемый ответ
  был неверен, поэтому тот acceptance не засчитывается как корректный recovery.
- Reactor получил отдельные failed/discard states. После отказа он дренирует
  chunks, уже стоящие в pipeline, отвергает команды до RSET и только после RSET
  возвращается к command state. Независимый TLS raw peer подтвердил два
  последовательных rejected BDAT chunks, отказ NOOP до RSET, затем RSET/NOOP;
  delivery acceptance повторно подтвердил invalid-LAST и 26 214 401-октетный
  over-limit drain с восстановлением.
- На Linux Rust 1.97 Docker gate прошли strict Clippy, 705 unit tests, 1 CLI
  test и doc-tests; один UCD corpus test ignored из-за отсутствующего файла.
  Runtime/test images и контейнер удалены. G17 остаётся ACTIVE; BINARYMIME и
  полный RFC 3030 conformance suite не проверялись.
- Повторный review обнаружил literal CRLF typo в fallback-ответе sequence error;
  wire-visible ветка не была достигнута acceptance. Литерал исправлен, strict
  Docker gate и свежие raw/SMTP-to-IMAP acceptance пройдены повторно. Этот
  fallback case отдельно не достигался клиентом и не заявляется как wire-tested.

### 2026-09-27: G17 ManageSieve literal Store mutations and reopen, scoped PASS T271

- Независимый TLS-клиент аутентифицировал Bob и отправил `PUTSCRIPT` с
  non-synchronizing literal-именем и synchronizing literal-исходником. Затем
  `RENAMESCRIPT` принял два non-synchronizing literal-имени, а `SETACTIVE`
  активировал новое имя; `LISTSCRIPTS` показал единственную ожидаемую запись.
- Тот же production Linux server container был перезапущен с тем же bind-mounted
  `./data/acceptance-managesieve-literal-store`. После restart `LISTSCRIPTS`
  сохранил новое активное имя, а `GETSCRIPT` вернул исходные байты без изменений.
  Это независимый RFC 5804 wire peer, не клиент Litterkeep.
- Windows Rust 1.97.1 `cargo test --all-targets --locked`: 807 library и 1 CLI
  test passed, 3 ignored; strict all-target Clippy прошёл. Python `py_compile`,
  `cargo fmt --check` и `git diff --check` прошли. Rust source не менялся;
  Linux runtime image собран из актуального source через cache hit, поэтому
  свежий Linux Rust suite не заявляется.
- T271 добавлен в acceptance matrix как scoped PASS. Test fixture оставлен;
  созданные runtime container и image удалены, отсутствие подтверждено.
  Обязательный SCRAM-SHA-1 и полный RFC 5804 профиль остаются незакрытыми,
  G17 остаётся ACTIVE.

### 2026-09-27: G17 implicit TLS composition for all mail listeners, scoped PASS T272

- Выделена именованная композиция TLS listener-ов в
  `src/reactor/server_entry.rs`; `src/reactor.rs` сокращён ниже ограничения
  300 строк. Linux CLI `serve-all-mail-tls` включает SMTP, IMAP, POP3 и
  ManageSieve в одном reactor-е и с общим владельцем Store.
- Независимый TLS peer проверил CA/SAN и handshake перед greeting-ом на каждом
  listener-е. SMTP EHLO/AUTH PLAIN, IMAP CAPABILITY/AUTHENTICATE PLAIN, POP3
  CAPA/AUTH PLAIN и ManageSieve greeting/AUTHENTICATE PLAIN прошли; plaintext
  upgrade-команды внутри TLS не рекламировались.
- Windows Rust 1.97.1: 807 library + 1 CLI tests passed, 3 ignored; fmt и
  strict Clippy прошли. Свежая Linux Rust 1.97.1 Docker-сборка из изменённого
  source: strict Clippy, 907 library + 1 CLI tests passed, 3 ignored. Новый
  независимый black-box скрипт прошёл py_compile и Linux wire acceptance.
- Fixture `./data/acceptance-all-implicit-tls-20260927` сохранена. Временный
  Docker runtime container и созданный image удалены, отсутствие проверено.
  Это не тест Litterkeep app. RFC 8314 применим здесь к submission, IMAP и
  POP3; ManageSieve implicit TLS является дополнительным режимом сервера, не
  заявлением о норме RFC. Полные протоколы и G17 остаются ACTIVE.

### 2026-09-27: G17 RFC 5804 AUTHENTICATE mechanism literals, scoped PASS T273

- RFC 5804 §2.1 требует `String` для SASL mechanism и optional initial
  response. AUTHENTICATE теперь принимает PLAIN mechanism в quoted,
  synchronizing literal и non-synchronizing literal формах; optional response
  может отсутствовать либо передаваться quoted/literal. Framing вынесен в
  отдельные именованные модули, без превышения лимита 300 строк.
- Независимый TLS peer успешно аутентифицировал Bob, отправив PLAIN mechanism
  и Base64 response как non-synchronizing literals. Неизвестный CRAM mechanism
  с literal response был отвергнут; следующий pipelined NOOP получил `OK`.
  Linux unit tests отдельно проверяют sync mechanism continuation и отказной
  framing. Litterkeep app не запускалось.
- Rust 1.97.1 Linux Docker: strict Clippy прошёл, 911 library + 1 CLI tests
  passed, 3 ignored. Windows fmt, strict Clippy и 807 library + 1 CLI tests
  passed; Linux-only reactor tests на Windows cfg-исключены. Python `py_compile`
  и независимый wire acceptance прошли.
- Fixture `./data/acceptance-managesieve-auth-mechanism-20260927` сохранена;
  временные container и test image удалены, отсутствие подтверждено. Этот
  checkpoint закрывает только literal форму механизма; обязательный
  SCRAM-SHA-1 и прочие открытые protocol gaps остаются, G17 ACTIVE.

### 2026-09-27: G17 RFC 5804 synchronizing AUTHENTICATE literals, scoped PASS T274

- Усилено именно wire-доказательство RFC 5804 §1.2: independent TLS client
  передал mechanism как synchronizing literal и получил continuation, затем
  передал PLAIN response вторым synchronizing literal и получил отдельный
  continuation перед payload. После этого Bob успешно аутентифицировался.
- В том же запуске CRAM с non-synchronizing literal response получил отказ,
  а следующий NOOP успешно завершился. SMTP, IMAP и POP3 проверки общего
  acceptance script также прошли.
- Rust source/tests не менялись с T273. Linux runtime image собран с cache hit
  на release source layer, поэтому новый Linux Rust suite не заявляется;
  авторитетный свежий Linux strict-Clippy/full-suite результат остаётся T273:
  911 library + 1 CLI tests, 3 ignored. Для этого acceptance прошли Python
  `py_compile`, `cargo fmt --check` и `git diff --check`.
- Fixture `./data/acceptance-managesieve-auth-sync-20260927` сохранена;
  временные runtime container и созданный image удалены. Peer независимый,
  Litterkeep app не проверялось; SCRAM-SHA-1 и общий G17 остаются открытыми.

### 2026-09-27: G17 RFC 5804 challenge-response synchronizing literal, scoped PASS T275

- Закрыт отдельный framing-вариант после SASL challenge. Для PLAIN без
  initial-response сервер выдаёт пустой challenge; независимый STARTTLS peer
  отправил synchronizing literal marker, получил `OK "ready for literal data"`
  и лишь затем передал payload. Bob успешно аутентифицирован. Тест также
  сохраняет проверки отмены challenge и повторного входа.
- Linux receiver unit теперь проверяет sync continuation; Python acceptance
  проверяет тот же обмен на живом Linux server. Свежий Rust 1.97.1 Docker build,
  strict Clippy и полный тестовый набор прошли: 911 library + 1 CLI tests,
  3 ignored. Windows fmt, strict Clippy и 807 library + 1 CLI tests прошли,
  3 ignored; Python `py_compile` прошёл.
- Fixture `./data/acceptance-managesieve-challenge-sync-20260927` сохранена.
  Созданные для этапа Docker container и test image удалены, отсутствие
  проверено. Acceptance использовал независимый протокольный peer, а не
  Litterkeep app. SCRAM-SHA-1 остаётся обязательным незакрытым пробелом;
  полный профиль G17 остаётся ACTIVE.

### 2026-09-28: G18 POP3 RFC 5034 AUTH failure state restoration, scoped PASS T276

- Независимый Linux STARTTLS peer выбрал `USER bob` и проверил, что после
  отказных AUTH состояний он сохраняется: неподдерживаемый механизм, пустой и
  некорректный initial response, cancellation, некорректный continuation,
  неверные credentials и отсутствующий account. После этого валидный по
  RFC 1939 40-октетный неверный `PASS` получил `authentication failed`, а не
  `USER required`; затем новый `USER` и контрольный PLAIN вход завершились
  успешно. Секрет читался из fixture-файла и не передавался в argv.
- Добавлен отдельный 104-строчный black-box сценарий и обновлена матрица
  `docs/protocol-coverage.md`. Windows Rust 1.97.1 fmt, strict Clippy и тесты
  прошли: 807 library + 1 CLI, 3 ignored. Python `py_compile` и независимый
  wire acceptance прошли. Linux Rust layers в Docker были cache hits, поэтому
  новый Linux Rust test run не заявляется; Linux runtime peer использовал
  release binary из неизменённого Rust source.
- Fixture `./data/acceptance-pop3-sasl-prior-user-20260927` сохранена.
  Созданные runtime container и test image удалены, отсутствие проверено.
  Это независимый POP3 peer, не Litterkeep app и не закрытие других SASL
  механизмов; полный G18 остаётся ACTIVE.
### 2026-09-28: G18 RFC 2183 unknown disposition parameter handling, scoped PASS T330

- `Content-Disposition` разбирает параметры независимо: неизвестный charset у
  RFC 2231 `filename*` не теряет известные disposition и `size=12`; дубликаты
  одного имени не разрешаются выбором первого или последнего значения.
- Добавлены unit-регрессии с quoted semicolon, неизвестным charset и повтором
  параметра. Независимый Python IMAP STARTTLS acceptance подтвердил BODYSTRUCTURE
  с disposition/SIZE и точное сохранение исходного RFC822 literal.
- Rust 1.97.1: fmt, strict all-target Clippy, `cargo test --locked --all-targets`
  прошли (915 library + 1 CLI, 4 ignored); py_compile и diff check прошли.
  Использованы уже имеющиеся Docker images, новый тестовый container удалён.
- Ограничение: проверен независимый клиент, не Litterkeep app. Это scoped
  acceptance RFC 2183 §2.8, не завершение registry coverage или G18; цель остаётся
  ACTIVE.
### 2026-09-28: G18 RFC 2183 unknown disposition fallback, scoped PASS T331

- Неизвестный `Content-Disposition` type теперь проецируется как `attachment`,
  как требует RFC 2183 §2.8; параметры, включая `filename`, сохраняются.
- Unit-тесты проверяют fallback и параметр. Независимый IMAP STARTTLS acceptance
  проверил BODYSTRUCTURE для неизвестного типа, сохранение filename и неизменность
  исходного RFC822 literal.
- Rust 1.97.1: fmt, strict all-target Clippy, all-target tests прошли
  (916 library + 1 CLI, 4 ignored); Python compile и diff check прошли.
  Новый тестовый контейнер удаляется после проверки, образы переиспользованы.
- Это scoped MIME/IMAP acceptance независимым клиентом, не Litterkeep app
  interoperability и не закрытие полной MIME registry coverage; G18 остаётся
  ACTIVE.
### 2026-09-28: G18 RFC 2183 size parameter syntax, scoped PASS T332

- Проверка Content-Disposition типизирует `size` и принимает только unquoted
  последовательность одной или более ASCII-цифр, как задано RFC 2183 §2.7.
  Невалидный параметр исключается независимо от соседнего корректного `filename`.
- Unit-регрессии проверили нецифровые, quoted и RFC 2231 формы и сохранение
  ведущих нулей;
  независимый IMAP STARTTLS acceptance подтвердил, что BODYSTRUCTURE не содержит
  `SIZE=12x`, но содержит filename, а RFC822 literal остаётся точным.
- Rust 1.97.1 fmt, strict all-target Clippy, all-target tests прошли
  (919 library + 1 CLI, 4 ignored); Python compile и diff check прошли. Docker
  Linux wire acceptance прошёл при Cargo target внутри контейнера, созданный
  контейнер затем удалён, образы переиспользованы.
- Ограничение: RFC date-time синтаксис для creation-date/modification-date/
  read-date остаётся непроверенным; независимый peer, не Litterkeep app.
  G18 остаётся ACTIVE.
### 2026-09-28: G18 RFC 2183 date disposition parameters, scoped PASS T333

- `creation-date`, `modification-date`, `read-date` публикуются только из
  обязательной quoted-формы с допустимой Gregorian датой/временем и numeric
  timezone. Weekday, если передан, проверяется относительно локальной даты.
- Добавлены unit-регрессии для всех трёх имён, malformed date, named zone,
  unquoted значения и неправильного weekday. Независимый IMAP STARTTLS
  acceptance подтвердил проекцию трёх валидных значений и отбрасывание
  невозможной даты, сохранив filename и исходный RFC822 literal.
- Общий RFC 5322 baseline parser теперь отвергает weekday, расходящийся с датой;
  Rust 1.97.1 fmt, strict Clippy и all-target tests прошли (923 library +
  1 CLI, 4 ignored); Python compile, diff check и Linux wire acceptance прошли.
  Docker использовал существующие образы и одноразовый Cargo target в контейнере;
  созданный контейнер удалён.
- Leap-second `:60` сохраняется и подтверждён в date projection; календарный
  SEARCH использует представимую соседнюю секунду без смены дня. Некоторые
  obsolete RFC 822 формы и Litterkeep app interop не проверены. G18 ACTIVE.
### 2026-09-28: G18 RFC 822 numeric-zone obsolete years, scoped PASS T334

- Обработчик date-time принимает obsolete two-digit years с RFC 5322 §4.3 pivot
  (00-49 -> 2000-2049, 50-99 -> 1950-1999) и three-digit years как 1900+year.
  Weekday сверяется после нормализации года; RFC 2183 numeric timezone остаётся
  обязательной, поэтому named obsolete zones не разрешены.
- Unit-регрессии проверяют обе границы pivot, year `123` и отклонение короткого
  года; независимый IMAP STARTTLS acceptance возвращает BODYSTRUCTURE с датой
  `97` и проверяет byte-exact RFC822 FETCH.
- Rust 1.97.1 fmt, strict Clippy и all-target tests прошли (924 library +
  1 CLI, 4 ignored); Python compile/diff check и Linux wire acceptance прошли.
  Использованы существующие Docker images и ephemeral Cargo target; контейнер
  удалён.
- Это не полная obsolete RFC 822 date-time грамматика, Litterkeep app interop не
  проверен; G18 остаётся ACTIVE.
### 2026-09-28: G18 RFC 5322 date-time lexical widths, scoped PASS T335

- `Date` parser ограничивает day одной/двумя ASCII-цифрами и hour/minute/second
  ровно двумя цифрами; seconds остаются optional, `60` поддержан по RFC.
  Обновлена общая граница IMAP SEARCH и RFC 2183 disposition date validation.
- Unit и независимый IMAP STARTTLS BODYSTRUCTURE acceptance отвергают
  трёхзначный day, однозначные компоненты времени и трёхзначный hour; валидный
  adjacent filename сохраняется, RFC822 literal совпадает побайтно.
- Rust 1.97.1 fmt, strict Clippy, all-target tests прошли (925 library +
  1 CLI, 4 ignored); Python compile и diff check прошли. Linux wire acceptance
  прошёл на существующих Docker images, созданный контейнер удалён.
- Это ограниченный RFC 5322 numeric-timezone профиль; obsolete варианты кроме
  numeric-year и прочие RFC 2183 gaps остаются. G18 ACTIVE.
### 2026-09-28: G18 RFC 822 CFWS comments in MIME dates, scoped PASS T336

- Общий date-time parser удаляет корректные RFC 822 comments до проверки
  календарных компонентов; nested comments между компонентами сохраняются как
  допустимый CFWS. Незакрытый comment приводит к отказу. Требование RFC 2183 к
  числовой timezone не менялось.
- Unit-регрессии проверяют допустимый вложенный CFWS и отказ от незакрытого
  comment. Независимый Python 3.14 IMAP STARTTLS peer проверил BODYSTRUCTURE и
  побайтный RFC822 FETCH у Linux Docker сервера. Это не проверка приложения
  Litterkeep и не полная проверка obsolete date-time grammar.
- Windows Rust 1.97.1: fmt, Python compile, all-target tests прошли
  (927 library + 1 CLI, 4 ignored). Linux Rust 1.97.1: fmt, strict Clippy и
  all-target tests прошли (1036 library + 1 CLI, 4 ignored). Использованы
  существующие Docker images; созданный сервисный container удалён, новые
  images не создавались. G18 остаётся ACTIVE.
### 2026-09-28: G18 RFC 5322 obsolete timezone profile split, scoped PASS T337

- IMAP `SENT*` использует перечисленные RFC 5322 §4.3 зоны: UT/GMT,
  EST/EDT, CST/CDT, MST/MDT, PST/PDT и military A-I/K-Z. Военные зоны
  и неизвестные 3-5-буквенные alphabetic zone names нормализуются к `-0000`
  согласно RFC-рекомендации без out-of-band данных; J и malformed/overlong
  имена отвергаются.
- RFC 2183 date-параметры используют отдельную typed numeric-only policy и
  продолжают отвергать буквенные зоны. Unit tests и независимый IMAP STARTTLS
  проверили UTC date boundary, точный SENTON набор, BODYSTRUCTURE omission и
  неизменность RFC822 octets.
- Windows Rust 1.97.1: fmt, strict Clippy и all-target tests прошли
  (931 library + 1 CLI, 4 ignored). Независимые Python peers запускались на
  Windows против сервера в Linux Docker; это не проверка Litterkeep app.
- Linux Rust 1.97.1: fmt, strict Clippy и all-target tests прошли
  (1040 library + 1 CLI, 4 ignored). Существующие Docker images переиспользованы,
  созданные containers удалены, images не создавались. G18 остаётся ACTIVE.

### 2026-09-28: G18 RFC 7888 LITERAL+ и доставка BYE, scoped PASS T338

- IMAP теперь рекламирует полный `LITERAL+` вместо `LITERAL-`. Marker остаётся
  стандартным `{n+}`: non-synchronizing LOGIN/SEARCH аргументы bounded 64 KiB,
  APPEND bounded общим APPENDLIMIT 25 MiB. Размер проверяется до выделения
  receiver buffer; oversized non-sync команда получает `BYE [TOOBIG]`.
- Независимый Python STARTTLS peer проверил CAPABILITY до/после TLS, literal
  LOGIN и SEARCH, APPEND payload больше 4096 octets и точные FETCH bytes,
  MULTIAPPEND; oversized APPEND marker без передачи payload получил `BYE
  [TOOBIG]`. Первая wire-проверка нашла, что termination закрывала сессию до
  передачи BYE. Исправлен lifecycle: протокольный отказ сначала flush-ит
  ответ и TLS close-notify, затем освобождает lease.
- Windows Rust 1.97.1: fmt, strict Clippy и all-target tests прошли
  (931 library + 1 CLI, 4 ignored). Linux Rust 1.97.1: fmt, strict Clippy,
  all-target tests прошли (1040 library + 1 CLI, 4 ignored), release binary
  собран. Независимый Python клиент тестировал Linux Docker сервер, это не
  проверка приложения Litterkeep. Fixture `./data/g18-literal-plus-20260928`
  оставлен; одноразовый container удалён, image не создавался.
- G18 остаётся ACTIVE; поддержка не рекламируемых буквальных форм, полный
  IMAP4rev2 и остальные протокольные RFC gaps продолжают проверяться.

### 2026-09-28: G18 SMTP DSN coverage audit, scoped GAP T339

- Повторно проверен путь RFC 3461 от SMTP parser до local delivery: `RCPT TO`
  параметры не сохраняются в transaction/envelope, запрос AppendMany переносит
  только recipient strings, а durable queue принимает recipient без DSN
  attributes. Поэтому RFC 3461 не рекламируется и не реализуется объявлением
  capabilities или частичным приёмом `NOTIFY`/`ORCPT`.
- Независимая parser-проверка уже фиксирует `RCPT TO ... NOTIFY=SUCCESS` как
  `UnsupportedParameter`; wire-клиентская проверка именно SMTP `555` в этом
  этапе не запускалась. Существующий внутренний outbound RFC 3464 DSN не
  доказывает входящую ESMTP DSN поддержку.
- Контейнеров/образов Litterkeep в Docker inventory нет; другие контейнеры и
  образы сохранены. Это аудит пробела, не PASS реализации RFC 3461.
- G18 остаётся ACTIVE. Для полного DSN нужны per-recipient typed attributes,
  долговечный формат с безопасной совместимостью/recovery, разбор NOTIFY и
  ORCPT, MAIL RET/ENVID, связанные правила генерации и wire acceptance.

### 2026-09-28: G18 RFC 1870 SIZE MAIL line allowance, scoped PASS T340

- Исправлен calculation command receiver limit: RFC 1870 добавляет 26 октетов
  для `SIZE=<1..20 digits>`, и этот allowance суммируется с 10 октетами
  SMTPUTF8 и 16 октетами BINARYMIME. Восемь комбинаций теперь выбираются через
  typed enum и exhaustive `match`; регистр параметров сохраняет RFC
  case-insensitivity.
- Тесты проверяют границы SIZE=538, SIZE+SMTPUTF8=548,
  SIZE+BINARYMIME=554, combined=564, а также ровно 538-октетную командную
  строку. Windows Rust 1.97.1 all-target: 933 library + 1 CLI, 4 ignored;
  fmt и strict Clippy прошли. Linux Rust 1.97.1 Docker all-target:
  1042 library + 1 CLI, 4 ignored; fmt и strict Clippy прошли.
- Независимый Python STARTTLS peer проверил на реальном Linux runtime строки
  размером ровно 538/564 октета (receiver пропускает к parser-у, который
  отвечает 555 на дополнительный неизвестный параметр) и 539/565 октетов
  (receiver возвращает 500); NOOP после каждого ответа получил 250. Также
  повторно проверены базовые 512/513. Это не interop с приложением Litterkeep.
  Docker использовал существующий `rust:1.97-bookworm`; тестовый runtime
  container остановлен и отсутствует, новый image не создавался. Linux build
  artifacts оставлены в `./data/g18-smtp-size-line-linux/target`, отдельные
  runtime/bootstrap fixtures в `./data/g18-smtp-size-line-wire`.
- G18 остаётся ACTIVE; это не закрывает прочие SMTP, IMAP, POP3, MIME, Sieve и
  ManageSieve пробелы.

### 2026-09-28: G18 RFC 6856 UTF8 USER capability, scoped PARTIAL T341

- POP3 CAPA теперь заявляет `UTF8 USER`, а grammar USER/PASS разрешает корректный
  UTF-8 в аргументе при сохранении общей границы 255 октетов для строки и
  40 символов для credential. Остальные команды остаются ASCII-профиля.
- Добавлены Rust regressions для multibyte аргументов, предела символов и
  запрета high-bit текста в остальных командах. Независимый Python STARTTLS
  peer подтвердил capability, UTF-8 USER/PASS до UTF8, USER после UTF8,
  восстановление после неудачной проверки через AUTH PLAIN, STAT и QUIT.
- Проверка независимым POP3 клиентом не означает интероперабельность Litterkeep.
  RFC 6856 §2.2 SASLprep canonical mapping для USER пока отсутствует; учётные
  имена сохраняют текущую точную идентичность, поэтому этап отмечен PARTIAL.
  Legacy charset conversion и другие пробелы RFC 6856 остаются открытыми.
- Docker использовал существующий `rust:1.97-bookworm`, новый образ не создавался.
  Временный runtime container остановлен, удалён через `--rm`, отсутствие и
  закрытие порта проверяются. Тестовые данные этапа находятся в `./data`.
- G18 остаётся ACTIVE.

### 2026-09-28: G18 RFC 4013 USER identity mapping, scoped PARTIAL T342

- POP3 USER/PASS use SASLprep RFC 4013 query profile. New account identities are
  stored with the stored profile, and common local login / SMTP mailbox routing
  uses the same prepared key. Unit checks cover compatibility mapping, duplicate
  canonical keys, prohibited input, delivery route equality, and Postmaster.
- Acceptance bootstrap now generates 20 random bytes as 40 hex characters, so
  fresh credentials fit POP3 PASS's RFC 1939 maximum. Existing persisted secrets
  remain unchanged on repeated initialization: a pre-change fixture with 48-char
  secrets was re-initialized by the Linux binary with created=0/existing=4 and
  its credential-file SHA-256 stayed identical. These old passwords remain too
  long for USER/PASS until separately changed; AUTH PLAIN remains available.
- Independent Linux STARTTLS peer logged into canonical account `pop` by USER
  `ｐｏｐ` plus the valid 40-character PASS, then read STAT and QUIT. The same
  peer checked invalid UTF-8 credentials, UTF8 mode after USER, AUTH PLAIN retry,
  and the advertised `UTF8 USER` capability. This is not Litterkeep app interop.
- No legacy account/mailbox-key migration is provided when SASLprep changes the
  old stored identity. New account creation scans existing PHC/SCRAM keys and
  refuses a canonical collision without rewriting a legacy record. Such old
  identities need a separate safe migration procedure; therefore this
  checkpoint remains PARTIAL. Legacy charset conversion also remains open.
- Windows Rust 1.97.1 strict fmt/Clippy/all-target tests pass (940 library,
  1 CLI, 4 ignored). Linux Docker Rust 1.97.1 strict fmt/Clippy/all-target tests
  pass (1049 library, 1 CLI, 4 ignored) and release build passes. Runtime
  container used `--rm` and was verified absent; no image created. Artifacts are
  isolated under `./data/g18-pop3-saslprep-identity-linux` and
  `./data/g18-pop3-saslprep-identity-wire`.
- G18 остаётся ACTIVE.

### 2026-09-28: IMAP coverage documentation consistency checkpoint

- Сверена матрица IMAP с unit coverage `parses_every_base_rfc_9051_search_key`,
  SEARCH evaluator and UTF8 tokenizer tests. Исправлены устаревшие заявления,
  будто базовые SEARCH keys отсутствуют и UTF8=ACCEPT не реализован.
- Уточнено, что покрытие базовых ключей не означает полного SEARCH/ESEARCH
  профиля, а IMAP4rev2 и дополнительные extensions по-прежнему не заявлены.
- Счётчик charset labels синхронизирован с независимой SMTP -> IMAP wire
  проверкой: 42 labels. Windows Rust 1.97.1 `cargo test --lib imap::` прошёл:
  137 passed. Reactor session tests на Windows не запускались, потому что этот
  reactor-набор не входит в Windows lib-test target. Код протокола не менялся;
  этот checkpoint не является реализационным этапом и не закрывает IMAP4rev2.

### 2026-09-28: G18 RFC 5804 script-name validation and LISTSCRIPTS framing, T353 PARTIAL

- Store теперь проверяет script names по RFC 5804/Net-Unicode: управляющие C0/C1,
  U+2028/U+2029, начальный BOM и неназначенные Unicode 17.0.0 scalars запрещены;
  128 Unicode characters принимаются. Slash больше не запрещён произвольным
  filesystem-style правилом.
- `LISTSCRIPTS` сериализует имена как quoted strings с экранированием quote и
  backslash. Независимый TLS peer проверил Unicode/special-name round-trip,
  CRLF response injection rejection и границу 128/129 символов.
- Windows Rust 1.97.1 all-target tests: 968 library + 1 CLI, 4 ignored; fmt и
  strict Clippy прошли. Linux Rust 1.97.1 all-target tests: 1081 library + 1 CLI,
  4 ignored; strict Clippy и release build прошли. Litterkeep app не проверялось.
- Docker использовал существующий `rust:1.97-bookworm`; временные контейнеры
  удалены и отсутствие проверено, образ не создавался. Артефакты и bootstrap
  аккаунты этапа изолированы в `./data/g18-managesieve-script-names-linux`.
- NFC output recommendation и private-use guidance RFC 5198 остаются открыты;
  этап ограничен указанным именовым профилем, полный ManageSieve не объявляется.

### 2026-09-28: G18 RFC 5198 NFC ManageSieve script names, T354 PARTIAL

- Проверка имён и NFC-канонизация выделены в `src/store/sieve_script_name.rs`.
  Все операции Store разрешают входные варианты через Unicode 17 NFC; выдача
  `LISTSCRIPTS` и новые snapshot-файлы содержат канонические имена.
- Старые decomposed snapshot-файлы открываются с NFC-ключами. Если прежние
  имена схлопываются в один ключ, загрузка явно завершается ошибкой, а не
  перезаписывает скрипт. Unit-тесты проверяют aliases, migration, collision и
  повторное открытие.
- Независимый TLS-клиент загружает decomposed имя, получает скрипт через тот
  же decomposed alias и проверяет NFC-форму в `LISTSCRIPTS`.
- Windows Rust 1.97.1 `cargo test --locked --all-targets`: 971 library + 1 CLI
  прошли, 4 ignored; fmt и strict Clippy прошли. В Linux Rust 1.97.1 all-target
  тестах прошли 1084 library + 1 CLI, 4 ignored; strict Clippy и release build
  также прошли.
- Независимый STARTTLS клиент проверил decomposed upload, GETSCRIPT по alias,
  NFC listing и повторное чтение после перезапуска сервера. Runtime container
  остановлен и проверен отсутствующим; Rust image с исходным ID переиспользован,
  новые образы не создавались. Артефакты изолированы в
  `./data/g18-managesieve-script-names-linux`.
- Не закрыты private-use guidance RFC 5198, interoperability с приложением и
  полный профиль ManageSieve.

### 2026-09-28: G18 SCRAM-SHA-256 cryptographic foundations, T355 PARTIAL

- Без новых production dependencies добавлены собственные safe-Rust
  SHA-256, HMAC-SHA-256 и одноблочный PBKDF2-HMAC-SHA-256. Вычислительная
  стоимость KDF ограничена 200000 итерациями.
- Проверки используют SHA-256 vectors RFC 6234, HMAC vectors RFC 4231 и
  PBKDF2 vector RFC 7914 §11. Сверены пустое/короткое/многоблочное сообщение,
  границы padding, миллион октетов и HMAC ключ длиннее hash block.
- Windows Rust 1.97.1 all-target: 977 library + 1 CLI passed, 4 ignored;
  fmt и strict Clippy прошли. Linux Rust 1.97.1 all-target: 1090 library +
  1 CLI passed, 4 ignored; strict Clippy и release build также прошли.
- Использован существующий image `rust:1.97-bookworm` с ID
  `sha256:0e2bcaef56d041a486784e54104a81aebe0da44bd03019bd70bc0401e42e4a97`;
  контейнеры тестов одноразовые (`--rm`), новый image не создавался. Артефакты
  этапа оставлены только в `./data/g18-scram-sha256-foundation`.
- Это криптографическая основа, не поддержка SCRAM-SHA-256: verifier storage,
  state machine, протокольный wire acceptance и channel binding пока отсутствуют;
  механизм не рекламируется.

### 2026-09-28: G18 SCRAM-SHA-256 verifier and record, T356 PARTIAL

- Добавлены `ScramSha256Verifier` и независимый versioned
  `litterkeep-scram-sha256-v1` record. Пароль подготавливается SASLprep до KDF;
  SaltedPassword выдаётся bounded PBKDF2-HMAC-SHA-256 с локальным диапазоном
  4096..200000 итераций. Proof проверяется constant-time сравнением StoredKey;
  ServerKey формирует ответную подпись.
- Unit regression воспроизводит опубликованный RFC 7677 §3 SCRAM-SHA-256 пример:
  соль, client proof и server signature; отдельные проверки покрывают неверный
  proof, SASLprep equivalents, пределы password/cost, строгий parse и record
  encode/reopen round trip.
- Windows Rust 1.97.1 all-target: 984 library + 1 CLI passed, 4 ignored;
  fmt и strict Clippy прошли. Linux Rust 1.97.1 all-target: 1097 library +
  1 CLI passed, 4 ignored; strict Clippy и release build прошли. Артефакты
  этапа размещены в `./data/g18-scram-sha256-verifier`; использован прежний
  Docker image, временные контейнеры удалены, новый image не создавался.
- Отдельный record ещё не подключён к Accounts Store, ManageSieve command
  lifecycle или protocol capability. Сквозной wire exchange, TLS resumption
  policy и SCRAM-SHA-256-PLUS/channel binding не подтверждены; механизм не
  рекламируется.

### 2026-09-28: G18 SCRAM-SHA-256 Accounts owner storage, T357 PARTIAL

- Создание аккаунта сохраняет SHA-256 SCRAM verifier в отдельном sidecar рядом
  с действующими password и SCRAM-SHA-1 записями. При миграции старого аккаунта
  sidecar появляется только после успешной проверки пароля; неверный пароль его
  не создаёт. Ошибка создания очищает обе SCRAM-записи.
- Типизированный запрос bounded storage-worker channel загружает verifier через
  Accounts owner; Debug-представление запроса/ответа скрывает credential.
- Windows Rust 1.97.1 `cargo test --locked --all-targets`: 985 library + 1 CLI
  прошли, 4 ignored; fmt и strict Clippy прошли. Linux Rust 1.97.1 all-target:
  1098 library + 1 CLI прошли, 4 ignored; strict Clippy и release build прошли.
- Использован существующий Docker image `rust:1.97-bookworm`, ID
  `sha256:0e2bcaef56d041a486784e54104a81aebe0da44bd03019bd70bc0401e42e4a97`;
  disposable-контейнеры с `--rm` отсутствуют, новые images не создавались.
  Артефакты тестового этапа изолированы в `./data/g18-scram-sha256-accounts`.
- Это интеграция хранения, не поддержка wire-протокола: ManageSieve SASL state
  machine пока не подключена, механизм не рекламируется; PLUS/channel binding и
  независимый SCRAM клиент не проверялись.

### 2026-09-28: G18 ManageSieve SCRAM-SHA-256 TLS wire, T358 PASS

- SCRAM-SHA-256 подключён через typed ManageSieve AUTHENTICATE command, bounded
  initial/continuation framing и отдельные session transitions. Credential lookup
  проходит через typed request ограниченного Accounts owner worker-а.
- SHA-256 обмен формирует server-first nonce challenge, проверяет canonical
  Base64, `n,,` channel-binding input, точный nonce, 32-октетный client proof и
  mutual server signature. Ошибка proof возвращает server-final
  `e=invalid-proof`; отмена и невалидные кадры сбрасывают SASL state.
- TLS-only capability после TLS объявляет `PLAIN SCRAM-SHA-1 SCRAM-SHA-256`;
  до STARTTLS SASL список остаётся пустым. Независимый Linux implicit-TLS peer
  прошёл SHA-256 success, signature, invalid proof, UNAUTHENTICATE и NOOP recovery;
  SHA-1 peer также прошёл. Отдельный STARTTLS peer подтвердил capability
  transitions, но SHA-256 authentication поверх STARTTLS отдельно не запускал.
- Windows Rust 1.97.1 all-target: 988 library + 1 CLI passed, 4 ignored; fmt и
  strict Clippy прошли. Linux Rust 1.97.1 all-target: 1101 library + 1 CLI
  passed, 4 ignored; strict Clippy и release build прошли. Python peers прошли
  `py_compile` и wire acceptance.
- Переиспользован установленный Docker image `rust:1.97-bookworm` с ID
  `sha256:0e2bcaef56d041a486784e54104a81aebe0da44bd03019bd70bc0401e42e4a97`;
  временные контейнеры с `--rm` отсутствуют, новый image не создавался.
  Артефакты и runtime fixture находятся в `./data/g18-scram-sha256-managesieve`.
- Это acceptance независимого Python клиента, а не Litterkeep app. SCRAM-SHA-256
  PLUS/channel binding и фактический SHA-256 exchange через STARTTLS остаются
  открытыми; общий профиль ManageSieve и цель G18 остаются ACTIVE.

### 2026-09-28: G18 ManageSieve SCRAM-SHA-256 через STARTTLS, T359 PASS

- Независимый peer расширен режимом `--starttls`: до upgrade он проверяет
  отсутствие SASL механизмов, посылает STARTTLS и валидирует серверный
  сертификат; после upgrade проверяет secure greeting и полный SCRAM-SHA-256
  обмен. Клиент независимо рассчитывает proof и сверяет server signature.
- Linux runtime acceptance прошёл на существующем release binary Rust 1.97.1
  внутри установленного `rust:1.97-bookworm`; валидный proof, неверный proof,
  UNAUTHENTICATE и NOOP recovery подтверждены. `python -m py_compile` прошёл.
- Использована существующая runtime fixture `./data/g18-scram-sha256-managesieve`.
  Тестовый контейнер `litterkeep-g18-scram-sha256-starttls` остановлен и
  проверен отсутствующим; image не создавался. Другие запущенные контейнеры
  не изменялись.
- Это проверка независимого тестового клиента, а не приложения Litterkeep.
  SCRAM-SHA-256-PLUS/channel binding и application interoperability остаются
  неподтверждёнными; общий профиль ManageSieve и цель G18 остаются ACTIVE.

### 2026-09-28: G18 ManageSieve MAXREDIRECTS capability, T360 PASS

- RFC 5804 defines `MAXREDIRECTS` as the per-evaluation redirect-action limit.
  The evaluator's existing value 16 now has one exported constant shared by
  normal `redirect` and `redirect :copy`, and ManageSieve publishes the exact
  `"MAXREDIRECTS" "16"` capability in every capability block.
- A focused Rust boundary test proves that 16 actions execute and action 17
  fails with typed `RedirectLimit`. An independent Python STARTTLS peer confirms
  capability presence before TLS, after TLS, after PLAIN authentication, after
  UNAUTHENTICATE, and on explicit CAPABILITY; it also passes the existing TLS
  and authentication transitions.
- Windows Rust 1.97.1 all-target: 989 library + 1 CLI passed, 4 ignored; fmt and
  strict Clippy passed. Linux Rust 1.97.1 all-target: 1102 library + 1 CLI
  passed, 4 ignored; strict Clippy and release build passed. Python 3.14.7
  `py_compile` and `git diff --check` passed.
- Reused existing `rust:1.97-bookworm` image ID
  `sha256:0e2bcaef56d041a486784e54104a81aebe0da44bd03019bd70bc0401e42e4a97`
  and `./data/g18-scram-sha256-managesieve` build/runtime fixtures. Disposable
  build/runtime containers are absent; no image was created. Other running
  containers were not modified.
- This closes capability reporting and its boundary evidence only; broader
  Sieve/ManageSieve profiles and Litterkeep app interoperability remain open.
  G18 remains ACTIVE.

### 2026-09-28: G18 ManageSieve SCRAM-SHA-256-PLUS integration, T361 PASS

- `ServerTls` exposes the RFC 9266 `tls-exporter` value only after a completed
  TLS 1.3 handshake, using `EXPORTER-Channel-Binding`, empty context and 32-byte
  output. Other protocol versions and pre-handshake states return unavailable.
- A separate rustls client/server handshake independently derives the same
  exporter bytes; a pending ClientHello test verifies no binding is exposed
  before completion. ManageSieve now has typed SHA-256-PLUS command/FSM paths,
  advertises PLUS only when TLS exporter bytes are available, parses only
  `p=tls-exporter,,`, and validates canonical `c=` against the exact GS2 header
  plus exporter before proof verification. Unit coverage verifies valid proof,
  server signature and rejection with a different exporter.
- Windows Rust 1.97.1 all-target: 992 library + 1 CLI passed, 4 ignored; fmt
  and strict Clippy passed. Linux Rust 1.97.1 all-target: 1105 library + 1 CLI
  passed, 4 ignored; fmt, strict Clippy and release build passed. The existing
  `rust:1.97-bookworm` image ID is unchanged; the disposable container was
  removed with `--rm`, no image was created. Cargo target/cache live under
  `./data/g18-scram-sha256-managesieve/plus-target` and `cargo-home`.
- A separate Rustls client completed implicit-TLS ManageSieve against the Linux
  release server, verified the CA chain and TLS 1.3, checked PLUS advertisement,
  bound canonical `c=` to its RFC 9266 exporter, sent a valid proof, and verified
  the server signature. The peer reuses this crate's tested SHA/HMAC/PBKDF2
  helpers, so it is an independent transport peer, not an independent crypto
  oracle. The disposable runtime container is absent; no image was created.
- T361 is PASS for server-side PLUS and the independent live wire exchange.
  Litterkeep app interoperability remains unverified; G18 stays ACTIVE.

### 2026-09-28: G19 POP3 SCRAM-SHA-256, T362 PASS

- POP3 parses and advertises `SCRAM-SHA-256` only after TLS. Its typed AUTH
  flow reads the dedicated verifier through the bounded Accounts owner, checks
  the RFC 7677 proof, sends server-final, waits for acknowledgement, and only
  then requests the maildrop snapshot. Failed proof never authenticates.
- Windows Rust 1.97.1 all-target: 994 library + 1 CLI passed, 4 ignored; fmt,
  strict Clippy and `git diff --check` passed. Linux Rust 1.97.1 all-target:
  1107 library + 1 CLI passed, 4 ignored; fmt, strict Clippy and release build
  passed. Python 3.14 syntax check passed.
- Independent Python STARTTLS acceptance verifies CAPA before/after TLS and
  USER/transaction states, preserves SCRAM-SHA-1 invalid-proof/retry/lease
  scenarios, and completes SCRAM-SHA-256 with mutual proof, server signature,
  cancellation recovery, maildrop access and lease reuse. Legacy fixture
  verifier is provisioned by a successful USER/PASS login before testing. Existing `rust:1.97-bookworm`
  image reused; disposable runtime container removed and verified absent; no
  image created. Artifacts remain under `./data/g19-pop3-scram-linux`.
- T362 is PASS for POP3 SCRAM-SHA-256 and the tested STARTTLS exchange.
  POP3 PLUS, other mechanisms and Litterkeep app interoperability remain
  unverified; G19 stays ACTIVE.

### 2026-09-28: G19 POP3 SCRAM-SHA-256 implicit-TLS wire, T363 PASS

- A separate Python peer validates the server certificate against the local CA,
  then receives greeting only after implicit TLS, checks the TLS CAPA mechanism
  list, independently calculates SCRAM-SHA-256 client proof and server
  signature, accesses the maildrop with STAT and closes via QUIT. An old
  acceptance account's verifier is first created by normal USER/PASS over the
  same verified implicit-TLS listener.
- `python -m py_compile` passes. Live acceptance passes against the Linux Rust
  1.97.1 release server in the existing `rust:1.97-bookworm` image. The
  disposable listener container is absent, no image was created, and existing
  fixtures/build cache remain under `./data/g19-pop3-scram-linux`.
- T363 is PASS for this implicit-TLS exchange only. It does not establish
  Litterkeep app interoperability or POP3 SCRAM-SHA-256-PLUS support; G19 stays
  ACTIVE.

### 2026-09-28: G14 RFC 9394 PARTIAL SEARCH/FETCH checkpoint, T383 PASS

- IMAP now parses RFC 9394 oldest/newest rank ranges, returns ESEARCH PARTIAL
  with compact UID/sequence sets or NIL, applies SAVE to only returned matches
  (except the COUNT interaction), and applies UID FETCH PARTIAL after filtering
  the requested UID set. FETCH \Seen mutation is limited to returned messages.
  PARTIAL is advertised only after both SEARCH and UID FETCH wire paths exist.
- Windows Rust 1.97.1 full suite: 1004 library + 1 CLI passed, 4 ignored;
  strict all-target Clippy, formatting and Python 3.14 syntax checks pass.
  Linux Rust 1.97.1 Docker suite: 1124 library + 1 CLI passed, 4 ignored;
  strict Clippy and release build pass.
- Independent `tests/blackbox/imap_partial.py` STARTTLS acceptance verified
  capability, positive/reverse/negative ranges, NIL, partial SAVE and UID FETCH
  Seen behavior against the Linux server. Existing Rust image was reused; both
  temporary containers were removed and absence verified; no image was created.
  Runtime and build artifacts are under `./data/g22-imap-partial-live` and
  `./data/g22-imap-partial-linux`.
- T383 covers the tested RFC 9394 semantics only. Full RFC 9051 compatibility
  and Litterkeep app interoperability remain unverified; G14 stays RUNNING.

### 2026-09-28: G14 RFC 9755 UTF8=ACCEPT capability discovery, T384 PASS

- Advertised `UTF8=ACCEPT` in all IMAP CAPABILITY states after auditing the
  implemented mailbox-name, internationalized APPEND, FETCH ENVELOPE and
  mode-gated UTF-8 response paths. Capability discovery does not imply
  `UTF8=ONLY`, IMAP4rev2, or full RFC 9755 conformance.
- Independent STARTTLS acceptance checks capability before STARTTLS, after TLS
  and LOGIN, and after ENABLE; APPEND with an internationalized header is
  refused before ENABLE and accepted after it. The same test checks UTF-8
  mailbox CREATE/LIST and FETCH ENVELOPE projection. Existing APPEND acceptance
  was updated to require the capability and retain its legacy/enabled checks.
- Windows Rust 1.97.1 full suite: 1004 library + 1 CLI passed, 4 ignored;
  strict Clippy/fmt and Python syntax pass. Linux Rust 1.97.1 Docker suite:
  1124 library + 1 CLI passed, 4 ignored; strict Clippy and release build pass.
- Linux runtime uses `./data/g22-imap-utf8-capability`; test/release artifacts
  reuse `./data/g22-imap-partial-linux`. Existing Rust image reused; temporary
  containers removed and verified absent; no image created.
- T384 proves capability discovery and the listed extension paths only.
  Remaining RFC 9755 requirements, IMAP4rev2 and Litterkeep app interoperability
  remain unverified; G14 stays RUNNING.

### 2026-09-29: G18 Sieve syntax case-insensitivity, T403 PASS

- RFC 5228 §2.4.1 requires non-string tokens to ignore ASCII case and quoted or
  multiline strings to remain case-sensitive. Sieve commands, tests,
  capabilities, match/address/size tags and comparator identifiers now pass
  through typed enums before AST construction. User string values are retained
  unchanged.
- Regression tests parse and execute mixed/upper-case HEADER, FILEINTO and
  ADDRESS paths and preserve exact `Invoices/2026` folder spelling.
- Windows Rust 1.97.1 full suite: 1045 library + 1 CLI passed, 4 ignored;
  focused parser tests, strict all-target Clippy, rustfmt and diff check pass.
- Follow-up Linux acceptance: an independent Python ManageSieve peer verifies
  STARTTLS, PLAIN auth, CHECKSCRIPT acceptance for mixed-case syntax and
  rejection of an unknown required extension against the Linux release server.
  Windows Rust 1.97.1 all-target suite passes with 1045 library + 1 CLI tests
  (4 ignored); Linux Rust 1.97.1 all-target suite passes with 1166 library + 1
  CLI tests (4 ignored), strict Clippy and rustfmt.
- Linux acceptance artifacts remain under
  `./data/g27-sieve-case-insensitive`; temporary build/init/listener/test
  containers were removed, the existing Rust image was reused, and no image was
  created. Litterkeep app interoperability and full RFC 5228 coverage remain
  unverified; G18 stays ACTIVE.

### 2026-09-29: G18 Sieve runtime match capture state, T422 PARTIAL

- The bounded condition evaluator now carries per-script byte captures from a
  successful `:matches` through nested command branches. Capture zero stores the
  matched input; later entries preserve wildcard order. Failed comparisons do
  not replace prior state, and `allof`/`anyof` retain left-to-right
  short-circuit behavior so skipped tests have no capture side effects.
- Regression tests cover a decoded Subject wildcard capture, preservation on a
  failed comparison, and the absence of captures from an `anyof` test skipped
  after an earlier success.
- Windows Rust 1.97.1 all-target: 1088 library tests + 1 CLI passed, 4 ignored;
  strict Clippy, rustfmt and `git diff --check` pass. Linux Rust 1.97.1
  all-target: 1214 library + 1 CLI passed, 4 ignored; optimized release build
  passes. Existing `rust:1.97-bookworm`, `./data/g23-cargo-home`, and
  `./data/g18-relayed-linux-target` reused. Disposable containers used `--rm`
  and were removed; no image was created. Linux Clippy and rustfmt are not
  installed in this image's toolchain and were not run.
- This is runtime capture state only. Sieve `set`, interpolation, modifiers,
  string test, advertised Variables capability, and full RFC 5229 conformance
  remain unimplemented; G18 stays ACTIVE.

### 2026-09-29: G18 RFC 5229 variable store and expansion core, T425 PARTIAL

- Добавлен отдельный библиотечный `VariableStore` с ASCII-case-insensitive
  именами, поддержкой минимум 128 переменных, 32 символов имени и 4000 Unicode
  символов значения. Значения сверх лимита обрезаются по символам. One-pass
  expansion обрабатывает `${name}` и numeric captures, неизвестные корректные
  имена дают пустую строку, некорректные ссылки остаются буквальными.
- Для защиты от раздувания результата установлен явный локальный предел 65 536
  символов с typed `ExpansionLimit`; в RFC capability это не заявляется.
  Capability `variables` по-прежнему не рекламируется: новый engine ещё не
  подключён к AST, Sieve strings, `set`, `string` и modifiers; validation
  `${10}` и namespaces также не реализована.
- Windows Rust 1.97.1 all-target: 1095 library + 1 CLI passed, 4 ignored;
  strict Clippy, rustfmt и diff check pass. Linux Docker Rust 1.97.1 all-target:
  1221 library + 1 CLI passed, 4 ignored; release build passes. Повторное
  использование общей старой target-папки дало incremental-cache copy error;
  успешный прогон выполнен в `./data/g18-variables-linux-target`. Существующий
  image и Cargo cache переиспользованы, контейнер с `--rm` удалён, image не
  создавался. Linux Clippy/rustfmt в образе отсутствуют.
- Это проверенное reusable ядро, но не RFC 5229 conformance; G18 остаётся ACTIVE.

### 2026-09-29: G18 RFC 5229 expansion reference errors, T426 PARTIAL

- Уточнено поведение expansion engine: syntactically valid numeric ссылки выше
  поддерживаемого диапазона `${0}`..`${9}` возвращают отдельную typed ошибку,
  а syntactically valid namespace-ссылка завершается `UnsupportedNamespace`,
  пока соответствующее extension не подключено. Невалидные ссылки остаются
  буквальными, включая dotted text с незаконным identifier.
- Все 6 unit tests Variables engine проходят; полный Windows Rust 1.97.1
  all-target: 1095 library + 1 CLI passed, 4 ignored. Strict Clippy, rustfmt и
  diff check pass.
- Это hardening отдельно тестируемого ядра. Parser-time diagnostics, expansion
  из Sieve AST и Variables capability по-прежнему отсутствуют; RFC 5229
  conformance не заявляется, G18 остаётся ACTIVE.

### 2026-09-29: G18 RFC 5229 implementation-boundary audit, T423 PARTIAL

- Rechecked the current Variables design against the RFC Editor's RFC 5229 text.
  The extension is still not advertised. The audit now records single-pass
  reference expansion, case-insensitive names, quoting/encoded-character order,
  runtime-at-command expansion, unknown-reference behavior, `set` restrictions,
  `string` semantics, modifier precedence, and minimum implementation limits.
- The audit explicitly requires `${0}` and `${1}`..`${9}`, rejects unsupported
  higher capture indices, and calls out Unicode character-length behavior and
  bounded expansion as acceptance criteria before capability advertisement.
- Documentation-only checkpoint. No protocol conformance or runtime support is
  claimed; code tests were not rerun because no code remains changed in this
  checkpoint. G18 stays ACTIVE.

### 2026-09-29: G18 Sieve address-list evaluation beyond 64 entries, T424 PARTIAL

- Removed an undocumented `take(64)` from Sieve address projection. RFC 5228
  requires the address test to match any address in supported structured
  headers; the former cap silently produced false negatives for later entries.
  Parsing remains bounded by the server's 25 MiB message limit.
- A regression builds a 65-mailbox address-list and verifies the final mailbox
  is retained. Windows Rust 1.97.1 all-target: 1089 library + 1 CLI passed, 4
  ignored; strict Clippy, rustfmt and diff check pass. Linux Docker Rust 1.97.1
  all-target: 1215 library + 1 CLI passed, 4 ignored; release build passes.
  Existing image and `./data` Cargo/target paths reused; disposable container
  used `--rm`; no image created. Linux Clippy/rustfmt unavailable in image.
- This proves removal of one Sieve address projection cap, not all RFC 5228
  semantics or Litterkeep application interoperability; G18 remains ACTIVE.
