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
| Authentication | LOGIN | PASS | Только TLS, общий Argon2 worker, повторный вход запрещён; username/password принимают atom, quoted-string и проверенные synchronizing literals до 64 KiB; raw STARTTLS acceptance проверил два последовательных literal и authenticated NOOP |
| Authentication | AUTHENTICATE PLAIN | PASS | Continuation, cancel, atom/quoted astring, пустой SASL-IR marker, authzid policy, только TLS; прочие механизмы не рекламируются |
| Mailbox discovery | LIST, LSUB, STATUS | PARTIAL | CAPABILITY advertises implemented `LIST-EXTENDED` (RFC 5258) and `LIST-STATUS` (RFC 5819); modified UTF-7, `*`/`%`, root discovery, empty selection `()`, slash-delimited relative reference prefix with canonical response names, SUBSCRIBED, RECURSIVEMATCH with CHILDINFO for SUBSCRIBED/SPECIAL-USE, RETURN (CHILDREN), RETURN (SUBSCRIBED), and RETURN STATUS (MESSAGES/RECENT/UIDNEXT/UIDVALIDITY/UNSEEN/DELETED/SIZE/APPENDLIMIT) verified; RECENT reports unclaimed durable messages before SELECT and session-owned recent UIDs after SELECT, consistently in STATUS and LIST STATUS; combined UTF8=ACCEPT LIST/STATUS wire responses preserve Unicode mailbox names; missing names are selected only by SUBSCRIBED and retain `\\NonExistent`; multi-namespace/breakout naming is not implemented |
| Namespace | NAMESPACE | PASS | Advertised and tested; personal namespace is `""` with `/` delimiter, unavailable classes are NIL |
| Extension negotiation | ENABLE | PARTIAL | Atom and quoted-string token forms are distinguished; unsupported capability atoms are ignored; authenticated `ENABLE UTF8=ACCEPT` activates a connection-local mode and returns `ENABLED UTF8=ACCEPT`; extension remains hidden from CAPABILITY until the full profile gate passes |
| Mailbox mutation | CREATE, DELETE, RENAME | PASS | Durable journal, subtree rename, retired folder IDs |
| Subscription | SUBSCRIBE, UNSUBSCRIBE | PASS | Durable subscription независимо от существования папки |
| Selection | SELECT, EXAMINE | PASS | UIDVALIDITY, UIDNEXT, FLAGS, PERMANENTFLAGS, включая `\\*` для создания пользовательских keywords, Recent |
| Message insertion | APPEND | PARTIAL | Bounded literal, strict parenthesized system flags/keywords, quoted-string-only RFC date-time INTERNALDATE, APPENDUID после fsync; quoted flags, unquoted dates and malformed flag lists are rejected before literal continuation; global 25 MiB limit is advertised through APPENDLIMIT and over-limit sizes are rejected with tagged TOOBIG before payload; MULTIAPPEND and non-synchronizing literal extensions are not advertised |
| Reading | FETCH, UID FETCH | PARTIAL | Сгруппированные items, ALL/FAST/FULL с RFC-совместимым раскрытием FULL через BODY, BODY и BODYSTRUCTURE, UID, RFC822/RFC822.HEADER/RFC822.TEXT legacy items с правильными wire labels, BODY sections/partial и BINARY/BINARY.PEEK/BINARY.SIZE; decoded MIME leaf parts, Base64/QP, literal8 для NUL, decoded partial, UNKNOWN-CTE и Seen effects проверены. RFC 6532 message/global принимает identity, Base64 и quoted-printable CTE с отдельным source для decoded вложенных ranges; неизвестный CTE сохраняет opaque octet-stream fallback. Полная rev2 grammar остаётся |

