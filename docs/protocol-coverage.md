# Покрытие протоколов

## TLS listener composition

Linux-команда `serve-all-mail-tls` обслуживает SMTP submission, IMAP, POP3 и
ManageSieve одним reactor-ом с общим владельцем Store и выполняет TLS handshake
до выдачи каждого greeting-а. Независимый Linux TLS wire acceptance проверил
сертификат, greeting, протокольные capabilities и SASL PLAIN для всех четырёх
listener-ов; STARTTLS/STLS не рекламируются внутри уже защищённого канала.
Это подтверждает только этот режим запуска. Внешняя совместимость с клиентом
Litterkeep и полнота RFC-профилей остаются отдельными непроверенными вопросами.

[RFC 8314](https://www.rfc-editor.org/rfc/rfc8314.html) рассматривает SMTP
submission, IMAP и POP3; ManageSieve включён в общую команду как локальное
архитектурное удобство, но это не расширение утверждений о сфере RFC 8314.

## SMTP

RFC 3461 `DSN` рекламируется через EHLO; независимые raw-socket peers
подтвердили capability, принятие MAIL `RET`/`ENVID` и RCPT `NOTIFY`/`ORCPT`,
завершение DATA и отказ этих параметров после HELO. Принятый typed envelope
проходит через DATA/BDAT к единственному Store owner; Store сохраняет его в
commit-linked sidecar и восстанавливает после reopen. Это доказывает wire
negotiation и envelope persistence, но не полную семантику итоговых уведомлений
для каждого маршрута или Litterkeep app interoperability.

RFC 3461 требует применить запросы условий `NOTIFY`, возврата содержимого и
корреляции envelope/получателя либо не заявлять о поддержке. Внутреннее
планирование failure report после окончательного отказа и часть MIME-семантики
уже реализованы и проверены. Сочетание параметров SMTP wire-path с каждым
итоговым сценарием доставки всё ещё покрыто частично. Failure notice теперь сохраняется вместе с
terminal queue transition и восстанавливается после перезапуска; DSN append
идемпотентен по committed Store transaction, а очередь очищает notice отдельным
durable acknowledgement. Unit/failpoint tests и полный Windows/Linux suite
проверили эти переходы. Независимый runtime crash-kill сценарий ещё не проверен.
Success report serializer теперь создаёт отдельный `Action: delivered` / `Status:
2.0.0` outcome и возвращает только заголовки даже при `RET=FULL`; unit vectors
проверяют также null reverse-path. Сам serializer требует явный `NOTIFY=SUCCESS`
для конкретного RCPT и не формирует отчёт для omitted policy, FAILURE-only или
NEVER. Store теперь до source commit записывает checksummed success intent для
каждого eligible RCPT с локальным mailbox target; после recovery видны только
intents, чья source transaction committed. Unit integration покрывает duplicate
RCPT policy, discard и crash до commit. Если несколько RCPT разрешаются в один
локальный mailbox, Store фиксирует одно сообщение, а независимые SUCCESS intents
сохраняет по каждому eligible RCPT ordinal; T402 проверяет оба ключа после
reopen. Для outbound redirect queue route теперь хранит все source ordinals,
если несколько RCPT того же локального mailbox сведены в одну Sieve evaluation.
Recovery, deferred retry и DSN idempotency key сохраняют список. Store
восстанавливает typed metadata для каждого origin, а failure serializer
формирует отдельный RFC 3464 recipient block для каждого origin, который имеет
право на уведомление. Исторические T385/T404 ниже описывают checkpoint до этих
исправлений, а не текущее поведение.
Consumer доступен как typed bounded
StorageWorker operation: он восстанавливает envelope и исходный blob, принимает
существующий локальный sender account либо передаёт внешний reverse-path через
durable outbound SMTP job, строит `Action: delivered` и публикует отчёт
идемпотентно. Commit отчёта подтверждает intent и устраняет его
из pending-представления после recovery; failpoint проверяет retry при сбое до
commit. Store owner выполняет ограниченный sweep при старте и после каждой
bounded-команды, поэтому восстановленные intents обрабатываются без отдельного
протокольного запроса. Ошибки видны в stderr, исходная доставка не отзывается,
pending intent остаётся для retry. Success intents обходятся по устойчивому
cursor-у в `BTreeSet`, а StorageWorker продолжает bounded-страницы до 100 записей
по idle timer-у; independent StorageWorker test с backlog 101 подтверждает
продолжение без клиентского запроса. Committed report удаляет intent из
оперативного pending-индекса, recovery фильтрует подтверждённые записи, поэтому
лимит ограничивает реально просмотренные элементы. Для существующего внешнего reverse-path
Store создаёт outbound job с пустым SMTP reverse-path; независимый Linux SMTP
peer подтвердил `MAIL FROM:<>`, внешний RCPT, `Action: delivered`, отсутствие
исходного тела и удаление job после принятия. Доставка проверена только до
явно настроенного relay peer: публичный DNS/MX и интернет не проверялись.
Пакетный sweep после ошибки Store применяет экспоненциальную задержку в памяти
процесса от 100 ms до 30 s и сбрасывает её после успешного прохода; deterministic
unit tests проверяют границы и cap. Расписание повтора отдельного intent пока
не durable и не сохраняется между перезапусками; dead-letter policy остаётся GAP.
Delay report scheduling, downstream SMTP parameter propagation и полная
сквозная семантика повторной доставки остаются неподдержанными. RFC 3461 §5.2.3 требует
`delivered` DSN при локальной доставке с `NOTIFY=SUCCESS`; RFC считает её
успешной, когда письмо уже доступно для IMAP/POP retrieval. SMTP принимает
запросы по wire-протоколу, а committed SUCCESS intents создаются для локальной
mailbox delivery. Задержанные уведомления пока не создаются, а relay-путь не
передаёт RFC 3461 параметры следующему MTA. Внутренние
typed delivered serializer, commit-linked success intent, Store-owner consumer
и доставка отчёта локально/через явно настроенный relay реализованы. Внешняя
capability DSN объявлена и negotiation проверена T405, однако это частичная,
не полная реализация RFC 3461.
Независимый Linux wire
peer зафиксировал прежнее отсутствие capability и отказ MAIL/RCPT до включения
расширения; более поздний T405 проверил текущую EHLO negotiation. Ранний
независимый Linux socket peer также проверил отказ `555 5.5.4` для `ORCPT=utf-8`
до включения wire-поддержки. Отдельный Linux Store integration test подтвердил
durable round-trip typed envelope. RFC 6533 задаёт для `utf-8` ORCPT три разные
формы xtext, unitext и native UTF-8 с разными условиями SMTPUTF8. Теперь
внутренний parser декодирует 7-bit-safe `utf-8-addr-xtext` и
`utf-8-addr-unitext`, проверяя результат как UTF-8 mailbox; sidecar v2 сохраняет
его type и octets, а version 1 продолжает читаться. Parser сводит xtext и
unitext к одному семантическому UTF-8 адресу, не сохраняя способ кодирования.
Независимый Linux peer подтвердил точный отказ 555 5.5.4 для xtext и буквального
Unicode unitext до включения wire-поддержки; этот отказ является историческим.
Native UTF-8 form policy и SMTPUTF8-specific form constraints остаются
неполными. Уведомления о задержке не реализованы, downstream SMTP parameter
propagation не проверена. Positive SUCCESS report доступен для поддерживаемых
committed intents, запрошенных через SMTP `NOTIFY=SUCCESS`.

Исторические T385/T404 acceptance-записи фиксируют прежние контрольные точки,
а не текущее состояние. Сейчас durable queue jobs сохраняют все исходные RCPT
ordinal для сгруппированного redirect, Store восстанавливает typed DSN metadata
каждого источника, а failure policy выбирает подходящие источники и
сериализует отдельную RFC 3464 recipient-группу для каждого из них. T412 добавил
регрессию для двух eligible FAILURE-источников и двух recipient-групп. T405
независимо проверил wire negotiation, но multi-origin report подтверждён
unit/Store тестами, не отдельным сетевым acceptance.
Eligibility policy покрывает SUCCESS/FAILURE/DELAY: default не запрашивает
SUCCESS, разрешает обязательный FAILURE и допускает опциональный DELAY; явный
список ограничивает outcome, NEVER подавляет всё. Delay reports не генерируются.
Генератор failure reports переносит `Original-Envelope-ID` и типизированный `Original-Recipient`,
сохраняет отдельный `Final-Recipient`, соблюдает разделение per-message и
per-recipient блоков, а при сохранённом `RET=HDRS` не включает тело письма.
Для UTF-8 адресов используется RFC 6533 `message/global-delivery-status`, а
для UTF-8 заголовков только `RET=HDRS` используется `message/global-headers`;
ASCII-варианты используют стандартные DSN media types. Эти правила проверены
unit regressions на Windows и Linux; independent SMTP/IMAP acceptance ранее
проверил один failure report, а T405 отдельно подтвердил DSN negotiation.
Задержанные reports и полная downstream relay семантика остаются GAP. Failure-
report crash window закрыт durable intent и idempotent report key (T389), но
независимый crash-kill runtime acceptance ещё не проводился.
RFC 3461 §§4.3, 5.2.6, 5.2.7 и 6 описывает эти требования.
См. [RFC 3461](https://www.rfc-editor.org/rfc/rfc3461.html).

Генерируемый сервером внутренний failure report сериализует `Final-Recipient`
как `rfc822; mailbox` без SMTP path угловых скобок; unit regression проверяет
это поле и запрещает прежнюю форму. Независимый Linux acceptance прошёл полный
путь ManageSieve STARTTLS redirect, SMTP submission, файловая очередь, ответ
внешнего peer `550 5.1.1`, возврат через RSET/QUIT и чтение созданного failure
DSN стандартным IMAP STARTTLS клиентом из Alice INBOX. Клиент проверяет MIME
report type, `Action: failed` и точное поле `Final-Recipient`. Этот сценарий
проверяет generated-report syntax и чтение через IMAP, а T405 отдельно
проверяет RFC 3461 ESMTP negotiation. Их сочетание пока не доказывает полный
объём условий и downstream relay семантики.
См. [RFC 3464 §2.3.2](https://www.rfc-editor.org/rfc/rfc3464.html#section-2.3.2).

Submission поддерживает implicit TLS и STARTTLS, EHLO, AUTH PLAIN, MAIL SIZE,
несколько RCPT, DATA с однократным dot-unstuffing, RSET, NOOP и QUIT. Также
рекламируются RFC 2920 `PIPELINING` и RFC 6152 `8BITMIME`. Независимый
STARTTLS peer подтвердил PIPELINING: один сетевой write содержит MAIL,
ошибочный RCPT, успешный RCPT и DATA; ответы должны сохранять порядок, принятый
адресат не теряется, а письмо читается Bob через IMAP. Остальные SMTP extensions
оцениваются отдельно. `BODY=8BITMIME` разрешает high-bit octets,
которые сохраняются до IMAP, а без него DATA с такими octets отклоняется после
dot terminator. `BODY=7BIT` и режим без BODY ограничивают содержимое US-ASCII.
DATA parser отдельно отслеживает NUL и после полного dot terminator отвергает
его кодом `554 5.6.0`; этот октет запрещён в MIME 7bit и 8bit data. Лимит
размера сохраняет отдельный ответ `552 5.3.4`. Unit-проверка и независимый
STARTTLS wire-тест подтверждают отказ NUL в обоих режимах, enhanced status,
возврат в command state, последующую валидную доставку и отсутствие отвергнутых
сообщений в IMAP. В wire-тесте восстановление подтверждено `NOOP`, не `RSET`.
Теперь тот же DATA validator отвергает одиночные CR и LF: independent
STARTTLS peer проверил оба байта при обоих BODY-профилях, ответ `554 5.6.0`,
последующий NOOP, валидную SMTP delivery и отсутствие четырёх rejected messages
в IMAP. MIME line endings принимаются только как исходная пара CRLF; отдельный
BINARYMIME regression сохраняет произвольные NUL/high-bit/bare CR/LF octets в
BDAT. См. [RFC 2045 §§2.7-2.9](https://www.rfc-editor.org/rfc/rfc2045.html#section-2.7)
и [RFC 6152 §3](https://www.rfc-editor.org/rfc/rfc6152.html#section-3).
Незавершённая DATA wire-строка ограничена до чтения CRLF: независимый STARTTLS
peer отправляет более 26 MiB без разделителя, затем dot terminator, получает
552 и успешно продолжает той же сессией командой NOOP. Это проверяет bounded
input и синхронизацию после отказа, но не означает сохранение отвергнутого
сообщения или проверку приложения Litterkeep. См. [RFC 5321 §4.5.3.1.6](https://www.rfc-editor.org/rfc/rfc5321.html#section-4.5.3.1.6).
До TLS AUTH запрещён. Ненулевой envelope sender должен принадлежать
authenticated owner. RFC 6409 §3.2 разрешённый `MAIL FROM:<>` не отклоняется
только из-за пустого reverse-path: authenticated identity даёт MSA return path;
локальная доставка сохраняет `Return-Path: <>`. Независимый implicit-TLS peer
проверяет также, что чужой ненулевой sender получает `550 5.7.1` согласно
RFC 6409 §6.1. Local route не является open relay.

Входящий SMTP command idle deadline составляет пять минут, а DATA body использует
отдельный трёхминутный срок бездействия на принимаемый блок. Таймер учитывает
фактические socket reads, не повторную обработку уже буферизованных строк.
Независимый Linux STARTTLS peer после AUTH дождался TCP EOF через 300.1 секунды
в command state; отдельный peer после `354` и неполного body block получил EOF
через 180.1 секунды. Детерминированные lifecycle tests отдельно проверяют
границу срока, освобождение lease и взаимодействие с pending worker. Это
server-side timeout acceptance, не тест приложения Litterkeep.

Повторная wire-проверка отделяет сброс срока от первоначального deadline:
после первого неполного DATA block независимый STARTTLS peer ждал 120 секунд,
передал свежие body octets и получил TCP EOF через 180.1 секунды после второго
socket read. Тем самым проверено, что новый фактически принятый блок запускает
полный трёхминутный срок заново, а не только что первоначальный срок истекает.

RFC 3030 `CHUNKING` объявляется через EHLO во всех режимах SMTP и принимает
`BDAT n [LAST]` с точным octet framing, нулевыми и промежуточными chunks.
Промежуточные chunks получают отдельный 250; `LAST` проверяет согласованный
7BIT/8BITMIME/BINARYMIME профиль и передаёт message единственному durable
Store owner.
Ошибка размера или последовательности переводит envelope в failed state до
RSET, а отвергнутый payload точно дренируется без потери следующей команды.
Независимый Linux TLS peer проверил zero-length и два message chunks, успешную
доставку Alice -> Bob и точное совпадение декодированных MIME attachment bytes;
второй raw peer проверил capability, отказную BDAT без MAIL/RCPT и сохранение
границы перед pipelined BDAT и NOOP. После 503 на отвергнутый chunk сервер
сохраняет failed state: следующий pipelined chunk дренируется и получает 503,
команды до RSET тоже получают 503. RSET очищает состояние, после него NOOP
получает 250. Отдельно проверены invalid content на LAST и chunk размера
26 214 401 октет: ответы 552 отправлены только после drain, затем RSET и NOOP
успешно восстановили сессию. Перезапуск Store между отправкой и IMAP чтением не
выполнялся. Inbound объявляет `BINARYMIME` вместе с `CHUNKING`, принимает
`MAIL ... BODY=BINARYMIME` и завершает только BDAT-транзакции. RFC 3030 text
parts и multipart preamble/epilogue проверяются на canonical CRLF без DATA
ограничения строки в 1000 октетов; non-text bodies сохраняются непрозрачно.
`DATA` после такого MAIL получает 503 и переводит сессию в режим, требующий
RSET. Если MIME-заголовки содержат UTF-8 field values, BINARYMIME LAST также
требует установленный в MAIL флаг `SMTPUTF8`; без флага сообщение отклоняется.
Independent Linux implicit-TLS peer проверил обе ветви: без MAIL SMTPUTF8
UTF-8 заголовки отклонены на BDAT LAST, с MAIL SMTPUTF8 exact payload принят,
извлечён через IMAP и пережил restart. Это ранняя forced-parameter проверка;
новая проверка объявления capability и conforming U-label пути описана ниже.
Independent peer подтвердил
полный SMTP -> файловый Store -> IMAP сценарий для NUL, high-bit, bare LF/CR bytes, запрет DATA,
отказ на noncanonical text, восстановление через RSET, точное чтение Bob и
повторное чтение после рестарта сервера. Это локальный independent peer, не
Litterkeep app. Outbound relay реализует RFC 3030 для MIME binary parts:
требует одновременно peer capabilities `BINARYMIME` и
`CHUNKING`, валидирует MIME headers и canonical CRLF текстовых частей, объявляет
точный BDAT payload size и посылает последовательные bounded chunks с ожиданием
отдельного `250` после каждого. DATA-ограничение 1000 октетов не переносится на
`text/*` в BINARYMIME: требуется только canonical CRLF; multipart preamble и
epilogue проверяются без изменения binary body parts. После 4xx/5xx следующий
chunk не отправляется.
Plain TCP и implicit TLS independent peers подтвердили точные байты, включая
NUL/high-bit octets и нетекстовые bare CR/LF; отдельные тесты подтвердили отказ
без capability и остановку после 451. RFC 5321 MAIL line limit включает
добавочные RFC 3030 octets и суммируется с RFC 6531 allowance. Это unit peer
coverage, не outbound доставка на реальный внешний сервер или Litterkeep app.
См. [RFC 3030 §§2-3](https://www.rfc-editor.org/rfc/rfc3030.html#section-2).

По рекомендации RFC 8314 §3.4 implicit-TLS SMTP-сессия отвечает на
peer-инициированный `close_notify` после обычного NOOP reply. Независимый Linux
TLS peer подтверждает SMTP reply и завершает TLS `unwrap()` без SMTP QUIT.
Отдельный сквозной acceptance проходит SMTP AUTH и DATA через implicit TLS,
читает успешный `250`, отправляет `close_notify` без QUIT и находит доставленное
письмо в Bob INBOX через IMAP. Peer close до получения DATA acknowledgement
отдельно не проверен.
Отдельное acceptance проверяет тот же lifecycle для IMAP; POP3 проверяется
отдельно ниже. Это closure coverage, не полный SMTP/TLS профиль.

Дополнительный RFC 2920 §3.2 STARTTLS peer отправил MAIL, единственный
несуществующий RCPT и DATA одним write. Получены упорядоченные `250`, `550`,
`503`: DATA не открыл content state без принятого адресата, а следующий NOOP
и QUIT прошли без потери синхронизации. Это проверка поведения all-rejected
pipeline, а не общее доказательство всего SMTP расширения.

SMTP AUTH PLAIN поддерживает оба RFC 4954 варианта: initial response в команде
и отдельный ответ после пустого `334 `. SASL cancel `*` и недекодируемый ответ
получают 501; после отмены или ошибки command stream восстанавливается. После
успешной аутентификации повторный AUTH отклоняется кодом 503. Независимый
Python `smtplib` на STARTTLS проверил continuation, initial-response и recovery;
другие SASL mechanisms не реализованы и не рекламируются. Ответ continuation
ограничен общим RFC 5321 максимумом 512 октетов вместе с CRLF: 513 октетов
получают 500, после чего клиентская сессия возвращается в command state.
Проверено независимым STARTTLS black-box. См. [RFC 5321 §4.5.3.1.4](https://www.rfc-editor.org/rfc/rfc5321.html#section-4.5.3.1.4).

Legacy Argon2 migration имеет отдельный независимый implicit-TLS acceptance:
fixture переносит bootstrap `.phc2` в `.phc`, затем стандартный Python SMTP
клиент выполняет AUTH PLAIN. После входа `.phc2` появился, а байты `.phc`
совпали с исходными. Для проверки приоритета второй вход проходит, пока
legacy-файл временно переименован; тест восстанавливает его даже при ошибке.
Сценарий `tests/blackbox/smtp_legacy_phc_migration.py` использует отдельное
`./data/g18-legacy-phc-20260928` и подтверждает только SMTP wire/auth/storage,
не IMAP/POP3/ManageSieve или Litterkeep app.

Аргумент HELO проверяется как единственный RFC 5321 Domain. EHLO принимает
единственный Domain либо валидный IPv4/IPv6 address-literal; пустой аргумент,
лишние слова, лишние пробелы и ошибочные DNS labels получают 501. Валидные
приветствия принимаются независимым Linux raw-TCP клиентом после серии отказов.

Неизвестные SMTP-команды получают `500 5.5.2`, а неверный аргумент известной
команды - `501 5.5.2`. В том же соединении после обоих отказов принимаются
следующие команды. Это различие кодов задано RFC 5321 §4.2.2 и проверяется
воспроизводимым Linux raw-TCP сценарием
[`smtp_unknown_command.py`](../tests/blackbox/smtp_unknown_command.py); тест не
использует клиент Litterkeep.
Аргументы HELP/NOOP, где grammar допускает только ASCII, не проходят
Unicode-aware `trim`: NBSP не маскирует невалидный байт как пустой optional
argument. `NOOP ` и `HELP ` возвращают 501; затем NOOP/QUIT в том же TCP
соединении подтверждают recovery. VRFY/EXPN имеют отдельную RFC 6531 String
грамматику, которая допускает UTF-8 query даже при отсутствии reply marker-а.
Сервер не возвращает UTF-8 identity в ответе.
Парсер отдельно понимает RFC 6531 §3.7.4.2 запросы VRFY/EXPN с valueless
`SMTPUTF8` и UTF-8 query, сохраняет режим reply encoding и отвергает параметр
со значением. Marker обозначает способность клиента принимать UTF-8 в ответе и
не запрещает UTF-8 в самом query. Сервер отвечает privacy-safe `252`/`502` и
не выдаёт non-ASCII identity. Независимый Linux wire test проверил
ASCII/UTF-8 query с marker-ом и без него, malformed marker и session recovery.
Это не interoperability-тест конкретного клиента.

В production `MAIL FROM:` и `RCPT TO:` angle-path начинается сразу после
двоеточия: пробел и табуляция между delimiter-ом команды и `<` получают 501,
а последующая команда остаётся синхронизирована. Проверено unit-тестом parser-а
и независимым raw-TCP клиентом на Linux. См. [RFC 5321 §4.1.2](https://www.rfc-editor.org/rfc/rfc5321.html#section-4.1.2).
После `>` MAIL-параметры и параметры RCPT могут отделяться только одиночным
ASCII SP согласно grammar RFC 5321: HTAB, повторный или завершающий пробел
получают 501. Несколько корректных MAIL-параметров с одиночным SP остаются
допустимы; независимый raw-TCP клиент проверяет отказ и восстановление сессии.

SMTP session требует HELO/EHLO перед MAIL/RCPT/DATA. HELO включает только базовый
SMTP режим: ESMTP MAIL-параметры, AUTH и STARTTLS требуют EHLO. Успешные HELO и
EHLO сбрасывают незавершённый envelope; это проверяется независимым клиентом.
MAIL во время открытой transaction получает 503 без изменения sender или списка
получателей; прежний конверт остаётся пригоден для DATA. Это правило порядка
RFC 5321 проверено unit FSM-тестом и сквозной SMTP-to-IMAP доставкой. Отдельный
implicit-TLS тест подтвердил приоритет 503 над sender-authorization отказом:
после активного envelope чужой MAIL получил 503, а DATA завершил первоначальную
доставку с неизменённым `Return-Path`.
`RSET` доступен в любой момент, в том числе сразу после STARTTLS и до нового
EHLO/AUTH. Независимый Linux TLS peer проверил `250` для RSET в этом состоянии,
а последующие EHLO/NOOP/QUIT подтвердили сохранение синхронизации. См.
[RFC 5321 §4.1.4](https://www.rfc-editor.org/rfc/rfc5321.html#section-4.1.4).
Общий SMTP path limit для MAIL FROM и RCPT TO равен 256 октетам вместе с
угловыми скобками. Парсер принимает граничный path и отвергает path на 257
октетах; unit и независимые STARTTLS black-box проверяют обе команды.
Для обычного `local-part@domain` парсер принимает local-part до 64 октетов и
отклоняет более длинный в MAIL FROM и RCPT TO. Сетевой тест отделяет syntax
error от последующей проверки sender/recipient policy. Это ограничение длины,
а не утверждение о полной проверке RFC mailbox grammar или SMTPUTF8.
ASCII dot-atom и RFC 5321 quoted-string local-part проверяются до policy слоя
в обеих командах. Quoted-pair декодируется для сопоставления с локальным
аккаунтом; завершающий `>` и `@` внутри quoted local-part не путаются с
разделителями path. Независимый STARTTLS peer отправил письмо от
`"alice"@litterkeep.test` к `"bob"@litterkeep.test`, после чего отдельный IMAP
peer нашёл письмо в INBOX Bob. Устаревший SMTP source-route `@domain,@domain:`
принимается после проверки каждого элемента и отбрасывается до локальной
маршрутизации согласно RFC 5321 §4.1.2; независимый SMTP-to-IMAP тест доказал,
что конечный mailbox получает письмо, а route не включает forwarding.
Пустые или неверно разделённые dot-atom точки, пустые доменные labels и DNS
labels длиннее 63 октетов отклоняются как syntax error. U-label domain
обрабатывается отдельно по IDNA/SMTPUTF8 правилам, описанным выше.
IPv4 address-literals и зарегистрированный `IPv6:` tag проверяются отдельно от
DNS-domain грамматики: десятичные IPv4 octets ограничены диапазоном 0..255,
IPv6 включая сжатую и IPv4-embedded форму проверяется стандартным Rust IP
parser-ом. Невалидный IP и незарегистрированный tag получают syntax error.
Корректные literals доходят до политики: они не совпадают с локальными доменами,
поэтому sender получает 553, recipient 550, без relay. Это покрывает только
текущий IPv4/IPv6 синтаксис и не утверждает, что произвольные будущие tags
поддержаны. Проверенный набор стандартизованных tags сверяется с
[реестром IANA](https://www.iana.org/assignments/smtp/#smtp-address-literal-tags).
Входящий SMTPUTF8 path принимает Unicode local-part и IDNA-valid U-label
домены, требует valueless MAIL параметр `SMTPUTF8` и канонизирует домен в
A-label перед локальной маршрутизацией. EHLO объявляет SMTPUTF8 вместе с
обязательной 8BITMIME. Независимые Rust и raw-TCP проверки покрывают parser,
capability, отсутствие параметра и сброс маркера транзакции через RSET.
В `smtp::punycode` добавлен самостоятельный RFC 3492 Bootstring encoder/decoder
с overflow checks и bounded input; все 19 примеров RFC 3492 §7.1 проверяются
unit-тестом в обоих направлениях, включая исправленное RFC Editor написание
русского примера I. ASCII-пример S относится только к Bootstring и не является
IDNA-меткой. Это только переносная кодировка code points. Дополнительно
`smtp::idna17` классифицирует Unicode
17 code points по сгенерированному официальному RFC 5892 Derived Property
набору: PVALID, CONTEXTJ и CONTEXTO остаются разными typed результатами;
прочие значения дают `DisallowedOrUnassigned`. Тесты проверяют представительные
символы и каждый класс, а не весь IDNA conformance corpus. RFC 5892 прямо
отделяет свойство code point от валидности label. `to_alabel` переводит
проверенную NFC U-label в Punycode A-label и
проверяет leading/trailing/reserved hyphens и 63-октетный encoded label limit.
CONTEXTO для U+00B7/U+0375/U+05F3/U+05F4/U+30FB и двух Arabic digit sets
проверяется по Unicode 17 Scripts таблице и соседним символам; невалидный
контекст получает отдельную ошибку. CONTEXTJ использует Unicode 17
Canonical_Combining_Class=Virama и DerivedJoiningType: ZWJ требует предыдущий
virama, ZWNJ допускает virama или RFC joining context с пропуском Transparent.
Ошибочный контекст отклоняется. RFC 5893 Bidi Rule использует Unicode 17
DerivedBidiClass и активируется для всех labels домена, если хотя бы одна
содержит R, AL или AN. ASCII SMTP address parser использует проверку LDH и
полный round-trip для входных A-label. `to_alabel_domain` преобразует U-label
домен в A-label DNS form; SMTP path использует это преобразование до route
lookup. Встроенная таблица RFC 5892 Derived Property сверена со всеми
1,112,064 Unicode scalar values из официального Unicode 17 `Idna2008.txt`;
это exhaustive property comparison, не проверка полной U-label/domain
conformance suite, RFC 5891 registration workflow или каждой границы Punycode
и whole-domain алгоритма. DNS MX discovery и удалённая доставка U-label
доменов здесь не проверены.
См. [RFC 3492](https://www.rfc-editor.org/rfc/rfc3492.html) и
[RFC 5890](https://www.rfc-editor.org/rfc/rfc5890.html),
[RFC 5891](https://www.rfc-editor.org/rfc/rfc5891.html) и
[RFC 5892](https://www.rfc-editor.org/rfc/rfc5892.html),
[RFC 5893](https://www.rfc-editor.org/rfc/rfc5893.html).
MAIL с таким reverse-path требует флаг SMTPUTF8; без него parser возвращает
отдельную typed policy error, сериализуемую как `550 5.6.7` для отправителя
согласно RFC 6531 §3.5. Reactor guard отклоняет UTF-8 RCPT без флага активного
MAIL кодом 553 5.6.7. Независимый Linux TCP peer проверяет enhanced status
`5.6.7` отдельно для `553` RCPT и `550` MAIL, а также достижение account lookup (550 на
намеренно отсутствующий UTF-8 account) при корректном SMTPUTF8. Дополнительный
forced-extension acceptance создал opt-in Unicode account отдельной командой
dev bootstrap, доставил ему MIME-вложение по SMTP STARTTLS и извлёк его через
IMAP STARTTLS/SASL PLAIN;
отдельное чтение после перезапуска подтвердило durable delivery. Вложение
совпало побайтно, SHA-256 `7f4e114fb71bb6cf784bda9f0d77d06e3ccb15cfdc667bbad322dcef81da20f8`.
Это историческая T84 forced-extension проверка, созданная до T195: тогда
SMTPUTF8 намеренно передавался без advertisement и проверялся только внутренний
путь доставки, не interoperability; её запись в acceptance.tsv помечена
SUPERSEDED. Актуальный conforming wire acceptance приведён ниже. Сам флаг не
требует BODY=8BITMIME: 8-bit тела по-прежнему
разрешаются только BODY=8BITMIME, а UTF-8 заголовки требуют SMTPUTF8. Отдельный conforming STARTTLS клиент далее
передаёт U-label `пользователь@bücher.test`, читает MIME вложение по IMAP и
независимо сравнивает точные octets. Account
credentials, folder catalog, subscriptions, Sieve snapshots и внутренние
mailbox keys сохраняют bounded UTF-8 идентификаторы в отдельном hex namespace;
прямой Store/Accounts restart test проверяет эти слои, а conforming acceptance
выше отдельно проверяет SMTP/IMAP network путь. Ограниченный DATA/transport
checkpoint разбирает MAIL флаг SMTPUTF8, разрешает UTF-8 field values, но
оставляет field names ASCII, как требует RFC 6532 §3; outbound требует peer
capabilities перед MAIL.
Независимый Linux SMTP peer подтвердил `BODY=8BITMIME SMTPUTF8` и сохранность
UTF-8 header octets. Conforming U-label SMTP/IMAP delivery проверена отдельным
клиентом выше. Полная грамматика RFC 6532 для UTF-8 в structured fields и
совместимость с Litterkeep app остаются непроверенными. Имена полей остаются
ASCII, не являются RFC 6532 extension.
RFC 5321 Postmaster принимается как bare `<Postmaster>` без домена и как
`postmaster@domain` для локально обслуживаемых доменов; регистр reserved local
name и домена не влияет на route. Независимый STARTTLS тест отправил DATA и
проверил запись в отдельный durable `postmaster` mailbox без создания
парольного аккаунта. Административный способ читать этот системный mailbox
пока не предоставлен, поэтому проверен SMTP acceptance/storage, не операторский
workflow и не совместимость конкретного почтового клиента.
Командная строка ограничена 512 октетами с CRLF: независимая проверка принимает
точно 512, отклоняет 513 ответом 500 и подтверждает синхронизацию следующей
командой NOOP.
Envelope допускает не менее 100 успешных RCPT. После заполнения текущего
буфера сервер отвечает `452 4.5.3` на следующий адрес, не теряя ранее
принятые адресаты; DATA по прежнему envelope остаётся допустимым. Независимый
STARTTLS SMTP/IMAP black-box проверил 100 ответов 250, ответ 452 на 101-й,
успешную отправку и чтение результата Bob по IMAP. Это проверка лимита буфера,
а не полной семантики адресной политики или распределения ошибок RCPT.

RFC 5321 VRFY отвечает `252` без проверки и раскрытия существования mailbox;
EXPN распознаётся, но возвращает `502`, так как сервер не предоставляет mailing
list expansion. HELP выдаёт многострочный список поддержанных команд, включая
AUTH, STARTTLS и BDAT; HELP с аргументом подсказывает вызвать общий список; NOOP
игнорирует необязательный параметр.
Независимая проверка подтверждает ответы до и во время активного envelope.

RFC 2034 `ENHANCEDSTATUSCODES` объявляется в каждом варианте EHLO, включая
plaintext и TLS-состояния. Ответы SMTP классов 2xx, 4xx и 5xx снабжаются
enhanced code; исключения RFC сохраняются для initial greeting и HELO/EHLO,
а continuation-коды 3xx его не получают. Многострочный HELP повторяет один
enhanced code на каждой строке, причём тест сверяет класс enhanced-кода с
основным SMTP-кодом. Независимый STARTTLS socket peer проверяет
объявление extension, ответы AUTH/envelope/DATA/QUIT и ошибку неизвестного
командного слова; это wire coverage RFC 2034, а не полное SMTP acceptance.

RFC 1870 SIZE объявляется с фактическим фиксированным пределом сообщения.
Единственный корректный `MAIL FROM ... SIZE=n` разбирается до DATA; значение
выше предела получает 552, некорректная грамматика или повтор SIZE получают
501. SIZE расширяет максимальную длину строки MAIL ещё на 26 октетов согласно
RFC 1870; этот allowance суммируется с дополнительными 10 октетами SMTPUTF8 и
16 октетами BINARYMIME. Типизированная классификация учитывает регистр и
комбинации параметров. Boundary unit tests проверяют все восемь комбинаций
лимитов и полную 538-октетную SIZE-границу. Неподдерживаемые ESMTP параметры
MAIL/RCPT получают 555, а не игнорируются.
Независимый STARTTLS black-box проверяет эти ответы и восстановление сессии.
Отдельная Python wire-проверка на Linux runtime проверила строки ровно 538 и
564 октета: dispatcher пропустил их к parser-у (555 на дополнительный
неподдерживаемый параметр), а 539 и 565 октетов были остановлены receiver-ом
с кодом 500. После каждой границы NOOP получил 250. Это подтверждает реальные
границы приема SMTP-команд, а не только unit calculation.
Если размер тела превышает лимит уже во время DATA, сервер прекращает
накапливать тело, но продолжает потоково читать до завершающей точки; после неё
возвращает 552 и сохраняет синхронизацию команд. Проверено независимым клиентом
на теле размером 25 MiB + 1 байт с последующим NOOP.
Изменение предела MAIL проверено unit-тестами на Windows и полным Linux
Rust 1.97.1 suite, а также указанным независимым STARTTLS wire peer. См.
[RFC 1870 §3](https://www.rfc-editor.org/rfc/rfc1870.html#section-3).
Независимый клиент передал UTF-8 MIME text с `Content-Transfer-Encoding: 8bit`
через SMTP и сравнил payload octets после IMAP `BODY.PEEK[TEXT]`. Проверены
также 7BIT rejection, oversized DATA drain и синхронизация последующего NOOP.
Это проверка конкретного high-bit text сценария, не исчерпывающая сертификация
RFC 6152 для всех MIME content и SMTP peer комбинаций. SMTPUTF8 отдельно
рекламируется с 8BITMIME; его U-label acceptance описана выше.
Строки DATA ограничены 1000 octets вместе с CRLF.
Граница между заголовками и телом отслеживается инкрементально на каждой
принятой CRLF-строке. Парсер больше не сканирует накопленное сообщение заново
для каждого следующего сегмента, поэтому классификация DATA не имеет
квадратичного числа проходов при большом количестве коротких строк. Состояние
границы покрыто Linux unit regression; Linux Docker gate прошёл strict Clippy,
695 library tests, 1 CLI test и doc-tests (1 внешний Unicode corpus test ignored).
При приёме DATA каждая строка с начальной точкой теряет ровно один
transparency dot; RFC line bound проверяется после снятия transparency и
включает CRLF. Linux unit regression проверяет одну, две и три точки, а также
граничную строку ровно в 1000 октетов и отказ для 1001.
Отдельный независимый STARTTLS wire acceptance подтвердил ровно 1000 после
unstuffing для dot-stuffed строки, отказ на 1001 и успешный NOOP после отказа.
Расширенный acceptance проверил обе границы для 7BIT и BODY=8BITMIME, включая
high-bit octet; IMAP read-back подтвердил побайтное сохранение принятых строк
и отсутствие двух отвергнутых длинных сообщений.
Незавершённая входная DATA-строка, уже превысившая максимально возможный wire
размер допустимой строки, удаляется из session input buffer и переводит только
текущую DATA transaction в oversized state. Reactor продолжает считывать до
terminator, после которого возвращает 552 и сохраняет возможность NOOP. Linux
STARTTLS peer передал без CRLF данные больше прежнего общего session input cap,
получил 552 после terminator и 250 на NOOP. Это подтверждает ограниченное
накопление и синхронизацию, но не измеряет RSS процесса.
Исходящий SMTP transport применяет transparency из RFC 5321 §4.5.2: удваивает
точку только в начале строки, передавая окончание DATA отдельно. Независимый
peer снял transparency и сравнил полное сообщение с источником, включая строки
с одиночной и двойной точкой и отсутствующий в source завершающий CRLF. Также
регрессионно подтверждено, что уже имеющийся конечный CRLF не дублируется.
Перед envelope outbound path проверяет CRLF framing и RFC 5321 максимум 1000
octets вместе с CRLF (dot-stuffing octet не считается): независимый peer
подтвердил доставку строки ровно в 1000 октетов и отказ для 1001 без `MAIL`.
Проверен независимый SMTP peer; межсерверные комбинации и Litterkeep-клиент
этими тестами не сертифицируются.

## IMAP

Реализованы STARTTLS/implicit TLS, LOGIN, AUTHENTICATE PLAIN с SASL-IR, LIST,
LSUB, CREATE/RENAME/DELETE, SELECT/EXAMINE, STATUS, APPEND, SEARCH, FETCH,
BODYSTRUCTURE, MIME sections/partial, STORE, COPY, MOVE, EXPUNGE, UID EXPUNGE,
IDLE/DONE и durable Recent. UIDVALIDITY и UID переживают restart.

По RFC 9051 §6.3.2 повторные SELECT/EXAMINE закрывают прежнюю mailbox до
попытки нового выбора и перед новыми ответами выдают `* OK [CLOSED]`. Если
новый выбор не удался, session возвращается к authenticated state без выбранной
mailbox и без expunge. Независимый Linux STARTTLS acceptance проверяет CLOSED
ordering, отказ FETCH до новой SELECT и последующий NOOP. При успешном выборе
сервер также выдаёт обязательный RFC 9051 §6.3.2 untagged LIST с каноническим
именем папки перед прочими выбранными-mailbox ответами; acceptance проверяет
SELECT INBOX, SELECT уникальной пользовательской папки и EXAMINE INBOX с
READ-ONLY completion. Для default Archive это LIST также сообщает `\\Archive`,
взятую из typed special-use каталога, а не выведенную из имени. Это не заявляет
полный профиль.
EXAMINE также возвращает `PERMANENTFLAGS ()`, тогда как read-write SELECT
сохраняет допустимые системные flags и `\\*`; независимый wire peer проверяет
обе ветви, чтобы read-only режим не рекламировал постоянные изменения.
`FLAGS` selection response объединяет system flags и реально сохранённые
mailbox keywords; acceptance создаёт уникальный keyword через APPEND и видит его
после SELECT и EXAMINE.

Implicit-TLS IMAP отвечает на peer-инициированный `close_notify` после
успешного tagged NOOP reply. Независимый Linux TLS peer завершил `unwrap()`;
это подтверждает TLS closure для этого сценария, не полный IMAP4rev2 профиль.

`AUTHENTICATE PLAIN` принимает механизм и SASL-IR как IMAP `astring`: atom и
quoted string; маркер `=` переводится в пустой initial response, отличный от
отсутствующего ответа с continuation. Пустые PLAIN credentials отвергаются, а
сессия остаётся пригодной для следующей команды. Независимый STARTTLS `imaplib`
проверил continuation, atom SASL-IR, quoted mechanism/response, пустой marker,
capability до/после TLS и отказ plaintext authentication. Другие SASL mechanisms
и channel binding не реализованы и не рекламируются.
IMAP quoted-pair принимает только escaped `"` и `\\` по RFC 3501; иной
символ после обратной косой черты возвращает tagged BAD, сохраняя командный
тег. Независимый STARTTLS `imaplib` тест подтвердил BAD и успешный NOOP после
лексической ошибки. UTF8=ACCEPT рекламируется; ENABLE активирует
connection-local mode: legacy quoted-string отклоняет high-bit UTF-8 до
negotiation, а quoted mailbox names разрешаются после неё. Независимый STARTTLS
acceptance проверил positive и negative переходы, включая malformed UTF-8,
C0/C1 controls и ASCII-only atom. Ограниченный synchronizing-literal receiver
поддерживает оба LOGIN astring, mailbox аргументы CREATE/DELETE/RENAME,
SUBSCRIBE/UNSUBSCRIBE, LIST/LSUB/STATUS, COPY/MOVE, SELECT/EXAMINE и UID
COPY/MOVE, а также SEARCH/UID SEARCH. Независимый raw STARTTLS acceptance
проверил LOGIN, CREATE, LIST и SELECT; APPEND использует отдельный receiver.
Это ограниченный набор грамматических позиций: LITERAL+ рекламируется, а
receiver обрабатывает указанные поддержанные command families, не любые
аргументы произвольной IMAP-команды.
Внутренние CRLF учитываются в literal octet count и сохраняются как данные:
STARTTLS LOGIN получил CRLF-containing password literal, вернул обычный auth
failure, после чего принял правильный login. Raw NUL отклоняется; LOGIN password
literals сохраняются побайтно, включая невалидный UTF-8, до authentication
owner-а. Текстовые username/mailbox/search аргументы требуют корректный UTF-8 в
выбранном string mode. Это не полная поддержка произвольных literal octets для
всех команд. Receiver сохраняет байты отдельным типом токена. Независимый STARTTLS
acceptance проверил tagged BAD для UTF-8 quoted LOGIN, восстановление сессии и
UTF-8 BODY literal. RFC 9755 explicit SEARCH CHARSET получает BAD; wire-проверены
обычный и смешанный регистр keyword, после чего implicit UTF-8 SEARCH работает.
UTF-8 mailbox CREATE/SUBSCRIBE/UNSUBSCRIBE/LIST/LSUB/STATUS/SELECT, APPEND,
COPY/MOVE/RENAME/DELETE прошли независимую STARTTLS проверку; после COPY/MOVE
RFC822 bytes совпадают точно. Независимый capability acceptance проверяет
UTF8=ACCEPT до TLS, после TLS/LOGIN и после ENABLE. Это не закрывает весь
профиль RFC 9755; оставшиеся обязательства приведены в
`imap-profile.md`; это не объявляет IMAP4rev2.
FETCH ENVELOPE учитывает connection-local string mode: до ENABLE Unicode,
полученный из RFC 2047 encoded-word, кодируется как октетный literal, а после
ENABLE как UTF-8 quoted-string. Независимый STARTTLS `imaplib` acceptance
прочитал одно письмо в обеих сессиях и подтвердил точную форму заголовка и
отсутствие high-bit байтов внутри legacy quoted-string. Это не проверяет
downgrade исходного RFC822 заголовка, все остальные IMAP response strings или
полный RFC 9755 профиль. Требование не отправлять UTF-8 в quoted-string до
явного ENABLE установлено [RFC 9755 §3](https://www.rfc-editor.org/rfc/rfc9755.html#section-3);
literal остаётся базовой IMAP string формой по [RFC 3501 §4.3](https://www.rfc-editor.org/rfc/rfc3501.html#section-4.3).
Тот же negotiated mode протянут по всему FETCH BODY/BODYSTRUCTURE дереву:
Content-ID/Description, MIME type parameters, disposition filename, languages,
location и вложенный message ENVELOPE проходят mode-aware сериализацию. Unit
тест покрывает Unicode Content-ID, decoded description, RFC 2231 parameters и
обе wire формы; независимый STARTTLS клиент проверил до/после ENABLE DESCRIPTION,
NAME и FILENAME на одном сохранённом MIME-part. Capability проверена новым
сквозным acceptance; это не закрывает все остальные RFC 9755 обязанности.
Дополнительная комбинированная STARTTLS проверка включает `ENABLE UTF8=ACCEPT`,
создаёт mailbox с `é` и запрашивает `LIST RETURN STATUS`: соседние LIST и
STATUS ответы сохраняют UTF-8 имя и порядок запрошенных числовых полей. Это
проверка совместной сериализации RFC 9755/RFC 5819, не готовность всего профиля.
CAPABILITY объявляет `LIST-EXTENDED` по RFC 5258 и `LIST-STATUS` по RFC 5819;
независимый STARTTLS тест проверяет оба объявления до и после TLS upgrade.
`RECENT` разрешён как атрибут STATUS по RFC 3501: до SELECT возвращается число
неполученных durable-сообщений, после SELECT STATUS и LIST RETURN STATUS
используют один session-owned recent UID набор. Это не снимает ограничения
частичного IMAP4rev1 профиля.

RFC 2971 `ID` is advertised and accepted in every IMAP state. The parser
validates `NIL` or a field/value list, enforces the 30-pair, 30-octet field,
1024-octet value and unique-name limits, then discards client-supplied identity
data. The response is always `* ID NIL`, avoiding host/build fingerprinting.
Independent Linux STARTTLS wire acceptance covers the capability before and
after TLS, NIL/list requests, synchronizing literals in both field and value
positions, duplicate-name rejection, privacy response and NOOP recovery. The
shared bounded literal receiver recognizes the RFC list-opening `(` directly
before the first literal marker. This implements the identification extension
only; it does not claim full IMAP4rev2 or all registered extensions.
The same acceptance sends non-synchronizing literal forms for both fields and
values without waiting for continuations, after checking the advertised
capability. RFC 7888 allows non-synchronizing literals in any IMAP command;
this server caps command-argument literals at 64 KiB and uses the advertised
APPENDLIMIT for APPEND. The ID test covers small payloads and the global
oversize boundary. RFC 2971 ID field/value limits are also checked at their
wire boundaries: 30-octet field and 1024-octet value are accepted, while 31
and 1025 octets respectively receive tagged BAD without disclosing ID data;
subsequent NOOP commands confirm session recovery.

RFC 7888 `LITERAL+` включён в CAPABILITY. Synchronizing literal-ы сохраняют
continuation; non-synchronizing `{n+}` не вызывает continuation и ограничен
64 KiB для command arguments и APPENDLIMIT для APPEND. Независимый STARTTLS
клиент проверяет APPEND payload больше 4096 octets и его byte-exact round-trip.
При превышении объявленного лимита сервер закрывает соединение с
`BYE [TOOBIG]`, не выделяя недопустимый payload.
Нулевой APPEND literal получает tagged `NO APPEND cancelled`; independent peer
проверяет после него `NOOP` и пустой mailbox через `SELECT`, а затем продолжает
работу в той же STARTTLS-сессии. RFC 3502 MULTIAPPEND реализован поверх общего
literal receiver: каждый synchronizing literal получает собственный continuation,
а завершённая команда передаётся одним batch-запросом Store owner-у. Успех
возвращает APPENDUID с UID-set в порядке добавления; failure injection проверяет,
что после сбоя первой mailbox frame ни одно сообщение не видно после recovery.
Независимый STARTTLS `imaplib` сценарий отправляет два сообщения с разными
флагами и NUL/0xFF в первом body, сверяет UID-set `1 3,4`, затем читает оба
RFC822 тела побайтно и проверяет FLAGS.
RFC 3502 §3.1 требует, чтобы нулевой message literal завершал APPEND ответом
`NO` и отменял всю операцию. Отдельная unit-проверка Store добавляет пакет с
валидным первым письмом и нулевым вторым, закрывает Store и после reopen
подтверждает отсутствие обоих сообщений. Независимый STARTTLS peer выполняет
тот же сценарий через MULTIAPPEND continuation framing и проверяет `NO` плюс
пустой mailbox после SELECT. Это доказывает отмену пакета без частичной
публикации, не только одиночный пустой APPEND.

RFC 7889 APPENDLIMIT объявляет единый предел `26214400` octets, равный
действующему `MAX_MESSAGE_SIZE`. STATUS и LIST RETURN STATUS возвращают
APPENDLIMIT для mailbox; oversize APPEND получает tagged `NO [TOOBIG]` до
continuation и приёма payload. Независимый STARTTLS клиент проверил capability,
STATUS, LIST STATUS и отклонение размера `26214401`; полная загрузка сообщения
ровно предельного размера отдельным wire-тестом не измерялась.

`FETCH` принимает одиночные и сгруппированные data items, раскрывает ALL, FAST
и FULL, обязательно включает ровно один UID item в каждый ответ UID FETCH даже
если клиент его не запросил, и сохраняет literal как точные октеты. Обычный
FETCH не получает UID автоматически. Отдельный независимый STARTTLS тест
проверяет обе границы, а Rust unit tests проверяют автоматическое включение и
подавление дубликатов.
RFC 9051 также требует UID в FETCH updates, вызванных UID STORE. Независимый
STARTTLS тест удаляет первое из двух писем и проверяет прямой ответ UID STORE
для второго: wire содержит sequence number 1, UID 2, FLAGS и ровно один UID.
`BODY` и `BODYSTRUCTURE` поддерживаются как отдельные запросы; FULL раскрывается
в BODY согласно RFC 9051, а не в BODYSTRUCTURE.
`BODYSTRUCTURE` включает RFC 9051 body-MD5 slot и корректный RFC 2183
Content-Disposition extension для scalar/message частей; multipart начинает
extensions с parameter list после subtype. Параметры включая RFC 2231 filename декодируются общим MIME parser-ом;
невалидное или повторное поле не экспортируется. RFC 2231 percent escapes
принимают HEXDIG в любом регистре согласно регистронезависимой ABNF. Независимый
Python `imaplib` через implicit TLS выполнил APPEND и BODYSTRUCTURE FETCH с
RFC 2231 UTF-8 filename, содержащим lowercase и uppercase escapes, и прямым
UTF-8 в quoted MIME-параметре `name`; BODYSTRUCTURE вернул оба Unicode значения
и подтвердил сохранность исходных RFC822 октетов. Прямая форма использует
RFC 6532 quoted-string extension и не заявляет SMTPUTF8 transfer.
Отдельный независимый SMTP STARTTLS -> IMAP STARTTLS сценарий проверяет
segmented RFC 2231 `filename*0*`/`filename*1*` с `UTF-8'en-US'`: BODYSTRUCTURE
возвращает объединённое Unicode имя `😀 résumé.txt`, а RFC822.HEADER сохраняет
исходные сегменты и language tag byte-exact. Это подтверждает данный wire
вектор, но не актуальную registry validity BCP 47 и не Litterkeep app.
T329 отдельно проверил конфликт regular `filename=` и extended `filename*=` в
исходном письме независимым Linux STARTTLS клиентом: BODYSTRUCTURE не публикует
конфликтующее значение, а RFC822 FETCH возвращает исходные байты. Litterkeep
app interop не подтверждено. RFC 2045 §5.1 и RFC 2231 §7 требуют, чтобы имя,
`=` и значение parameter шли без пробельных октетов между ними: MIME parser
отвергает SP/HTAB до или после разделителя в regular, extended и continuation
формах. Пробелы после `;` и RFC 822 comments отдельно сохраняют допустимую
семантику. Unit и независимый IMAP wire acceptance подтверждают безопасный
fallback для Content-Type, отбрасывание повреждённого filename при сохранении
валидного Content-Disposition в BODYSTRUCTURE и побайтную сохранность исходного
RFC822. Это не заявляет полную
совместимость со всеми MIME клиентами или interoperability приложения Litterkeep.
RFC 2183 §2.8 говорит игнорировать нераспознанные параметры disposition.
Следующий wire-вектор добавляет неизвестный RFC 2231 charset в `filename*` и
известный `size=12`: сервер исключает только имя с неподдерживаемым charset,
сохраняет disposition и SIZE в BODYSTRUCTURE и возвращает исходное RFC822 письмо
побайтно. Это ограниченный acceptance неизвестного параметра, не заявление о
полной registry coverage MIME charset.
Тот же раздел требует трактовать неизвестный disposition type как `attachment`
и сохранять смысл его параметров. Парсер BODYSTRUCTURE применяет этот fallback
в типизированном MIME представлении, не переписывая исходный заголовок письма.
Unit-проверки фиксируют fallback и сохранение filename; до отдельного wire
acceptance эта узкая проверка не считается клиентской interoperability.
RFC 2183 §2.7 задаёт `size` как unquoted непустые десятичные цифры. MIME parser
исключает только некорректный `size` и сохраняет соседние параметры; unit и
независимый IMAP STARTTLS вектор проверяют нецифровую, quoted и RFC 2231 формы
рядом с валидным filename.
RFC 2183 §§2.4-2.6 date-параметры теперь публикуются только как quoted значения
с корректной Gregorian date, допустимым временем, числовой timezone и согласованным
weekday. Один strict RFC 5322 baseline parser используется также для IMAP SEARCH;
unit и независимый IMAP STARTTLS acceptance покрывают все три параметра и
отбрасывание неверной даты без потери filename; `:60` leap-second также
проверяется и сохраняется в projection, тогда как календарный SEARCH сохраняет
тот же день, используя представимое значение секунды. Это не полная RFC 822
date-time грамматика: некоторые obsolete формы остаются непокрытыми.
RFC 5322 §4.3 numeric-zone obsolete year forms now use the specified pivot:
two-digit years 00-49 map to 2000-2049, 50-99 to 1950-1999, and three-digit
years add 1900. Unit vectors cover both pivot edges and 123 -> 2023; independent
IMAP STARTTLS BODYSTRUCTURE acceptance verifies a commented year `97` and keeps
the original RFC822 octets unchanged. Named obsolete zones remain rejected by
the RFC 2183 numeric-timezone requirement; this does not close all obsolete
date-time grammar.
RFC 5322 §3.3 lexical widths now constrain day to one/two ASCII digits and
hour/minute/second to exactly two; seconds remain optional and `60` is accepted.
Unit and independent IMAP STARTTLS BODYSTRUCTURE regressions reject wider day,
one-digit clock fields and a three-digit hour while retaining the valid adjacent
filename; original message octets remain unchanged.
RFC 822 CFWS comments in quoted RFC 2183 date parameters are removed before
calendar validation; nested comments between date-time components are accepted,
while an unterminated comment invalidates the parameter. Unit tests and an
independent Python IMAP STARTTLS peer verify the projected date and exact original
RFC822 octets. This does not implement every obsolete date-time placement or
relax the required numeric timezone.
RFC 5322 Date search has a distinct profile from RFC 2183 MIME date parameters:
`UT`/`GMT` and the eight registered North American obsolete zones are normalized
to their RFC offsets; military zones use `-0000` as recommended without out-of-band
data, and unknown 3-5-letter alphabetic names use `-0000` per RFC guidance. The
complete A-I/K-Z military-zone range is unit-tested in both cases; reserved `J`
and malformed/overlong names are rejected. RFC 2183 continues to reject every
alphabetic zone. Unit tests and independent IMAP STARTTLS peers verify UTC day
boundaries, the exact `SENTON` result, MIME date omission, and byte-exact RFC822
FETCH. This is not support for unknown historical zone spellings or Litterkeep app
interoperability; see [RFC 5322 §4.3](https://www.rfc-editor.org/rfc/rfc5322.html#section-4.3)
and [RFC 2183 §2](https://www.rfc-editor.org/rfc/rfc2183.html#section-2).
RFC 2231 §4.1 допускает charset/language metadata в начале первой encoded
continuation секции. Ранее `Content-Disposition` parser терял language hint
после общего параметрического разбора; `Disposition` теперь сохраняет typed
BCP 47 tag рядом с декодированным значением, включая при построении MIME tree.
Unit-регрессии проверяют оба уровня. IMAP BODYSTRUCTURE намеренно не расширен
нестандартным полем и продолжает сериализовать только декодированное значение
параметра согласно своему wire-профилю.
Внешняя whitespace-обрезка structured MIME значений ограничена ASCII SP/HTAB:
Unicode NBSP перед media type, параметром, unquoted value и disposition token
отвергается, а NBSP внутри quoted parameter сохраняется как данные. Регрессия
до фикса воспроизводилась на NBSP перед `text/plain`; после фикса прошли
параметрические и disposition unit tests, Windows 569 library + 1 binary test,
fmt и strict Clippy, Linux Docker release build, strict Clippy, 620 library +
1 binary test и doc-tests. `tests/blackbox/mime_structured_wsp.py` через
независимый Python `imaplib` APPEND/FETCH подтвердил MIME fallback для NBSP в
Content-Type, сохранение quoted NBSP и непринятие NBSP перед disposition token,
Content-Language и Content-Location; NBSP в unstructured Content-Description
сохранён. Исходные RFC822 literal bytes совпали. Это не проверка совместимости
Litterkeep.
`Content-Language` сохраняет
один или несколько структурно корректных BCP 47 tags; `Content-Location`
сохраняется как единственный непустой unfolded URI-reference по синтаксису
RFC 3986. Поддерживаются absolute URI и relative reference; percent-encoding
проверяется, но не декодируется. Не выполняются разрешение ссылок, загрузка,
проверка допустимости схем, IRI/Unicode URI или реестр URI. Невалидное либо
повторное поле не экспортируется. Поля сериализуются строго после disposition
в RFC 9051 порядке.
`BODY` использует отдельный basic encoder и не получает extension fields.
Независимый STARTTLS `imaplib` acceptance сравнил wire-ответы `BODY` и
`BODYSTRUCTURE`: первый не содержит extensions, второй содержит language,
location и RFC 2231 attachment filename. Оба профиля возвращают MIME basic
поля `Content-ID` и `Content-Description`. Описание декодируется по RFC 2047;
Content-ID принимает единственное поле в формах dot-atom, quoted id-left и
domain-literal id-right, включая UTF-8 atext по RFC 6532. RFC 5322
`obs-id-left`/`obs-id-right` CFWS вокруг dot-separated atoms нормализуется;
obsolete local-part также принимает последовательность atom/quoted-string
words с точками вне кавычек. Comments и whitespace вокруг quoted/atom words
и точек семантически невидимы. Контрольные octets и malformed quoted-pairs
остаются невалидными. Повторные или синтаксически некорректные значения не
проецируются.
Независимый STARTTLS acceptance подтвердил quoted id-left, IPv6 domain literal,
obsolete CFWS и quoted-word формы, включая comments, в обоих профилях; UTF-8
identifiers и RFC 2047 description проверены unit-тестами. Прочие protocol fields и RFC
пробелы остаются непокрытыми.
Единственный корректный `Content-MD5` выводится в scalar/message body-MD5 slot
после строгой Base64 проверки и проверки длины декодированного digest в 16
октетов. Значение передаётся без пересчёта и сервер не проверяет совпадение с
текущими body octets. Повторы или некорректный digest пропускаются. Поле не
попадает в `BODY` и не занимает multipart extension slot; независимый STARTTLS
acceptance подтвердил обе wire-проекции.
`BODY` поддерживает полное сообщение, HEADER/TEXT, HEADER.FIELDS/NOT, числовые
MIME paths, вложенный `message/rfc822` и partial ranges; durable `\\Seen`
различает BODY/RFC822, PEEK и EXAMINE. `NAMESPACE`, `UNSELECT` и `ENABLE`
рекламируются как доступные команды текущего IMAP4rev1 профиля. Независимый
STARTTLS клиент подтвердил, что ENABLE игнорирует неизвестный capability,
возвращает пустой `ENABLED` и не меняет CAPABILITY. Сетевая проверка также
подтвердила отказ от quoted capability и сквозное совпадение байтов вложения;
отдельная grammar-проверка получает tagged BAD для `ENABLE X%`, сохраняя
игнорирование корректного неизвестного atom `X-UNKNOWN`;
проверки BODY/Seen, headers, body, flags и UNSELECT подтверждены отдельно.
RFC 9051 BINARY, BINARY.PEEK и BINARY.SIZE декодируют MIME CTE до
partial slicing; BINARY применяет literal8 при NUL, нормализует текст к CRLF
и выдаёт tagged NO [UNKNOWN-CTE] для неизвестного кодирования. Независимый
STARTTLS acceptance отправил вложение через SMTP и сравнил все байты BINARY;
также проверены SIZE, decoded partial, Seen/PEEK и UNKNOWN-CTE. Это не проверка
клиента Litterkeep. BINARY включён в CAPABILITY в plaintext, TLS и authenticated
состояниях; отдельный STARTTLS wire acceptance проверил точный capability atom
до переключения, после него и после LOGIN. Это закрывает только discovery для
уже реализованного RFC 3516 extension; остальные требования IMAP4rev2 остаются
непокрытыми.
Для IMAP4rev1 совместимости `RFC822.HEADER` возвращает заголовки под legacy
data-item label и не меняет `\\Seen`; `RFC822.TEXT` возвращает точные body
octets под своим label и durable устанавливает `\\Seen`. Независимый
SMTP-to-IMAP acceptance проверяет labels, bytes и Seen-переходы.
SEARCH использует UTF-8 по умолчанию и поддерживает явные UTF-8/US-ASCII;
неподдерживаемая кодировка возвращает `NO [BADCHARSET ...]`, а Unicode-criteria
при US-ASCII отвергаются ответом NO. `BODY`/`TEXT` декодируют Base64 и
quoted-printable текстовые MIME parts с US-ASCII, UTF-8, ISO-8859-1,
Windows-1251 или Windows-1252; UTF-16, UTF-16BE и UTF-16LE также реализованы
для RFC 2047 encoded words и RFC 2231 extended parameters, но не используются
для поиска в `text/*` body parts согласно RFC 2781. RFC 2047 encoded words
декодируются в outer и
MIME-part headers только как полные токены в начале field-body или после LWS.
Embedded и слитные формы остаются raw text; это новое правило пока unit-only.
По RFC 2047 §2 delimiter lookahead теперь ограничен 75 символами каждого
encoded-word. Для 32 768 oversized malformed starters debug unit-test ранее
занимал 5.21 секунды, после исправления 0.04 секунды на той же Windows-среде;
Linux test-stage повторил сам вектор за 0.04 секунды. Это bounded-lookahead и
unit coverage, не измерение production throughput или внешнего клиента.
RFC 2231 `charset*language` suffix encoded-word сохраняется отдельно в typed
API, тогда как header/phrase projections возвращают decoded text. Новые
language suffix grammar cases unit-only.
RFC 2231 extended parameters сохраняют BCP 47 language metadata в MIME tree;
проверяется синтаксис RFC 5646, но не актуальная registry validity. Проверки
metadata и допустимого опущенного charset пока unit-only.
RFC 2047 encoded-word и RFC 2231 extended parameter также декодируют UTF-16,
UTF-16BE и UTF-16LE с проверкой byte order, нечётной длины и surrogate pairs.
Начальный BOM у UTF-16 выбирает порядок и удаляется как signature; без BOM
используется big-endian после проверки первых двух октетов; generic UTF-16 с
меньшим числом исходных октетов отклоняется. RFC 2781 регистрирует семейство UTF-16 как
неподходящее для MIME `text/*`, поэтому SEARCH не декодирует такие body parts.
Профиль проверен unit-тестами; отдельный socket acceptance не запускался.
Независимый STARTTLS
тест подтвердил, что BODY не ищет outer Subject и бинарное
application/octet-stream вложение, а TEXT включает decoded outer headers.
Неизвестная charset и невалидная transfer encoding не сравниваются как raw
octets. Часть с неизвестным Content-Transfer-Encoding рассматривается как
непрозрачный `application/octet-stream` согласно RFC 2045 §6.4: её тело не
попадает в текстовый SEARCH projection, а исходные MIME-заголовки сохраняются.
Независимая SMTP STARTTLS -> IMAP проверка подтвердила fallback type в
BODYSTRUCTURE, сохранение исходных заголовков и отсутствие совпадения SEARCH
BODY. IMAP BINARY для такой части завершается `NO [UNKNOWN-CTE]` согласно
RFC 9051; тот же клиент проверяет response code.
MIME tree отвергает Base64, quoted-printable и неизвестный CTE на всех
multipart/message composite entities согласно RFC 2045 §6.4; повторное поле
CTE отклоняется как неоднозначное, поэтому позднее 8bit не маскирует ранее
указанное кодирование. Multipart delimiter collection ограничен остатком
глобального MIME part budget до накопления диапазонов, а не только последующей
проверкой дерева. При разборе вложенной MIME-части внешний multipart delimiter
завершает текущую внутреннюю multipart entity даже при отсутствии её close
delimiter; политика распространяется на любую глубину вложения через
multipart и message/rfc822. Для верхнего multipart без closing delimiter
сохраняется отказ. Проверки parser-а unit-only.
MIME Base64 CTE по RFC 2045 §6.8 игнорирует октеты вне Base64-алфавита,
сохраняя строгую проверку синтаксиса RFC 2047 encoded-word. Unit-проверки
покрывают оба профиля. Независимый SMTP STARTTLS -> IMAP STARTTLS клиент
доставляет multipart с `!#` внутри Base64 body и сверяет точные исходные octets
через `BINARY.PEEK`; encoded-word decoder этим сценарием не затрагивается.
Общий MIME header-block parser удаляет CRLF только у реальных строк заголовка,
сворачивает continuation с WSP и отвергает отдельный CR внутри обычной или
folded строки. RFC 5322 folding требует CRLF перед WSP; регрессии bare CR
проверены unit-тестами, отдельный socket acceptance не запускался.
Имена полей валидируются по [RFC 5322 §3.6.8](https://www.rfc-editor.org/rfc/rfc5322.html#section-3.6.8) `ftext`: печатный US-ASCII без `:`;
легальная пунктуация extension field names сохраняется, а пробелы, control и
не-ASCII в имени отвергаются. Независимый STARTTLS клиент через APPEND/FETCH
подтвердил, что MIME parsing принимает punctuation extension field и сохраняет
полный RFC822 literal до байта. Это не проверка Litterkeep app.
RFC 2045 MIME-поля (кроме Content-Disposition) допускают RFC 822 comments;
Content-Transfer-Encoding удаляет вложенные comments вне quoted-string перед
разбором token. Сам CTE value ограничен одним RFC 2045 ASCII token: по краям
принимаются только SP/HTAB, внутренние separators и Unicode whitespace
отвергаются; неизвестный корректный ASCII token остаётся typed UNKNOWN-CTE.
Unit-регрессия воспроизводит прежнее принятие NBSP и проверяет отказ после
исправления. Независимый ManageSieve CHECKSCRIPT подтвердил те же границы в
vacation `:mime`: comments принимаются, NBSP отвергается. Unit-тест и независимый
SMTP STARTTLS -> IMAP STARTTLS
acceptance проверили вложенный comment перед Base64, escaped right parenthesis,
comment после token, BODYSTRUCTURE encoding и точные декодированные octets.
Исходные RFC822 bytes при доставке сохранены без нормализации.

MIME parser и IMAP BODYSTRUCTURE распознают RFC 6532 `message/global`.
`7bit`, `8bit` и `binary` используют исходные байты, Base64 и
quoted-printable декодируют вложенное сообщение в отдельный Arc-owned source,
чьи offsets разрешаются относительно этого source. ENVELOPE, BODYSTRUCTURE,
nested BODY HEADER и BINARY part traversal работают на decoded содержимом;
unknown CTE не разбирается как письмо, а остаётся opaque
`application/octet-stream`. Вектора проходят unit-тесты, но отдельный network
black-box `tests/blackbox/imap_message_global.py` теперь выполняет APPEND и
проверяет на независимом STARTTLS клиенте BODYSTRUCTURE, сохранение Base64
wire-body через BODY[1], UTF-8 вложенные BODY[1.1]/BODY[1.HEADER], decoded
BINARY[1.1] и неизменность исходного RFC822 literal. Прогон выполнен на Linux
Rust 1.97 Docker runtime с отдельным каталогом `./data/g18-message-global-wire`;
повторён под `ENABLE UTF8=ACCEPT` в `./data/g19-rfc9755-message-global-enabled`.
В обоих случаях вложенная структура и все octets совпали. Litterkeep app не
проверялось. Это acceptance конкретного media type, а не полный сертификат MIME
или IMAP4rev2. RFC 9755 §6 допускает для IMAP4rev1 с включённым UTF8=ACCEPT как
`message/rfc822` representation, так и RFC 3501 representation; текущий ответ
`MESSAGE/GLOBAL` остаётся валидным. См.
[RFC 6532 §3.5](https://www.rfc-editor.org/rfc/rfc6532.html#section-3.5),
[§3.7](https://www.rfc-editor.org/rfc/rfc6532.html#section-3.7)
и [RFC 9051 §9](https://www.rfc-editor.org/rfc/rfc9051.html#section-9).
Quoted-printable decoder удаляет буквальные SP/TAB на концах hard lines и
объекта, не удаляя закодированные `=20`/`=09`; encoder сериализует пробелы и
TAB как hex-octets, чтобы перенос строки не менял содержимое. Независимый SMTP
STARTTLS -> IMAP STARTTLS BINARY.PEEK подтвердил удаление SP/TAB в hard-line
padding и сохранение decoded `=20`. SP/TAB перед soft break `=CRLF` являются
содержимым `qp-segment` и сохраняются, тогда как SP/TAB после последнего
символа hard line удаляются как transport padding. Unit-регрессии проверяют
обе позиции; новый большой-входной вектор проверяет 8 KiB непрерывного WSP
перед hard CRLF и soft break. Decoder обрабатывает каждый пробельный участок
одним проходом, избегая квадратичного повторного lookahead. Отдельные
[RFC 2045 §6.7](https://www.rfc-editor.org/rfc/rfc2045.html#section-6.7)
векторы проверяют нормативные примеры и полное тело. Для исходящего
Sieve vacation отдельный
ManageSieve -> SMTP submission -> durable queue -> независимый SMTP peer
сценарий подтвердил UTF-8 round-trip, кодирование буквальных SP/TAB как
`=20`/`=09` и физические строки не длиннее 76 октетов. Проверялся именно
независимый peer, не Litterkeep. В этом пути также обнаружено и исправлено
добавление лишнего пустого CRLF outbound SMTP transport-ом, если MIME payload
уже завершался CRLF; обычный и TLS transport теперь используют общую запись
DATA. Проверка существующего Litterkeep клиента и других MTA остаётся открытой.
При повреждённом quoted-printable `=` escape входящий decoder сохраняет
буквальный `=` и следующий символ без преобразования, как рекомендует RFC 2045
§6.7 для robust decoder. Генераторский Sieve `:mime` validator использует
отдельный strict путь и malformed escape отвергает. Независимый SMTP STARTTLS
клиент передал `=G1` и неполный `=A`, а IMAP STARTTLS BINARY.PEEK подтвердил
точные декодированные octets с сохранёнными escape. Независимый ManageSieve
CHECKSCRIPT принял корректный quoted-printable soft break, включая SP перед
`=CRLF`, и отверг malformed hex escape и незакодированные завершающие SP/HTAB.
RFC 2045 §6.7 decoder boundary теперь также проверяет все 256 октетов как
`=HH` и как robust lowercase `=hh`, а также все 93 literal-safe ASCII значения
из диапазонов 33..60 и 62..126. Это exhaustive octet mapping, не полное
покрытие line-length, transport, malformed-input и MIME interoperability
профиля. Отдельная fail-closed политика отклоняет raw C0 control octets
(кроме TAB), DEL и high-bit octets, поскольку RFC 2045 §6.7 разрешает их только
в `=HH` форме. Ошибка BINARY projection передаётся как `NO [UNKNOWN-CTE]`, а
исходный RFC822 literal остаётся доступен без `\\Seen`. Независимый TLS IMAP
peer APPEND-ит malformed QP literal с NUL, VT и 0xFF, проверяет этот response
code, последующий NOOP и неизменённые bytes через `BODY.PEEK[]`. Rust
регрессии проверяют каждый запрещённый octet и сохранение UNKNOWN-CTE.
Malformed `=` escape по-прежнему имеет отдельную tolerant recovery policy;
эта проверка не означает полное покрытие quoted-printable профиля и не
проверяет доставку таких octets через SMTP или совместимость приложения.
Внутренний QP encoder для vacation-response также сверяется со всеми
256 отдельными входными октетами и смешанным полным диапазоном: round-trip,
hard CRLF и предел 76 символов проверены. Escape token теперь собирается в
фиксированные stack bytes вместо форматирования временной heap-строки для
каждого байта; output Vec при необходимости всё ещё может перераспределяться.
`7bit` и `8bit` CTE проверяют профиль RFC 2045 §§2.7-2.8: не более 998
октетов между CRLF, отсутствие NUL и одиночных CR/LF; `7bit` дополнительно
отвергает high-bit octets, `8bit` их сохраняет. `binary` остаётся прозрачным.
Независимый IMAP STARTTLS APPEND -> BINARY.PEEK принял 998-октетную строку
7bit и отверг 999-октетную, сохранил high-bit octets для 8bit и отверг их для
7bit. Проверены NUL для обоих профилей и bare CR/LF. Для последних использован
прямой IMAP literal: стандартный `imaplib.append()` нормализует line endings
до отправки и не подходит для этих malformed fixtures. Это проверка IMAP
хранения и MIME decode, не SMTP DATA acceptance.
Параметр `multipart` boundary проверяется по RFC 2046 §5.1.1: 1..70 ASCII
октетов из `bchars`, без завершающего пробела; покрыты длины и запрещённые
символы unit-тестами. Parser принимает transport-padding у opening/closing
delimiter и исключает CRLF, непосредственно предшествующий первому delimiter,
из preamble; собственный CRLF preamble сохраняется. Сканирование останавливается
на первом close-delimiter, поэтому boundary-похожая строка внутри epilogue не
создаёт часть и не расходует part budget. Независимый Python `imaplib`
STARTTLS-клиент отправил padded delimiters и epilogue с последующим boundary-
похожим MIME-заголовком; BODYSTRUCTURE содержит только ожидаемые две части,
BODY[1] и BINARY.PEEK вложения совпали, а RFC822 FETCH подтвердил точные исходные
bytes, включая preamble и epilogue. Это проверка IMAP APPEND/FETCH, не SMTP
доставки и не клиента Litterkeep.
Вложенная MIME-часть без Content-Type в `multipart/digest` получает default
`message/rfc822` по RFC 2046 §5.1.5; само вложенное сообщение использует
`text/plain` default для своего body. В остальных multipart подтипах отсутствие
Content-Type по-прежнему даёт `text/plain`. Типизация и вложенные ranges
проверены unit-тестами. MIME body-part с пустой областью заголовков теперь
принимается: первая CRLF завершает header area, а последующие октеты точно
сохраняются как body. Unit-проверка BODYSTRUCTURE подтверждает вложенные
`MESSAGE/RFC822`, ENVELOPE, `TEXT/PLAIN` и размеры исходных octets для digest
part. Парсер и протокольная проекция проверены unit-тестами; socket acceptance
также выполнен независимым STARTTLS `imaplib` peer: после APPEND BODYSTRUCTURE
содержит `DIGEST` и вложенные `MESSAGE/RFC822`/`TEXT/PLAIN`, а RFC822 FETCH
побайтно совпадает с headerless digest source. Это проверка независимым
клиентом, не приложением Litterkeep.
Второй независимый STARTTLS FETCH проверил RFC 3501 §6.4.5 / RFC 9051 §6.4.5
для этого embedded message: `BODY.PEEK[1]` возвращает RFC822-содержимое вложения,
`[1.HEADER]` и `[1.TEXT]` разделяют его заголовок и текст, `[1.1]` адресует
единственную MIME body-part. Все четыре ответа совпали с точными ожидаемыми
octets; клиент Litterkeep не запускался.
Для того же дерева исправлен traversal RFC 9051 `BINARY`: `multipart/digest`
теперь считается multipart-контейнером, а numeric path после его
`MESSAGE/RFC822` child продолжается от embedded MIME entity. Независимый STARTTLS
FETCH подтвердил `BINARY.PEEK[1.1]` декодированный body и `BINARY.SIZE[1.1]` с
совпадающей длиной. Ранее этот путь завершался `NO`; SMTP и клиент Litterkeep не
проверялись.
Unit matrix также проходит embedded `MESSAGE/RFC822`/`MESSAGE/GLOBAL` leaves в
`multipart/mixed`, `multipart/alternative`, `multipart/related` и
`multipart/digest`. Дополнительный независимый Linux STARTTLS peer APPEND-ит все
эти subtype variants и проверяет BODYSTRUCTURE, `BINARY.PEEK[1.1]` после
quoted-printable decode и совпадающий `BINARY.SIZE[1.1]`. Вариант
`message/global` проверяется как child `multipart/mixed`; остальные используют
`message/rfc822`, а digest child остаётся headerless. Это покрытие именно этих
numeric paths, не полная MIME или IMAP rev2 interoperability и не приложение
Litterkeep.
По RFC 9051 §6.4.5 partial offset/count для `BINARY` применяются к уже
декодированным section octets. Ранний wire probe `<2.4>` вернул `ssag` и origin
`<2>`, но первые четыре байта совпадают с encoded source prefix и сами по себе
не отличали порядок decode/slice; этот результат не считается доказательством
этого порядка. Усиленный probe для всех пяти fixtures запросил offset `8`,
попадающий внутрь quoted-printable `=20`, и получил `bo`; дополнительно проверены
offset `0`, частичное усечение у конца (`dy`), ровно EOF, offset за EOF и count,
превышающий остаток. Проверка подтвердила декодированные offsets и пустой literal
в двух EOF-случаях.
В writable mailbox этот же независимый клиент проверяет, что полный и partial
`BINARY.PEEK` оставляют `\Seen` unset. Partial count `0` отвергается tagged `BAD`
как для `BINARY.PEEK`, так и для `BODY.PEEK`; после каждого отказа `NOOP` проходит,
то есть IMAP-сессия остаётся пригодной. Это подтверждение RFC 9051 §6.4.5 для
указанных FETCH форм, не проверка клиента Litterkeep.
Для смешанного `FETCH (BODY[] BINARY[1])` найдено состояние, где `BODY[]` может
успешно прочитаться, но `BINARY[1]` завершается `NO [UNKNOWN-CTE]`. До исправления
reactor записывал `\Seen` перед вычислением всех items, поэтому неуспешный FETCH
мог менять mailbox. Теперь сериализация каждого выбранного сообщения проверяется
до durable Seen mutation; Linux regression test подтверждает, что такой набор не
ставит отметку. Независимый Linux STARTTLS клиент отправил смешанный FETCH,
получил NO [UNKNOWN-CTE], выполнил NOOP и затем подтвердил по FLAGS, что
\Seen остался unset. Это acceptance независимого клиента, не приложения
Litterkeep.
RFC 9051 §6.4.5 рекомендует включать FLAGS в FETCH-ответ, если BODY fetch
изменил флаги сообщения. После durable Seen update сервер теперь добавляет
актуальный FLAGS item, только если клиент сам его не запросил. Независимый
STARTTLS тест подтверждает, что BODY.PEEK не добавляет FLAGS, первый BODY
возвращает FLAGS с \Seen, а повторный BODY без нового перехода не добавляет их.
Отдельный BINARY acceptance по RFC 3516 проверяет тот же FLAGS transition для
decoded leaf data: BINARY.PEEK не ставит Seen, BINARY возвращает точные decoded
октеты и FLAGS с Seen, повторный BINARY не добавляет FLAGS без нового изменения.
Для RFC 9051 §6.4.5 partial диапазоны у HEADER.FIELDS и HEADER.FIELDS.NOT
применяются после формирования подмножества header. Независимый STARTTLS тест
проверяет оба selector-а на fixtures с разным порядком полей: offset 0 возвращает
Subject после предшествующего невыбранного поля для HEADER.FIELDS и From после
исключённого Subject для HEADER.FIELDS.NOT. Проверены exact bytes и response
origin <0>; тест не заменяет остальные FETCH cases или полную rev2 совместимость.
Отсутствующий или синтаксически повреждённый `Content-Type` получает
рекомендуемый RFC 2045 §5.2 fallback `text/plain`/US-ASCII; сырой MIME header
не переписывается. Независимый STARTTLS клиент отправил оба варианта через SMTP
и проверил по IMAP BODYSTRUCTURE effective `TEXT/PLAIN`, исходный RFC822 и
сохранение malformed header без нормализации. Для двух полей Content-Type
применяется рекомендуемый [RFC 2045 §5.2 fallback](https://www.rfc-editor.org/rfc/rfc2045.html#section-5.2): MIME-проекция становится `TEXT/PLAIN`, оригинальные поля остаются в RFC822. Независимый STARTTLS SMTP→IMAP сценарий подтвердил BODYSTRUCTURE и точный RFC822 round-trip; US-ASCII декодирование body остаётся unit-only.
RFC 5322 address groups разбираются в типизированные start/member/end элементы:
FETCH ENVELOPE выдаёт group marker (`NIL NIL group NIL`), mailbox участников
и end marker (`NIL NIL NIL NIL`), включая пустую группу. SEARCH FROM/TO/CC/BCC
использует эту же проекцию и находит имя группы и адрес участника. Независимый
STARTTLS black-box проверил группу с тремя mailbox, пустую Cc-группу и Bcc
member. В address projection вложенные comments удаляются с учётом quoted-pair,
скобки внутри quoted display name сохраняются; ENVELOPE и SEARCH не видят текст
комментариев. Независимый STARTTLS acceptance проверил From/To comments и
групповую адресацию. Разделитель addr-spec выбирается вне quoted local-part и
domain-literal: независимый STARTTLS acceptance подтвердил ENVELOPE с
`"a@b"` в mailbox-компоненте и `example.test` в hostname, а SEARCH нашёл этот
адрес. Завершающий CFWS после `;` группы допускает комментарий и не включает
его в ENVELOPE; тест также отвергает mailbox после уже закрытой группы.
Obsolete source-route в угловом адресе игнорируется по RFC 5322 §4.4:
ENVELOPE и SEARCH используют только route destination. `obs-domain-list`
сохраняет допустимые пустые позиции после запятой и требует `@domain` у каждого
непустого route-элемента; посторонний токен без `@` отклоняется. Unit tests
проверяют несколько route domains, пустые позиции и ошибочные элементы.
Независимый wire-клиент для отрицательных случаев не запускался. Другие obsolete
address forms и несовпадения RFC 5322 по-прежнему ограничивают parser.
Там же поддержан CFWS вокруг точек в obsolete local-part/domain: wire
ENVELOPE и SEARCH сворачивают `first (comment) . last @ example . test` к
`first.last@example.test`. Quoted local-part и domain-literal из этой
нормализации исключены и проверяются отдельными unit fixtures. Для quoted
display-name отдельно проверено снятие quoted-pair: `\"` и `\\` в исходном
header становятся буквальными `"` и `\` в ENVELOPE.
Quoted display-name отвергает неэкранированную внутреннюю DQUOTE, сохраняя
decode корректного quoted-pair. Это подтверждено unit regression; независимый
wire-сценарий для некорректного display-name не запускался.
При разборе angle-addr scanner держит `@` и `>` внутри квадратного
domain-literal вне addr-spec и angle-разделителей. Unit regression проверяет
сохранение обоих символов в ENVELOPE; независимый wire-клиент не запускался.
Comment removal также сохраняет скобки и содержимое внутри domain-literal, где
они являются `dtext`, и отвергает unmatched closing bracket. Это покрыто unit
тестом совместно с `@` и `>`; полный допустимый набор `dtext` не проверен.
Внутри domain-literal scanner не интерпретирует круглые скобки как comments;
проверены и balanced, и unbalanced dtext, а obsolete quoted-pair может защитить
закрывающий `]`. Это unit-only покрытие отдельных байтов, не полного набора
`dtext`.
`normalize_dot_cfws` распознаёт escaped `]` внутри obsolete literal и не сворачивает
следующий ` . ` как адресный CFWS. `strip_comments` проверяет dtext и сохраняет
исходную quoted-pair до этой нормализации. Parser regression покрывает форму с
trailing comment; независимое wire-подтверждение этого сочетания отсутствует.
Проверка `dtext` принимает видимые ASCII-символы, разрешённые пробелы,
obs-NO-WS-CTL и UTF-8 non-ASCII по RFC 6532. Unit regression покрывает все
диапазоны obs-NO-WS-CTL и отдельно подтверждает, что NUL/CR/LF допускаются
только через obsolete quoted-pair. ENVELOPE unit test проверяет synchronizing
literal с точной длиной для каждого из NUL, CR и LF.
RFC 5322 obs-qp допускает quoted NUL/CR/LF в исходном поле, но quoting
backslash семантически невидим. Независимый Python imaplib STARTTLS APPEND/FETCH
для формы без comments наблюдал synchronizing literal с 11 октетами и без
backslash; parser regression также проверяет escaped closer после comment
stripping. Это не полный набор interoperability сценариев. Полный набор
domain-literal форм и interoperability с Litterkeep не подтверждены.
После закрывающей `>` angle-addr parser разрешает только CFWS, включая
комментарии, и отвергает произвольный хвост или второй angle-addr вместо
молчаливого усечения адреса. Unit regression проверяет оба отказа и сохранение
допустимого trailing comment; независимый wire-клиент для этого случая ещё не
запускался.
RFC 2047 encoded-word распознаётся как слово только в unquoted display-name
phrase. Пробел между соседними encoded-word не выводится; испорченное слово
остаётся видимым текстом и не блокирует сообщение; quoted display-name не
декодируется. Независимый STARTTLS клиент подтвердил ENVELOPE и FROM SEARCH
для encoded phrase, а также буквальное содержимое quoted-name. В phrase
контексте Q encoding ограничен ASCII letters/digits и `!*+-/=_`; encoded
punctuation допустим, а сырой запрещённый знак оставляет слово видимым.
Границы allowlist проверены unit-only, не независимым клиентом.
CFWS и разделители phrase обрабатываются только как ASCII SP/HTAB: RFC 6532
не-ASCII символы, включая NBSP, в display-name не схлопываются в обычный пробел.
Независимый STARTTLS ENVELOPE acceptance подтвердил точное сохранение NBSP в
UTF-8 display-name; полный набор интернационализированных address forms ещё не
покрыт.
Для obsolete address-list проверены leading/repeated/trailing null members:
они не формируют ENVELOPE addresses и не сдвигают порядок двух mailbox.
`RETURN (SAVE)` сохраняет UIDs в состоянии одной сессии, а `$` SEARCH criterion
сопоставляет их как для SEARCH, так и UID SEARCH. Успешный SELECT, SAVE-команда
с ответом NO, EXPUNGE и MOVE сбрасывают/синхронизируют результат; обычный SEARCH
его не меняет. При одном SAVE ESEARCH response подавляется, а SAVE с MIN/MAX
сохраняет соответствующий край. Независимый клиент проверяет сохранение,
UID-проекцию, отсутствие результата в другой сессии и reset после SELECT/error.
`$` поддерживается для FETCH/STORE, COPY/UID COPY и UID EXPUNGE; пустое
сохранённое множество проверено как валидный непересекающийся набор. Reactor
исполняет следующую команду после tagged completion текущей, поэтому зависимая
команда видит SEARCH SAVE. Независимый STARTTLS клиент отправляет SEARCH SAVE и
COPY `$` подряд, до чтения ответов, затем подтверждает ровно одно скопированное
письмо. На этом основании CAPABILITY объявляет ESEARCH и SEARCHRES.
Явный `SEARCH RETURN` выдаёт ESEARCH с correlator, UID-маркером и опциями
ALL/COUNT/MIN/MAX. Пустой result set сохраняет запрошенные поля: ALL/MIN/MAX
не выводятся, COUNT выводится как 0 только если запрошен, ESEARCH response
при этом остаётся. Пустой `RETURN ()` эквивалентен ALL. Legacy SEARCH остаётся
для текущего IMAP4rev1 профиля. MIME
charset вне проверенного набора остаются непроверенными. Независимые Linux
STARTTLS SMTP -> IMAP acceptance дополнительно доставил 8BITMIME text body с
`charset=windows-874`, после чего Bob запросил `SEARCH CHARSET UTF-8 BODY` со
synchronizing literal `ภาษาไทย`; результат включил доставленное сообщение.
Это доказывает Windows-874 decoding в IMAP MIME text projection и его передачу
в evaluator. Другой сценарий проверил 29 однобайтовых body charset: ISO-8859-1..10
и 13..16, TIS-620, ISO-8859-11, IBM866, KOI8-R/U, Windows-874 и Windows-1250..1258. KOI8-R
и KOI8-U mappings unit-tested для всех 256 octets по RFC 1489 и RFC 2319;
независимый Python codec проверил русский и украинский текст в SMTP -> IMAP
SEARCH цепочке. Это не доказывает остальные charset. В частности, известные
зарегистрированные многобайтовые семейства GBK/GB18030 теперь используют
строгий собственный decoder без runtime зависимости: GBK и GB18030 labels идут
через WHATWG GB18030 state machine, двухбайтовый index и 207 four-byte ranges.
Индекс содержит 23 940 указателей; Rust тест проверяет каждую позицию, а
независимая SMTP -> IMAP acceptance проверяет Python `gbk` для `简体中文` и
`gb18030` для `中文𠀀😀`. Таблицы воспроизводятся
`tools/generate_gb18030_tables.py`; скрипт закрепляет SHA-256 официальных
WHATWG index-файлов и не участвует в runtime/build. Эта проверка не закрывает
Big5/Big5-HKSCS теперь также используют собственный строгий decoder и плотный
O(1) WHATWG index lookup. Все 19 782 pointer positions сверяются с декодером;
четыре нестандартных одиночных pointer-а корректно разворачиваются в пары
Unicode scalars. Независимый SMTP -> IMAP charset matrix проверяет обычный Big5
текст и все четыре expansion mappings через Python `big5hkscs` codec.
Первичные
источники: [WHATWG GBK decoder](https://encoding.spec.whatwg.org/#gbk-decoder),
[two-byte index](https://encoding.spec.whatwg.org/index-gb18030.txt) и
[four-byte ranges](https://encoding.spec.whatwg.org/index-gb18030-ranges.txt),
[WHATWG Big5 decoder](https://encoding.spec.whatwg.org/#big5-decoder),
[Big5 index](https://encoding.spec.whatwg.org/index-big5.txt) и
[IANA charset registry](https://www.iana.org/assignments/character-sets/).
EUC-JP добавлен отдельным strict decoder: ASCII, SS2 half-width katakana,
двухбайтовый JIS X 0208 и SS3/JIS X 0212 читаются без runtime-зависимости.
Обе WHATWG таблицы закреплены по SHA-256; runtime acceptance проверяет японский
текст, kana и отдельные JIS X 0212 знаки. Unit suite сравнивает все 8 836
wire-позиции каждой ветки с соответствующей таблицей. На этом этапе ещё
неподдержанными были ISO-2022-JP, Shift_JIS и EUC-KR.
Первичные источники: [WHATWG EUC-JP decoder](https://encoding.spec.whatwg.org/#euc-jp-decoder),
[JIS X 0208 index](https://encoding.spec.whatwg.org/index-jis0208.txt) и
[JIS X 0212 index](https://encoding.spec.whatwg.org/index-jis0212.txt).
ISO-2022-JP теперь декодируется конечным автоматом WHATWG с typed ASCII,
Roman, Katakana и JIS X 0208 состояниями; пары JIS используют уже закреплённую
таблицу X 0208. Некорректные и неполные escape/pair формы отклоняются, а MIME
текст и каждый RFC 2047 encoded-word должны завершаться в ASCII. Независимый
SMTP -> IMAP acceptance проверил японский body и ISO-2022-JP RFC 2047 Subject
через отдельные BODY и SUBJECT criteria. Katakana designation входит в
WHATWG compatibility decoder, но RFC 1468 явно исключает kana из собственного
mail profile; поэтому это не заявление полного RFC 1468 conformance. Тесты
сверяют encoded-word с ASCII reset и отказ без него. Источники: [WHATWG
ISO-2022-JP decoder](https://encoding.spec.whatwg.org/#iso-2022-jp-decoder),
[RFC 1468](https://www.rfc-editor.org/rfc/rfc1468.html) и [RFC 2047 §3](https://www.rfc-editor.org/rfc/rfc2047.html#section-3).
Shift_JIS добавлен как строгий decoder WHATWG §12.3.1: ASCII и U+0080,
half-width kana, диапазоны lead/trail, JIS X 0208 index и Windows EUDC
private-use mapping U+E000..U+E757. Тест сверяет все 11 280 синтаксически
допустимых двухбайтовых pointers с JIS index либо EUDC mapping и все 256
одиночных octets. Независимый SMTP -> IMAP STARTTLS acceptance использует
Python `shift_jis` codec и подтверждает поиск японского текста после доставки.
Приложение Litterkeep этим независимым клиентом не проверялось. Источники:
[WHATWG Shift_JIS decoder](https://encoding.spec.whatwg.org/#shift_jis-decoder),
[IANA Character Sets registry](https://www.iana.org/assignments/character-sets/).
EUC-KR добавлен как строгий decoder WHATWG §13.1. Он использует закреплённый
index с 23 940 pointer slots, ASCII и пары lead/trail по алгоритму стандарта;
undefined mappings, malformed и truncated ввод приводят к ошибке. Acceptance
использует независимый Python `euc_kr` codec и проверяет `안녕하세요` через SMTP
STARTTLS -> IMAP UTF-8 SEARCH. WHATWG decoder принимает расширенный набор
Windows-949 lead/trail форм, поэтому эта проверка не утверждает только RFC 1557
94x94 wire grammar или совместимость с приложением. Источники: [WHATWG EUC-KR
decoder and index](https://encoding.spec.whatwg.org/#euc-kr-decoder),
[RFC 1557](https://www.rfc-editor.org/rfc/rfc1557.html) и
[IANA Character Sets registry](https://www.iana.org/assignments/character-sets/).
RFC 1556 MIME labels ISO-8859-6-E/I и ISO-8859-8-E/I распознаются отдельными
typed variants, применяя соответствующую base octet mapping для IMAP SEARCH.
Suffix E обозначает explicit, I implicit bidi presentation. Сервер не выполняет
ни контрольную explicit presentation, ни implicit bidi layout algorithm и не
заявляет визуальный bidi renderer; примеры лишь проверяют mapping, сохранение
назначенных control code points и прохождение SMTP -> IMAP SEARCH. Источники:
[RFC 1556](https://www.rfc-editor.org/rfc/rfc1556.html),
[RFC 2046 §4.1.2](https://www.rfc-editor.org/rfc/rfc2046.html#section-4.1.2) и
[IANA Character Sets registry](https://www.iana.org/assignments/character-sets/).
IANA aliases добавлены как отдельные перечисленные варианты только для уже
поддерживаемых ISO-8859 профилей: `iso_8859-*`, Latin/Greek/Hebrew/Arabic
имена и `csISO*`; Windows-1251/1252/1255/1256 проверяются через IANA `cs`-
aliases, а Windows-1252 также через RFC 1345 alias `cp1252`. Unit tests
проверяют каждый добавленный alias и отклоняют незарегистрированный `x-cp1252`.
Это не полная
поддержка IANA Character Sets registry: неподдерживаемые charset остаются
ошибкой и не угадываются по префиксу.
RFC 3629 регистрирует MIME label `UTF-8`; parser принимает его и alias `csUTF8`,
который также указан реестром IANA, используя тот же строгий Rust decoder.
Независимая SMTP STARTTLS -> IMAP SEARCH матрица передает alias в MIME body и
находит сообщение; всего проверено 42 charset labels. Rust unit test фиксирует
обе границы: `csUTF8` принимается, `utf8` отклоняется как незарегистрированное
имя. Сервер не нормализует произвольные dash/underscore/case формы и добавляет
только подтвержденные registered aliases. Это не проверка взаимодействия с
Litterkeep приложением. Источники: [RFC 2978 §2.3](https://www.rfc-editor.org/rfc/rfc2978.html#section-2.3),
[RFC 3629 §8](https://www.rfc-editor.org/rfc/rfc3629.html#section-8) и
[IANA Character Sets registry](https://www.iana.org/assignments/character-sets).
Отдельная
SMTP -> IMAP wire-проверка доставила RFC 2047 Subject encoded-word варианты UTF-16,
UTF-16BE/LE и UTF-32 с generic BE BOM, UTF-32BE/LE; UTF-8 SEARCH literal
нашёл ровно одно соответствующее письмо для каждого label. Это подтверждает
данный набор encoded-word -> ENVELOPE SEARCH сценариев, а не все MIME charset,
поля заголовков или Litterkeep interoperability. SEARCH поддерживает
ASCII quoted strings и synchronizing literals до 64 KiB; независимый STARTTLS
тест проверил BODY, UID, UTF-8, пустой literal и два последовательных literals
в HEADER с отдельным continuation для каждого; pipelined command после literal
сохраняется. Parser acceptance перечисляет все базовые SEARCH keys RFC 9051,
evaluator покрыт соответствующими unit и сетевыми сценариями. Объявление
SEARCHRES не означает полной
совместимости IMAP4rev1 или IMAP4rev2: остаются SEARCH/ESEARCH расширения и
другие несовпадения формальных грамматик; см. [матрицу IMAP](imap-profile.md).
`SENTBEFORE`/`SENTON`/`SENTSINCE` извлекают дату из RFC 5322 `Date` header;
внешние и weekday CFWS trim ограничен SP/HTAB, поэтому NBSP не делает
malformed дату поисково-валидной. Независимый STARTTLS `imaplib` APPEND/SEARCH
подтвердил SENTON match только для одной валидной даты из пяти, при четырёх
NBSP-вариантах заголовка. Это не расширяет обработку obsolete Date syntax.
Нормы: [RFC 3501 §6.4.5](https://www.rfc-editor.org/rfc/rfc3501.html#section-6.4.5),
[RFC 5322 §3.3](https://www.rfc-editor.org/rfc/rfc5322.html#section-3.3).

## POP3

Поддерживаются STLS и implicit TLS, USER/PASS, STAT, LIST, UIDL, RETR, DELE,
RSET и QUIT. Maildrop lease исключает двух разрушающих владельцев; DELE
фиксируется только успешным QUIT.
Для RFC 8314 есть отдельный implicit-TLS listener `serve-pop3-tls`;
`POP3_SSL` ранее подтвердил отсутствие plaintext greeting до TLS handshake.
После успешного POP3 QUIT session cleanup дожидается отправки `+OK`, затем
инициирует TLS `close_notify` и освобождает socket после flush alert согласно
рекомендации RFC 8314 §3.4. Независимый TLS socket acceptance проверяет порядок
ответа и успешный `unwrap()` клиентом. При peer-initiated `close_notify` TLS
transport сохраняет close state; reactor сначала обрабатывает уже полученные
POP3-команды и дожидается pending Store completion, затем дренирует ответы,
инициирует ответный alert и освобождает ресурс через общий lifecycle cleanup.
Независимый implicit-TLS peer аутентифицировался и захватил maildrop, отправил
alert без QUIT, завершил `unwrap()`, затем второй клиент успешно захватил тот же
maildrop; отдельное соединение проверило QUIT reply и ответный alert. Это
проверка двух POP3 closure путей и освобождения maildrop lease, не всех причин
закрытия TLS для остальных протоколов. См.
[RFC 8314 §3.4](https://www.rfc-editor.org/rfc/rfc8314.html#section-3.4).
AUTHORIZATION FSM требует `USER` перед `PASS`, отклоняет повторный `USER`
после положительного ответа, а после отклонённого USER или failed PASS очищает
имя и требует новую команду USER. Независимый STLS клиент проверил этот порядок с ошибочным и затем
успешным паролем; это не подтверждает прочие authentication mechanisms.
RFC 1939 §5 разрешает `NOOP` только в TRANSACTION: сервер отклоняет его в
AUTHORIZATION и после USER, не теряя незавершённую пару USER/PASS; после входа
отвечает `+OK`. Unit FSM и независимый STARTTLS peer проверяют оба отказа,
восстановление аутентификации и последующий NOOP в TRANSACTION.
`USER` принимает ровно один непустой username, а `PASS` сохраняет все октеты
аргумента после первого разделителя, включая внутренние и крайние пробелы.
RFC 1939 §3 задаёт максимум 40 символов для каждого аргумента команды;
сервер принимает USER/PASS ровно на 40 печатных ASCII октетах и отвергает
41-й, при этом сохраняет пробелы внутри значения PASS. Username больше 40
октетов ранее ошибочно принимался лишь из-за общего предела RFC 2449 в 255
октетов для всей строки. RFC 1939 §7.3 разрешает трактовать пробелы `PASS` как
часть пароля. Unit и независимый STARTTLS peer проверяют границы и продолжение
USER/PASS после отказа; существующая Windows unit regression сверяет exact
password bytes в typed auth request;
независимый STARTTLS peer подтверждает, что неверный пароль с пробелами дошёл
до credential owner и последующий `USER` восстанавливает AUTHORIZATION.
CAPA включает bounded RFC 2449 командную грамматику: parser требует CRLF,
printable ASCII и одиночные пробелы; лимит команды составляет 255 октетов
вместе с CRLF. Независимый STLS клиент принял 255, отверг 256 и проверил
синхронизацию последующих команд и успешную аутентификацию. Status/greeting и
каждая строка CAPA/LIST/UIDL ограничиваются 512 октетами вместе с CRLF;
независимый STLS acceptance проверил эти ответы и сквозную доставку SMTP->POP3.
`RESP-CODES` сообщает о расширенных кодах ответов; на успешную аутентификацию
при занятом другим POP3-сеансом maildrop сервер отвечает `-ERR [IN-USE]`.
Независимая проверка удерживала первую lease и подтвердила код для PASS второго
клиента в AUTHORIZATION; capability проверена в обоих состояниях.
Новый независимый Linux STARTTLS peer прочитал полный CAPA multiline frame в
AUTHORIZATION до TLS, после TLS, после успешного USER и в TRANSACTION. Теги
`USER`, `UIDL`, `TOP`, `RESP-CODES`, `UTF8` и `LANG` сохранялись во всех
четырёх состояниях; `STLS` заменялся на `SASL PLAIN` после TLS. Это подтверждает
эту таблицу CAPA-состояний, но не полный профиль RFC 2449/5034 и не клиента
Litterkeep.
`LIST n` и `UIDL n` возвращают одиночный `+OK` с номером и данными; без аргумента
обе команды используют multiline frame. LIST octet size считает CRLF-представление
сообщения и не включает дублированные dot-stuffing октеты. Независимый Linux
клиент проверил обе формы, сравнил LIST размер с RETR после удаления dot-stuffing;
unit-тест отдельно подтвердил LF-only raw message нормализацию.
UIDL строится из mailbox entry UID, а не из изменяемого заголовка Message-ID.
Store сохраняет allocator в журнале и не перераспределяет UID после EXPUNGE.
Регрессия открывает новую POP3-сессию после закрытия и повторного открытия Store,
сверяет точные UIDL octets, затем удаляет первое сообщение и подтверждает, что
следующее письмо получает новый идентификатор. Это unit evidence для стабильности
UIDL поверх durable Store. Дополнительный независимый Linux STARTTLS peer
T295 доставляет два письма через SMTP, закрывает POP3-соединение без `QUIT`,
перезапускает сервер на том же bind-mounted `./data` каталоге и сверяет UIDL.
Затем он удаляет первое письмо через POP3 `DELE`/`QUIT`, доставляет третье через
SMTP и проверяет, что UID оставшегося письма не изменился и новый UID не совпал
с прежними. Это протокольное acceptance независимым Python-клиентом, не проверка
интероперабельности с приложением Litterkeep.
POP3 нормализует CRLF только как пару: одиночный CR в сохранённом RFC822
сообщении остаётся частью данных и учитывается в LIST size. Регрессия проверяет
байты RETR/TOP и размер для CR перед обычным символом и в конце последней строки.
После DELE отмеченное сообщение исключается из STAT totals и многострочных
LIST/UIDL, а одиночные LIST n/UIDL n для него возвращают `-ERR`; номера других
сообщений не перенумеровываются в текущем snapshot. RSET восстанавливает
видимость, успешный QUIT durable удаляет только отмеченный UID. Повторный
`DELE n` до RSET также возвращает `-ERR`, как требует RFC 1939 §5. Unit и
двухсообщенческий SMTP->POP3 black-box проверяют RFC 1939 §§5-6.
RETR/TOP отдельно ограничивают status, сохраняя допустимые длинные строки тела:
проверено точное чтение 700-октетной строки. Это ограниченный набор проверок,
а не сертификация остальных требований RFC 1939/2449 или совместимости клиента.
Независимый Linux STARTTLS acceptance проверяет RFC 1939 TOP: `TOP 1 0`
возвращает headers с разделяющей пустой строкой без body, `TOP 1 1` ровно одну
строку, а запрос больше длины короткого body возвращает всё тело. Сценарий
передаёт `.first` и `..second` через SMTP; byte-exact POP3 результат одновременно
проверяет снятие SMTP и POP3 dot-transparency по одному октету на границе.

`AUTH PLAIN`, `AUTH SCRAM-SHA-1` и `AUTH SCRAM-SHA-256` поддерживаются только
после TLS: до STLS механизмы отсутствуют в CAPA, после него сервер рекламирует
`SASL PLAIN SCRAM-SHA-1 SCRAM-SHA-256` и убирает `STLS`. PLAIN поддерживает initial response и
continuation, отмену через `*`, строгую Base64-проверку и сохранение SASL
capability после входа. SCRAM-SHA-1 использует canonical Base64, общий
SASLprep query профиль и Accounts-owned verifier lookup; client proof проверяется
до передачи отдельного Base64 server-final challenge, за которым следует
обязательный пустой client response и обычный `+OK` без дополнительного payload.
Независимый Linux STARTTLS peer проверяет успешный mutual proof, неверный proof,
обе формы client-first, отказ plaintext CAPA, CAPA после TLS, `-ERR [IN-USE]`
при занятом maildrop, восстановление соединения и повторный захват lease после
QUIT. Отдельный peer проверяет PLAIN initial/continuation и CAPA по состояниям.
SCRAM-SHA-256 использует свой SHA-256 verifier sidecar через Accounts owner,
выполняет mutual proof и не запрашивает maildrop lease до пустого SASL
acknowledgement. Отдельный Python STARTTLS peer независимо проверяет SHA-256
proof/server signature и сохраняет предыдущие SHA-1, неверный proof, lease
conflict/reuse сценарии. Отдельный peer с проверкой CA завершил тот же
SCRAM-SHA-256 exchange по implicit TLS, включая CAPA, STAT и QUIT. Это не
проверка Litterkeep client. SCRAM-SHA-256-PLUS рекламируется только поверх
завершённого TLS 1.3 с RFC 9266 exporter; GS2 ограничен `p=tls-exporter,,`, а
canonical `c=` проверяется как точная GS2 header и exporter до proof.
Независимые Rustls Linux peers проверяют implicit TLS и STARTTLS: сертификат,
pre/post-upgrade CAPA, exporter, PLUS mutual proof, server signature, STAT и
QUIT. Peer повторно использует локальные SHA/HMAC/PBKDF2 helpers и не является
независимым crypto oracle. Остальные SASL
mechanisms не заявляются; полный RFC 5034 профиль не заявляется.
Во время незавершённого `AUTH PLAIN` единственная команда отмены SASL exchange
это строка `*`. Любой иной клиентский ответ обязан быть Base64; обычный SMTP
текст `NOOP` и повторный `AUTH` не исполняются как вложенные команды, получают
501, завершают неудачный AUTH и оставляют соединение доступным для следующей
команды. Независимый STARTTLS peer проверяет обе строки, последующий `NOOP 250`
и успешный новый continuation login. См. [RFC 4954 §4](https://www.rfc-editor.org/rfc/rfc4954.html#section-4).
Единственный `=` в initial-response позиции разбирается как пустой ответ согласно
RFC 5034, а не как некорректный Base64. Для PLAIN пустой payload отклоняется как
невалидные credentials; независимый TLS socket тест подтвердил повтор AUTH после
отказа и последующий вход с верным payload.
По RFC 5034 §4 неудачный AUTH оставляет сессию так, как если бы AUTH не
выполнялся. Unit-проверки подтверждают сохранение ранее выбранного USER при
ошибочном SASL payload; независимый Linux STARTTLS acceptance
`tests/blackbox/pop3_sasl_prior_user.py` подтверждает сохранение USER Bob после
отказа не рекламируемого `DIGEST-MD5` с корректным Base64 initial response
(T370), пустого и некорректного initial response,
cancellation, некорректного continuation и backend-отказов для неверных
credentials/аккаунта. Затем допустимый 40-октетный PASS доходит до проверки
credentials и возвращает `authentication failed`, после чего независимый PLAIN
вход завершается успешно. Отдельная проверка отправляет валидный Base64 PLAIN
continuation размером 348 октетов (с 256-октетным паролем), больше POP3 лимита
команды 255; он доходит до проверки credentials, а не отвергается как длинная
команда. Acceptance peer использует initial-response только если вся AUTH
команда с CRLF укладывается в 255 октетов; длинный payload переключает на пустой
`+ ` challenge и continuation согласно RFC 5034 §4. Это подтверждает данный
размер и тестовый клиентский выбор, не реальное приложение и не максимальный
SASL payload. continuation длиннее 1 MiB теперь ограниченно отбрасывается до
CRLF, завершается `-ERR authentication failed` и не теряет следующую pipelined
команду; Linux unit-тесты покрывают split CRLF, а независимый STARTTLS peer
проверил payload 1 MiB + 1 и последующий PASS с сохранённым USER. Это задаёт
серверный предел отклонения, а не подтверждает приём любого возможного PLAIN
payload, поэтому полного RFC 5034 соответствия по-прежнему нет. Отдельный
STARTTLS peer проверяет валидный PLAIN continuation с 4096-байтовым паролем,
верхней границей SCRAM verifier проекта: Base64 response занимает 5468 октетов,
доходит до проверки credentials и получает ожидаемый отказ на неверном пароле.
Это подтверждает эту настройку PLAIN, не максимальный SCRAM nonce и не все
теоретические ответы поддерживаемых механизмов по RFC 5034 §4. Секреты
берутся из локального fixture, а не передаются
в argv. Этот сценарий закрывает перечисленные ветви восстановления, не полный
RFC 5034 профиль и не поддержку других SASL-механизмов.

Частично закрыт размерный профиль RFC 5034 §4. SCRAM attribute parser больше
не задаёт конечный размер, POP3 SASL continuation обходит общий 1 MiB input
budget, а challenge в SASL phase обходит generic MAX_OUTPUT. Входной и
выходной Vec используют fallible reserve; если накопление входа не удаётся,
сервер дренирует текущую строку до CRLF и завершает AUTH отказом.

Независимый T409 TLS peer передаёт 2 MiB client nonce, принимает такой же
длинный server nonce, независимо вычисляет client proof, проверяет server
signature и доходит до maildrop (`STAT`, `QUIT`). Base64 строки больше бывшего
1 MiB continuation ceiling. Ранее T375/T378/T379 проверяли 16/256/700 KiB,
а T401 подтверждал успешную аутентификацию на 700 KiB. Это реальная точка
2 MiB, не доказательство всего диапазона. Base64 decode и сохранённый
client-first state всё ещё требуют память,
пропорциональную сообщению; allocator exhaustion не прогонялся. В T410
attribute parser переведён на borrowed slices, final parser хранит unsigned
transcript как срез, а AuthMessage копируется только один раз в выходной буфер.
Регрессия pointer identity подтверждает отсутствие копии отдельного 2 MiB
атрибута. В T411 POP3 и ManageSieve также передают владение decoded client-first
строкой; GS2 prefix сдвигается на месте, а nonce становится диапазоном внутри
сохранённого bare transcript, без отдельной копии. 2 MiB pointer-identity test и
повторный независимый TLS proof acceptance проходят. Это сокращает копии
transcript-а, но peak-memory footprint всего обмена не измерялся. RFC 5802
§5.1 не задаёт максимальную длину nonce, поэтому T377 теперь PARTIAL, а не
PASS; крайние размеры, ограничение совокупной памяти и отказ allocator-а
требуют дальнейшего отдельного аудита.

После успешного AUTH `CAPA` в TRANSACTION сохраняет строку `SASL` и список
механизмов, как требует RFC 5034 §3, несмотря на запрет повторного AUTH из §4.
Это подтверждают session unit test и независимый STARTTLS peer после успешного
PLAIN входа. Проверен указанный переход, а не весь профиль POP3.

Для SCRAM client-first проекта (256 KiB) может породить более длинный
client-final из-за серверного nonce suffix и обязательных SCRAM полей. Отдельный
bounded лимит client-final это учитывает; T375 проверил предыдущую границу
16 KiB + 72 октета, а T378 подтвердил client-final 256 KiB + 72 октета до
проверки proof. Это снимает конфликт размеров на проверенных границах, но не
подтверждает неограниченные SASL payload или полный RFC 5034 профиль.

RFC 6856: capability `UTF8 USER` объявляется в обоих состояниях. Параметр
`USER` разрешает UTF-8 в аргументах USER/PASS до и после команды `UTF8`;
число символов credentials ограничено 40 по RFC 1939, а вся POP3 строка
ограничена 255 октетами по RFC 2449. Независимый Linux STARTTLS peer проверил
многобайтные USER/PASS до переключения, USER после переключения и восстановление
через AUTH PLAIN с последующим STAT в той же сессии. Это не проверка реальной
клиентской реализации. `UTF8` принимается в RFC-состоянии AUTHORIZATION согласно
§2.1, а `LANG` в AUTHORIZATION и TRANSACTION согласно §3.1. Положительный `USER` оставляет POP3
сессию в AUTHORIZATION до успешного PASS/AUTH; внутренняя фаза `UserAccepted` не
является отдельным wire state. Поэтому обе команды допустимы между USER и PASS.
Они сохраняют pending USER, что проверяется последующим корректно сформированным
PASS до backend-аутентификации. Успешная команда UTF8 включает raw UTF-8 message
mode и запрещает последующий STLS.
Аргументы USER/PASS проходят SASLprep RFC 4013 query profile; создание account
записывает prepared identity по stored profile, а общий локальный route выдаёт
такой же ключ для логина и SMTP-доставки. Новые acceptance credentials теперь
состоят из 20 случайных октетов в hex (40 символов), чтобы POP3 PASS мог их
передать. Повторный init не меняет уже сохранённые secrets: прежние
48-символьные тестовые пароли остаются длиннее POP3 PASS limit и требуют
отдельной смены пароля для USER/PASS входа.
До переключения `LIST`, `TOP` и `RETR` возвращают
ошибку для любого high-bit octet и не отправляют такое сообщение как ASCII;
`[UTF8]` используется, когда обнаружена корректная non-ASCII UTF-8
последовательность, а непрозрачные high-bit octets получают обычный `-ERR`.
Независимый Linux SMTP->POP3 socket acceptance отправил 8BITMIME UTF-8 письмо,
получил `[UTF8]` от LIST/TOP/RETR в ASCII mode, затем включил UTF8 до USER и
сравнил точные байты RETR. Отдельный IMAP APPEND с единственным `0xFF` проверил
обычный отказ для LIST/TOP/RETR до UTF8; unit-тест подтвердил, что этот октет
не выдаётся и после переключения UTF8, поскольку содержимое не является
валидным UTF-8 и legacy charset conversion не реализован. Новый independent
STARTTLS сценарий после переключения получает `-ERR` для opaque octets и
byte-check-ит RETR валидного UTF-8 письма. Acceptance также проверил CAPA
до/после TLS и отказ STLS после UTF8. MIME-дерево
рекурсивно обнаруживает `Content-Transfer-Encoding: binary`, включая вложенные
части всех `multipart/*`; POP3 `TOP` и `RETR` отклоняют такое сообщение даже в
UTF8 mode. Независимый Linux acceptance сохранил fixture через IMAP APPEND и
подтвердил отказ обеих команд. Если ограниченный MIME parser не может проверить
структуру, `TOP` и `RETR` также завершаются `-ERR`, а сообщение сохраняется в
maildrop. Unit и независимый Linux acceptance проверяют оба отказа для обеих
команд и сохранение сессии. Это fail-closed политика для malformed или
неподдержанной MIME-структуры и она может ограничить POP3-доступность таких
сообщений; IMAP и файловое хранение не затрагиваются. Остальные ограничения RFC
6856 сохраняются: legacy charset conversion. Создание нового аккаунта проверяет
старые credential keys и отказывает при найденном SASLprep alias, не выбирая
один из конфликтующих владельцев. Автоматическая миграция старых account и
mailbox keys, имя которых меняется при SASLprep, не реализована; такие legacy
identities нельзя считать проверенными после обновления. Повреждённые
raw octets сами по себе не считаются UTF-8
строками. `LANG` объявляется и реализует listing единственной локали `en`, выбор
`en` и серверной локали через `*`; корректный, но неподдерживаемый basic language
range получает `-ERR`. Так как поддержана только английская локаль, выбор языка
не меняет тексты ответов. Многоязычная локализация, остальные RFC 6856 сценарии
и интеграция с реальным клиентом не проверялись.
Независимый STARTTLS peer проверил RFC 6856 `LANG` в AUTHORIZATION и
TRANSACTION, обе выдачи CAPA, listing, `EN`, `*`, отказ `de` и возможность
продолжить transaction-команды после неуспешного выбора языка.

## Sieve и ManageSieve

Актуальный проверенный профиль и его ограничения описаны отдельно в
[sieve-coverage.md](sieve-coverage.md), чтобы сохранять компактность этой
матрицы.
В частности, независимый Linux socket acceptance проверяет RFC 5804
capability transitions ManageSieve вокруг STARTTLS и AUTHENTICATE, а отдельный
peer проверяет NOOP до аутентификации и обязательный echo response code `TAG`
для quoted string. `UNAUTHENTICATE` сбрасывает состояние и owner, сохраняет TLS
и допускает повторный AUTHENTICATE; независимый TLS peer это проверяет.
Implicit-TLS SCRAM-SHA-1 и SCRAM-SHA-256 wire exchanges независимо сверяют proof
и server signature. После STARTTLS независимый peer проверяет пустой список SASL
до TLS, публикацию `PLAIN SCRAM-SHA-1 SCRAM-SHA-256` и полный SHA-256 proof plus
server-signature exchange после TLS upgrade. Capability также объявляет
`MAXREDIRECTS "16"`, синхронизированный с пределом evaluator-а и проверенный
unit boundary test-ом.
SCRAM-SHA-256 использует отдельный verifier sidecar и bounded Accounts owner
lookup. TLS 1.3 `tls-exporter` primitive сверяется между независимыми rustls
endpoints с параметрами RFC 9266 и подключён к typed ManageSieve
SCRAM-SHA-256-PLUS: capability появляется только при доступном exporter,
GS2 допускает только `p=tls-exporter,,`, а `c=` сверяется с canonical Base64
точной шапки и exporter до проверки proof. Есть FSM тесты корректного proof,
server signature и несовпадающего exporter. Независимое PLUS подключение по
живому TLS socket завершено отдельным Rustls peer-ом: подтверждены CA chain,
TLS 1.3, capability, RFC 9266 binding и mutual SCRAM signature. Peer повторно
использует SHA/HMAC/PBKDF2 helpers этой реализации, поэтому не является
независимым криптографическим oracle. Это не проверка Litterkeep app, полный
профиль RFC 5804 ещё не закрыт.

См. [Sieve/ManageSieve coverage](sieve-coverage.md) и
[dependency/security notes](dependencies.md).
ManageSieve mutation quota failures now use the RFC 5804 `NO (QUOTA)` response
code; Store boundary and response-mapping unit tests verify that ordinary disk
errors do not become quota responses. Independent Linux TLS wire acceptance
reaches the 17-script boundary and confirms the failed PUTSCRIPT leaves the
16-entry catalog unchanged. Litterkeep client interoperability remains
unverified.
RFC 5804 §2.8 SETACTIVE is checked over the independent TLS path: empty name
succeeds with no active script, unknown name gets `NO (NONEXISTENT)`,
LISTSCRIPTS shows activation and removes ACTIVE after empty-name deactivation,
and a repeated deactivation succeeds. This is server/store evidence, not
Litterkeep interoperability.
The same TLS acceptance attempts DELETESCRIPT on the active script, expects
`NO (ACTIVE)`, confirms the script remains active, then deletes it successfully
after deactivation, as specified by RFC 5804 §2.10.
RFC 5804 §2.11 rename collision returns `NO (ALREADYEXISTS)` without changing
either script; renaming an active script preserves its ACTIVE designation under
the new name. Store reopen and independent Linux TLS acceptance verify both
properties, including exact script source bytes. Litterkeep interoperability
is not verified.

ManageSieve tokenizer rejects unterminated quoted strings and unsupported
quoted-string escapes as syntax errors instead of accepting a valid command
prefix. Parser tests and the independent Linux wire scenario verify both
negative cases. T328 subsequently connected SCRAM-SHA-1 to ManageSieve TLS
authentication and an independent TLS wire test; STARTTLS and application
interoperability are not established.
NOOP accepts an optional RFC 5804 string as either quoted text or synchronizing
and non-synchronizing client literals. Linux reactor tests cover literal
consumption and pipeline boundaries; independent wire acceptance confirms the
continuation and exact TAG responses. This does not establish literal support
for other ManageSieve command arguments. GETSCRIPT, DELETESCRIPT and SETACTIVE
also accept a final synchronizing or non-synchronizing literal string, including
the zero-length SETACTIVE disable form. Linux reactor tests verify typed command
conversion; independent wire acceptance checks continuation and state handling.
PUTSCRIPT accepts a literal name followed by quoted or literal source, with
sync/non-sync framing and a following pipelined NOOP covered. RENAMESCRIPT
accepts either name as a sync or non-sync literal independently; receiver tests
cover two sync names, mixed quoted/literal names, UTF-8 and pipeline retention.
HAVESPACE accepts a bounded literal name followed by one decimal size; receiver
tests check split input, strict numeric arity and pipeline preservation.
Independent Linux TLS acceptance now verifies successful Store mutation through
literal arguments: PUTSCRIPT receives a non-synchronizing literal name and a
synchronizing source literal, RENAMESCRIPT consumes two non-synchronizing
literal names, and SETACTIVE keeps the renamed script active. After restarting
the same server against the same data directory, LISTSCRIPTS and GETSCRIPT
confirm the active name and exact source bytes. This closes only the tested
literal positions and durable path; other uncovered string positions remain
unsupported, and Litterkeep application interoperability is not claimed.

## Outbound SMTP

Маршрут задаётся явно как IP:port и typed TLS policy. Scheduler durable,
single-flight, применяет bounded retry/backoff и DSN. Pool повторно выдаёт только
чистое соединение после success либо подтверждённого RSET.
После EHLO клиент разбирает multiline reply и typed extension keywords. Для
сообщения с high-bit octets в MIME body отправляется `BODY=8BITMIME`, если peer
рекламировал `8BITMIME`; без capability доставка завершается permanent failure
до `MAIL`. Для валидных UTF-8 header values и Unicode local-part в sender или
recipient envelope клиент требует от peer `SMTPUTF8` вместе с `8BITMIME` и
добавляет `SMTPUTF8` к MAIL. При нескольких адресатах флаг устанавливается,
если хотя бы один forward-path интернационализирован. Только UTF-8 заголовки
дополнительно требуют `BODY=8BITMIME`; ASCII message с Unicode envelope не
получает ненужный BODY параметр. RFC 6531 extension не отправляется, если peer
не объявил обе capabilities. Targeted builder tests проверяют envelope-only
случай и отказ peer без SMTPUTF8; прежний независимый Linux SMTP peer проверял
wire параметры для UTF-8 headers. Сквозной outbound MTA и Litterkeep app не
проверялись.
Outbound SMTP ограничивает каждую строку greeting и reply, включая каждую
строку multiline EHLO, значением RFC 5321 §4.5.3.1.5: не более 512 октетов с
CRLF. Reader не накапливает сверх лимита и закрывает соединение при превышении
или неверном окончании строки. Независимый peer подтвердил приём точной
границы 512 и отказ для 513 октетов в greeting и EHLO до `MAIL`.
Как SMTP SIZE client outbound transport разбирает EHLO `SIZE`, `SIZE 0` и
числовой maximum. При объявленном расширении `MAIL FROM` содержит точное число
передаваемых message octets, включая добавленный terminal CRLF, но без
terminator и dot-stuffing; без объявления параметр не отправляется. Размер
выше фиксированного maximum отвергается до `MAIL`. Независимый peer проверяет
wire command и все три исхода. Implicit-TLS unit peer проверяет совместное
использование `SIZE` и `BODY=8BITMIME`.
Для валидного MIME-дерева с `Content-Transfer-Encoding: binary` relay никогда
не подменяет payload командой DATA/8BITMIME. Если peer не объявляет одновременно
`BINARYMIME` и `CHUNKING`, команда MAIL не отправляется, и job получает
permanent failure согласно допустимой политике [RFC 3030 §3](https://www.rfc-editor.org/rfc/rfc3030.html#section-3).
При наличии обеих extensions используется точное BDAT framing; автоматическое
преобразование в Base64 пока не выполняется.
По RFC 1870 EHLO numeric `SIZE` не ограничен 20 цифрами; это ограничение
применяется к value команды MAIL. Peer limit выше диапазона `u64` сохраняется
отдельным typed случаем, поэтому корректное capability не теряется. Rust и
независимый peer тесты проверяют 21-значный предел и точное MAIL declaration;
см. [RFC 1870 §3](https://www.rfc-editor.org/rfc/rfc1870.html#section-3).
Outbound reply handling теперь привязано к фазе SMTP: только greeting 220,
`DATA` continuation 354, `QUIT` 221 и AUTH success 235 принимаются в этих
контекстах; команда `MAIL`, `RCPT`, финальная DATA completion и reset требуют
positive completion 2yz, при этом 4yz остаётся deferred, а 5yz permanent.
Неожиданный 3yz не продвигает состояние команды. Код каждой строки multiline
reply обязан совпадать с первой. Независимый peer подтвердил, что 354 и 221 в
ответ на MAIL не приводят к отправке RCPT; unit tests проверяют известные
фазовые ответы, будущий 2yz код и смешанные коды multiline.
RFC 5321 §4.2 разрешает клиенту обрабатывать reply code без текста: outbound
reader принимает `250\r\n` и `250 \r\n`, сохраняя трёхзначный numeric code,
CRLF и multiline separator; для будущих числовых кодов исход клиентской
операции выбирается по первому разряду. Отдельный TCP peer прошёл полную отправку с bare
220/250/354/221 ответами; Litterkeep и внешние MTA в этом тесте не участвовали.
Ответ `421` трактуется отдельно от прочих временных отказов: очередь получает
`Deferred`, но соединение считается закрываемым. Plaintext и implicit-TLS pools
не отправляют `RSET` и не переиспользуют такой socket. Независимые SMTP test peers
проверили закрытие после `421` на RCPT и успешную следующую доставку через новое
соединение. Остальные 4yz по-прежнему могут вернуть соединение в pool только
после успешного `RSET`.
Исходящий клиент начинает с `EHLO`; при однострочном `500` «command not
recognized» повторяет идентификацию через `HELO` и работает без ESMTP
capabilities. Policy-отказ `550` не вызывает downgrade. Независимые plaintext и
implicit-TLS peers проверили wire-порядок `EHLO` -> `HELO` -> `MAIL` и доставку
без не объявленных `SIZE`/`8BITMIME` параметров.
Исходящий SMTP использует отдельные socket timeouts по RFC 5321 §4.5.3.2:
300 секунд для greeting/MAIL/RCPT и обычных команд, 120 секунд для ответа на
DATA, 180 секунд на каждую системную отправку DATA и 600 секунд на финальный
ответ после terminator. Пять настроек принимаются в положительных секундах через
`LITTERKEEP_SMTP_*_TIMEOUT_SECONDS`; отсутствие значения оставляет нормативный
default, некорректные значения отклоняются при старте outbound worker-а.
Unit-тест сверяет таблицу фаз и валидирует partial overrides/ошибки; тестовый
transport может задать короткий единый предел, а wire-тест подтверждает timeout
зависшего greeting. Дополнительный независимый TCP peer с короткими phase-specific
overrides удерживает соединение открытым до timeout для greeting, команд EHLO,
MAIL и RCPT, ожидания `354` на DATA и финального ответа после фактически
полученного DATA terminator. Это проверяет шесть реальных точек ожидания.
[RFC 5321 §3.8](https://www.rfc-editor.org/rfc/rfc5321.html#section-3.8) и
[§4.1.1.10](https://www.rfc-editor.org/rfc/rfc5321.html#section-4.1.1.10)
требуют завершать SMTP session командой QUIT, а sender SHOULD ждать reply.
Standalone plaintext и implicit-TLS пути отправляют QUIT после завершённого
SMTP-level результата, если peer не прислал 421 и сам не закрывает канал.
Проверки локальными peers подтверждают `QUIT/221` после успеха; plaintext
также проверяет QUIT после RCPT 450/550. При timeout ответа на QUIT уже
подтверждённый результат DATA сохраняется. TLS peer-тест с 100-ms коротким
timeout проверяет реальный socket; pooled TLS соединение не отправляет QUIT
между повторными транзакциями. При явном shutdown owner отправляет QUIT во все
idle plaintext и implicit-TLS pool sessions, не выполняя блокирующий сетевой
I/O из Drop; локальные peers проверяют QUIT/221 на pool, worker, service и
scheduler shutdown. Ошибка одной idle session не отменяет попытку завершить
остальные и передаётся вызывающему owner. Нормативный 5-минутный wall-clock
timeout, отказ QUIT в одной из нескольких idle sessions и интеграция с внешним
MTA не проверялись.
Для исходящего BDAT промежуточный ответ на каждый chunk теперь
использует отдельный 5-минутный command timeout, а BDAT LAST - 10-минутный
DATA-termination timeout; 3-минутный write timeout сохраняется на каждый chunk.
Unit-тесты проверяют выбор фазы и LAST-маркер. Socket-level применение этих
интервалов проверяется локальным TCP peer: delayed intermediate ACK переживает
отсутствие ответа дольше короткого command timeout и приводит к timeout, тогда
как delayed LAST ACK успешно приходит позже command timeout, но до termination
timeout. Это проверяет plaintext socket с короткими тестовыми интервалами, а не
нормативные минуты на wall clock; отдельный TLS peer для BDAT не запускался.
Отдельный rustls unit peer проверяет переход greeting -> EHLO
для implicit TLS при различающихся greeting/command deadlines.
DATA передаётся потоково без дополнительной копии полного message: RFC-ограниченная
строка формирует bounded fragment, dot-stuffing применяется до записи, а каждый
фрагмент отправляется через `write_all`. Unit writer с partial writes проверяет
границу фрагмента и точные octets; независимый Linux SMTP peer сверил около 2 MiB
payload после dot-unstuffing. Это не измерение peak RSS и не тест максимального
25 MiB сообщения.
# IMAP RFC 9394 PARTIAL checkpoint

`tests/blackbox/imap_partial.py` independently verifies the RFC 9394 extension
over STARTTLS: the server advertises PARTIAL, ESEARCH returns forward and
newest-relative rank ranges (including NIL for a wholly absent range), SAVE
retains the reported subset unless COUNT is also requested, and UID FETCH
PARTIAL selects ranks after UID-set filtering. A BODY FETCH acceptance also
confirms that only returned messages acquire `\\Seen`. The Linux Rust 1.97.1
suite, strict Clippy and release build pass. This is scoped extension evidence,
not full RFC 9051 conformance or Litterkeep application interoperability.
