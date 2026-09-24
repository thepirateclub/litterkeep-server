# Покрытие протоколов

## SMTP

Submission поддерживает implicit TLS и STARTTLS, EHLO, AUTH PLAIN, MAIL SIZE,
несколько RCPT, DATA с однократным dot-unstuffing, RSET, NOOP и QUIT. До TLS
AUTH запрещён. Envelope sender должен принадлежать authenticated owner. Local
route не является open relay.

## IMAP

Реализованы STARTTLS/implicit TLS, LOGIN, AUTHENTICATE PLAIN с SASL-IR, LIST,
LSUB, CREATE/RENAME/DELETE, SELECT/EXAMINE, STATUS, APPEND, SEARCH, FETCH,
BODYSTRUCTURE, MIME sections/partial, STORE, COPY, MOVE, EXPUNGE, UID EXPUNGE,
IDLE/DONE и durable Recent. UIDVALIDITY и UID переживают restart.

`FETCH` принимает одиночные и сгруппированные data items, раскрывает ALL, FAST
и FULL, включает UID в ответы UID FETCH и сохраняет literal как точные октеты.
`BODY` поддерживает полное сообщение, HEADER/TEXT, HEADER.FIELDS/NOT, числовые
MIME paths, вложенный `message/rfc822` и partial ranges; durable `\\Seen`
различает BODY/RFC822, PEEK и EXAMINE. `NAMESPACE`, `UNSELECT` и `ENABLE`
рекламируются как доступные команды текущего IMAP4rev1 профиля. Независимый
STARTTLS клиент подтвердил, что ENABLE игнорирует неизвестный capability,
возвращает пустой `ENABLED` и не меняет CAPABILITY. Сетевая проверка также
подтвердила отказ от quoted capability и сквозное совпадение байтов вложения;
проверки BODY/Seen, headers, body, flags и UNSELECT подтверждены отдельно.
SEARCH использует UTF-8 по умолчанию и поддерживает явные UTF-8/US-ASCII;
неподдерживаемая кодировка возвращает `NO [BADCHARSET ...]`, а Unicode-criteria
при US-ASCII отвергаются ответом NO. Декодирование MIME transfer encodings,
encoded-word заголовков и ESEARCH пока не завершено. Это не означает полной
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