Для `message/global` `7bit`, `8bit`, `binary`, Base64 и quoted-printable
строят вложенную структуру; non-identity CTE получает отдельный decoded source,
а его offsets не смешиваются с исходными wire ranges.
BODYSTRUCTURE возвращает `MESSAGE/GLOBAL`, `BODY[1]` остаётся encoded wire body,
`BODY[1.1]` выбирает decoded вложенную часть, а BODY HEADER и BINARY selection
проходят во вложенные части. Неизвестный CTE сохраняет opaque
`application/octet-stream` fallback. Проверены unit-векторы для ownership,
ENVELOPE и section numbering; независимый Linux STARTTLS black-box дополнительно
сверяет encoded BODY[1], decoded BODY[1.1], nested UTF-8 header, BINARY и полный
RFC822 literal. Litterkeep app не проверялось. Это не меняет CAPABILITY и не
объявляет IMAP4rev2.
| Search | SEARCH, UID SEARCH | PARTIAL | Все базовые SEARCH keys RFC 9051 распознаются и оцениваются; исчерпывающая проверка parser-а перечисляет каждый key. UTF-8 default, UTF-8/US-ASCII CHARSET, BADCHARSET, synchronizing string literals до 64 KiB, ESEARCH RETURN ALL/COUNT/MIN/MAX, decoded MIME text и исключение неизвестного CTE из текстового projection проверены; MIME text supports US-ASCII, UTF-8, ISO-8859-1, Windows-1251 and Windows-1252; RETURN SAVE and `$` SEARCH criterion work per session with SELECT/error reset and MIN/MAX selection; FETCH/UID FETCH and STORE/UID STORE `$` map saved UIDs to the active number space, COPY/UID COPY `$` resolve saved UIDs in their respective addressing modes, UID EXPUNGE `$` intersects saved UIDs with `\\Deleted`, empty sets are safe, and pipelined SEARCH SAVE followed by COPY is verified; ESEARCH and SEARCHRES are advertised; other MIME charsets and non-base extensions remain |
| Flag mutation | STORE, UID STORE | PASS | Системные flags и регистронезависимые keywords: Replace/add/remove, SILENT, один durable batch, read-only denial; parenthesized and unparenthesized forms are parsed without dropping delimiters; nested/unbalanced lists and quoted flags rejected |
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
| HEADER, BODY, TEXT | Parser/evaluator и ограниченный MIME charset/transfer decoding реализованы; body charset: US-ASCII, UTF-8, ISO-8859-1, Windows-1251, Windows-1252; см. SEARCH строку основной таблицы |
| SUBJECT, FROM, TO, CC, BCC | Parser/evaluator используют ту же decoded ENVELOPE projection, что и FETCH ENVELOPE; адрес сравнивается по имени, mailbox, host и `mailbox@host`, без RFC 5322 wire delimiters. Obsolete null members пропускаются без потери mailbox order. Разделитель addr-spec выбирается вне quoted local-part и domain-literal; quoted-pair внутри quoted display-name декодируется. Obsolete source-route отбрасывается до разбора целевого addr-spec. CFWS вокруг точек в obsolete dot-separated local-part/domain нормализуется; содержимое quoted local-part и domain-literal сохраняется. Завершающий CFWS после `;` группы отделяется от её содержимого. RFC 5322 группы сериализуются start/member/end адресными структурами; comments удаляются до ENVELOPE/SEARCH. Остальные obsolete forms и malformed syntax ограничивают общий parser |
| Address groups and addr-spec | FETCH ENVELOPE, SEARCH address keys | PARTIAL | RFC 5322 group markers, empty groups, nested and trailing comments, obsolete null members, display-name quoted-pair, RFC 2047 encoded words in unquoted display phrases, quoted local-part containing `@`, source-route destination, obsolete CFWS around dots, and address SEARCH are covered; malformed encoded words remain visible and quoted display names remain literal; other obsolete forms and grammar mismatches remain limited |
| KEYWORD, UNKEYWORD | Реализованы typed parser/evaluator и сетевые проверки; keywords входят в APPEND v4 и versioned flags journal v2. Старые append v1-v3 и flags v1 читаются как раньше; FETCH FLAGS, STORE и COPY/MOVE сохраняют keywords. STORE обновляет оба класса flags одним checksum/commit frame |

