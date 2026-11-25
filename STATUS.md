# Статус проекта

Страница отвечает на три вопроса: насколько сервер готов, что уже работает и
проверено, а что ещё запланировано. Детальные протокольные профили остаются в
[`docs/`](docs/README.md), машинно-читаемая матрица приёмки — в
[`docs/acceptance.tsv`](docs/acceptance.tsv).

> **Коротко:** это самостоятельный почтовый сервер на Rust с собственными
> протокольными парсерами, MIME/Sieve-разбором и файловым хранилищем. Реализованы
> и проверены SMTP submission, IMAP4rev1, POP3 и ManageSieve на уровне
> wire-профиля. Проект не претендует на полноту RFC и не заявляет совместимости
> с реальными почтовыми клиентами и серверами.

A self-hosted Rust mail server with its own protocol parsers, MIME/Sieve
implementation and filesystem store. SMTP submission, IMAP4rev1, POP3 and
ManageSieve are implemented and verified at wire level. The project does not
claim full RFC conformance or interoperability with production mail clients.

## Как читать этот документ

Обозначения взяты из проектной конвенции и используются одинаково во всех
доках:

| Обозначение | Значение |
|---|---|
| **PASS** | Есть типизированный парсер, переход состояния реактора или исполнения и профильный тест. Функция подтверждена независимой wire-проверкой. |
| **PARTIAL** | Намеренно ограниченная семантика. `CAPABILITY` не выдаёт её за более широкое расширение. Граница названа явно. |
| **NOT STARTED** | Не реализовано и не рекламируется. |

Дата сверки: **2026-11-25**. Источники: `README.md`, `docs/imap-profile.md`,
`docs/protocol-coverage.md`, `docs/sieve-extension-audit.md`,
`docs/sieve-coverage.md`, `docs/acceptance.md`.

## Сводная готовность

