# Покрытие протоколов

## SMTP

Submission поддерживает implicit TLS и STARTTLS, EHLO, AUTH PLAIN, MAIL SIZE,
несколько RCPT, DATA с однократным dot-unstuffing, RSET, NOOP и QUIT. Также
рекламируется RFC 6152 `8BITMIME`: `BODY=8BITMIME` разрешает high-bit octets,
которые сохраняются до IMAP, а без него DATA с такими octets отклоняется после
dot terminator. `BODY=7BIT` и режим без BODY ограничивают содержимое US-ASCII.
До TLS AUTH запрещён. Envelope sender должен принадлежать authenticated owner.
Local route не является open relay.

SMTP session требует HELO/EHLO перед MAIL/RCPT/DATA. HELO включает только базовый
SMTP режим: ESMTP MAIL-параметры, AUTH и STARTTLS требуют EHLO. Успешные HELO и
EHLO сбрасывают незавершённый envelope; это проверяется независимым клиентом.
MAIL во время открытой transaction получает 503 без изменения sender или списка
получателей; прежний конверт остаётся пригоден для DATA. Это правило порядка
RFC 5321 проверено unit FSM-тестом и сквозной SMTP-to-IMAP доставкой.
Общий SMTP path limit для MAIL FROM и RCPT TO равен 256 октетам вместе с
угловыми скобками. Парсер принимает граничный path и отвергает path на 257
октетах; unit и независимые STARTTLS black-box проверяют обе команды.
Для обычного `local-part@domain` парсер принимает local-part до 64 октетов и
отклоняет более длинный в MAIL FROM и RCPT TO. Сетевой тест отделяет syntax
error от последующей проверки sender/recipient policy. Это ограничение длины,
а не утверждение о полной проверке RFC mailbox grammar или SMTPUTF8.
Обычный ASCII dot-atom дополнительно проверяется до policy слоя в обеих
командах: пустые или неверно разделённые точки, пустые доменные labels,
не-ASCII домены и DNS labels длиннее 63 октетов отклоняются как syntax error.
Это ограниченная грамматика: quoted local-parts, domain literals, source routes
и SMTPUTF8 не поддержаны и не объявляются совместимыми.
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
list expansion. HELP выдаёт многострочный список команд, HELP с аргументом
подсказывает вызвать общий список; NOOP игнорирует необязательный параметр.
Независимая проверка подтверждает ответы до и во время активного envelope.

RFC 1870 SIZE объявляется с фактическим фиксированным пределом сообщения.
Единственный корректный `MAIL FROM ... SIZE=n` разбирается до DATA; значение
выше предела получает 552, некорректная грамматика или повтор SIZE получают
501. Неподдерживаемые ESMTP параметры MAIL/RCPT получают 555, а не игнорируются.
Независимый STARTTLS black-box проверяет эти ответы и восстановление сессии.
Если размер тела превышает лимит уже во время DATA, сервер прекращает
накапливать тело, но продолжает потоково читать до завершающей точки; после неё
возвращает 552 и сохраняет синхронизацию команд. Проверено независимым клиентом
на теле размером 25 MiB + 1 байт с последующим NOOP.
Независимый клиент передал UTF-8 MIME text с `Content-Transfer-Encoding: 8bit`
через SMTP и сравнил payload octets после IMAP `BODY.PEEK[TEXT]`. Проверены
также 7BIT rejection, oversized DATA drain и синхронизация последующего NOOP.
Это проверка конкретного high-bit text сценария, не исчерпывающая сертификация
RFC 6152 для всех MIME content и SMTP peer комбинаций. SMTPUTF8 не рекламируется.
Строки DATA ограничены 1000 octets вместе с CRLF.

## IMAP

Реализованы STARTTLS/implicit TLS, LOGIN, AUTHENTICATE PLAIN с SASL-IR, LIST,
LSUB, CREATE/RENAME/DELETE, SELECT/EXAMINE, STATUS, APPEND, SEARCH, FETCH,
BODYSTRUCTURE, MIME sections/partial, STORE, COPY, MOVE, EXPUNGE, UID EXPUNGE,
IDLE/DONE и durable Recent. UIDVALIDITY и UID переживают restart.

