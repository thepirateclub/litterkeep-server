# Матрица IMAP4rev1

Матрица фиксирует фактически исполняемый профиль, а не желаемый список.
`PASS` означает наличие типизированного parser-а, reactor state transition и
профильного теста. `PARTIAL` означает намеренно ограниченную семантику, которую
CAPABILITY не выдаёт за более широкое расширение.

| Состояние или раздел | Команды и свойства | Статус | Проверяемая граница |
|---|---|---|---|
| Any | CAPABILITY, NOOP, LOGOUT | PASS | Tagged/untagged ответы, bounded line decoder |
| TLS | STARTTLS | PASS | Сброс plaintext state и пошаговый rustls handshake |
| Authentication | LOGIN | PASS | Только TLS, общий Argon2 worker, повторный вход запрещён |
| Authentication | AUTHENTICATE PLAIN | PASS | Continuation, cancel, SASL-IR, authzid policy, только TLS |
| Mailbox discovery | LIST, LSUB, STATUS | PASS | modified UTF-7, `*`/`%`, delimiter, durable catalog |
| Mailbox mutation | CREATE, DELETE, RENAME | PASS | Durable journal, subtree rename, retired folder IDs |
| Subscription | SUBSCRIBE, UNSUBSCRIBE | PASS | Durable subscription независимо от существования папки |
| Selection | SELECT, EXAMINE | PASS | UIDVALIDITY, UIDNEXT, FLAGS, PERMANENTFLAGS, Recent |
| Message insertion | APPEND | PASS | Bounded literal, flags, INTERNALDATE, APPENDUID после fsync |
| Reading | FETCH, UID FETCH | PASS | FLAGS, INTERNALDATE, ENVELOPE, BODYSTRUCTURE, MIME sections, partial |
| Search | SEARCH, UID SEARCH | PARTIAL | ALL, SEEN, UNSEEN, DELETED; иные keys получают BAD |
| Flag mutation | STORE, UID STORE | PASS | Replace/add/remove, SILENT, durable batch, read-only denial |
| Copy and move | COPY, UID COPY, MOVE, UID MOVE | PASS | Atomic links/tombstones, COPYUID, multi-session projection |
| Deletion | EXPUNGE, UID EXPUNGE, CLOSE | PASS | Durable tombstones, UIDPLUS, sequence-map notifications |
| Synchronization | CHECK | PASS | Tagged completion после ранее завершённых durable операций |
| Waiting | IDLE, DONE | PASS | Event wakeup без session thread и без polling loop |
| Special use | LIST SPECIAL-USE, RETURN, CREATE USE | PASS | Durable enum attributes, без догадки по имени папки |

CAPABILITY объявляет `IMAP4rev1`, `IDLE`, `UIDPLUS`, `MOVE` и `SPECIAL-USE`.
`AUTH=PLAIN SASL-IR` доступен только внутри TLS. До STARTTLS сервер выдаёт
`LOGINDISABLED`; plaintext listener без TLS также не рекламирует парольный
механизм и отвечает `PRIVACYREQUIRED` на LOGIN/AUTHENTICATE.

При live-доставке все уже selected sessions получают собственный `\Recent`.
Первый последующий SELECT атомарно забирает durable кандидат; следующий SELECT
его не видит. Такое детерминированное правило находится в разрешённой RFC 3501
неоднозначности для нескольких одновременно открытых соединений.

Не заявлены IMAP4rev2, UTF8=ACCEPT, CONDSTORE, QRESYNC, NAMESPACE, COMPRESS,
SORT, THREAD и полный набор SEARCH keys. Их отсутствие не маскируется пустым
успешным результатом.
