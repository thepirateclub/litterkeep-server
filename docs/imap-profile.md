# Матрица текущего IMAP4rev1 и целевого IMAP4rev2

Сервер пока объявляет IMAP4rev1. Целевой современный baseline для следующего
этапа - RFC 9051 IMAP4rev2; его нельзя объявлять до выполнения обязательной
семантики и сетевых acceptance-тестов. Наличие расширений не выводится из того,
что клиент случайно использовал соответствующую команду.

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
| Namespace | NAMESPACE | PASS | Advertised and tested; personal namespace is `""` with `/` delimiter, unavailable classes are NIL |
| Extension negotiation | ENABLE | PARTIAL | Atom and quoted-string token forms are distinguished; unsupported capability atoms are ignored; empty `ENABLED` and stable CAPABILITY verified; no enable-able extension is implemented yet |
| Mailbox mutation | CREATE, DELETE, RENAME | PASS | Durable journal, subtree rename, retired folder IDs |
| Subscription | SUBSCRIBE, UNSUBSCRIBE | PASS | Durable subscription независимо от существования папки |
| Selection | SELECT, EXAMINE | PASS | UIDVALIDITY, UIDNEXT, FLAGS, PERMANENTFLAGS, Recent |
| Message insertion | APPEND | PASS | Bounded literal, flags, INTERNALDATE, APPENDUID после fsync |
| Reading | FETCH, UID FETCH | PARTIAL | Сгруппированные items, ALL/FAST/FULL, UID, RFC822 bytes, BODY HEADER/TEXT/HEADER.FIELDS, MIME parts и partial; BODY/PEEK Seen effects проверены; полная rev2 grammar остаётся |
| Search | SEARCH, UID SEARCH | PARTIAL | Sequence/UID sets, system/session flags, address/header/body/text, size, internal/sent dates, AND/OR/NOT/groups; UTF-8 default, UTF-8/US-ASCII CHARSET, BADCHARSET, ESEARCH RETURN ALL/COUNT/MIN/MAX, decoded MIME text and UNKNOWN-CTE verified; RETURN SAVE and `$` SEARCH criterion work per session with SELECT/error reset and MIN/MAX selection; generic `$` message sets in FETCH/STORE/COPY/EXPUNGE are not implemented and SEARCHRES is not advertised; other MIME charsets and full key set remain |
| Flag mutation | STORE, UID STORE | PASS | Replace/add/remove, SILENT, durable batch, read-only denial |
| Copy and move | COPY, UID COPY, MOVE, UID MOVE | PASS | Atomic links/tombstones, COPYUID, multi-session projection |
| Deletion | EXPUNGE, UID EXPUNGE, CLOSE | PASS | Durable tombstones, UIDPLUS, sequence-map notifications |
| Selection lifecycle | UNSELECT | PASS | Advertised and tested; returns to authenticated state without expunging `\\Deleted` |
| Synchronization | CHECK | PASS | Tagged completion после ранее завершённых durable операций |
| Waiting | IDLE, DONE | PASS | Event wakeup без session thread и без polling loop |
| Special use | LIST SPECIAL-USE, RETURN, CREATE USE | PASS | Durable enum attributes, без догадки по имени папки |

## SEARCH key audit по RFC 9051

Базовые ключи sequence-set, UID, ALL, system/session flags, internal/sent
dates, size, NOT/OR и группировка имеют parser и evaluator. HEADER/BODY/TEXT и
MIME decoding проверены отдельно. При этом синтаксис и семантика не полностью
совпадают с RFC:

| RFC 9051 key group | Фактическое состояние |
|---|---|
| `<sequence-set>`, UID, ALL, ANSWERED/UNANSWERED, DELETED/UNDELETED, DRAFT/UNDRAFT, FLAGGED/UNFLAGGED, SEEN/UNSEEN, RECENT/OLD/NEW | Parser/evaluator реализованы; RECENT является session projection |
| BEFORE/ON/SINCE, SENTBEFORE/SENTON/SENTSINCE, LARGER/SMALLER | Parser/evaluator реализованы; даты сравниваются по календарному дню согласно профилю |
| NOT, OR, скобочная группа и неявный AND | Parser/evaluator реализованы |
| HEADER, BODY, TEXT | Parser/evaluator и ограниченный MIME charset/transfer decoding реализованы; см. SEARCH строку основной таблицы |
| SUBJECT, FROM, TO, CC, BCC | Поиск реализован по декодированным RFC 5322 header values; это не эквивалентно точной IMAP ENVELOPE projection на malformed/необычных заголовках, поэтому семантика остаётся PARTIAL |
| KEYWORD, UNKEYWORD | Не реализованы: durable message model и FLAGS journal пока хранят только системный битовый набор; пользовательские keyword flags нельзя корректно искать или выдавать |

Следующий SEARCH checkpoint требует добавить пользовательские flags как
долговечную часть message metadata с корректным APPEND/STORE/COPY/MOVE,
FETCH FLAGS, recovery и совместимостью существующих журналов; затем реализовать
KEYWORD/UNKEYWORD и независимые сетевые тесты. Отдельным изменением нужно
сопоставить адресные SEARCH criteria с той же ENVELOPE projection, что выдаёт
FETCH ENVELOPE, чтобы избежать расхождения между чтением и поиском.

CAPABILITY объявляет `IMAP4rev1`, `IDLE`, `UIDPLUS`, `MOVE`, `SPECIAL-USE`,
`NAMESPACE`, `UNSELECT` и `ENABLE`. ENABLE игнорирует неизвестные и пока не
поддерживаемые расширения по RFC и возвращает пустой `ENABLED`, поскольку сервер
пока не включает ни одной дополнительной возможности. `IMAP4rev2` и
`UTF8=ACCEPT` не активируются и не рекламируются.
`AUTH=PLAIN SASL-IR` доступен только внутри TLS. До STARTTLS сервер выдаёт
`LOGINDISABLED`; plaintext listener без TLS также не рекламирует парольный
механизм и отвечает `PRIVACYREQUIRED` на LOGIN/AUTHENTICATE.

При live-доставке все уже selected sessions получают собственный `\Recent`.
Первый последующий SELECT атомарно забирает durable кандидат; следующий SELECT
его не видит. Такое детерминированное правило находится в разрешённой RFC 3501
неоднозначности для нескольких одновременно открытых соединений.

Не заявлены IMAP4rev2, UTF8=ACCEPT, CONDSTORE, QRESYNC, COMPRESS,
SORT, THREAD и полный набор SEARCH keys. Их отсутствие не маскируется пустым
успешным результатом.