Базовый набор SEARCH keys RFC 9051 закрыт parser/evaluator acceptance; это не
означает полного SEARCH wire grammar. Quoted strings и synchronizing literals
до 64 KiB проверены, но non-synchronizing LITERAL+ не объявляется. Ограниченный
synchronizing-literal receiver принимает оба `astring` аргумента `LOGIN` и
mailbox `astring` команд CREATE, DELETE, RENAME, SUBSCRIBE, UNSUBSCRIBE, LIST,
LSUB, STATUS, COPY, MOVE, SELECT и EXAMINE, включая UID COPY/MOVE. Независимый
raw STARTTLS-клиент проверил continuation и успешное выполнение CREATE, LIST,
SELECT с literal mailbox. SEARCH/UID SEARCH и APPEND имеют собственные
проверенные пути. Это не означает поддержку literals в прочих аргументах всех
команд; не синхронизируемое расширение LITERAL+ не рекламируется. Адресные
SEARCH criteria и FETCH ENVELOPE используют одну projection; вложенные comments
и quoted-pairs удаляются из семантических полей. Obsolete forms и malformed
RFC 5322 syntax остаются ограничениями address parser-а.

Wire framing synchronizing literal отсчитывает объявленные октеты, поэтому CRLF
внутри payload не завершает значение досрочно. Independent STARTTLS LOGIN
acceptance передал CRLF внутри password literal, получил обычный отказ
аутентификации и затем успешно вошёл с правильными credentials в той же сессии.
Raw NUL отклоняется как бинарная строка. LOGIN password literal сохраняется
побайтно, включая невалидные UTF-8 sequences, и передаётся authentication owner-
у; username, mailbox и SEARCH string literals остаются text-valued production и
требуют допустимое для их режима UTF-8 представление. Это не поддержка полного
произвольного octet space во всех IMAP аргументах.

G14 keyword checkpoint (2026-09-24): custom flags и KEYWORD/UNKEYWORD прошли
Windows tests, Linux Rust 1.97 strict Clippy/254 tests и STARTTLS IMAP black-box
с повторным открытием data directory. Остальной профиль G14 остаётся RUNNING.

G14 BINARY checkpoint (2026-09-25): RFC 9051 BINARY, BINARY.PEEK и
BINARY.SIZE распознаются как typed FETCH items. MIME CTE декодируется до
применения partial range; NUL в ответе сериализуется literal8, текстовые
части нормализуются к CRLF, неизвестная transfer encoding даёт tagged
NO [UNKNOWN-CTE]. Независимый STARTTLS black-box доставил вложение по SMTP,
сравнил каждый decoded octet через BINARY и проверил SIZE, partial, Seen/PEEK и
UNKNOWN-CTE. Это не проверка приложения Litterkeep. Другие RFC 9051 FETCH items,
команды и расширения ещё не закрыты, поэтому профиль остаётся IMAP4rev1/PARTIAL.

G14 FETCH BODY checkpoint (2026-09-25): отдельный BODY data item возвращает
нерасширяемую MIME body structure. Макрос FULL раскрывается ровно как FLAGS,
INTERNALDATE, RFC822.SIZE, ENVELOPE и BODY; BODYSTRUCTURE остаётся отдельным
запросом. Windows/Linux unit gates и независимый STARTTLS SMTP/IMAP attachment
black-box прошли. Litterkeep app не проверялось; IMAP4rev2 не объявляется.

G17 tokenizer checkpoint (2026-09-26): unquoted atom rejects an embedded
double quote; quoted-string remains a separate token form. Independent STARTTLS
wire acceptance received tagged BAD and then issued NOOP successfully to verify
session recovery. This closes only the quoted-special delimiter case, not the
remaining atom-special context rules or IMAP4rev2 profile.

G17 tag checkpoint (2026-09-26): parser and malformed-UTF-8 recovery use the
RFC 9051 `tag` production, not generic `atom`: `]` is accepted as a response
special and `+` is excluded. Unit and STARTTLS wire acceptance cover valid tag,
error correlation, and safe fallback for the excluded character.