| Подсистема | Статус | Что подтверждено | Детали |
|---|---|---|---|
| SMTP submission | **PASS** | Implicit TLS и STARTTLS, `AUTH PLAIN`, `DSN` через EHLO, `MAIL RET`/`ENVID`, `RCPT NOTIFY`/`ORCPT`, DATA и BDAT, phase timeouts по RFC 5321. Полная семантика итоговых DSN-уведомлений по каждому маршруту остаётся неподтверждённой | [protocol-coverage](docs/protocol-coverage.md#smtp) |
| IMAP4rev1 | **PARTIAL** | 21 группа команд и свойств; полный профиль ещё не закрыт, точные статусы — в [матрице](docs/imap-profile.md) | [imap-profile](docs/imap-profile.md) |
| IMAP4rev2 (RFC 9051) | **NOT STARTED** | Целевой baseline следующего этапа. Объявлять нельзя до выполнения обязательной семантики и сетевой приёмки | [imap-profile](docs/imap-profile.md) |
| POP3 | **PASS** | STARTTLS/STLS и implicit TLS, `USER`/`PASS`, `RETR`/`DELE`, maildrop leases, UIDL через restart | [protocol-coverage](docs/protocol-coverage.md#pop3) |
| ManageSieve | **PASS** | STARTTLS, SASL PLAIN, SCRAM-SHA-1, SCRAM-SHA-256, SCRAM-SHA-256-PLUS, `CHECKSCRIPT`/`PUTSCRIPT`/`LISTSCRIPTS`/`GETSCRIPT`/`SETACTIVE`/`DELETESCRIPT`/`RENAMESCRIPT`, literals | [sieve-coverage](docs/sieve-coverage.md) |
| Sieve | **PARTIAL** | Базовый RFC 5228 профиль, `fileinto`, `redirect`, `keep`/`discard`, header/address/size tests, diagnostics. Внутренний профиль RFC 5229 и декодирование RFC 5228 `encoded-character` реализованы, но **не рекламируются** | [sieve-coverage](docs/sieve-coverage.md), [audit](docs/sieve-extension-audit.md) |
| Файловое хранилище | **PASS** | Формат `litterkeep-store-v2`, durable journals, atomic blob publish, recovery, `check`, `backup` | [storage-format](docs/storage-format.md) |
| Исходящая доставка | **PASS** | Durable очередь, single-flight scheduler на Linux, retry policy, локальный DSN после permanent failure, TLS route с закреплённым SAN и CA. Проверен явный relay peer; публичные DNS/MX и доставка в интернет не проверялись | [protocol-coverage](docs/protocol-coverage.md#outbound-smtp) |
| Эксплуатационный контур | **PASS** | `init`, `seed`, `runtime stats/tasks/resources/inspect/cancel`, `check`, `backup`, `sieve-errors` | [architecture](docs/architecture.md) |

## Приёмка в цифрах

Матрица [`docs/acceptance.tsv`](docs/acceptance.tsv) содержит 439 проверок:

| Статус | Сколько | Значение |
|---|---:|---|
| `PASS` | 349 | Проверка выполнена и подтверждена независимым наблюдением |
| `PARTIAL` | 79 | Подтверждена только часть границы, остальное названо явно |
| `SUPERSEDED` | 7 | Ранняя контрольная точка, заменена более поздней проверкой |
| `BLOCKED` | 1 | Проверка не могла быть выполнена по внешней причине |
| `FAIL` | 1 | Проверка выполнена и не прошла |
| `GAP` | 1 | Зафиксированный пробел без попытки проверки |
| `NOT RUN` | 1 | Подготовлено, но не запускалось |

Формат колонок и смысл статусов разобраны в
[`docs/acceptance.md`](docs/acceptance.md#формат-матрицы).

## Что сделано и проверено

### SMTP submission

Один процесс принимает submission на 1465 (implicit TLS) и 1587 (STARTTLS).
TLS handshake завершается до выдачи любого greeting-а, поэтому учётные данные
не уходят открытым текстом. `AUTH PLAIN` работает только после TLS.

Принят typed envelope и проходит через DATA и BDAT к единственному владельцу
Store, сохраняется в commit-linked sidecar и восстанавливается после reopen.
RFC 3461 `DSN` рекламируется через EHLO; независимые raw-socket peers
подтвердили capability, приём `MAIL RET=HDRS ENVID=...`, `RCPT NOTIFY=FAILURE
ORCPT=...`, завершение DATA и отказ тех же параметров после HELO.

Фаза доставки ограничена RFC 5321 timeouts, настраиваемыми при старте
процесса. Ошибка ввода-вывода уничтожает грязное соединение, durable job
переводится в retry policy, а канал не возвращается в pool.

### IMAP4rev1

Работают `CAPABILITY`, `ID`, `NOOP`, `LOGOUT`, `STARTTLS`, `LOGIN`,
`AUTHENTICATE PLAIN` с SASL-IR, `NAMESPACE`, `ENABLE`, `CREATE`/`DELETE`/
`RENAME`, `SUBSCRIBE`/`UNSUBSCRIBE`, `SELECT`/`EXAMINE`, `APPEND`, `FETCH`,
`UID FETCH`, `SEARCH`, `UID SEARCH`, `STORE`, `UID STORE`, `COPY`/`MOVE`,
`EXPUNGE`/`UID EXPUNGE`, `CLOSE`, `UNSELECT`, `CHECK`, `IDLE`/`DONE`,
`LIST SPECIAL-USE`, `RETURN`, `CREATE USE`.

Ключевые подтверждённые свойства: `APPEND` атомарно сохраняет начальные флаги
и `INTERNALDATE` вместе с UID; событийный IDLE будит только выбранные сессии и
после durable APPEND возвращается к обычным командам через `DONE`; durable
каталог папок отдаёт `SPECIAL-USE` по умолчанию; `RENAME` сохраняет
стабильные внутренние ID всему поддереву; `DELETE` окончательно выводит ID из
оборота и отказывает для INBOX или папки с детьми; `LSUB` показывает
отсутствующие подписки как `\Noselect`; `STATUS` читает счётчики без выбора
папки; `EXPUNGE` перестраивает sequence numbers, не переиспользуя UID, и
запрещён после `EXAMINE`.

Точная построчная матрица со статусом и проверяемой границей — в
[`docs/imap-profile.md`](docs/imap-profile.md).

### POP3

`serve-pop3-starttls` на 1110 и `serve-pop3-tls` на 1995. STLS обязателен
до `PASS`, либо implicit TLS. Проверен порядок команд, отказ `NOOP` вне
TRANSACTION по RFC 1939 §5, лимит 40 символов на аргумент по RFC 1939 §3,
`UIDL` переживает restart, удаление через maildrop lease.

### ManageSieve

Все четыре listener-а в одном процессе, поэтому каталог скриптов, SMTP
доставка и IMAP-видимость разделяют ровно одного владельца Store. Реализованы
STARTTLS, SASL PLAIN с initial-response и challenge/response flow, SCRAM-SHA-1,
SCRAM-SHA-256 и SCRAM-SHA-256-PLUS при наличии TLS 1.3 exporter, синхронные и
фрагментированные literals, полный набор команд каталога скриптов.

### Sieve

Рекламируемая capability строго ограничена:
`fileinto subaddress envelope copy vacation vacation-seconds`.

Внутренне реализован, но **не рекламируется** профиль RFC 5229: `set`, тест
`string`, modifier-ы, раскрытие переменных в condition/action strings, цели
`fileinto`/`redirect`, строки `vacation` и селекторы envelope/comparator.
Также реализовано декодирование RFC 5228 `encoded-character` (`${hex:...}`,
`${unicode:...}`), и строковые октеты сохраняются через лексер, сопоставление и
actions. Реклама этих расширений требует отдельной приёмки.

Диагностика ошибок доступна владельцу через `sieve-errors`: каждая строка
содержит delivery transaction, account, фактически исполненную версию скрипта
и типизированный код ошибки. Журнал ограничен 1 MiB, и исчерпание квоты
отклоняет transaction до SMTP success, а не принимает письмо без обещанной
диагностики.

### Файловое хранилище

Blob публикуется через temporary file, `fsync`, atomic rename и directory sync.
Mailbox links, флаги, папки, подписки, Recent, Sieve, diagnostics, очередь и
global transaction commits используют framed append-only journals. Полный
неверный frame считается corruption, неполный tail безопасно обрезается.

UID монотонен внутри mailbox и не переиспользуется после EXPUNGE. COPY делит
immutable blob, MOVE публикует destination и source tombstone одной
transaction. Автоматической миграции с ранних dev-форматов нет: неизвестный или
непомеченный каталог с mail data отклоняется.

### Эксплуатационный контур

`init` создаёт локальные acceptance-аккаунты с Argon2id PHC и случайными
паролями в `test-accounts.tsv`. `seed` заполняет тестовые ящики по манифесту
`tests/fixtures/seed-manifest.tsv` и не удваивает fixtures при повторном
запуске. `check` берёт эксклюзивную блокировку Store, выполняет штатное
восстановление и читает каждый blob, на который ссылается mailbox или ожидающая
доставка; orphan blobs только подсчитываются. `backup` требует остановленного
Store owner-а.

Runtime control socket возвращает только метаданные жизненного цикла: состояние
задачи, generation-protected ID и удерживаемые ресурсы. Тела писем, пароли и
TLS-ключи не возвращаются и не логируются.

## Частично реализовано

| Область | Статус | Что ограничено |
|---|---|---|
| IMAP `CAPABILITY`, `ID`, `NOOP`, `LOGOUT` | PARTIAL | `ID` отвечает `* ID NIL`, чтобы не выдавать отпечаток реализации. Неперечисленные расширения не поддерживаются |
| IMAP `LIST`, `LSUB`, `STATUS` | PARTIAL | Multi-namespace и breakout naming не реализованы |
| IMAP `ENABLE` | PARTIAL | Поддержан `UTF8=ACCEPT`; `UTF8=ONLY` не поддерживается и не рекламируется |
| IMAP `APPEND` | PARTIAL | Международные заголовки подтверждены, но полный RFC 9755 profile gate не закрыт |
| IMAP `FETCH`, `UID FETCH` | PARTIAL | Ограниченный набор section/partial reads; полный RFC 3501 набор не закрыт |
| IMAP `SEARCH`, `UID SEARCH` | PARTIAL | Evaluator реализован, но заявка на полный RFC 9051 key group не сделана |
| IMAP address groups | PARTIAL | `ENVELOPE` и SEARCH address keys реализованы частично |
| Sieve `variables` (RFC 5229) | PARTIAL | Профиль работает внутренне, capability не рекламируется |
| Sieve `encoded-character` | PARTIAL | Декодирование работает, capability не рекламируется |
| POP3 аутентификация | PASS с ограничением | Подтверждён успешный пароль; прочие механизмы не подтверждены |
| Linux-only reactor | Ограничение платформы | На Windows собирается и тестируется, но `serve` и иные reactor-команды там не запускаются |

## Запланировано, но не начато

| Расширение / цель | RFC | Что потребуется |
|---|---|---|
| IMAP4rev2 baseline | RFC 9051 | Полный набор key group и обязательная семантика плюс сетевая приёмка |
| `UTF8=ONLY` | RFC 9755 | Отдельный профиль: reject legacy modified UTF-7 и `NO [CANNOT]` для UTF-8-зависимых команд до `ENABLE` |
| Sieve `body` | RFC 5173 | MIME-aware `:raw`, `:content`, `:text` без совпадений через границы частей, с ограничением CPU и памяти |
| Sieve `reject`, `ereject` | RFC 5429 | Оценка Sieve до SMTP-принятия DATA; post-accept DSN не эквивалентен `ereject` |
| Sieve `imap4flags` | RFC 5232 | `setflag`, `addflag`, `removeflag`, `hasflag` и `:flags` с durable мутацией флагов |
| Sieve IMAP events | RFC 6785 | Поддержка IMAP-флагов и определённый event context для исполнения |
| Sieve `mailboxid` | RFC 9042 | Устойчивые mailbox OBJECTID, `:mailboxid` у `fileinto` и `mailboxidexists` |
| Sieve mailbox metadata | RFC 5490 | `mailboxexists`, metadata-тесты и обработка по metadata |
| Sieve special-use delivery | RFC 8579 | `specialuse`, `:specialuse` и согласование с typed special-use каталогом |

Полный инвентарь расширений с указанием следующей необходимой границы — в
[`docs/sieve-extension-audit.md`](docs/sieve-extension-audit.md).

## Осознанные ограничения

Это не дефекты и не задачи, а зафиксированные границы проекта.

- **Не предназначен для публичного интернета.** Считается исследовательской
  реализацией. Перед внешним exposure требуется front-end с настоящим TLS и
  политикой против спама.
- **Один владелец Store.** Параллельный второй writer отклоняется: `check`,
  `backup` и `sieve-errors` открывают Store как единственный writer и потому
  не должны идти параллельно с сервером.
- **Нет автоматической миграции формата.** Переход на `litterkeep-store-v2` из
  ранних dev-форматов не выполняется.
- **Windows не запускает сервер.** Реактор опирается на Linux `poll(2)`;
  Windows-хост используется для сборки, `fmt`, `clippy` и тестов.
- **Ограничения по размеру.** DATA и ответ IMAP ограничены 25 MiB, бюджет
  immutable-blob в каталоге данных — тоже 25 MiB. Превышение отклоняется до
  создания временного blob.
- **Маршрутизация исходящей почты** принимает только явный числовой `IP:port`:
  скрытый blocking DNS из transport worker-а исключён.
- **Один route-профиль для Sieve redirect** — типизированный домен `peer.test`.
  Запрещённый route даёт implicit keep и Sieve diagnostic, а повреждённый job
  получает terminal `5.7.1`, а не бессмысленный retry.

## Как воспроизвести проверку

```sh
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
cargo test --locked
cargo build --locked --release --bin litterkeep-server
cargo run --locked -- help
```

Закреплённый тулчейн — Rust **1.97.0** (`rust-toolchain.toml`), он же в
`Cargo.toml` и в Docker-образе `rust:1.97-bookworm`.

Тот же набор выполняется в CI на каждом пуше: `fmt` и `clippy` — на ubuntu и
windows, тесты — на ubuntu и windows, плюс release-сборка. Бейдж вверху
README показывает текущее состояние.

Черновые black-box проверки лежат в `tests/blackbox/`. Они требуют живого
сервера и Linux-окружения; README описывает порядок запуска. Машинно-читаемая
матрица фактически выполненных проверок — `docs/acceptance.tsv`.

## Где искать подробности

| Документ | О чём |
|---|---|
| [`docs/README.md`](docs/README.md) | Навигация по документации и порядок чтения |
| [`docs/architecture.md`](docs/architecture.md) | Процесс, владение ресурсами, долговечность |
| [`docs/runtime-lifecycle.md`](docs/runtime-lifecycle.md) | Задачи, ожидание, пробуждение, отмена |
| [`docs/storage-format.md`](docs/storage-format.md) | Формат файлового хранилища и recovery |
| [`docs/decisions.md`](docs/decisions.md) | Принятые архитектурные решения и их цена |
| [`docs/dependencies.md`](docs/dependencies.md) | Production-зависимости и собственные примитивы |
| [`docs/security.md`](docs/security.md) | Security profile |
| [`docs/client-setup.md`](docs/client-setup.md) | Подключение почтового клиента |
| [`docs/protocol-coverage.md`](docs/protocol-coverage.md) | Подробное покрытие SMTP, IMAP, POP3, Sieve, outbound |
| [`docs/imap-profile.md`](docs/imap-profile.md) | Построчная матрица IMAP со статусами |
| [`docs/sieve-coverage.md`](docs/sieve-coverage.md) | Профиль Sieve и ManageSieve |
| [`docs/sieve-extension-audit.md`](docs/sieve-extension-audit.md) | Инвентарь расширений Sieve |
| [`docs/unicode-data.md`](docs/unicode-data.md) | Закреплённая версия Unicode data |
| [`docs/acceptance.md`](docs/acceptance.md) | Итоговая приёмка по задачам |
| [`docs/acceptance.tsv`](docs/acceptance.tsv) | Машинно-читаемая матрица приёмки |
| [`GOAL.md`](GOAL.md) | Журнал разработки по этапам |
