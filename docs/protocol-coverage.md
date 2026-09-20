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

