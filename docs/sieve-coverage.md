# Sieve и ManageSieve

Есть bounded parser/executor для обязательного профиля, fileinto, redirect,
keep/discard, header/address/size tests и diagnostics. ManageSieve поддерживает
STARTTLS, SASL PLAIN and SCRAM-SHA-1, literals, CHECKSCRIPT, PUTSCRIPT, LISTSCRIPTS, GETSCRIPT,
SETACTIVE, DELETESCRIPT и RENAMESCRIPT.

RFC 5804 §2.1 string initial-response для AUTHENTICATE PLAIN теперь принимает
и в bounded client-to-server literal форме. Literal проходит через ту же
TLS-before-auth политику; независимая Linux проверка до TLS подтверждает
разбор аргумента ответом `NO "encryption required"`, но не проверяет пароль.
SCRAM-SHA-1 подключён к TLS-only capability и ManageSieve AUTHENTICATE.
Независимый Python TLS peer проверяет client proof, server signature, отказ
неверного proof и последующий NOOP. Litterkeep app interoperability и STARTTLS
SCRAM wire acceptance не проверялись.

RFC 5804 §2.1 challenge/response flow для PLAIN тоже поддержан: отсутствие
initial-response приводит к пустому quoted challenge; ответ может быть quoted
строкой либо bounded client literal, а `*` отменяет exchange. T262 проверил
non-synchronizing literal response независимым Linux STARTTLS peer. T275 отдельно
проверил synchronizing literal после challenge: клиент получил обязательный
continuation до отправки payload и затем успешно аутентифицировал Bob. Проверен
также повторный вход после отмены. Полное соответствие ManageSieve SASL всё ещё
не достигнуто. T327 добавил typed bounded-worker доступ к verifier через
единственного владельца Accounts.
Собственные SHA-1/HMAC-SHA-1/PBKDF2 модули теперь связаны в SCRAM StoredKey и
ServerKey derivation. Полный пример RFC 5802 §5.1 подтверждает client proof и
server signature, а iteration/salt границы ограничены до дорогого KDF вызова.
Argon2 verifier сам по себе не является SCRAM verifier. SASLprep,
Argon2 `.phc2` account storage и безопасная legacy `.phc` миграция реализованы;
T317 подтверждает legacy-переход независимым SMTP TLS клиентом, но это не
проверка ManageSieve SASL. T319 добавил crypto primitive проверки SCRAM proof и
вычисления server signature. T320 добавил versioned случайно salted sidecar для
новых аккаунтов и строгую загрузку verifier. T321 backfill-ит sidecar для
существующего `.phc2` только после успешного парольного входа; legacy `.phc`
миграция также публикует SCRAM sidecar. T322 добавил bounded syntax parser;
T323 исправил его ASCII-only value restriction согласно RFC 5802 §7 и добавил
typed client-first parser с `n,,`, SASLprep username, nonce и optional
extensions. Server-first/client-final/server-final semantic parsing и SASL
state machine подключены в T328. T324 добавил server-first builder, T325
client-final verifier, а T326 typed server-final message. T328 подключил
ManageSieve session/reactor и проверил exchange независимым TLS peer. STARTTLS
и Litterkeep app interop отдельно остаются непроверенными. SCRAM-SHA-256/PLUS
не реализованы. Детали записаны в [учёте зависимостей](dependencies.md).

