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
| Any | CAPABILITY, NOOP, LOGOUT | PASS | Tagged/untagged ответы, bounded line decoder; безаргументные команды отвергают лишние аргументы |
| TLS | STARTTLS | PASS | Сброс plaintext state и пошаговый rustls handshake |
| Authentication | LOGIN | PASS | Только TLS, общий Argon2 worker, повторный вход запрещён |
| Authentication | AUTHENTICATE PLAIN | PASS | Continuation, cancel, SASL-IR, authzid policy, только TLS |
| Mailbox discovery | LIST, LSUB, STATUS | PARTIAL | modified UTF-7, `*`/`%`, root discovery, empty selection `()`, slash-delimited relative reference prefix with canonical response names, SUBSCRIBED, RECURSIVEMATCH with CHILDINFO for SUBSCRIBED/SPECIAL-USE, RETURN (CHILDREN), RETURN (SUBSCRIBED), and RETURN STATUS (MESSAGES/UIDNEXT/UIDVALIDITY/UNSEEN/DELETED/SIZE) verified; missing names are selected only by SUBSCRIBED and retain `\\NonExistent`; multi-namespace/breakout naming is not implemented |
| Namespace | NAMESPACE | PASS | Advertised and tested; personal namespace is `""` with `/` delimiter, unavailable classes are NIL |
| Extension negotiation | ENABLE | PARTIAL | Atom and quoted-string token forms are distinguished; unsupported capability atoms are ignored; empty `ENABLED` and stable CAPABILITY verified; no enable-able extension is implemented yet |
| Mailbox mutation | CREATE, DELETE, RENAME | PASS | Durable journal, subtree rename, retired folder IDs |
| Subscription | SUBSCRIBE, UNSUBSCRIBE | PASS | Durable subscription независимо от существования папки |
| Selection | SELECT, EXAMINE | PASS | UIDVALIDITY, UIDNEXT, FLAGS, PERMANENTFLAGS, включая `\\*` для создания пользовательских keywords, Recent |
| Message insertion | APPEND | PASS | Bounded literal, системные flags и keywords, INTERNALDATE, APPENDUID после fsync |
| Reading | FETCH, UID FETCH | PARTIAL | Сгруппированные items, ALL/FAST/FULL, UID, RFC822 bytes, BODY HEADER/TEXT/HEADER.FIELDS, MIME parts и partial; BODY/PEEK Seen effects проверены; полная rev2 grammar остаётся |
| Search | SEARCH, UID SEARCH | PARTIAL | Sequence/UID sets, system/session flags, KEYWORD/UNKEYWORD, address/header/body/text, size, internal/sent dates, AND/OR/NOT/groups; UTF-8 default, UTF-8/US-ASCII CHARSET, BADCHARSET, ESEARCH RETURN ALL/COUNT/MIN/MAX, decoded MIME text and UNKNOWN-CTE verified; MIME text supports US-ASCII, UTF-8, ISO-8859-1 and Windows-1251, rejects undefined CP1251 octet; RETURN SAVE and `$` SEARCH criterion work per session with SELECT/error reset and MIN/MAX selection; FETCH/UID FETCH `$` maps saved UIDs to the active number space and handles empty results; `$` in STORE/COPY/UID EXPUNGE, pipelined SAVE dependencies and SEARCHRES advertisement remain unsupported; other MIME charsets and full key set remain |
| Flag mutation | STORE, UID STORE | PASS | Системные flags и регистронезависимые keywords: Replace/add/remove, SILENT, один durable batch, read-only denial |
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
| HEADER, BODY, TEXT | Parser/evaluator и ограниченный MIME charset/transfer decoding реализованы; body charset: US-ASCII, UTF-8, ISO-8859-1, Windows-1251; см. SEARCH строку основной таблицы |
| SUBJECT, FROM, TO, CC, BCC | Parser/evaluator используют ту же decoded ENVELOPE projection, что и FETCH ENVELOPE; адрес сравнивается по имени, mailbox, host и `mailbox@host`, без RFC 5322 delimiters. Неподдержанная/невалидная address syntax остаётся ограничением общего ENVELOPE parser-а |
| KEYWORD, UNKEYWORD | Реализованы typed parser/evaluator и сетевые проверки; keywords входят в APPEND v4 и versioned flags journal v2. Старые append v1-v3 и flags v1 читаются как раньше; FETCH FLAGS, STORE и COPY/MOVE сохраняют keywords. STORE обновляет оба класса flags одним checksum/commit frame |

Следующий SEARCH checkpoint теперь должен закрыть оставшиеся ключи и проверку
RFC grammar. Адресные SEARCH criteria и FETCH ENVELOPE используют одну
projection, но оба наследуют ограничения общего RFC 5322 address parser-а на
группах и malformed syntax.

G14 keyword checkpoint (2026-09-24): custom flags и KEYWORD/UNKEYWORD прошли
Windows tests, Linux Rust 1.97 strict Clippy/254 tests и STARTTLS IMAP black-box
с повторным открытием data directory. Остальной профиль G14 остаётся RUNNING.

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

Parser выдаёт типизированный отказ для лишних аргументов у CAPABILITY, LOGOUT,
NOOP, IDLE и STARTTLS. Tagged BAD содержит читаемый текст причины: пустой
resp-text допустим в RFC 9051, но стандартный Python imaplib аварийно завершает
разбор ответа с пустым BAD, поэтому проверяется и межклиентская совместимость.