G17 command boundary checkpoint (2026-09-26): argument extraction no longer
skips list delimiter tokens. Command verbs and UID subcommands require atom
tokens, so parenthesized or quoted `NOOP` cannot be accepted as a command.
Parser tests and independent STARTTLS acceptance verify tagged BAD and session
recovery. This does not yet make the generic lexer enforce every atom-special.

G17 astring checkpoint (2026-09-26): LOGIN and SELECT/EXAMINE now validate raw
atom tokens against `ASTRING-CHAR`; special characters require quoted form,
except the permitted `]` response-special. LOGIN rejects excess arguments.
Parser cases and independent STARTTLS acceptance verify `%` rejection in LOGIN;
this does not yet imply every command argument production is covered.

G17 LIST checkpoint (2026-09-26): basic LIST validates its reference as an
astring and its pattern as `list-mailbox`; bare wildcards `%` and `*` and
response-special `]` remain legal, while brace/backslash/quote require a quoted
string. Parser tests and independent STARTTLS acceptance verified rejection of
bare `{` and successful completion of a bare `*` pattern.

G17 flag grammar checkpoint (2026-09-26): STORE accepts exactly one valid
parenthesized flag-list or a non-empty unparenthesized sequence of flag atoms;
nested, unbalanced, quoted, and trailing structural forms are rejected. APPEND
uses the same strict flag production and rejects quoted flags before requesting
literal bytes. Parser and independent STARTTLS acceptance cover these errors.

G17 APPEND date-time checkpoint (2026-09-26): RFC 9051 `date-time` is
accepted only as quoted-string. An unquoted date-looking atom is rejected
before literal continuation; a valid quoted date still reaches continuation
and completes with an empty literal in independent STARTTLS acceptance. This
does not close remaining APPEND grammar or the IMAP4rev2 profile.

CAPABILITY объявляет `IMAP4rev1`, `IDLE`, `UIDPLUS`, `MOVE`, `SPECIAL-USE`,
`NAMESPACE`, `UNSELECT`, `ENABLE`, `ESEARCH` и `SEARCHRES`. ENABLE игнорирует
неизвестные и пока не поддерживаемые расширения по RFC. В authenticated state
`ENABLE UTF8=ACCEPT` активирует режим на текущем соединении и возвращает
`ENABLED UTF8=ACCEPT`; capability намеренно скрыта, пока не пройдёт весь
профиль RFC 9755. `APPENDLIMIT=26214400` объявляет единый предельный размер
сообщения в октетах; STATUS и LIST RETURN STATUS возвращают такое же значение
для каждого mailbox. `IMAP4rev2` не заявляется.
`AUTH=PLAIN SASL-IR` доступен только внутри TLS. До STARTTLS сервер выдаёт
`LOGINDISABLED`; plaintext listener без TLS также не рекламирует парольный
механизм и отвечает `PRIVACYREQUIRED` на LOGIN/AUTHENTICATE.

При live-доставке все уже selected sessions получают собственный `\Recent`.
Первый последующий SELECT атомарно забирает durable кандидат; следующий SELECT
его не видит. Такое детерминированное правило находится в разрешённой RFC 3501
неоднозначности для нескольких одновременно открытых соединений.

Не заявлены IMAP4rev2, CONDSTORE, QRESYNC, COMPRESS,
SORT, THREAD и полный набор SEARCH keys. Их отсутствие не маскируется пустым
успешным результатом.