`FETCH` принимает одиночные и сгруппированные data items, раскрывает ALL, FAST
и FULL, включает UID в ответы UID FETCH и сохраняет literal как точные октеты.
`BODY` и `BODYSTRUCTURE` поддерживаются как отдельные запросы; FULL раскрывается
в BODY согласно RFC 9051, а не в BODYSTRUCTURE.
`BODY` поддерживает полное сообщение, HEADER/TEXT, HEADER.FIELDS/NOT, числовые
MIME paths, вложенный `message/rfc822` и partial ranges; durable `\\Seen`
различает BODY/RFC822, PEEK и EXAMINE. `NAMESPACE`, `UNSELECT` и `ENABLE`
рекламируются как доступные команды текущего IMAP4rev1 профиля. Независимый
STARTTLS клиент подтвердил, что ENABLE игнорирует неизвестный capability,
возвращает пустой `ENABLED` и не меняет CAPABILITY. Сетевая проверка также
подтвердила отказ от quoted capability и сквозное совпадение байтов вложения;
проверки BODY/Seen, headers, body, flags и UNSELECT подтверждены отдельно.
RFC 9051 BINARY, BINARY.PEEK и BINARY.SIZE декодируют MIME CTE до
partial slicing; BINARY применяет literal8 при NUL, нормализует текст к CRLF
и выдаёт tagged NO [UNKNOWN-CTE] для неизвестного кодирования. Независимый
STARTTLS acceptance отправил вложение через SMTP и сравнил все байты BINARY;
также проверены SIZE, decoded partial, Seen/PEEK и UNKNOWN-CTE. Это не проверка
клиента Litterkeep. Остальные требования IMAP4rev2 остаются непокрытыми.
SEARCH использует UTF-8 по умолчанию и поддерживает явные UTF-8/US-ASCII;
неподдерживаемая кодировка возвращает `NO [BADCHARSET ...]`, а Unicode-criteria
при US-ASCII отвергаются ответом NO. `BODY`/`TEXT` декодируют Base64 и
quoted-printable текстовые MIME parts с US-ASCII, UTF-8 или ISO-8859-1; RFC 2047
encoded words декодируются в outer и MIME-part headers. Независимый STARTTLS
тест подтвердил, что BODY не ищет outer Subject и бинарное
application/octet-stream вложение, а TEXT включает decoded outer headers.
Неизвестная charset и невалидная transfer encoding не сравниваются как raw
octets. Если у текстовой MIME-части неизвестный Content-Transfer-Encoding,
SEARCH завершается tagged `NO [UNKNOWN-CTE]` без частичного результата.
RFC 5322 address groups разбираются в типизированные start/member/end элементы:
FETCH ENVELOPE выдаёт group marker (`NIL NIL group NIL`), mailbox участников
и end marker (`NIL NIL NIL NIL`), включая пустую группу. SEARCH FROM/TO/CC/BCC
использует эту же проекцию и находит имя группы и адрес участника. Независимый
STARTTLS black-box проверил группу с тремя mailbox, пустую Cc-группу и Bcc
member. RFC-комментарии и obsolete address forms ещё не поддержаны полностью.
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
charset вне проверенного набора остаются непроверенными. SEARCH поддерживает
quoted strings и synchronizing literals до 64 KiB, включая несколько literals
в одной команде. Parser acceptance перечисляет все базовые SEARCH keys RFC 9051,
evaluator покрыт соответствующими unit и сетевыми сценариями. Объявление
SEARCHRES не означает полной
совместимости IMAP4rev1 или IMAP4rev2: остаются SEARCH/ESEARCH расширения и
другие несовпадения формальных грамматик; см. [матрицу IMAP](imap-profile.md).

## POP3

Поддерживаются STLS и implicit TLS, USER/PASS, STAT, LIST, UIDL, RETR, DELE,
RSET и QUIT. Maildrop lease исключает двух разрушающих владельцев; DELE
фиксируется только успешным QUIT.
AUTHORIZATION FSM требует `USER` перед `PASS`, отклоняет повторный `USER`
после положительного ответа, а после отклонённого USER или failed PASS очищает
имя и требует новую команду USER. Независимый STLS клиент проверил этот порядок с ошибочным и затем
успешным паролем; это не подтверждает прочие authentication mechanisms.
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
`LIST n` и `UIDL n` возвращают одиночный `+OK` с номером и данными; без аргумента
обе команды используют multiline frame. LIST octet size считает CRLF-представление
сообщения и не включает дублированные dot-stuffing октеты. Независимый Linux
клиент проверил обе формы, сравнил LIST размер с RETR после удаления dot-stuffing;
unit-тест отдельно подтвердил LF-only raw message нормализацию.
RETR/TOP отдельно ограничивают status, сохраняя допустимые длинные строки тела:
проверено точное чтение 700-октетной строки. Это ограниченный набор проверок,
а не сертификация остальных требований RFC 1939/2449 или совместимости клиента.

