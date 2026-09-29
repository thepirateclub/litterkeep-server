# Итоговая приёмка

## RFC 3461 SMTP DSN negotiation

T405: после включения RFC 3461 wire capability независимые Python raw-socket
клиенты проверяют `250-DSN` в EHLO, принятие `MAIL RET=HDRS ENVID=...` и
`RCPT NOTIFY=FAILURE ORCPT=...`, завершённую DATA транзакцию, а также отказ тех
же параметров после HELO. Запускались
`tests/blackbox/smtp_dsn_negotiation.py` и
`tests/blackbox/smtp_dsn_refusal.py` против Linux Rust 1.97.1 release сервера
в отдельном Store. Это доказывает ESMTP negotiation и envelope acceptance,
не отправку итоговых DSN через сеть, не downstream relay и не взаимодействие
с Litterkeep app. Более ранние T349/T350 фиксируют старое состояние отказа до
включения capability и не описывают текущий сервер.

T406: собственная реализация теперь публикует failure DSN внешнему исходному
отправителю через durable outbound job с пустым reverse-path. Linux-only тест
проверяет permanent RCPT отказ, сохранённость задания после reopen, второе
SMTP соединение, multipart/report и отсутствие исходного тела при RET=HDRS.
Windows full suite (1053 library + 1 CLI, 4 ignored), strict Clippy и rustfmt
прошли. Сам Linux тест пока не компилировался: доступный Rust container не
содержит рабочего `cc` linker. Следовательно, T406 остаётся PARTIAL, а не
сетевой PASS; Litterkeep app не подключался.

Машиночитаемая матрица находится в `docs/acceptance.tsv`. Статус G12 не
завышается: 44 G12 checks имеют PASS, T23 имеет NOT RUN, T25 имеет BLOCKED.
Отдельный G17 T28 добавлен в ту же матрицу и не меняет оценку G12.

G17 vacation acceptance использовал отдельный Linux Store `data/g17-sieve-vacation-rfc45`.
Независимый клиент установил active vacation script через ManageSieve, передал
три исходных сообщения SMTP и проверил после restart через IMAP, что все три
остались в Bob INBOX. Linux `queue-drain` передал ровно один автоответ
независимому SMTP peer: второй отвечаемый адрес был подавлен по истории, а
письмо с чужим To не вызвало ответ. Peer проверил `MAIL FROM:<>`, RCPT и
Auto-Submitted/To/Subject. Это серверный протокольный acceptance, не проверка
Litterkeep app и не third-party MTA interoperability.

T82 проверяет устаревшую форму SMTP source-route из RFC 5321 §4.1.2 отдельно
для MAIL FROM и RCPT TO. Независимый STARTTLS peer передал quoted локальные
адреса с route-элементами, а независимый IMAP STARTTLS peer подтвердил оба
сообщения в INBOX Bob. Проверка доказывает локальную доставку конечному mailbox;
внешняя пересылка по source-route не выполняется. Это не проверка
взаимодействия с Litterkeep app.

T83 проверяет IPv4 и зарегистрированную tagged IPv6 форму в MAIL FROM и
RCPT TO. Независимый STARTTLS клиент различает malformed literal (`501`) и
синтаксически корректный, но не локальный адрес (`553` для sender и `550` для
recipient); это подтверждает прохождение parser-а без ослабления локальной
маршрутизации или включения relay. В тех же запусках source-routed доставка и
чтение обычного письма Bob через IMAP STARTTLS остаются рабочими. Сценарий:
`tests/blackbox/smtp_mailbox_syntax.py`.

T98 проверяет RFC 5321 §4.2.2: неизвестные SMTP-команды получают 500,
аргумент у распознанной DATA - 501; NOOP после отказов остаётся синхронизирован
в том же соединении. Повторяемый сценарий:
`tests/blackbox/smtp_unknown_command.py`. Проверка независимым raw-TCP клиентом,
не приложением Litterkeep.

G17 outbound SMTP reply parser принимает bare reply code и пустой reply text
по RFC 5321 §4.2. Unit-покрытие проверяет форматы строк; независимый TCP peer
провёл всю delivery транзакцию с пустыми текстами greeting, EHLO, MAIL, RCPT,
DATA, completion и QUIT. Это не тест с Litterkeep или внешним MTA.

T100 проверяет RFC 2045 §6.7: буквальные SP/TAB перед завершающим `=` soft
break сохраняются как содержимое `qp-segment`, а transport-padding перед hard
CRLF удаляется. Независимые SMTP STARTTLS -> IMAP STARTTLS BINARY.PEEK и
ManageSieve STARTTLS CHECKSCRIPT подтвердили точные octets и приём корректной
MIME-формы. Это тесты независимого клиента, не Litterkeep.

T292 проверяет RFC 5322 obs-qp в domain-literal: unit test покрывает диапазоны
obs-NO-WS-CTL, quoted/unquoted NUL/CR/LF; независимый imaplib-клиент
помещает через APPEND тестовое сообщение с quoted NUL и читает ENVELOPE.
Ответ использует IMAP synchronizing literal; его 11 октетов содержат decoded
host без семантически невидимого quoting backslash. Первоначальная проверка
ошибочно считала backslash частью адреса и была исправлена. Это один контрольный
символ и один ENVELOPE путь без комментариев. Дополнительная parser regression
проверяет escaped closer в адресе с trailing comment. Это не полное
interoperability доказательство RFC 5322/9051.

## Чистый G12 прогон

На новом container-local `/data` bootstrap создал четыре аккаунта. Повтор не
изменил passwords, PHC или CA. Seed дал `created=35 existing=0`, повтор и повтор
после process restart дали `created=0 existing=35`. Независимый Python-клиент
подтвердил 35 fixture IDs и SHA-256 восьми decoded attachments.

После Android-отправки независимый STARTTLS IMAP-клиент с полным login Bob нашёл
ровно одно письмо `G12-Litterkeep-UI`. После остановки writer `check` сообщил
5 mailboxes, 37 links, 36 referenced blobs, 0 orphans и 0 pending deliveries.
Финальный backup скопировал 63 файла и 14,381,601 bytes; восстановленная копия
прошла checker. Предыдущий clean backup также подтвердил одинаковые SHA-256
secret-файла и CA.

## Слои evidence

- `server-protocol`: Rust tests и независимые Python wire clients.
- `client-integration`: физический TECNO CH6i и установленный litterkeep.
- `manual-device`: UI-наблюдение без очистки существующих app data/cache.

Litterkeep действительно проверил TLS/CA, подключил Alice, получил каталог
папок и отправил plain message Bob. Attachment UI отсутствовал, а Bob setup не
завершился, поэтому получение и скачивание вложения приложением не заявлены.

## Не выполнено

T23 не запускался, потому что отдельный third-party MTA не был настроен. T25
заблокирован текущим клиентским UI/вторым account setup, а не серверным Python
тестом. Сырые локальные измерения сохранены под `data/g12-clean` и не входят в
Git, поскольку содержат локальный PKI и runtime artifacts.

## Обновление RFC 3461/3464, T412

Старые строки T385/T404 в `acceptance.tsv` являются историческими checkpoint-
ами. Текущее состояние отличается: SMTP DSN negotiation независимо проверена
в T405, а Store и failure serializer сохраняют и выводят отдельную recipient-
группу для каждого eligible исходного RCPT. T412 проверяет две такие группы
unit-регрессией и полным Windows/Linux Rust gate. Независимый сетевой сценарий
с несколькими report-группами ещё не запускался; это не проверка приложения
Litterkeep. Delay DSN и передачу DSN-параметров downstream relay сервер пока
не поддерживает.