Parser выдаёт типизированный отказ для лишних аргументов у CAPABILITY, LOGOUT,
NOOP, IDLE и STARTTLS. Tagged BAD содержит читаемый текст причины: пустой
resp-text допустим в RFC 9051, но стандартный Python imaplib аварийно завершает
разбор ответа с пустым BAD, поэтому проверяется и межклиентская совместимость.
Quoted-pair в quoted-string текущего IMAP4rev1 профиля допускает только `\"`
и `\\`; произвольный `\x` теперь лексически отвергается, а исходный tag
сохраняется в tagged BAD. Также legacy quoted-string отвергает raw high-bit
characters, пока UTF8=ACCEPT не реализован и не согласован. Синхронизирующий
SEARCH literal обрабатывается отдельно от legacy atom/quoted-string: UTF-8
BODY/UID literals и пустой literal проверены независимым STARTTLS клиентом.
High-bit characters внутри atom отвергаются tagged BAD, пока UTF8=ACCEPT не
включён; это проверено unit и независимым STARTTLS acceptance. После ENABLE
UTF-8 допустим только в quoted-string, а malformed UTF-8 и запрещённые C0/C1
mailbox scalars должны получать tagged BAD; их независимая STARTTLS acceptance
покрывает также восстановление сессии через NOOP. Последовательные
synchronizing literals в одном SEARCH HEADER, включая два continuation, также
проверены; хвостовая pipelined команда сохраняется. Unit и wire acceptance
подтвердили tagged BAD и выполнение NOOP после ошибки. UTF8=ACCEPT уже
переключается через ENABLE, но не заявляется через CAPABILITY до полного
профиля; IMAP4rev2 не заявляется.
Разделителем command grammar считается только RFC `SP`: HTAB не превращается
в пробел между tag и verb, а CTL в atom отвергаются. HTAB остаётся допустимым
в quoted-string, тогда как NUL/CR/LF там отклоняются. Эти границы покрыты
unit-тестом; независимый STARTTLS acceptance подтвердил tagged BAD на TAB между
tag и NOOP и последующий успешный NOOP.
Если строка команды не декодируется как UTF-8, reactor сохраняет безопасный
ASCII tag перед ответом BAD; повреждённый UTF-8 внутри самого tag использует
untagged fallback, не отражая недоверенные bytes. Независимый STARTTLS
black-box проверил tagged BAD на malformed UTF-8 quoted LOGIN, untagged BAD
для повреждённого tag без отражения octets и успешный NOOP после ошибок.
При восстановлении tag применяется именно грамматика `tag = 1*<ASTRING-CHAR
except "+">`: закрывающая скобка допустима как `resp-special`, тогда как plus
запрещён. Это не совпадает с грамматикой atom и проверяется отдельными cases.
Само исправление не меняет рекламируемый профиль.

## EAI UTF-8 extension: RFC 9755 audit