Для client-to-server literals receiver сначала читает ровно объявленное число
octets, затем обязательный CRLF конца command line; только после него начинает
разбор следующей pipelined команды. Это соответствует framing из RFC 5804
§§1.1 и 4. Конечный string argument не считается завершённым сразу после
payload. Независимый Linux STARTTLS wire-тест подтвердил ожидание разделителя и
сохранение следующей команды; интеграция с клиентом Litterkeep этим тестом не
проверялась. См. [RFC 5804](https://www.rfc-editor.org/rfc/rfc5804.html).

ManageSieve TLS negotiation follows RFC 5804 §§1.7, 2.2: before STARTTLS the
server advertises an empty SASL mechanism list, then after TLS it reissues the
capability block with SASL PLAIN and without STARTTLS. The same refreshed block
is sent after successful AUTHENTICATE and includes the authenticated OWNER;
explicit CAPABILITY preserves OWNER. Repeated STARTTLS is rejected in both
authenticated and unauthenticated encrypted sessions. Independent Linux
socket acceptance verifies greeting, explicit CAPABILITY, TLS transition,
PLAIN authentication and the post-auth transition. Implicit-TLS greeting is
unit-tested; separate implicit-TLS network acceptance and Litterkeep client
interoperability remain unverified. Capability generation always returns
`VERSION "1.0"` with `IMPLEMENTATION` and `SIEVE`, as required for a server
implementing this RFC version; unit and Linux socket tests check the greeting
and common post-transition capability path. The recommended RFC 5804
`UNAUTHENTICATE` extension is advertised in all states. Its command resets the
ManageSieve FSM and the reactor's separate authenticated-account owner, keeps
TLS active, and returns the refreshed capabilities without OWNER. Unit tests
and the independent TLS socket acceptance verify command denial after the
transition and successful reauthentication on the same TLS session.
RFC 5804 §2.13 NOOP is accepted before
authentication; an optional quoted tag is returned in the required `(TAG
"...")` response code with quoted-string escaping. Unit tests and the
independent `tests/blackbox/managesieve_noop_tag.py` Linux socket acceptance
verify untagged/tagged NOOP, escaping and LOGOUT. NOOP literal tags are
supported in synchronizing and non-synchronizing forms, while other command
literal arguments remain unverified. RFC 5804 §2.1 mechanisms PLAIN and
SCRAM-SHA-1 are advertised after TLS. T328 independently verified SCRAM
client proof, server signature, invalid-proof response, and subsequent NOOP
recovery over implicit TLS. STARTTLS SCRAM and Litterkeep app interoperability
remain unverified.

RFC 5804 §2.8 requires `SETACTIVE` to carry an `active-sieve-name`; the empty
string disables active filtering. The parser now rejects a missing argument and
maps `""` to the store's `None` deactivation value, while non-empty script names
remain unchanged. Parser tests cover missing, empty, ordinary and extra
arguments; the independent NOOP wire scenario also asserts that missing
`SETACTIVE` is rejected as a syntax error. RFC 5804 §2.8 additionally says
disabling with an empty string while no script is active is not an error, and
recommends `NONEXISTENT` for an unknown script. Store missing-script errors now
retain a typed cause; ManageSieve returns `NO (NONEXISTENT)` without changing
the original `NotFound` kind used internally. Independent Linux TLS acceptance
`tests/blackbox/managesieve_setactive.py` verifies empty deactivation before
activation, unknown-name rejection, activation in LISTSCRIPTS, deactivation of
that active script, idempotent empty deactivation, and deletion after disable.
This checks the server and Store owner, not Litterkeep app interoperability.

RFC 5804 §2.10 forbids deleting the active script and recommends the `ACTIVE`
response code. The Store now carries a typed active-script failure to the
ManageSieve completion layer, which replies `NO (ACTIVE)` without mutating the
catalog. The same independent TLS scenario attempts DELETESCRIPT while the
script is active, confirms the error and unchanged ACTIVE listing, then
deactivates and successfully deletes it. This is server/store wire coverage,
not Litterkeep app interoperability.

RFC 5804 §2.11 recommends `ALREADYEXISTS` when RENAMESCRIPT targets an
existing script and permits renaming the active script while keeping it active
under its new name. Typed Store collision errors now produce
`NO (ALREADYEXISTS)`; Store reopen coverage verifies collision atomicity and
durable active-name transfer. Independent Linux TLS acceptance
`tests/blackbox/managesieve_rename.py` verifies the exact collision response,
both original scripts after rejection, then activates and renames one script
and confirms its bytes and ACTIVE projection. This is not Litterkeep app
interoperability.

ManageSieve command tokenization now rejects malformed quoted strings instead
of silently dropping an invalid suffix: unterminated strings and unsupported
backslash escapes become syntax errors. Parser regression and independent
Linux wire acceptance in `tests/blackbox/managesieve_noop_tag.py` verify both
forms while retaining valid NOOP tag behavior. T328 has since implemented and
independently wire-tested the required SCRAM-SHA-1 mechanism. SCRAM-SHA-256/PLUS,
STARTTLS SCRAM acceptance and Litterkeep app interoperability remain open.

RFC 5804 §1.2 and §4 define command string values as quoted or literal. The
NOOP optional tag accepts bounded sync and non-sync literal-c2s forms;
independent Linux wire acceptance checks the continuation exchange, direct
non-sync payload, exact TAG echoes and pipeline preservation. GETSCRIPT,
DELETESCRIPT and SETACTIVE also accept final sync/non-sync literal-c2s values.
The zero-octet SETACTIVE literal maps to the same disable state as a quoted
empty string. Linux unit and wire checks cover command conversion and framing.
PUTSCRIPT accepts a literal script name followed by quoted or literal source;
both synchronization modes are covered by Linux unit tests and independent
wire coverage exercises sync name plus quoted source and non-sync name plus
non-sync source with a pipelined NOOP. An additional independent TLS acceptance
uses a non-sync literal name plus sync source, then performs successful
RENAMESCRIPT with two non-sync literal names and activates the result. After a
server restart against the same Store, LISTSCRIPTS preserves the renamed ACTIVE
entry and GETSCRIPT returns byte-identical source. This proves the tested
literal mutation and persistence paths, not every command string position or
Litterkeep application interoperability. RENAMESCRIPT receiver unit tests also
cover mixed forms, empty quoted old name and pipeline preservation.
HAVESPACE accepts a literal name plus one strict decimal size; independent wire
coverage confirms parser/state handling before authentication and pipeline
preservation, not the authenticated quota response. RFC 5804 §2.1 declares both
AUTHENTICATE mechanism and optional initial response as strings, so either may
use quoted or literal framing. The receiver now accepts a quoted or sync/non-sync
literal PLAIN mechanism followed by no response, a quoted response, or a
sync/non-sync literal response. It consumes the complete command before
returning the typed unsupported-mechanism result, preserving a pipelined NOOP.
Linux unit coverage exercises a synchronizing mechanism, and independent TLS
wire acceptance now separately checks synchronizing literal mechanism and
response: the server sends a continuation after each marker, then authenticates
Bob. The same peer rejects a literal CRAM response and successfully executes
the following NOOP. The previous non-synchronizing literal/literal combination
also passed independent wire acceptance.
Other command string positions not listed above remain unverified or unsupported.

RFC 5804 §1.6 ManageSieve script names now reject C0/C1 controls, U+2028/U+2029,
a leading BOM, and Unicode 17.0.0 unassigned scalars. The server accepts the
required 128 Unicode-character boundary (up to 512 UTF-8 octets) and does not
impose a filesystem-style slash restriction. `LISTSCRIPTS` emits names as
quoted strings with quote and backslash escaped, so a stored name cannot forge
another response line. Rust unit checks and an independent STARTTLS client
verify Unicode/special-character round-trip, CRLF/control rejection, the
128/129 boundary, and exact listing bytes. This does not claim complete
Net-Unicode NFC is now applied to stored and transmitted script names. Incoming
decomposed names remain accepted and resolve through their NFC alias. Legacy
catalogs are normalized in memory; canonical-name collisions fail closed rather
than silently dropping a script. RFC 5198 private-use guidance remains
outstanding, as does Litterkeep app interoperability.
See [RFC 5804 §1.6](https://www.rfc-editor.org/rfc/rfc5804.html#section-1.6)
and [RFC 5198 §2](https://www.rfc-editor.org/rfc/rfc5198.html#section-2).

ManageSieve script count, per-script byte size and serialized account catalog
are bounded in the Store. RFC 5804 §1.5 requires a `NO` response containing
the `QUOTA` response code when an imposed quota rejects a command. Store quota
failures now retain a typed source marker across the owner boundary, and
mutation completion maps only that marker to `NO (QUOTA)`; even a filesystem
`StorageFull` error retains the generic failure response. Store tests exercise the
script-count boundary and reactor response tests distinguish quota from disk
errors. Independent Linux TLS acceptance
`tests/blackbox/managesieve_quota.py` uploaded 16 scripts, checked that the
17th receives exact `NO (QUOTA) "quota exceeded"`, and verified LISTSCRIPTS
still returns exactly the original 16. This does not verify Litterkeep client
interoperability.

При runtime error частичный план доставки отбрасывается атомарно и применяется
только implicit keep; committed diagnostic хранит account, версию скрипта,
transaction ID и тип ошибки. Offline-команда `sieve-errors <data-dir>
<account>` печатает TSV с колонкой `outcome=implicit-keep-only`, явно отражая
отсутствие частично применённых Sieve actions. Store regression проверяет
durable диагностику и fallback после повторного открытия; CLI unit regression
проверяет строку отчёта. Независимый сетевой сценарий подтверждает implicit
keep при redirect-limit и redirect-loop; после остановки сервера команда
запущена на тот же persistent store и подтвердила обе committed строки CLI.

RFC 3894 `copy` реализован для `fileinto` и `redirect`; скрипт обязан явно
заявить `require "copy"`, а `fileinto` дополнительно требует `fileinto`.
`fileinto :copy` и `redirect :copy` добавляют действие, не отменяя implicit
keep. Unit tests проверяют capability gating и взаимодействие с последующим
`discard`. Независимый ManageSieve -> SMTP -> IMAP acceptance проверяет
`fileinto :copy`: полные RFC822 bytes в INBOX и `Invoices` совпадают. Новый
`tests/blackbox/sieve_redirect_copy_peer.py` активирует `redirect :copy` через
ManageSieve, проверяет исходную локальную копию через Bob IMAP, останавливает
единственного Store owner и дренирует durable job к независимому SMTP peer.
Peer envelope, точные RFC822 bytes, MIME attachment и его decoded octets
совпадают с локальной копией. Проверен независимый тестовый peer, не Litterkeep
или произвольный сторонний MTA. Правило сверено с RFC 3894 §3.

Повторный `fileinto` в одну папку не считается ошибкой и не создаёт второго
экземпляра письма согласно RFC 5228 §2.10.3. Store-level regression проверяет
сохранение единственной ссылки после restart; независимый ManageSieve -> SMTP
-> IMAP acceptance исполняет два одинаковых `fileinto :copy`, читает обе
папки и сверяет точные RFC822 bytes.

## RFC 5230 vacation: базовый профиль, оставшиеся RFC-пробелы

Parser/executor поддерживают единственный `vacation` action и его RFC-параметры;
ManageSieve объявляет `vacation` и `vacation-seconds`. Кроме RFC 5230 `:days`,
поддерживается RFC 6131 `:seconds`; указать оба параметра нельзя, а seconds
ограничен допустимым RFC диапазоном до локальной политики. Интервалы переводятся
в общий durable suppression expiry. Локальная политика ограничивает интервал
от 60 секунд до 365 дней, включая `:seconds 0`, и использует семь дней по
умолчанию. Store создаёт отдельный RFC 5322 response с `MAIL FROM:<>`, durable
suppression claim, loop/list/automated-message guards, RFC 2047 Subject и
quoted-printable UTF-8 body. `:mime` валидируется с учётом текущего 7BIT-only
SMTP transport. Unit и
Store tests проверяют generation, safety и durable suppression. Независимый
Linux acceptance в `./data/g17-sieve-vacation-rfc45` активировал script через
ManageSieve, доставил три письма через SMTP, передал ровно один из двух
допустимых vacation replies независимому SMTP peer и прочитал все три исходных
RFC822 через IMAP после restart. Peer проверил null reverse-path, адрес и поля
ответа. Тест подтверждает Sieve/Store/SMTP/IMAP интеграцию, но не интеграцию с
Litterkeep app или произвольным сторонним MTA.

По RFC 5230 §4.5 ответ допускается только когда адрес владельца указан в To,
Cc, Bcc, Resent-To, Resent-Cc или Resent-Bcc. Конечный RCPT является известным
адресом владельца, а `:addresses` добавляет дополнительные адреса и не
ограничивает ответ только ими. Unit и сетевой acceptance проверяют
дополнительный alias, duplicate suppression и отказ на чужом To.

История suppression переживает reopen, фиксируется только для committed
transaction ID и имеет тест на ёмкость минимум 1000 response identities. Queue
payload и claim публикуются одной Store transaction.
RFC 5230 §4.2 capacity regression восстановил 1000 уникальных claims с
максимально допустимыми длинами всех трёх ключей; отдельная проверка recovery
удаляет неполный tail. При достижении лимита 8 MiB Store компактизирует только
committed claims, сохраняет последнюю запись для каждого ключа, исключает
истёкшие claims и удаляет старейшие первыми, не опускаясь ниже 1000 записей.
Replacement journal использует временный файл и backup; recovery тестируется
для сбоев до и после публикации нового snapshot. Windows suite: 358 тестов;
Linux Docker target: 399 тестов, strict Clippy и release build. Не моделировался
физический отказ диска/ФС во время sync и rename.
Профиль также проверяет требования RFC 5230:
deduplication ответов по адресу отправителя и идентичности vacation-ответа,
период `:days`, максимум не менее 1000 записей истории, ограничения
автоответов для daemon/list сообщений, один vacation action на script и
сохранение implicit keep. Генерируемый ответ использует пустой SMTP
reverse-path и заголовки/связи ответа, определённые стандартом.
В `Auto-Submitted` распознавание `no` обрезает только RFC WSP (SP/HTAB), чтобы
Unicode NBSP не снимал защиту от loop-ов; malformed NBSP значения оставляют
автоответ подавленным. Keyword теперь разбирается в typed value с RFC 3834
CFWS comments и RFC 2045 token/quoted-string optional parameters; один
синтаксически валидный `no` допускает ответ, остальные keyword, malformed
значения и повтор поля подавляют его. Проверки парсера unit-only, отдельного
runtime acceptance для этих вариантов первоначально не было. Грамматика сверена с
[RFC 3834 §5.1](https://www.rfc-editor.org/rfc/rfc3834.html#section-5.1) и
[RFC 2045 §5.1](https://www.rfc-editor.org/rfc/rfc2045.html#section-5.1).
Отдельный Linux Docker acceptance `tests/blackbox/sieve_autosubmitted_wire.py`
передал восемь независимых входных сообщений SMTP через активный ManageSieve
script: два валидных `no` с CFWS/parameters породили ровно два ответа, а
automated/extension keywords, malformed parameter, duplicate field и NBSP
ответа не породили. Все восемь исходных писем прочитаны через IMAP; независимый
SMTP peer проверил null reverse-path и обе envelope destinations. Независимый
клиент принудительно передал `SMTPUTF8` для всех входных писем, хотя capability
не рекламируется; это серверный wire acceptance, не conformance-проверка
объявленного SMTPUTF8 или Litterkeep-клиента.

Обычный Sieve redirect по-прежнему ссылается на blob входящего письма, тогда как
`Store::enqueue_outbound_payload` создаёт отдельную transaction/blob для
сгенерированного сообщения. Интеграция стороннего MTA не проверена. Требования
сверены с RFC 5230 §§4.1-4.7 и 5.1-5.8.

RFC 5230 `:mime` CHECKSCRIPT теперь отвергает повторяющиеся
`Content-Transfer-Encoding` и тело, которое не декодируется заявленным
7bit/base64/quoted-printable. Используется общий типизированный MIME decoder;
его base64 проверяет финальность padding и нулевые pad bits. Unit regression
покрывает неверные символы, данные после padding, некорректный QP escape,
не-ASCII 7bit и duplicate CTE. Независимый
`tests/blackbox/managesieve_vacation_mime.py` по TLS ManageSieve подтвердил
CHECKSCRIPT для корректных 7bit, quoted-printable и Base64 entities, а также
отказ для повреждённого Base64, duplicate CTE и encoded multipart.
Общий CTE parser принимает только один ASCII token с внешним SP/HTAB; Unicode
whitespace и tspecials не нормализуются в известное имя encoding. RFC comments
в MIME entity удаляются до разбора token. Независимый CHECKSCRIPT подтвердил
приём `base64` с comments и отказ для NBSP после token.
Перед сохранением :mime-скрипта также проверяется `MIME-Version`: допускается
не более одного значения версии 1.0, RFC 822 comments удаляются без склейки
через реальный whitespace внутри значения; неизвестная версия, повторные поля
и whitespace-разбитая декларация отвергаются. Это не позволяет выдать MIME-
ответ с неподдерживаемой или ошибочно нормализованной версией. Границы
проверяются unit-тестами и независимым ManageSieve CHECKSCRIPT acceptance.

Отдельный `vacation_mime` validator применяет ограничения RFC 2045 §§2.7, 6.7
и 6.8: 7bit body не может содержать NUL, bare CR/LF и строки длиннее 998
октетов; Base64/QP требуют CRLF, разрешённые encoded octets и строки максимум
76 символов. QP также запрещает незакодированные завершающие пробелы и табы.
Unit cases проверяют обе границы строк, NUL, bare LF и SP перед soft break.
RFC 2045 [§6.7](https://www.rfc-editor.org/rfc/rfc2045.html#section-6.7)
разрешает whitespace перед завершающим `=` мягкого переноса:
это часть `qp-segment`, не transport-padding. Независимый ManageSieve CHECKSCRIPT
принимает такой корректный soft break и отвергает malformed hex escape и
незакодированные завершающие SP/HTAB. Эти cases проверяют end-to-end парсер
Sieve,
валидатор MIME и wire response, но не все граничные длины строк, которые
остаются unit-only.

RFC 2045 §6.4: encoded transfer forbidden for composite `multipart/*` and
`message/*`. Validator разбирает `Content-Type` в typed media type, отвергает
base64/QP на этих составных сущностях и повторный Content-Type. Unit cases
проверяют `multipart/mixed`, `multipart/related`, `message/partial`, comment
между type и slash, quoted boundary с круглыми скобками, допустимый 7bit
multipart и повторное поле. Независимый ManageSieve acceptance дополнительно
подтвердил отказ на encoded `multipart/mixed`; остальные перечисленные
structured-value границы остаются unit-only.

Общий MIME structured-value parser удаляет вложенные RFC comments вне
quoted-string и проверяет `type/subtype` как две непустые RFC token-части,
разделённые ровно одним slash. Неизвестные, но синтаксически корректные типы
сохраняются как typed `Other`; пустые, содержащие whitespace/tspecials и
лишний slash значения отвергаются. Unit tests покрывают comments вокруг
разделителя и внутри quoted parameter, nested/unterminated comments и
некорректные token-ы. Независимый TLS ManageSieve CHECKSCRIPT теперь также
принимает `multipart (outer(inner))/related` с quoted boundary, содержащим
скобки, и отвергает subtype с запрещённым tspecial. Остальные границы из
unit-набора ещё не получили отдельного wire case.

Для параметров Content-Type/Content-Disposition проверяются RFC 2045 §5.1
attribute=value, token/quoted-string и список tspecials. RFC 2231 attribute
грамматика исключает зарезервированные символы, а continuation номера должны
начинаться с секции 0 и далее идти без ведущих нулей. Unit cases покрывают
пробелы/tspecials в unquoted значениях, повреждённые имена, внутреннюю
неэкранированную кавычку, корректный escaped DQUOTE, пустой quoted value и
filename continuation. Сетевой MIME acceptance этих новых границ не выполнялся.
T329 добавил сетевую регрессию conflicting regular/extended filename в
Content-Disposition и подтвердил, что MIME tree не публикует спорное имя,
сохраняя оригинальные RFC822 bytes. Это консервативная политика ambiguity, а
не нормативное правило precedence RFC 2231. Следующий acceptance T330 проверяет
RFC 2183 §2.8: неизвестный charset отдельного `filename*` пропускается, а
известный `size=12` и disposition сохраняются; RFC822 literal остаётся точным.
Неизвестный disposition type должен проецироваться как `attachment`; локальные
unit-тесты и IMAP STARTTLS acceptance T331 подтверждают fallback и сохранение
filename при byte-exact сохранении исходного RFC822 literal.
RFC 2183 `size` принимает только непустые десятичные цифры: `size=12x`
отбрасывается отдельно, сохраняя валидный `filename`; это покрыто unit и IMAP
STARTTLS acceptance. Date-параметры закрыты узкими проверками T333-T336;
T337 отдельно подтверждает, что MIME RFC 2183 сохраняет numeric-only timezone,
даже когда RFC 5322 Date parser принимает зарегистрированные obsolete zones.
T333 добавил quoted RFC 2183 date-параметры с календарной валидацией,
числовой timezone, сверкой weekday и leap-second `:60`; unit и IMAP STARTTLS
проверили accepted и rejected формы. T334 добавил RFC 5322 pivot для numeric-
zone obsolete 2/3-digit year. Именованные зоны и некоторые прочие obsolete
RFC 822 variants всё ещё непокрыты, поэтому полная date-time совместимость не
заявляется.
T335 закрепил RFC 5322 ширины day и clock fields, включая optional seconds и
`:60`; независимый IMAP BODYSTRUCTURE test отвергает однозначные и чрезмерно
широкие numeric fields.
Whitespace около `=` в параметрах остается открытым и отдельно не ужесточается.

RFC 2231 extended payload decoder принимает только attribute-char или
percent-encoded octets, а hexadecimal digits в escapes принимаются без учёта
регистра по ABNF. Unit tests проверяют lowercase/mixed-case UTF-8 escapes в
общем parser-е и в Sieve vacation `:mime` validator; независимый IMAP wire test
подтверждает decoded filename в BODYSTRUCTURE. Проверяются mixed encoded и
unencoded continuation segments. Charset декодируется явным allowlist известных
профилей: US-ASCII, UTF-8, UTF-32, UTF-32BE, UTF-32LE,
ISO-8859-1, ISO-8859-2, ISO-8859-3, ISO-8859-4,
ISO-8859-5, ISO-8859-6, ISO-8859-7, ISO-8859-8,
ISO-8859-9, ISO-8859-10, TIS-620 (ISO-8859-11), ISO-8859-13, ISO-8859-14,
ISO-8859-15, ISO-8859-16,
Windows-1250, Windows-1251, Windows-1252, Windows-1253, Windows-1254,
Windows-1255, Windows-1256, Windows-1257, Windows-1258, Windows-874,
UTF-16, UTF-16BE и UTF-16LE. UTF-16 и UTF-32 profiles доступны для RFC 2047
и RFC 2231, но исключены из MIME `text/*` decoding при поиске: UTF-16 по
RFC 2781, а UTF-32 из-за IANA suitability registration. RFC 2231 начальная
секция может опускать charset и language:
непомеченный ASCII payload принимается, а high-bit octets без charset
отвергаются, поскольку Unicode API не может безопасно угадать их кодировку.
Языковой tag проверяется на well-formed BCP 47 структуру RFC 5646 и сохраняется
без изменения регистра в `StructuredValue` и каждом MIME tree entity. Проверка
не доказывает членство subtags в датированной IANA Language Subtag Registry.
RFC 2231 charset/language regressions unit-only.

ISO-8859-15 декодирует все 256 октетов по таблице Unicode Consortium и
принимает IANA labels `ISO_8859-15`, `Latin-9` и `csISO885915` вместе с
предпочтительным именем. Full-byte regression проверяет евро и все восемь
замен относительно ISO-8859-1; подключение через сетевой клиент отдельно не
проверялось. ISO-8859-16 также декодирует полный 256-октетный профиль Latin-10,
включая румынские comma-below буквы, центральноевропейские замены и IANA aliases
`ISO_8859-16:2001`, `ISO_8859-16`, `latin10`, `l10`, `iso-ir-226`,
`csISO885916`; mapping покрывается full-byte oracle, но сетевое подключение не
проверялось.

Зарегистрированный IANA charset [windows-1250](https://www.iana.org/assignments/charset-reg/windows-1250)
декодируется по [полной таблице Microsoft/Unicode](https://www.unicode.org/Public/MAPPINGS/VENDORS/MICSFT/WINDOWS/CP1250.TXT): все 256 октетов сверяются независимо, пять undefined positions
0x81/0x83/0x88/0x90/0x98 отклоняются, а IANA alias `cswindows1250` принимается.
`cp1250` намеренно не считается alias, согласно registry. Новые проверки unit-only.

IANA charset `windows-1253` реализуется отдельной Greek mapping: все байты
проверяются по таблице Microsoft/Unicode, 17 undefined octets не подменяются
replacement character, принимается alias `cswindows1253`, а `cp1253` не
принимается. Сетевой client acceptance для этой charset не выполнялся.

IANA charset [windows-1254](https://www.iana.org/assignments/charset-reg/windows-1254)
декодируется по полной [таблице Microsoft/Unicode CP1254](https://www.unicode.org/Public/MAPPINGS/VENDORS/MICSFT/WINDOWS/CP1254.TXT).
Пооктетный oracle проверяет все 256 значений и семь undefined positions;
турецкие I/Ğ/Ş замены и alias `cswindows1254` проверены, `cp1254` не принимается.
Сетевой client acceptance для этой charset не выполнялся.

IANA charset [windows-1255](https://www.iana.org/assignments/charset-reg/windows-1255)
декодируется по полной [таблице Microsoft/Unicode CP1255](https://www.unicode.org/Public/MAPPINGS/VENDORS/MICSFT/WINDOWS/CP1255.TXT).
Oracle сверяет все октеты, включая Hebrew points, Yiddish ligatures, RTL marks
и 23 undefined positions; `cswindows1255` принимается, `cp1255` не считается
зарегистрированным alias. Сетевой client acceptance не выполнялся.

IANA charset [windows-1256](https://www.iana.org/assignments/charset-reg/windows-1256)
декодируется по полной [таблице Microsoft/Unicode CP1256](https://www.unicode.org/Public/MAPPINGS/VENDORS/MICSFT/WINDOWS/CP1256.TXT).
Независимый oracle проверяет каждый octet, включая арабские расширенные
буквы, combining marks, ZWNJ/ZWJ и LRM/RLM; `cswindows1256` принимается,
`cp1256` не является зарегистрированным alias. Сетевой client acceptance
не выполнялся.

IANA charset [windows-1257](https://www.iana.org/assignments/charset-reg/windows-1257)
декодируется по [Microsoft/Unicode CP1257 mapping](https://www.unicode.org/Public/MAPPINGS/VENDORS/MICSFT/WINDOWS/CP1257.TXT).
Проверен полный диапазон octet values, 12 undefined positions отклоняются;
проверяются литовская/латышская диакритика и `cswindows1257`, тогда как
`cp1257` не принимается как незарегистрированный alias. Сетевой client
acceptance не выполнялся.

IANA charset [windows-1258](https://www.iana.org/assignments/charset-reg/windows-1258)
декодируется по [Microsoft/Unicode CP1258 mapping](https://www.unicode.org/Public/MAPPINGS/VENDORS/MICSFT/WINDOWS/CP1258.TXT).
Пооктетная проверка покрывает все значения, включая combining grave, acute,
hook, tilde и dot-below; Vietnamese sample сохраняет decomposed scalar order
без нормализации. `cswindows1258` принимается, `cp1258` по IANA не alias.
Сетевой client acceptance не выполнялся.

IANA charset [windows-874](https://www.iana.org/assignments/charset-reg/windows-874)
реализован по [Microsoft/Unicode CP874 table](https://www.unicode.org/Public/MAPPINGS/VENDORS/MICSFT/WINDOWS/CP874.TXT)
отдельно от TIS-620/ISO-8859-11. Полный octet space и 31 undefined positions
покрыты тестами; Thai sample и baht sign проверяют назначение, отличающееся
от ISO-8859-11. `cswindows874` принимается как IANA registry alias, `cp874`
согласно регистрации не alias. Wire acceptance не выполнялся.

Зарегистрированный IANA charset windows-1252 теперь декодируется как отдельный
профиль, не смешивая его с ISO-8859-1. Проверены все 32 octets диапазона
0x80..0x9F, RFC 2231 filename с encoded Euro/en dash и MIME body decoding в
IMAP SEARCH projection. Поддерживается только
каноническое MIME-имя windows-1252; WHATWG aliases вроде cp1252 не принимаются
как IANA MIME labels. Новые проверки unit-only.

Quoted и `text:` multiline strings принимают CRLF; multiline форма сохраняет
terminating CRLF тела, снимает один dot-stuffing octet и ограничена 65 536
байтами. Для `text:` допускаются SP/HTAB и hash-comment перед обязательным
CRLF, но не bracket-comment. Lexer unit tests и независимый ManageSieve
CHECKSCRIPT проверяют соответственно payload/dot-stuffing/пустое тело и
принятие корректного скрипта с отказом на отсутствующий terminator.
В базовом Sieve `require` разрешён только до первой команды, но несколько
объявлений `require` подряд допустимы; unit regression проверяет оба правила
по RFC 5228 §3.2. Quoted-string сохраняет CRLF, допускает `quoted-other`
escape с удалением обратной косой черты и отвергает NUL; из пробельных
разделителей вне строк принимаются только SP/HTAB/CRLF. `allof`/`anyof`
принимают непустой comma-separated `test-list` в круглых скобках; квадратные
скобки остаются формой непустого `string-list`, где запятая обязательна между
элементами и не допускается на краях. Unit и ManageSieve CHECKSCRIPT acceptance
проверяют эти границы по RFC 5228 §§2.4.2, 5.2-5.3 и 8.1-8.2. Это не означает
полного покрытия grammar RFC 5228 и extensions.
Hash-comment обязан завершаться CRLF; предварительная проверка также отвергает
одиночные CR/LF и NUL в quoted strings и обоих видах комментариев. Unit и
ManageSieve CHECKSCRIPT acceptance проверяют корректный CRLF и отказ на bare LF
по RFC 5228 §§2.1-2.3, 8.1.
`:user` и `:detail` реализованы как RFC 5233 `subaddress`; оба требуют явного
`require "subaddress"`, а ManageSieve greeting и CAPABILITY объявляют это
расширение. Сервер использует первый `+` в local-part как separator: без него
`:detail` не создаёт значение для сравнения, а `user+@domain` создаёт пустой
detail. CHECKSCRIPT и адресные unit tests проверяют эти случаи. Другие политики
разделителя subaddress пока не настраиваются. ADDRESS-PART, COMPARATOR и
MATCH-TYPE перед позиционными списками принимаются в любом порядке, как
предписывает RFC 5228 §2.6.2; повтор одного такого аргумента отвергается.
Unit-тест проверяет case-sensitive семантику `i;octet`, а ManageSieve
CHECKSCRIPT и SMTP -> Sieve -> IMAP acceptance проверяют grammar и исполнение
reordered script. `:matches` сравнивает UTF-8 byte stream с width wildcard `?`
в один octet для обязательных comparator-ов `i;octet` и `i;ascii-casemap`;
`\*`/`\?` в Sieve string после lexical unescape становятся literal wildcard.
Unit и SMTP -> Sieve -> IMAP acceptance проверяют эти правила по RFC 5228 §2.7.1.
`address` использует общий RFC 5322 parser, проверяет известные address-list
поля (включая обязательные RFC 5228 From/To/Cc/Bcc/Sender/Resent-From/
Resent-To), RFC 5322 reply/resent поля и `Disposition-Notification-To` из
RFC 8098 §2.1, значение которого задано как `mailbox-list`. Mailbox внутри
групп сравниваются, имя группы не сравнивается. Unit и независимая
ManageSieve -> SMTP -> IMAP acceptance проверяют MDN-поле; письмо попадает в
Invoices, а исходная INBOX копия побайтно совпадает благодаря implicit keep и
`fileinto :copy`. Проверено также, что совпадение с одноимённым Subject не
подменяет address match. Полевая матрица остаётся конечным allowlist:
произвольные пользовательские address-list headers не подтверждены.

Sieve `envelope` поддерживает `from` и `to`, `ADDRESS-PART`, match-type и
comparator по RFC 5228 §5.4; script обязан заявить `require "envelope"`.
ManageSieve объявляет capability. `from` берётся из исходного SMTP MAIL FROM до trace-заголовка;
null reverse-path сравнивается как пустая строка независимо от ADDRESS-PART.
`to` берётся из RCPT TO, сопоставленного локальному mailbox, и не выводится из
MIME `To`. При нескольких SMTP paths к одному mailbox delivery план
вычисляется один раз по первому соответствующему RCPT. Unit tests проверяют
capability, from/to и null sender; независимая SMTP -> Sieve -> IMAP acceptance
использует настоящий RCPT TO как условие помещения в папку.

Поддерживаемое optional-действие Sieve `fileinto` отвергается при static
validation, если script не заявил `require "fileinto"`. Это проверяется unit
regression и независимым ManageSieve CHECKSCRIPT; тот же сетевой acceptance
подтверждает доставку с объявленной capability.

Sieve `redirect` проверяет mailbox по грамматике RFC 5228 §2.4.2.3 через общий
RFC 5322 parser: допустим один `addr-spec` либо quoted phrase перед
`<addr-spec>`. Mailbox lists, source routes, groups и некорректные адреса
отвергаются при static validation. Unit tests и отдельный независимый
ManageSieve CHECKSCRIPT `tests/blackbox/managesieve_redirect_validation.py`
проверяют принятие и отказ. Сквозная доставка redirect после завершения source
Store через durable queue к независимому SMTP peer проверена в T119:
сравниваются envelope, полные RFC822 octets с локальной implicit-keep копией и
байты MIME-вложения. Это не подтверждает взаимодействие с Litterkeep или
произвольным внешним MTA.

Sieve `header` декодирует RFC 2047 encoded-word общей MIME-декодировкой до
сравнения в UTF-8 и игнорирует ведущие/завершающие SP и HTAB по RFC 5228 §5.7.
Unit покрывает полное преобразование ISO-8859-1 и ISO-8859-2, ASCII subsets
остальных зарегистрированных предпочитаемых ISO-8859 MIME labels, UTF-8
encoded-word и краевые пробелы. Для ISO-8859-2 тестируется полный диапазон
A0..FF и польский текст. ISO-8859-3 декодирует назначенные позиции и отвергает
undefined octets A5, AE, BE, C3, D0, E3 и F0; прочие неподдержанные high-bit
таблицы не преобразуются. ISO-8859-4 проверяет все assigned позиции и
отвергает undefined octets A5, AE, BE, C3, D0, E3 и F0. ISO-8859-5 проверяет
все 256 байтов, включая C1 controls и кириллическое отображение, отдельное от
Windows-1251. ISO-8859-6 проверяет арабские буквы и punctuation, сохраняет
ASCII/C1 controls, а undefined позиции отклоняет как ошибку. ISO-8859-7
проверяет Greek, Euro, final sigma и undefined байты AE, D2, FF.
ISO-8859-8 проверяет Hebrew text, directional marks, ASCII/C1 controls,
все назначенные позиции и отказ для undefined octets. Это visual-order базового
профиля; RFC 1556 -i/-e остаются отдельными неподдерживаемыми профилями.
ISO-8859-9 проверяет все 256 октетов по таблице Unicode, турецкие замены
и сохранение C1 controls; также принимаются IANA aliases `latin5` и `l5`.
ISO-8859-10 проверяет все 256 октетов Nordic/Latin-6 mapping по таблице Unicode,
ASCII/C1 controls, Icelandic/Nordic text и IANA aliases `latin6`/`l6`.
TIS-620 использует официальный sparse mapping с NBSP и Thai letters; тестирует
Thai sample, C1 range и каждый assigned/undefined octet. Предпочитаемое IANA
имя TIS-620 указывает ISO-8859-11 как alias; неопределённые DB..DE и FC..FF
отклоняются.
ISO-8859-13 проверяет Baltic Rim Unicode mapping для всех 256 байтов,
включая C1 controls, Baltic letters и изменённые кавычки/диакритические знаки;
принимаются IANA preferred label и зарегистрированный `csISO885913`.
ISO-8859-14 проверяет полный Celtic mapping, отличающиеся от Latin-1 letters,
C1 range и IANA aliases `latin8`, `l8`, `iso-celtic`, `csISO885914`.
Это реализует обязательный ASCII-subset минимум
[RFC 5228 §2.7.2](https://www.rfc-editor.org/rfc/rfc5228.html#section-2.7.2),
не полный набор конвертеров. Независимая
ManageSieve -> SMTP -> IMAP acceptance отправляет encoded UTF-8 Subject и
проверяет его Unicode-сопоставление при выборе папки. RFC 2047 decoder также
требует непустой `encoded-text` из печатных ASCII octets без SP
и `?`; Q-escape `=01` остаётся допустимым encoded octet. Unit tests покрывают
raw whitespace/control/non-ASCII и сохранение повреждённого слова в header
projection; новые grammar cases отдельно по сети не проверялись. Общий header
parser распознаёт целый encoded-word только в начале field-body или после
SP/HTAB и оставляет embedded/слитный токен обычным текстом по RFC 2047 §6.1.
RFC 2231 §5 encoded-word форма `charset*language` доступна через typed
`decode_with_metadata`: BCP 47 language сохраняется отдельно от decoded text.
Обычные `decode`, `decode_header` и `decode_phrase` возвращают только текст;
raw message headers остаются исходным источником metadata. Новые language-tag
границы покрыты unit-only, актуальная IANA registry validity не проверяется.
Эта граница пока покрыта unit-only. Header parser также игнорирует SP/HTAB
между field-name и `:`, как требует §2.4.2.2;
сетевой тест подтверждает, что `Subject \t:` маршрутизируется по Sieve, а IMAP
FETCH возвращает исходный байтовый заголовок без переписывания. Header matcher
принимает пустой ключ и различает `:contains` (пустая подстрока содержится в
значении существующего поля) от `:is` (пустое значение не равно непустому).
Невалидное имя поля в `header` test принимается как заведомо не совпадающее,
а не как ошибка скрипта; эти случаи проверены unit-тестом и ManageSieve CHECKSCRIPT.

Для `:contains` с обязательным comparator `i;ascii-casemap` применяется KMP:
поиск идёт за O(n + m), хранит O(m) нормализованных байтов ключа и prefix table,
не создавая копию всего значения заголовка. Сравнение сворачивает только ASCII
байты; прочие UTF-8 octets остаются точными. Adversarial unit test проверяет
линейный промах на большом значении и длинном повторяющемся ключе; в одном
локальном Windows Rust debug прогоне он изменился с 1.29s на 0.01s. Это одна
регрессионная замеренная конфигурация, не production benchmark. `i;octet`
`:contains` и glob `:matches` этой оптимизацией не затронуты; полный профиль
RFC 5228 и acceptance через приложение Litterkeep не подтверждены.

IANA charset [IBM866](https://www.iana.org/assignments/charset-reg/ibm866)
декодируется по опубликованной [таблице Unicode CP866](https://www.unicode.org/Public/MAPPINGS/VENDORS/MICSFT/PC/CP866.TXT).
Все 256 октетов сверяются с отдельным ожидаемым mapping; проверены русские
буквы, Ё/ё, украинские Є/є, псевдографика, знак валюты и NBSP. Принимаются
зарегистрированные имена IBM866, CP866, csIBM866 и 866. Проверка покрывает
MIME charset decoder unit-тестами; сетевое использование и клиентское
подключение отдельно не проверялись.

UTF-32, UTF-32BE и UTF-32LE зарегистрированные labels декодируются как Unicode
scalars в RFC 2047 encoded-word и RFC 2231 extended values. Generic UTF-32 по
IANA использует big-endian при отсутствии BOM и снимает только BE BOM; invalid
surrogates и scalars выше U+10FFFF отвергаются. Эти labels не допускаются для
MIME `text/*` decoding, поскольку IANA помечает их неподходящими для MIME text.
Unit-тесты проверяют decoder, RFC 2047 и RFC 2231 parser integration.
Независимый STARTTLS SMTP -> файловое хранилище -> IMAP acceptance также
подтверждает сохранение encoded Subject и выдачу decoded attachment filename
`😀` в BODYSTRUCTURE. Проверка выполнена скриптом
`tests/blackbox/smtp_imap_utf32_charset.py`; POP3, Sieve, ManageSieve и
приложение Litterkeep в этом сценарии не участвовали.

Отдельная интеграционная регрессия проверяет IBM866 не только decoder-ом:
RFC 2047 Base64 encoded-word темы `Привет` декодируется из CP866 octets, а
RFC 2231 `filename*` возвращает `Привет.txt`. Это parser unit acceptance;
отправка такого сообщения через SMTP и просмотр через IMAP/POP3 не проверены.
