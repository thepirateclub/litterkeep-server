# Покрытие протоколов

## SMTP

Submission поддерживает implicit TLS и STARTTLS, EHLO, AUTH PLAIN, MAIL SIZE,
несколько RCPT, DATA с однократным dot-unstuffing, RSET, NOOP и QUIT. До TLS
AUTH запрещён. Envelope sender должен принадлежать authenticated owner. Local
route не является open relay.

RFC 1870 SIZE объявляется с фактическим фиксированным пределом сообщения.
Единственный корректный `MAIL FROM ... SIZE=n` разбирается до DATA; значение
выше предела получает 552, некорректная грамматика или повтор SIZE получают
501. Неподдерживаемые ESMTP параметры MAIL/RCPT получают 555, а не игнорируются.
Независимый STARTTLS black-box проверяет эти ответы и восстановление сессии.
Если размер тела превышает лимит уже во время DATA, сервер прекращает
накапливать тело, но продолжает потоково читать до завершающей точки; после неё
возвращает 552 и сохраняет синхронизацию команд. Проверено независимым клиентом
на теле размером 25 MiB + 1 байт с последующим NOOP.
Это не закрывает прочие обязательные команды RFC 5321 и SMTPUTF8.

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

## Sieve и ManageSieve

Есть bounded parser/executor для обязательного профиля, fileinto, redirect,
keep/discard, header/address/size tests и diagnostics. ManageSieve поддерживает
STARTTLS, SASL PLAIN, literals, CHECKSCRIPT, PUTSCRIPT, LISTSCRIPTS, GETSCRIPT,
SETACTIVE, DELETESCRIPT и RENAMESCRIPT.

## Outbound SMTP

Маршрут задаётся явно как IP:port и typed TLS policy. Scheduler durable,
single-flight, применяет bounded retry/backoff и DSN. Pool повторно выдаёт только
чистое соединение после success либо подтверждённого RSET.