`AUTH PLAIN` поддержан только после TLS: до STLS механизм отсутствует в CAPA и
команда AUTH получает отказ; после STLS сервер рекламирует `SASL PLAIN` и убирает
`STLS`. Поддержаны initial response и continuation, отмена continuation через
`*`, строгая Base64-проверка и сохранение SASL capability после входа. Независимый
socket acceptance проверяет оба пути аутентификации, отказ от SCRAM, обновление
CAPA и запрет повторного AUTH после входа. Это ограниченный профиль RFC 5034,
не полная реализация: SCRAM и прочие SASL-механизмы не поддержаны, а
взаимодействие с litterkeep не проверялось.
Единственный `=` в initial-response позиции разбирается как пустой ответ согласно
RFC 5034, а не как некорректный Base64. Для PLAIN пустой payload отклоняется как
невалидные credentials; независимый TLS socket тест подтвердил повтор AUTH после
отказа и последующий вход с верным payload.
По RFC 5034 §4 неудачный AUTH оставляет сессию так, как если бы AUTH не
выполнялся. Unit-проверки подтверждают сохранение ранее выбранного USER при
ошибочном SASL payload; независимый Linux socket acceptance подтверждает
повторный AUTH после backend-отказа. Покрытие остальных ветвей отказа и
полного RFC профиля продолжает расширяться.

RFC 6856: `UTF8` capability объявляется в обоих состояниях без `USER`-параметра;
сама команда действует только до входа, включает raw UTF-8 message mode и
запрещает последующий STLS. До переключения `LIST`, `TOP` и `RETR` возвращают
`-ERR [UTF8]`, если выбранное письмо содержит корректные non-ASCII UTF-8
последовательности. Независимый Linux SMTP->POP3 socket acceptance отправил
8BITMIME UTF-8 письмо, получил `[UTF8]` от LIST/TOP/RETR в ASCII mode, затем
включил UTF8 до USER и сравнил точные байты RETR. Он также проверил CAPA до/после
TLS и отказ STLS после UTF8; unit-тесты проверяют guarded retrieval. MIME-дерево
рекурсивно обнаруживает `Content-Transfer-Encoding: binary`, включая вложенные
части всех `multipart/*`; POP3 `TOP` и `RETR` отклоняют такое сообщение даже в
UTF8 mode. Независимый Linux acceptance сохранил fixture через IMAP APPEND и
подтвердил отказ обеих команд. Это не полный RFC 6856: legacy charset conversion
и UTF-8 credentials/SASLprep не поддержаны. Если ограниченный MIME parser не
может разобрать структуру, бинарный CTE не классифицируется, поэтому такой случай
не считается проверенным. Повреждённые raw octets сами по себе не считаются UTF-8
строками. `LANG` объявляется и реализует listing единственной локали `en`, выбор
`en` и серверной локали через `*`; корректный, но неподдерживаемый basic language
range получает `-ERR`. Так как поддержана только английская локаль, выбор языка
не меняет тексты ответов. Многоязычная локализация, остальные RFC 6856 сценарии
и интеграция с реальным клиентом не проверялись.

## Sieve и ManageSieve

Есть bounded parser/executor для обязательного профиля, fileinto, redirect,
keep/discard, header/address/size tests и diagnostics. ManageSieve поддерживает
STARTTLS, SASL PLAIN, literals, CHECKSCRIPT, PUTSCRIPT, LISTSCRIPTS, GETSCRIPT,
SETACTIVE, DELETESCRIPT и RENAMESCRIPT.

## Outbound SMTP

Маршрут задаётся явно как IP:port и typed TLS policy. Scheduler durable,
single-flight, применяет bounded retry/backoff и DSN. Pool повторно выдаёт только
чистое соединение после success либо подтверждённого RSET.