Актуальный стандарт для UTF-8 режима IMAP4rev1 -
[RFC 9755](https://www.rfc-editor.org/rfc/rfc9755.html), опубликованный в 2025
году и заменивший RFC 6855. Он не расширяет IMAP4rev2: RFC 9051 уже включает
этот функциональный набор. Сервер остаётся IMAP4rev1 и `UTF8=ACCEPT` не
рекламирует; распознавание capability атома парсером не означает поддержки.

| Обязательство RFC 9755 | Текущее состояние | Что требуется для PASS |
|---|---|---|
| Capability и ENABLE | `ENABLE UTF8=ACCEPT` после аутентификации активирует session mode и возвращает `ENABLED UTF8=ACCEPT`; APPEND и mailbox subset прошли independent STARTTLS wire checks. NFC output проверен для LIST/LSUB/STATUS и старых mailbox aliases; capability скрыта до завершения remaining Net-Unicode response paths. Mailbox validator отвергает unassigned points из Unicode 17.0.0 | Завершить remaining response-path acceptance; только после полного profile gate рекламировать `UTF8=ACCEPT`; ENABLE остаётся additive на соединении |
| UTF-8 quoted-string | Mode-aware lexer включается session ENABLE; independent wire тест подтверждает BAD для UTF-8 CREATE до ENABLE и успешные mailbox commands после него. Malformed UTF-8, C0/C1 controls и UTF-8 atom получают BAD, за которым проходит NOOP | Разрешать валидные UTF-8 только в quoted-string после ENABLE; atom остаётся ASCII; расширить wire проверки на полный набор RFC 9755 |
| Имена mailbox | CREATE/DELETE/RENAME, SUBSCRIBE/UNSUBSCRIBE, APPEND, STATUS, COPY/MOVE, UID COPY/MOVE и LIST/LSUB используют mode-aware parser; независимый STARTTLS client проверил mutation и transfer между UTF-8 именами, включая tagged COPYUID и совпадение RFC822 bytes. Unicode 17 NFC применяется к новым именам и output. Существующие decomposed folder journals не переписываются: Store разрешает NFC-equivalent alias по stable folder ID; полный Store reopen подтверждает сохранение прежних mailbox bytes. Ambiguous canonical aliases отклоняются. CREATE с unassigned U+0378 получает BAD; unassigned scalars проверяются по Unicode 17.0.0 | Оставшиеся response paths требуют acceptance; RFC 9755 profile gate ещё не закрыт |
| Ответы с именами mailbox | LIST/LSUB/STATUS выбирают modified UTF-7 в Legacy и NFC UTF-8 quoted output после ENABLE; independent wire tests проверили LIST/LSUB/STATUS и composed output для старой decomposed folder записи; комбинированный `LIST RETURN STATUS` после ENABLE проверил соседство ответов, UTF-8 mailbox и запрошенные числовые поля. NAMESPACE пока выдаёт только пустой personal prefix | Сохранять legacy представление для не включённого клиента; проверить namespace/shared prefixes, когда такие имена появятся |
| APPEND internationalized headers | Independent Python `imaplib` STARTTLS acceptance подтверждает NO и пустой INBOX до ENABLE, затем APPENDUID и точное RFC822 FETCH совпадение байтов после ENABLE; четыре unit tests покрывают UTF-8 header, high-bit body, empty headers и отсутствие разделителя | Сохранить acceptance при полном profile gate; отдельно проверить malformed header policy, mailbox mode и CAPABILITY вместе с остальными RFC 9755 обязательствами |
| SEARCH charset | После ENABLE сервер отвечает BAD на явный CHARSET в выбранном mailbox state; независимый STARTTLS клиент проверил `CHARSET UTF-8` и смешанный регистр `cHaRsEt uTf-8`, затем успешно выполнил implicit UTF-8 SEARCH ALL и нашёл письмо по `SUBJECT "café"`. Unit проверяет отказ до store routing | RFC 9755 SHOULD: сохранять BAD для любого явно указанного CHARSET в UTF8 режиме; сохранить evaluator behavior для остальных criteria |
| LOGIN и SASL | LOGIN использует legacy IMAP string grammar; AUTHENTICATE PLAIN отдельный typed путь | Не расширять LOGIN для UTF-8 credentials; UTF-8 identity передавать через AUTHENTICATE и отдельно проверять identity policy |
| message/global | Независимый STARTTLS клиент включил `UTF8=ACCEPT` до APPEND и подтвердил `MESSAGE/GLOBAL`, encoded `BODY[1]`, decoded nested BODY/BINARY sections и исходный RFC822 literal byte-exact | Сохранить это поведение при оставшихся IMAP profile changes; RFC 9755 для IMAP4rev1 разрешает BODYSTRUCTURE как `message/rfc822` либо RFC 3501 вариант |
| UTF8=ONLY | Не поддерживается и не рекламируется | Отдельный профиль: подразумевает UTF8=ACCEPT, клиенты всё равно отправляют `ENABLE UTF8=ACCEPT`; требует reject legacy modified UTF-7 и `NO [CANNOT]` для UTF-8-dependent команд до ENABLE |

До прохождения этих gates `UTF8=ACCEPT` и `UTF8=ONLY` не должны появляться в
CAPABILITY. ENABLE уже включает UTF8=ACCEPT на соединении, но само по себе это
не означает завершения acceptance профиля. `UTF8=ONLY`
не является синонимом `UTF8=ACCEPT` и требует отдельного строгого режима. См.
[RFC 5198](https://www.rfc-editor.org/rfc/rfc5198.html) для Net-Unicode
нормализации, BOM и стабильности Unicode-таблиц. RFC 9755 Sections 3-7
являются нормативным источником для будущего этапа; отдельный runtime acceptance
пока не выполнялся. Начальная lexer-проверка покрывает только выбор режима и
не является отдельным acceptance item с PASS.
