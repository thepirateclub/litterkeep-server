# Покрытие протоколов

## SMTP

Submission поддерживает implicit TLS и STARTTLS, EHLO, AUTH PLAIN, MAIL SIZE,
несколько RCPT, DATA с однократным dot-unstuffing, RSET, NOOP и QUIT. Также
рекламируются RFC 2920 `PIPELINING` и RFC 6152 `8BITMIME`. Независимый
STARTTLS peer подтвердил PIPELINING: один сетевой write содержит MAIL,
ошибочный RCPT, успешный RCPT и DATA; ответы должны сохранять порядок, принятый
адресат не теряется, а письмо читается Bob через IMAP. Остальные SMTP extensions
оцениваются отдельно. `BODY=8BITMIME` разрешает high-bit octets,
которые сохраняются до IMAP, а без него DATA с такими octets отклоняется после
dot terminator. `BODY=7BIT` и режим без BODY ограничивают содержимое US-ASCII.
DATA parser отдельно отслеживает NUL и после полного dot terminator отвергает
его кодом `554 5.6.0`; этот октет запрещён в MIME 7bit и 8bit data. Лимит
размера сохраняет отдельный ответ `552 5.3.4`. Unit-проверка и независимый
STARTTLS wire-тест подтверждают отказ NUL в обоих режимах, enhanced status,
возврат в command state, последующую валидную доставку и отсутствие отвергнутых
сообщений в IMAP. В wire-тесте восстановление подтверждено `NOOP`, не `RSET`.
Теперь тот же DATA validator отвергает одиночные CR и LF: independent
STARTTLS peer проверил оба байта при обоих BODY-профилях, ответ `554 5.6.0`,
последующий NOOP, валидную SMTP delivery и отсутствие четырёх rejected messages
в IMAP. MIME line endings принимаются только как исходная пара CRLF; отдельный
BINARYMIME regression сохраняет произвольные NUL/high-bit/bare CR/LF octets в
BDAT. См. [RFC 2045 §§2.7-2.9](https://www.rfc-editor.org/rfc/rfc2045.html#section-2.7)
и [RFC 6152 §3](https://www.rfc-editor.org/rfc/rfc6152.html#section-3).
До TLS AUTH запрещён. Ненулевой envelope sender должен принадлежать
authenticated owner. RFC 6409 §3.2 разрешённый `MAIL FROM:<>` не отклоняется
только из-за пустого reverse-path: authenticated identity даёт MSA return path;
локальная доставка сохраняет `Return-Path: <>`. Независимый implicit-TLS peer
проверяет также, что чужой ненулевой sender получает `550 5.7.1` согласно
RFC 6409 §6.1. Local route не является open relay.

Входящий SMTP command idle deadline составляет пять минут, а DATA body использует
отдельный трёхминутный срок бездействия на принимаемый блок. Таймер учитывает
фактические socket reads, не повторную обработку уже буферизованных строк.
Независимый Linux STARTTLS peer после AUTH дождался TCP EOF через 300.1 секунды
в command state; отдельный peer после `354` и неполного body block получил EOF
через 180.1 секунды. Детерминированные lifecycle tests отдельно проверяют
границу срока, освобождение lease и взаимодействие с pending worker. Это
server-side timeout acceptance, не тест приложения Litterkeep.

Повторная wire-проверка отделяет сброс срока от первоначального deadline:
после первого неполного DATA block независимый STARTTLS peer ждал 120 секунд,
передал свежие body octets и получил TCP EOF через 180.1 секунды после второго
socket read. Тем самым проверено, что новый фактически принятый блок запускает
полный трёхминутный срок заново, а не только что первоначальный срок истекает.

RFC 3030 `CHUNKING` объявляется через EHLO во всех режимах SMTP и принимает
`BDAT n [LAST]` с точным octet framing, нулевыми и промежуточными chunks.
Промежуточные chunks получают отдельный 250; `LAST` проверяет согласованный
7BIT/8BITMIME/BINARYMIME профиль и передаёт message единственному durable
Store owner.
Ошибка размера или последовательности переводит envelope в failed state до
RSET, а отвергнутый payload точно дренируется без потери следующей команды.
Независимый Linux TLS peer проверил zero-length и два message chunks, успешную
доставку Alice -> Bob и точное совпадение декодированных MIME attachment bytes;
второй raw peer проверил capability, отказную BDAT без MAIL/RCPT и сохранение
границы перед pipelined BDAT и NOOP. После 503 на отвергнутый chunk сервер
сохраняет failed state: следующий pipelined chunk дренируется и получает 503,
команды до RSET тоже получают 503. RSET очищает состояние, после него NOOP
получает 250. Отдельно проверены invalid content на LAST и chunk размера
26 214 401 октет: ответы 552 отправлены только после drain, затем RSET и NOOP
успешно восстановили сессию. Перезапуск Store между отправкой и IMAP чтением не
выполнялся. Inbound объявляет `BINARYMIME` вместе с `CHUNKING`, принимает
`MAIL ... BODY=BINARYMIME` и завершает только BDAT-транзакции. RFC 3030 text
parts и multipart preamble/epilogue проверяются на canonical CRLF без DATA
ограничения строки в 1000 октетов; non-text bodies сохраняются непрозрачно.
`DATA` после такого MAIL получает 503 и переводит сессию в режим, требующий
RSET. Если MIME-заголовки содержат UTF-8 field values, BINARYMIME LAST также
требует установленный в MAIL флаг `SMTPUTF8`; без флага сообщение отклоняется.
Independent Linux implicit-TLS peer проверил обе ветви: без MAIL SMTPUTF8
UTF-8 заголовки отклонены на BDAT LAST, с MAIL SMTPUTF8 exact payload принят,
извлечён через IMAP и пережил restart. Это ранняя forced-parameter проверка;
новая проверка объявления capability и conforming U-label пути описана ниже.
Independent peer подтвердил
полный SMTP -> файловый Store -> IMAP сценарий для NUL, high-bit, bare LF/CR bytes, запрет DATA,
отказ на noncanonical text, восстановление через RSET, точное чтение Bob и
повторное чтение после рестарта сервера. Это локальный independent peer, не
Litterkeep app. Outbound relay реализует RFC 3030 для MIME binary parts:
требует одновременно peer capabilities `BINARYMIME` и
`CHUNKING`, валидирует MIME headers и canonical CRLF текстовых частей, объявляет
точный BDAT payload size и посылает последовательные bounded chunks с ожиданием
отдельного `250` после каждого. DATA-ограничение 1000 октетов не переносится на
`text/*` в BINARYMIME: требуется только canonical CRLF; multipart preamble и
epilogue проверяются без изменения binary body parts. После 4xx/5xx следующий
chunk не отправляется.
Plain TCP и implicit TLS independent peers подтвердили точные байты, включая
NUL/high-bit octets и нетекстовые bare CR/LF; отдельные тесты подтвердили отказ
без capability и остановку после 451. RFC 5321 MAIL line limit включает
добавочные RFC 3030 octets и суммируется с RFC 6531 allowance. Это unit peer
coverage, не outbound доставка на реальный внешний сервер или Litterkeep app.
См. [RFC 3030 §§2-3](https://www.rfc-editor.org/rfc/rfc3030.html#section-2).

По рекомендации RFC 8314 §3.4 implicit-TLS SMTP-сессия отвечает на
peer-инициированный `close_notify` после обычного NOOP reply. Независимый Linux
TLS peer подтверждает SMTP reply и завершает TLS `unwrap()` без SMTP QUIT.
Отдельный сквозной acceptance проходит SMTP AUTH и DATA через implicit TLS,
читает успешный `250`, отправляет `close_notify` без QUIT и находит доставленное
письмо в Bob INBOX через IMAP. Peer close до получения DATA acknowledgement
отдельно не проверен.
Отдельное acceptance проверяет тот же lifecycle для IMAP; POP3 проверяется
отдельно ниже. Это closure coverage, не полный SMTP/TLS профиль.

Дополнительный RFC 2920 §3.2 STARTTLS peer отправил MAIL, единственный
несуществующий RCPT и DATA одним write. Получены упорядоченные `250`, `550`,
`503`: DATA не открыл content state без принятого адресата, а следующий NOOP
и QUIT прошли без потери синхронизации. Это проверка поведения all-rejected
pipeline, а не общее доказательство всего SMTP расширения.

SMTP AUTH PLAIN поддерживает оба RFC 4954 варианта: initial response в команде
и отдельный ответ после пустого `334 `. SASL cancel `*` и недекодируемый ответ
получают 501; после отмены или ошибки command stream восстанавливается. После
успешной аутентификации повторный AUTH отклоняется кодом 503. Независимый
Python `smtplib` на STARTTLS проверил continuation, initial-response и recovery;
другие SASL mechanisms не реализованы и не рекламируются. Ответ continuation
ограничен общим RFC 5321 максимумом 512 октетов вместе с CRLF: 513 октетов
получают 500, после чего клиентская сессия возвращается в command state.
Проверено независимым STARTTLS black-box. См. [RFC 5321 §4.5.3.1.4](https://www.rfc-editor.org/rfc/rfc5321.html#section-4.5.3.1.4).

Аргумент HELO проверяется как единственный RFC 5321 Domain. EHLO принимает
единственный Domain либо валидный IPv4/IPv6 address-literal; пустой аргумент,
лишние слова, лишние пробелы и ошибочные DNS labels получают 501. Валидные
приветствия принимаются независимым Linux raw-TCP клиентом после серии отказов.

Неизвестные SMTP-команды получают `500 5.5.2`, а неверный аргумент известной
команды - `501 5.5.2`. В том же соединении после обоих отказов принимаются
следующие команды. Это различие кодов задано RFC 5321 §4.2.2 и проверяется
воспроизводимым Linux raw-TCP сценарием
[`smtp_unknown_command.py`](../tests/blackbox/smtp_unknown_command.py); тест не
использует клиент Litterkeep.
Аргументы HELP/NOOP, где grammar допускает только ASCII, не проходят
Unicode-aware `trim`: NBSP не маскирует невалидный байт как пустой optional
argument. `NOOP ` и `HELP ` возвращают 501; затем NOOP/QUIT в том же TCP
соединении подтверждают recovery. VRFY/EXPN имеют отдельную RFC 6531 String
грамматику, которая допускает UTF-8 query даже при отсутствии reply marker-а.
Сервер не возвращает UTF-8 identity в ответе.
Парсер отдельно понимает RFC 6531 §3.7.4.2 запросы VRFY/EXPN с valueless
`SMTPUTF8` и UTF-8 query, сохраняет режим reply encoding и отвергает параметр
со значением. Marker обозначает способность клиента принимать UTF-8 в ответе и
не запрещает UTF-8 в самом query. Сервер отвечает privacy-safe `252`/`502` и
не выдаёт non-ASCII identity. Независимый Linux wire test проверил
ASCII/UTF-8 query с marker-ом и без него, malformed marker и session recovery.
Это не interoperability-тест конкретного клиента.

В production `MAIL FROM:` и `RCPT TO:` angle-path начинается сразу после
двоеточия: пробел и табуляция между delimiter-ом команды и `<` получают 501,
а последующая команда остаётся синхронизирована. Проверено unit-тестом parser-а
и независимым raw-TCP клиентом на Linux. См. [RFC 5321 §4.1.2](https://www.rfc-editor.org/rfc/rfc5321.html#section-4.1.2).
После `>` MAIL-параметры и параметры RCPT могут отделяться только одиночным
ASCII SP согласно grammar RFC 5321: HTAB, повторный или завершающий пробел
получают 501. Несколько корректных MAIL-параметров с одиночным SP остаются
допустимы; независимый raw-TCP клиент проверяет отказ и восстановление сессии.

SMTP session требует HELO/EHLO перед MAIL/RCPT/DATA. HELO включает только базовый
SMTP режим: ESMTP MAIL-параметры, AUTH и STARTTLS требуют EHLO. Успешные HELO и
EHLO сбрасывают незавершённый envelope; это проверяется независимым клиентом.
MAIL во время открытой transaction получает 503 без изменения sender или списка
получателей; прежний конверт остаётся пригоден для DATA. Это правило порядка
RFC 5321 проверено unit FSM-тестом и сквозной SMTP-to-IMAP доставкой. Отдельный
implicit-TLS тест подтвердил приоритет 503 над sender-authorization отказом:
после активного envelope чужой MAIL получил 503, а DATA завершил первоначальную
доставку с неизменённым `Return-Path`.
`RSET` доступен в любой момент, в том числе сразу после STARTTLS и до нового
EHLO/AUTH. Независимый Linux TLS peer проверил `250` для RSET в этом состоянии,
а последующие EHLO/NOOP/QUIT подтвердили сохранение синхронизации. См.
[RFC 5321 §4.1.4](https://www.rfc-editor.org/rfc/rfc5321.html#section-4.1.4).
Общий SMTP path limit для MAIL FROM и RCPT TO равен 256 октетам вместе с
угловыми скобками. Парсер принимает граничный path и отвергает path на 257
октетах; unit и независимые STARTTLS black-box проверяют обе команды.
Для обычного `local-part@domain` парсер принимает local-part до 64 октетов и
отклоняет более длинный в MAIL FROM и RCPT TO. Сетевой тест отделяет syntax
error от последующей проверки sender/recipient policy. Это ограничение длины,
а не утверждение о полной проверке RFC mailbox grammar или SMTPUTF8.
ASCII dot-atom и RFC 5321 quoted-string local-part проверяются до policy слоя
в обеих командах. Quoted-pair декодируется для сопоставления с локальным
аккаунтом; завершающий `>` и `@` внутри quoted local-part не путаются с
разделителями path. Независимый STARTTLS peer отправил письмо от
`"alice"@litterkeep.test` к `"bob"@litterkeep.test`, после чего отдельный IMAP
peer нашёл письмо в INBOX Bob. Устаревший SMTP source-route `@domain,@domain:`
принимается после проверки каждого элемента и отбрасывается до локальной
маршрутизации согласно RFC 5321 §4.1.2; независимый SMTP-to-IMAP тест доказал,
что конечный mailbox получает письмо, а route не включает forwarding.
Пустые или неверно разделённые dot-atom точки, пустые доменные labels и DNS
labels длиннее 63 октетов отклоняются как syntax error. U-label domain
обрабатывается отдельно по IDNA/SMTPUTF8 правилам, описанным выше.
IPv4 address-literals и зарегистрированный `IPv6:` tag проверяются отдельно от
DNS-domain грамматики: десятичные IPv4 octets ограничены диапазоном 0..255,
IPv6 включая сжатую и IPv4-embedded форму проверяется стандартным Rust IP
parser-ом. Невалидный IP и незарегистрированный tag получают syntax error.
Корректные literals доходят до политики: они не совпадают с локальными доменами,
поэтому sender получает 553, recipient 550, без relay. Это покрывает только
текущий IPv4/IPv6 синтаксис и не утверждает, что произвольные будущие tags
поддержаны. Проверенный набор стандартизованных tags сверяется с
[реестром IANA](https://www.iana.org/assignments/smtp/#smtp-address-literal-tags).
Входящий SMTPUTF8 path принимает Unicode local-part и IDNA-valid U-label
домены, требует valueless MAIL параметр `SMTPUTF8` и канонизирует домен в
A-label перед локальной маршрутизацией. EHLO объявляет SMTPUTF8 вместе с
обязательной 8BITMIME. Независимые Rust и raw-TCP проверки покрывают parser,
capability, отсутствие параметра и сброс маркера транзакции через RSET.
В `smtp::punycode` добавлен самостоятельный RFC 3492 Bootstring encoder/decoder
с overflow checks и bounded input; все 19 примеров RFC 3492 §7.1 проверяются
unit-тестом в обоих направлениях, включая исправленное RFC Editor написание
русского примера I. ASCII-пример S относится только к Bootstring и не является
IDNA-меткой. Это только переносная кодировка code points. Дополнительно
`smtp::idna17` классифицирует Unicode
17 code points по сгенерированному официальному RFC 5892 Derived Property
набору: PVALID, CONTEXTJ и CONTEXTO остаются разными typed результатами;
прочие значения дают `DisallowedOrUnassigned`. Тесты проверяют представительные
символы и каждый класс, а не весь IDNA conformance corpus. RFC 5892 прямо
отделяет свойство code point от валидности label. `to_alabel` переводит
проверенную NFC U-label в Punycode A-label и
проверяет leading/trailing/reserved hyphens и 63-октетный encoded label limit.
CONTEXTO для U+00B7/U+0375/U+05F3/U+05F4/U+30FB и двух Arabic digit sets
проверяется по Unicode 17 Scripts таблице и соседним символам; невалидный
контекст получает отдельную ошибку. CONTEXTJ использует Unicode 17
Canonical_Combining_Class=Virama и DerivedJoiningType: ZWJ требует предыдущий
virama, ZWNJ допускает virama или RFC joining context с пропуском Transparent.
Ошибочный контекст отклоняется. RFC 5893 Bidi Rule использует Unicode 17
DerivedBidiClass и активируется для всех labels домена, если хотя бы одна
содержит R, AL или AN. ASCII SMTP address parser использует проверку LDH и
полный round-trip для входных A-label. `to_alabel_domain` преобразует U-label
домен в A-label DNS form; SMTP path использует это преобразование до route
lookup. Встроенная таблица RFC 5892 Derived Property сверена со всеми
1,112,064 Unicode scalar values из официального Unicode 17 `Idna2008.txt`;
это exhaustive property comparison, не проверка полной U-label/domain
conformance suite, RFC 5891 registration workflow или каждой границы Punycode
и whole-domain алгоритма. DNS MX discovery и удалённая доставка U-label
доменов здесь не проверены.
См. [RFC 3492](https://www.rfc-editor.org/rfc/rfc3492.html) и
[RFC 5890](https://www.rfc-editor.org/rfc/rfc5890.html),
[RFC 5891](https://www.rfc-editor.org/rfc/rfc5891.html) и
[RFC 5892](https://www.rfc-editor.org/rfc/rfc5892.html),
[RFC 5893](https://www.rfc-editor.org/rfc/rfc5893.html).
MAIL с таким reverse-path требует флаг SMTPUTF8; без него parser возвращает
отдельную typed policy error, сериализуемую как `550 5.6.7` для отправителя
согласно RFC 6531 §3.5. Reactor guard отклоняет UTF-8 RCPT без флага активного
MAIL кодом 553 5.6.7. Независимый Linux TCP peer проверяет enhanced status
`5.6.7` отдельно для `553` RCPT и `550` MAIL, а также достижение account lookup (550 на
намеренно отсутствующий UTF-8 account) при корректном SMTPUTF8. Дополнительный
forced-extension acceptance создал opt-in Unicode account отдельной командой
dev bootstrap, доставил ему MIME-вложение по SMTP STARTTLS и извлёк его через
IMAP STARTTLS/SASL PLAIN;
отдельное чтение после перезапуска подтвердило durable delivery. Вложение
совпало побайтно, SHA-256 `7f4e114fb71bb6cf784bda9f0d77d06e3ccb15cfdc667bbad322dcef81da20f8`.
Это историческая T84 forced-extension проверка, созданная до T195: тогда
SMTPUTF8 намеренно передавался без advertisement и проверялся только внутренний
путь доставки, не interoperability. Ее актуальный conforming wire acceptance
приведён ниже. Сам флаг не требует BODY=8BITMIME: 8-bit тела по-прежнему
разрешаются только BODY=8BITMIME, а UTF-8 заголовки требуют SMTPUTF8. Отдельный conforming STARTTLS клиент далее
передаёт U-label `пользователь@bücher.test`, читает MIME вложение по IMAP и
независимо сравнивает точные octets. Account
credentials, folder catalog, subscriptions, Sieve snapshots и внутренние
mailbox keys сохраняют bounded UTF-8 идентификаторы в отдельном hex namespace;
прямой Store/Accounts restart test проверяет эти слои, а conforming acceptance
выше отдельно проверяет SMTP/IMAP network путь. Ограниченный DATA/transport
checkpoint разбирает MAIL флаг SMTPUTF8, разрешает UTF-8 field values, но
оставляет field names ASCII, как требует RFC 6532 §3; outbound требует peer
capabilities перед MAIL.
Независимый Linux SMTP peer подтвердил `BODY=8BITMIME SMTPUTF8` и сохранность
UTF-8 header octets. Conforming U-label SMTP/IMAP delivery проверена отдельным
клиентом выше. Полная грамматика RFC 6532 для UTF-8 в structured fields и
совместимость с Litterkeep app остаются непроверенными. Имена полей остаются
ASCII, не являются RFC 6532 extension.
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
list expansion. HELP выдаёт многострочный список поддержанных команд, включая
AUTH, STARTTLS и BDAT; HELP с аргументом подсказывает вызвать общий список; NOOP
игнорирует необязательный параметр.
Независимая проверка подтверждает ответы до и во время активного envelope.

RFC 2034 `ENHANCEDSTATUSCODES` объявляется в каждом варианте EHLO, включая
plaintext и TLS-состояния. Ответы SMTP классов 2xx, 4xx и 5xx снабжаются
enhanced code; исключения RFC сохраняются для initial greeting и HELO/EHLO,
а continuation-коды 3xx его не получают. Многострочный HELP повторяет один
enhanced code на каждой строке, причём тест сверяет класс enhanced-кода с
основным SMTP-кодом. Независимый STARTTLS socket peer проверяет
объявление extension, ответы AUTH/envelope/DATA/QUIT и ошибку неизвестного
командного слова; это wire coverage RFC 2034, а не полное SMTP acceptance.

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
RFC 6152 для всех MIME content и SMTP peer комбинаций. SMTPUTF8 отдельно
рекламируется с 8BITMIME; его U-label acceptance описана выше.
Строки DATA ограничены 1000 octets вместе с CRLF.
Граница между заголовками и телом отслеживается инкрементально на каждой
принятой CRLF-строке. Парсер больше не сканирует накопленное сообщение заново
для каждого следующего сегмента, поэтому классификация DATA не имеет
квадратичного числа проходов при большом количестве коротких строк. Состояние
границы покрыто Linux unit regression; Linux Docker gate прошёл strict Clippy,
695 library tests, 1 CLI test и doc-tests (1 внешний Unicode corpus test ignored).
При приёме DATA каждая строка с начальной точкой теряет ровно один
transparency dot; RFC line bound проверяется после снятия transparency и
включает CRLF. Linux unit regression проверяет одну, две и три точки, а также
граничную строку ровно в 1000 октетов и отказ для 1001.
Отдельный независимый STARTTLS wire acceptance подтвердил ровно 1000 после
unstuffing для dot-stuffed строки, отказ на 1001 и успешный NOOP после отказа.
Расширенный acceptance проверил обе границы для 7BIT и BODY=8BITMIME, включая
high-bit octet; IMAP read-back подтвердил побайтное сохранение принятых строк
и отсутствие двух отвергнутых длинных сообщений.
Незавершённая входная DATA-строка, уже превысившая максимально возможный wire
размер допустимой строки, удаляется из session input buffer и переводит только
текущую DATA transaction в oversized state. Reactor продолжает считывать до
terminator, после которого возвращает 552 и сохраняет возможность NOOP. Linux
STARTTLS peer передал без CRLF данные больше прежнего общего session input cap,
получил 552 после terminator и 250 на NOOP. Это подтверждает ограниченное
накопление и синхронизацию, но не измеряет RSS процесса.
Исходящий SMTP transport применяет transparency из RFC 5321 §4.5.2: удваивает
точку только в начале строки, передавая окончание DATA отдельно. Независимый
peer снял transparency и сравнил полное сообщение с источником, включая строки
с одиночной и двойной точкой и отсутствующий в source завершающий CRLF. Также
регрессионно подтверждено, что уже имеющийся конечный CRLF не дублируется.
Перед envelope outbound path проверяет CRLF framing и RFC 5321 максимум 1000
octets вместе с CRLF (dot-stuffing octet не считается): независимый peer
подтвердил доставку строки ровно в 1000 октетов и отказ для 1001 без `MAIL`.
Проверен независимый SMTP peer; межсерверные комбинации и Litterkeep-клиент
этими тестами не сертифицируются.

## IMAP

Реализованы STARTTLS/implicit TLS, LOGIN, AUTHENTICATE PLAIN с SASL-IR, LIST,
LSUB, CREATE/RENAME/DELETE, SELECT/EXAMINE, STATUS, APPEND, SEARCH, FETCH,
BODYSTRUCTURE, MIME sections/partial, STORE, COPY, MOVE, EXPUNGE, UID EXPUNGE,
IDLE/DONE и durable Recent. UIDVALIDITY и UID переживают restart.

Implicit-TLS IMAP отвечает на peer-инициированный `close_notify` после
успешного tagged NOOP reply. Независимый Linux TLS peer завершил `unwrap()`;
это подтверждает TLS closure для этого сценария, не полный IMAP4rev2 профиль.

`AUTHENTICATE PLAIN` принимает механизм и SASL-IR как IMAP `astring`: atom и
quoted string; маркер `=` переводится в пустой initial response, отличный от
отсутствующего ответа с continuation. Пустые PLAIN credentials отвергаются, а
сессия остаётся пригодной для следующей команды. Независимый STARTTLS `imaplib`
проверил continuation, atom SASL-IR, quoted mechanism/response, пустой marker,
capability до/после TLS и отказ plaintext authentication. Другие SASL mechanisms
и channel binding не реализованы и не рекламируются.
IMAP quoted-pair принимает только escaped `"` и `\\` по RFC 3501; иной
символ после обратной косой черты возвращает tagged BAD, сохраняя командный
тег. Независимый STARTTLS `imaplib` тест подтвердил BAD и успешный NOOP после
лексической ошибки. UTF8=ACCEPT пока не рекламируется, но ENABLE активирует
connection-local mode: legacy quoted-string отклоняет high-bit UTF-8 до
negotiation, а quoted mailbox names разрешаются после неё. Независимый STARTTLS
acceptance проверил positive и negative переходы, включая malformed UTF-8,
C0/C1 controls и ASCII-only atom. Ограниченный synchronizing-literal receiver
поддерживает оба LOGIN astring, mailbox аргументы CREATE/DELETE/RENAME,
SUBSCRIBE/UNSUBSCRIBE, LIST/LSUB/STATUS, COPY/MOVE, SELECT/EXAMINE и UID
COPY/MOVE, а также SEARCH/UID SEARCH. Независимый raw STARTTLS acceptance
проверил LOGIN, CREATE, LIST и SELECT; APPEND использует отдельный receiver.
Это не общая поддержка literal для всех аргументов, LITERAL+ не рекламируется.
Внутренние CRLF учитываются в literal octet count и сохраняются как данные:
STARTTLS LOGIN получил CRLF-containing password literal, вернул обычный auth
failure, после чего принял правильный login. Raw NUL отклоняется; LOGIN password
literals сохраняются побайтно, включая невалидный UTF-8, до authentication
owner-а. Текстовые username/mailbox/search аргументы требуют корректный UTF-8 в
выбранном string mode. Это не полная поддержка произвольных literal octets для
всех команд. Receiver сохраняет байты отдельным типом токена. Независимый STARTTLS
acceptance проверил tagged BAD для UTF-8 quoted LOGIN, восстановление сессии и
UTF-8 BODY literal. RFC 9755 explicit SEARCH CHARSET получает BAD; wire-проверены
обычный и смешанный регистр keyword, после чего implicit UTF-8 SEARCH работает.
UTF-8 mailbox CREATE/SUBSCRIBE/UNSUBSCRIBE/LIST/LSUB/STATUS/SELECT, APPEND,
COPY/MOVE/RENAME/DELETE прошли независимую STARTTLS проверку; после COPY/MOVE
RFC822 bytes совпадают точно. CAPABILITY всё ещё скрывает UTF8=ACCEPT до полного
профиля. Актуальный audit RFC 9755 и оставшиеся обязательства приведён в
`imap-profile.md`; это не объявляет IMAP4rev2.
Дополнительная комбинированная STARTTLS проверка включает `ENABLE UTF8=ACCEPT`,
создаёт mailbox с `é` и запрашивает `LIST RETURN STATUS`: соседние LIST и
STATUS ответы сохраняют UTF-8 имя и порядок запрошенных числовых полей. Это
проверка совместной сериализации RFC 9755/RFC 5819, не готовность всего профиля.
CAPABILITY объявляет `LIST-EXTENDED` по RFC 5258 и `LIST-STATUS` по RFC 5819;
независимый STARTTLS тест проверяет оба объявления до и после TLS upgrade.
`RECENT` разрешён как атрибут STATUS по RFC 3501: до SELECT возвращается число
неполученных durable-сообщений, после SELECT STATUS и LIST RETURN STATUS
используют один session-owned recent UID набор. Это не снимает ограничения
частичного IMAP4rev1 профиля.

RFC 2971 `ID` is advertised and accepted in every IMAP state. The parser
validates `NIL` or a field/value list, enforces the 30-pair, 30-octet field,
1024-octet value and unique-name limits, then discards client-supplied identity
data. The response is always `* ID NIL`, avoiding host/build fingerprinting.
Independent Linux STARTTLS wire acceptance covers the capability before and
after TLS, NIL/list requests, synchronizing literals in both field and value
positions, duplicate-name rejection, privacy response and NOOP recovery. The
shared bounded literal receiver recognizes the RFC list-opening `(` directly
before the first literal marker. This implements the identification extension
only; it does not claim full IMAP4rev2 or all registered extensions.
The same acceptance sends `LITERAL-` forms for both fields and values without
waiting for continuations, after checking the advertised capability. RFC 7888
allows non-synchronizing literals in any IMAP command and caps them at 4096
octets for `LITERAL-`; this ID test covers small payloads, not the global
oversize boundary. RFC 2971 ID field/value limits are also checked at their
wire boundaries: 30-octet field and 1024-octet value are accepted, while 31
and 1025 octets respectively receive tagged BAD without disclosing ID data;
subsequent NOOP commands confirm session recovery.

RFC 7888 `LITERAL-` включён в CAPABILITY. Synchronizing literal-ы сохраняют
continuation; non-synchronizing `{n+}` не вызывает continuation и ограничен
4096 octets. Независимый STARTTLS клиент проверяет эту форму для LOGIN, SEARCH
и APPEND, включая byte-exact APPEND round-trip. При превышении 4096 octets
сервер закрывает соединение с `BYE [TOOBIG]`, не выделяя недопустимый payload.
Нулевой APPEND literal получает tagged `NO APPEND cancelled`; independent peer
проверяет после него `NOOP` и пустой mailbox через `SELECT`, а затем продолжает
работу в той же STARTTLS-сессии. RFC 3502 MULTIAPPEND реализован поверх общего
literal receiver: каждый synchronizing literal получает собственный continuation,
а завершённая команда передаётся одним batch-запросом Store owner-у. Успех
возвращает APPENDUID с UID-set в порядке добавления; failure injection проверяет,
что после сбоя первой mailbox frame ни одно сообщение не видно после recovery.
Независимый STARTTLS `imaplib` сценарий отправляет два сообщения с разными
флагами и NUL/0xFF в первом body, сверяет UID-set `1 3,4`, затем читает оба
RFC822 тела побайтно и проверяет FLAGS. LITERAL+ по-прежнему не рекламируется.

RFC 7889 APPENDLIMIT объявляет единый предел `26214400` octets, равный
действующему `MAX_MESSAGE_SIZE`. STATUS и LIST RETURN STATUS возвращают
APPENDLIMIT для mailbox; oversize APPEND получает tagged `NO [TOOBIG]` до
continuation и приёма payload. Независимый STARTTLS клиент проверил capability,
STATUS, LIST STATUS и отклонение размера `26214401`; полная загрузка сообщения
ровно предельного размера отдельным wire-тестом не измерялась.

`FETCH` принимает одиночные и сгруппированные data items, раскрывает ALL, FAST
и FULL, включает UID в ответы UID FETCH и сохраняет literal как точные октеты.
`BODY` и `BODYSTRUCTURE` поддерживаются как отдельные запросы; FULL раскрывается
в BODY согласно RFC 9051, а не в BODYSTRUCTURE.
`BODYSTRUCTURE` включает RFC 9051 body-MD5 slot и корректный RFC 2183
Content-Disposition extension для scalar/message частей; multipart начинает
extensions с parameter list после subtype. Параметры включая RFC 2231 filename декодируются общим MIME parser-ом;
невалидное или повторное поле не экспортируется. RFC 2231 percent escapes
принимают HEXDIG в любом регистре согласно регистронезависимой ABNF. Независимый
Python `imaplib` через implicit TLS выполнил APPEND и BODYSTRUCTURE FETCH с
RFC 2231 UTF-8 filename, содержащим lowercase и uppercase escapes, и прямым
UTF-8 в quoted MIME-параметре `name`; BODYSTRUCTURE вернул оба Unicode значения
и подтвердил сохранность исходных RFC822 октетов. Прямая форма использует
RFC 6532 quoted-string extension и не заявляет SMTPUTF8 transfer.
Внешняя whitespace-обрезка structured MIME значений ограничена ASCII SP/HTAB:
Unicode NBSP перед media type, параметром, unquoted value и disposition token
отвергается, а NBSP внутри quoted parameter сохраняется как данные. Регрессия
до фикса воспроизводилась на NBSP перед `text/plain`; после фикса прошли
параметрические и disposition unit tests, Windows 569 library + 1 binary test,
fmt и strict Clippy, Linux Docker release build, strict Clippy, 620 library +
1 binary test и doc-tests. `tests/blackbox/mime_structured_wsp.py` через
независимый Python `imaplib` APPEND/FETCH подтвердил MIME fallback для NBSP в
Content-Type, сохранение quoted NBSP и непринятие NBSP перед disposition token,
Content-Language и Content-Location; NBSP в unstructured Content-Description
сохранён. Исходные RFC822 literal bytes совпали. Это не проверка совместимости
Litterkeep.
`Content-Language` сохраняет
один или несколько структурно корректных BCP 47 tags; `Content-Location`
сохраняется как единственный непустой unfolded URI-reference по синтаксису
RFC 3986. Поддерживаются absolute URI и relative reference; percent-encoding
проверяется, но не декодируется. Не выполняются разрешение ссылок, загрузка,
проверка допустимости схем, IRI/Unicode URI или реестр URI. Невалидное либо
повторное поле не экспортируется. Поля сериализуются строго после disposition
в RFC 9051 порядке.
`BODY` использует отдельный basic encoder и не получает extension fields.
Независимый STARTTLS `imaplib` acceptance сравнил wire-ответы `BODY` и
`BODYSTRUCTURE`: первый не содержит extensions, второй содержит language,
location и RFC 2231 attachment filename. Оба профиля возвращают MIME basic
поля `Content-ID` и `Content-Description`. Описание декодируется по RFC 2047;
Content-ID принимает единственное поле в формах dot-atom, quoted id-left и
domain-literal id-right, включая UTF-8 atext по RFC 6532. RFC 5322
`obs-id-left`/`obs-id-right` CFWS вокруг dot-separated atoms нормализуется;
obsolete local-part также принимает последовательность atom/quoted-string
words с точками вне кавычек. Comments и whitespace вокруг quoted/atom words
и точек семантически невидимы. Контрольные octets и malformed quoted-pairs
остаются невалидными. Повторные или синтаксически некорректные значения не
проецируются.
Независимый STARTTLS acceptance подтвердил quoted id-left, IPv6 domain literal,
obsolete CFWS и quoted-word формы, включая comments, в обоих профилях; UTF-8
identifiers и RFC 2047 description проверены unit-тестами. Прочие protocol fields и RFC
пробелы остаются непокрытыми.
Единственный корректный `Content-MD5` выводится в scalar/message body-MD5 slot
после строгой Base64 проверки и проверки длины декодированного digest в 16
октетов. Значение передаётся без пересчёта и сервер не проверяет совпадение с
текущими body octets. Повторы или некорректный digest пропускаются. Поле не
попадает в `BODY` и не занимает multipart extension slot; независимый STARTTLS
acceptance подтвердил обе wire-проекции.
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
клиента Litterkeep. BINARY включён в CAPABILITY в plaintext, TLS и authenticated
состояниях; отдельный STARTTLS wire acceptance проверил точный capability atom
до переключения, после него и после LOGIN. Это закрывает только discovery для
уже реализованного RFC 3516 extension; остальные требования IMAP4rev2 остаются
непокрытыми.
Для IMAP4rev1 совместимости `RFC822.HEADER` возвращает заголовки под legacy
data-item label и не меняет `\\Seen`; `RFC822.TEXT` возвращает точные body
octets под своим label и durable устанавливает `\\Seen`. Независимый
SMTP-to-IMAP acceptance проверяет labels, bytes и Seen-переходы.
SEARCH использует UTF-8 по умолчанию и поддерживает явные UTF-8/US-ASCII;
неподдерживаемая кодировка возвращает `NO [BADCHARSET ...]`, а Unicode-criteria
при US-ASCII отвергаются ответом NO. `BODY`/`TEXT` декодируют Base64 и
quoted-printable текстовые MIME parts с US-ASCII, UTF-8, ISO-8859-1,
Windows-1251 или Windows-1252; UTF-16, UTF-16BE и UTF-16LE также реализованы
для RFC 2047 encoded words и RFC 2231 extended parameters, но не используются
для поиска в `text/*` body parts согласно RFC 2781. RFC 2047 encoded words
декодируются в outer и
MIME-part headers только как полные токены в начале field-body или после LWS.
Embedded и слитные формы остаются raw text; это новое правило пока unit-only.
По RFC 2047 §2 delimiter lookahead теперь ограничен 75 символами каждого
encoded-word. Для 32 768 oversized malformed starters debug unit-test ранее
занимал 5.21 секунды, после исправления 0.04 секунды на той же Windows-среде;
Linux test-stage повторил сам вектор за 0.04 секунды. Это bounded-lookahead и
unit coverage, не измерение production throughput или внешнего клиента.
RFC 2231 `charset*language` suffix encoded-word сохраняется отдельно в typed
API, тогда как header/phrase projections возвращают decoded text. Новые
language suffix grammar cases unit-only.
RFC 2231 extended parameters сохраняют BCP 47 language metadata в MIME tree;
проверяется синтаксис RFC 5646, но не актуальная registry validity. Проверки
metadata и допустимого опущенного charset пока unit-only.
RFC 2047 encoded-word и RFC 2231 extended parameter также декодируют UTF-16,
UTF-16BE и UTF-16LE с проверкой byte order, нечётной длины и surrogate pairs.
Начальный BOM у UTF-16 выбирает порядок и удаляется как signature; без BOM
используется big-endian после проверки первых двух октетов; generic UTF-16 с
меньшим числом исходных октетов отклоняется. RFC 2781 регистрирует семейство UTF-16 как
неподходящее для MIME `text/*`, поэтому SEARCH не декодирует такие body parts.
Профиль проверен unit-тестами; отдельный socket acceptance не запускался.
Независимый STARTTLS
тест подтвердил, что BODY не ищет outer Subject и бинарное
application/octet-stream вложение, а TEXT включает decoded outer headers.
Неизвестная charset и невалидная transfer encoding не сравниваются как raw
octets. Часть с неизвестным Content-Transfer-Encoding рассматривается как
непрозрачный `application/octet-stream` согласно RFC 2045 §6.4: её тело не
попадает в текстовый SEARCH projection, а исходные MIME-заголовки сохраняются.
Независимая SMTP STARTTLS -> IMAP проверка подтвердила fallback type в
BODYSTRUCTURE, сохранение исходных заголовков и отсутствие совпадения SEARCH
BODY. IMAP BINARY для такой части завершается `NO [UNKNOWN-CTE]` согласно
RFC 9051; тот же клиент проверяет response code.
MIME tree отвергает Base64, quoted-printable и неизвестный CTE на всех
multipart/message composite entities согласно RFC 2045 §6.4; повторное поле
CTE отклоняется как неоднозначное, поэтому позднее 8bit не маскирует ранее
указанное кодирование. Multipart delimiter collection ограничен остатком
глобального MIME part budget до накопления диапазонов, а не только последующей
проверкой дерева. При разборе вложенной MIME-части внешний multipart delimiter
завершает текущую внутреннюю multipart entity даже при отсутствии её close
delimiter; политика распространяется на любую глубину вложения через
multipart и message/rfc822. Для верхнего multipart без closing delimiter
сохраняется отказ. Проверки parser-а unit-only.
MIME Base64 CTE по RFC 2045 §6.8 игнорирует октеты вне Base64-алфавита,
сохраняя строгую проверку синтаксиса RFC 2047 encoded-word. Unit-проверки
покрывают оба профиля. Независимый SMTP STARTTLS -> IMAP STARTTLS клиент
доставляет multipart с `!#` внутри Base64 body и сверяет точные исходные octets
через `BINARY.PEEK`; encoded-word decoder этим сценарием не затрагивается.
Общий MIME header-block parser удаляет CRLF только у реальных строк заголовка,
сворачивает continuation с WSP и отвергает отдельный CR внутри обычной или
folded строки. RFC 5322 folding требует CRLF перед WSP; регрессии bare CR
проверены unit-тестами, отдельный socket acceptance не запускался.
Имена полей валидируются по [RFC 5322 §3.6.8](https://www.rfc-editor.org/rfc/rfc5322.html#section-3.6.8) `ftext`: печатный US-ASCII без `:`;
легальная пунктуация extension field names сохраняется, а пробелы, control и
не-ASCII в имени отвергаются. Независимый STARTTLS клиент через APPEND/FETCH
подтвердил, что MIME parsing принимает punctuation extension field и сохраняет
полный RFC822 literal до байта. Это не проверка Litterkeep app.
RFC 2045 MIME-поля (кроме Content-Disposition) допускают RFC 822 comments;
Content-Transfer-Encoding удаляет вложенные comments вне quoted-string перед
разбором token. Сам CTE value ограничен одним RFC 2045 ASCII token: по краям
принимаются только SP/HTAB, внутренние separators и Unicode whitespace
отвергаются; неизвестный корректный ASCII token остаётся typed UNKNOWN-CTE.
Unit-регрессия воспроизводит прежнее принятие NBSP и проверяет отказ после
исправления. Независимый ManageSieve CHECKSCRIPT подтвердил те же границы в
vacation `:mime`: comments принимаются, NBSP отвергается. Unit-тест и независимый
SMTP STARTTLS -> IMAP STARTTLS
acceptance проверили вложенный comment перед Base64, escaped right parenthesis,
comment после token, BODYSTRUCTURE encoding и точные декодированные octets.
Исходные RFC822 bytes при доставке сохранены без нормализации.

MIME parser и IMAP BODYSTRUCTURE распознают RFC 6532 `message/global`.
`7bit`, `8bit` и `binary` используют исходные байты, Base64 и
quoted-printable декодируют вложенное сообщение в отдельный Arc-owned source,
чьи offsets разрешаются относительно этого source. ENVELOPE, BODYSTRUCTURE,
nested BODY HEADER и BINARY part traversal работают на decoded содержимом;
unknown CTE не разбирается как письмо, а остаётся opaque
`application/octet-stream`. Вектора проходят unit-тесты, но отдельный network
black-box `tests/blackbox/imap_message_global.py` теперь выполняет APPEND и
проверяет на независимом STARTTLS клиенте BODYSTRUCTURE, сохранение Base64
wire-body через BODY[1], UTF-8 вложенные BODY[1.1]/BODY[1.HEADER], decoded
BINARY[1.1] и неизменность исходного RFC822 literal. Прогон выполнен на Linux
Rust 1.97 Docker runtime с отдельным каталогом `./data/g18-message-global-wire`;
повторён под `ENABLE UTF8=ACCEPT` в `./data/g19-rfc9755-message-global-enabled`.
В обоих случаях вложенная структура и все octets совпали. Litterkeep app не
проверялось. Это acceptance конкретного media type, а не полный сертификат MIME
или IMAP4rev2. RFC 9755 §6 допускает для IMAP4rev1 с включённым UTF8=ACCEPT как
`message/rfc822` representation, так и RFC 3501 representation; текущий ответ
`MESSAGE/GLOBAL` остаётся валидным. См.
[RFC 6532 §3.5](https://www.rfc-editor.org/rfc/rfc6532.html#section-3.5),
[§3.7](https://www.rfc-editor.org/rfc/rfc6532.html#section-3.7)
и [RFC 9051 §9](https://www.rfc-editor.org/rfc/rfc9051.html#section-9).
Quoted-printable decoder удаляет буквальные SP/TAB на концах hard lines и
объекта, не удаляя закодированные `=20`/`=09`; encoder сериализует пробелы и
TAB как hex-octets, чтобы перенос строки не менял содержимое. Независимый SMTP
STARTTLS -> IMAP STARTTLS BINARY.PEEK подтвердил удаление SP/TAB в hard-line
padding и сохранение decoded `=20`. SP/TAB перед soft break `=CRLF` являются
содержимым `qp-segment` и сохраняются, тогда как SP/TAB после последнего
символа hard line удаляются как transport padding. Unit-регрессии проверяют
обе позиции; новый большой-входной вектор проверяет 8 KiB непрерывного WSP
перед hard CRLF и soft break. Decoder обрабатывает каждый пробельный участок
одним проходом, избегая квадратичного повторного lookahead. Отдельные
[RFC 2045 §6.7](https://www.rfc-editor.org/rfc/rfc2045.html#section-6.7)
векторы проверяют нормативные примеры и полное тело. Для исходящего
Sieve vacation отдельный
ManageSieve -> SMTP submission -> durable queue -> независимый SMTP peer
сценарий подтвердил UTF-8 round-trip, кодирование буквальных SP/TAB как
`=20`/`=09` и физические строки не длиннее 76 октетов. Проверялся именно
независимый peer, не Litterkeep. В этом пути также обнаружено и исправлено
добавление лишнего пустого CRLF outbound SMTP transport-ом, если MIME payload
уже завершался CRLF; обычный и TLS transport теперь используют общую запись
DATA. Проверка существующего Litterkeep клиента и других MTA остаётся открытой.
При повреждённом quoted-printable `=` escape входящий decoder сохраняет
буквальный `=` и следующий символ без преобразования, как рекомендует RFC 2045
§6.7 для robust decoder. Генераторский Sieve `:mime` validator использует
отдельный strict путь и malformed escape отвергает. Независимый SMTP STARTTLS
клиент передал `=G1` и неполный `=A`, а IMAP STARTTLS BINARY.PEEK подтвердил
точные декодированные octets с сохранёнными escape. Независимый ManageSieve
CHECKSCRIPT принял корректный quoted-printable soft break, включая SP перед
`=CRLF`, и отверг malformed hex escape и незакодированные завершающие SP/HTAB.
Полный список RFC-векторов пока не покрыт.
`7bit` и `8bit` CTE проверяют профиль RFC 2045 §§2.7-2.8: не более 998
октетов между CRLF, отсутствие NUL и одиночных CR/LF; `7bit` дополнительно
отвергает high-bit octets, `8bit` их сохраняет. `binary` остаётся прозрачным.
Независимый IMAP STARTTLS APPEND -> BINARY.PEEK принял 998-октетную строку
7bit и отверг 999-октетную, сохранил high-bit octets для 8bit и отверг их для
7bit. Проверены NUL для обоих профилей и bare CR/LF. Для последних использован
прямой IMAP literal: стандартный `imaplib.append()` нормализует line endings
до отправки и не подходит для этих malformed fixtures. Это проверка IMAP
хранения и MIME decode, не SMTP DATA acceptance.
Параметр `multipart` boundary проверяется по RFC 2046 §5.1.1: 1..70 ASCII
октетов из `bchars`, без завершающего пробела; покрыты длины и запрещённые
символы unit-тестами. Parser принимает transport-padding у opening/closing
delimiter и исключает CRLF, непосредственно предшествующий первому delimiter,
из preamble; собственный CRLF preamble сохраняется. Сканирование останавливается
на первом close-delimiter, поэтому boundary-похожая строка внутри epilogue не
создаёт часть и не расходует part budget. Независимый Python `imaplib`
STARTTLS-клиент отправил padded delimiters и epilogue с последующим boundary-
похожим MIME-заголовком; BODYSTRUCTURE содержит только ожидаемые две части,
BODY[1] и BINARY.PEEK вложения совпали, а RFC822 FETCH подтвердил точные исходные
bytes, включая preamble и epilogue. Это проверка IMAP APPEND/FETCH, не SMTP
доставки и не клиента Litterkeep.
Вложенная MIME-часть без Content-Type в `multipart/digest` получает default
`message/rfc822` по RFC 2046 §5.1.5; само вложенное сообщение использует
`text/plain` default для своего body. В остальных multipart подтипах отсутствие
Content-Type по-прежнему даёт `text/plain`. Типизация и вложенные ranges
проверены unit-тестами. MIME body-part с пустой областью заголовков теперь
принимается: первая CRLF завершает header area, а последующие октеты точно
сохраняются как body. Unit-проверка BODYSTRUCTURE подтверждает вложенные
`MESSAGE/RFC822`, ENVELOPE, `TEXT/PLAIN` и размеры исходных octets для digest
part. Парсер и протокольная проекция проверены unit-тестами; socket acceptance
также выполнен независимым STARTTLS `imaplib` peer: после APPEND BODYSTRUCTURE
содержит `DIGEST` и вложенные `MESSAGE/RFC822`/`TEXT/PLAIN`, а RFC822 FETCH
побайтно совпадает с headerless digest source. Это проверка независимым
клиентом, не приложением Litterkeep.
Второй независимый STARTTLS FETCH проверил RFC 3501 §6.4.5 / RFC 9051 §6.4.5
для этого embedded message: `BODY.PEEK[1]` возвращает RFC822-содержимое вложения,
`[1.HEADER]` и `[1.TEXT]` разделяют его заголовок и текст, `[1.1]` адресует
единственную MIME body-part. Все четыре ответа совпали с точными ожидаемыми
octets; клиент Litterkeep не запускался.
Для того же дерева исправлен traversal RFC 9051 `BINARY`: `multipart/digest`
теперь считается multipart-контейнером, а numeric path после его
`MESSAGE/RFC822` child продолжается от embedded MIME entity. Независимый STARTTLS
FETCH подтвердил `BINARY.PEEK[1.1]` декодированный body и `BINARY.SIZE[1.1]` с
совпадающей длиной. Ранее этот путь завершался `NO`; SMTP и клиент Litterkeep не
проверялись.
Unit matrix также проходит embedded `MESSAGE/RFC822`/`MESSAGE/GLOBAL` leaves в
`multipart/mixed`, `multipart/alternative`, `multipart/related` и
`multipart/digest`. Дополнительный независимый Linux STARTTLS peer APPEND-ит все
эти subtype variants и проверяет BODYSTRUCTURE, `BINARY.PEEK[1.1]` после
quoted-printable decode и совпадающий `BINARY.SIZE[1.1]`. Вариант
`message/global` проверяется как child `multipart/mixed`; остальные используют
`message/rfc822`, а digest child остаётся headerless. Это покрытие именно этих
numeric paths, не полная MIME или IMAP rev2 interoperability и не приложение
Litterkeep.
По RFC 9051 §6.4.5 partial offset/count для `BINARY` применяются к уже
декодированным section octets. Ранний wire probe `<2.4>` вернул `ssag` и origin
`<2>`, но первые четыре байта совпадают с encoded source prefix и сами по себе
не отличали порядок decode/slice; этот результат не считается доказательством
этого порядка. Усиленный probe для всех пяти fixtures запросил offset `8`,
попадающий внутрь quoted-printable `=20`, и получил `bo`; дополнительно проверены
offset `0`, частичное усечение у конца (`dy`), ровно EOF, offset за EOF и count,
превышающий остаток. Проверка подтвердила декодированные offsets и пустой literal
в двух EOF-случаях.
В writable mailbox этот же независимый клиент проверяет, что полный и partial
`BINARY.PEEK` оставляют `\Seen` unset. Partial count `0` отвергается tagged `BAD`
как для `BINARY.PEEK`, так и для `BODY.PEEK`; после каждого отказа `NOOP` проходит,
то есть IMAP-сессия остаётся пригодной. Это подтверждение RFC 9051 §6.4.5 для
указанных FETCH форм, не проверка клиента Litterkeep.
Для смешанного `FETCH (BODY[] BINARY[1])` найдено состояние, где `BODY[]` может
успешно прочитаться, но `BINARY[1]` завершается `NO [UNKNOWN-CTE]`. До исправления
reactor записывал `\Seen` перед вычислением всех items, поэтому неуспешный FETCH
мог менять mailbox. Теперь сериализация каждого выбранного сообщения проверяется
до durable Seen mutation; Linux regression test подтверждает, что такой набор не
ставит отметку. Независимый Linux STARTTLS клиент отправил смешанный FETCH,
получил NO [UNKNOWN-CTE], выполнил NOOP и затем подтвердил по FLAGS, что
\Seen остался unset. Это acceptance независимого клиента, не приложения
Litterkeep.
Для RFC 9051 §6.4.5 partial диапазоны у HEADER.FIELDS и HEADER.FIELDS.NOT
применяются после формирования подмножества header. Независимый STARTTLS тест
проверяет оба selector-а на fixtures с разным порядком полей: offset 0 возвращает
Subject после предшествующего невыбранного поля для HEADER.FIELDS и From после
исключённого Subject для HEADER.FIELDS.NOT. Проверены exact bytes и response
origin <0>; тест не заменяет остальные FETCH cases или полную rev2 совместимость.
Отсутствующий или синтаксически повреждённый `Content-Type` получает
рекомендуемый RFC 2045 §5.2 fallback `text/plain`/US-ASCII; сырой MIME header
не переписывается. Независимый STARTTLS клиент отправил оба варианта через SMTP
и проверил по IMAP BODYSTRUCTURE effective `TEXT/PLAIN`, исходный RFC822 и
сохранение malformed header без нормализации. Для двух полей Content-Type
применяется рекомендуемый [RFC 2045 §5.2 fallback](https://www.rfc-editor.org/rfc/rfc2045.html#section-5.2): MIME-проекция становится `TEXT/PLAIN`, оригинальные поля остаются в RFC822. Независимый STARTTLS SMTP→IMAP сценарий подтвердил BODYSTRUCTURE и точный RFC822 round-trip; US-ASCII декодирование body остаётся unit-only.
RFC 5322 address groups разбираются в типизированные start/member/end элементы:
FETCH ENVELOPE выдаёт group marker (`NIL NIL group NIL`), mailbox участников
и end marker (`NIL NIL NIL NIL`), включая пустую группу. SEARCH FROM/TO/CC/BCC
использует эту же проекцию и находит имя группы и адрес участника. Независимый
STARTTLS black-box проверил группу с тремя mailbox, пустую Cc-группу и Bcc
member. В address projection вложенные comments удаляются с учётом quoted-pair,
скобки внутри quoted display name сохраняются; ENVELOPE и SEARCH не видят текст
комментариев. Независимый STARTTLS acceptance проверил From/To comments и
групповую адресацию. Разделитель addr-spec выбирается вне quoted local-part и
domain-literal: независимый STARTTLS acceptance подтвердил ENVELOPE с
`"a@b"` в mailbox-компоненте и `example.test` в hostname, а SEARCH нашёл этот
адрес. Завершающий CFWS после `;` группы допускает комментарий и не включает
его в ENVELOPE; тест также отвергает mailbox после уже закрытой группы.
Obsolete source-route в угловом адресе игнорируется по RFC 5322 §4.4:
ENVELOPE и SEARCH используют только route destination. Другие obsolete address
forms и остальные RFC 5322 несовпадения по-прежнему ограничивают parser.
Там же поддержан CFWS вокруг точек в obsolete local-part/domain: wire
ENVELOPE и SEARCH сворачивают `first (comment) . last @ example . test` к
`first.last@example.test`. Quoted local-part и domain-literal из этой
нормализации исключены и проверяются отдельными unit fixtures. Для quoted
display-name отдельно проверено снятие quoted-pair: `\"` и `\\` в исходном
header становятся буквальными `"` и `\` в ENVELOPE.
RFC 2047 encoded-word распознаётся как слово только в unquoted display-name
phrase. Пробел между соседними encoded-word не выводится; испорченное слово
остаётся видимым текстом и не блокирует сообщение; quoted display-name не
декодируется. Независимый STARTTLS клиент подтвердил ENVELOPE и FROM SEARCH
для encoded phrase, а также буквальное содержимое quoted-name. В phrase
контексте Q encoding ограничен ASCII letters/digits и `!*+-/=_`; encoded
punctuation допустим, а сырой запрещённый знак оставляет слово видимым.
Границы allowlist проверены unit-only, не независимым клиентом.
CFWS и разделители phrase обрабатываются только как ASCII SP/HTAB: RFC 6532
не-ASCII символы, включая NBSP, в display-name не схлопываются в обычный пробел.
Независимый STARTTLS ENVELOPE acceptance подтвердил точное сохранение NBSP в
UTF-8 display-name; полный набор интернационализированных address forms ещё не
покрыт.
Для obsolete address-list проверены leading/repeated/trailing null members:
они не формируют ENVELOPE addresses и не сдвигают порядок двух mailbox.
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
charset вне проверенного набора остаются непроверенными. Независимый Linux
STARTTLS SMTP -> IMAP acceptance дополнительно доставил 8BITMIME text body с
`charset=windows-874`, после чего Bob запросил `SEARCH CHARSET UTF-8 BODY` со
synchronizing literal `ภาษาไทย`; результат включил доставленное сообщение.
Это доказывает Windows-874 decoding в IMAP MIME text projection и его передачу
в evaluator, но не wire-покрытие остальных charset. SEARCH поддерживает
ASCII quoted strings и synchronizing literals до 64 KiB; независимый STARTTLS
тест проверил BODY, UID, UTF-8, пустой literal и два последовательных literals
в HEADER с отдельным continuation для каждого; pipelined command после literal
сохраняется. Parser acceptance перечисляет все базовые SEARCH keys RFC 9051,
evaluator покрыт соответствующими unit и сетевыми сценариями. Объявление
SEARCHRES не означает полной
совместимости IMAP4rev1 или IMAP4rev2: остаются SEARCH/ESEARCH расширения и
другие несовпадения формальных грамматик; см. [матрицу IMAP](imap-profile.md).
`SENTBEFORE`/`SENTON`/`SENTSINCE` извлекают дату из RFC 5322 `Date` header;
внешние и weekday CFWS trim ограничен SP/HTAB, поэтому NBSP не делает
malformed дату поисково-валидной. Независимый STARTTLS `imaplib` APPEND/SEARCH
подтвердил SENTON match только для одной валидной даты из пяти, при четырёх
NBSP-вариантах заголовка. Это не расширяет обработку obsolete Date syntax.
Нормы: [RFC 3501 §6.4.5](https://www.rfc-editor.org/rfc/rfc3501.html#section-6.4.5),
[RFC 5322 §3.3](https://www.rfc-editor.org/rfc/rfc5322.html#section-3.3).

## POP3

Поддерживаются STLS и implicit TLS, USER/PASS, STAT, LIST, UIDL, RETR, DELE,
RSET и QUIT. Maildrop lease исключает двух разрушающих владельцев; DELE
фиксируется только успешным QUIT.
Для RFC 8314 есть отдельный implicit-TLS listener `serve-pop3-tls`;
`POP3_SSL` ранее подтвердил отсутствие plaintext greeting до TLS handshake.
После успешного POP3 QUIT session cleanup дожидается отправки `+OK`, затем
инициирует TLS `close_notify` и освобождает socket после flush alert согласно
рекомендации RFC 8314 §3.4. Независимый TLS socket acceptance проверяет порядок
ответа и успешный `unwrap()` клиентом. При peer-initiated `close_notify` TLS
transport сохраняет close state; reactor сначала обрабатывает уже полученные
POP3-команды и дожидается pending Store completion, затем дренирует ответы,
инициирует ответный alert и освобождает ресурс через общий lifecycle cleanup.
Независимый implicit-TLS peer аутентифицировался и захватил maildrop, отправил
alert без QUIT, завершил `unwrap()`, затем второй клиент успешно захватил тот же
maildrop; отдельное соединение проверило QUIT reply и ответный alert. Это
проверка двух POP3 closure путей и освобождения maildrop lease, не всех причин
закрытия TLS для остальных протоколов. См.
[RFC 8314 §3.4](https://www.rfc-editor.org/rfc/rfc8314.html#section-3.4).
AUTHORIZATION FSM требует `USER` перед `PASS`, отклоняет повторный `USER`
после положительного ответа, а после отклонённого USER или failed PASS очищает
имя и требует новую команду USER. Независимый STLS клиент проверил этот порядок с ошибочным и затем
успешным паролем; это не подтверждает прочие authentication mechanisms.
RFC 1939 §5 разрешает `NOOP` только в TRANSACTION: сервер отклоняет его в
AUTHORIZATION и после USER, не теряя незавершённую пару USER/PASS; после входа
отвечает `+OK`. Unit FSM и независимый STARTTLS peer проверяют оба отказа,
восстановление аутентификации и последующий NOOP в TRANSACTION.
`USER` принимает ровно один непустой username, а `PASS` сохраняет все октеты
аргумента после первого разделителя, включая внутренние и крайние пробелы.
RFC 1939 §3 задаёт максимум 40 символов для каждого аргумента команды;
сервер принимает USER/PASS ровно на 40 печатных ASCII октетах и отвергает
41-й, при этом сохраняет пробелы внутри значения PASS. Username больше 40
октетов ранее ошибочно принимался лишь из-за общего предела RFC 2449 в 255
октетов для всей строки. RFC 1939 §7.3 разрешает трактовать пробелы `PASS` как
часть пароля. Unit и независимый STARTTLS peer проверяют границы и продолжение
USER/PASS после отказа; существующая Windows unit regression сверяет exact
password bytes в typed auth request;
независимый STARTTLS peer подтверждает, что неверный пароль с пробелами дошёл
до credential owner и последующий `USER` восстанавливает AUTHORIZATION.
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
Новый независимый Linux STARTTLS peer прочитал полный CAPA multiline frame в
AUTHORIZATION до TLS, после TLS, после успешного USER и в TRANSACTION. Теги
`USER`, `UIDL`, `TOP`, `RESP-CODES`, `UTF8` и `LANG` сохранялись во всех
четырёх состояниях; `STLS` заменялся на `SASL PLAIN` после TLS. Это подтверждает
эту таблицу CAPA-состояний, но не полный профиль RFC 2449/5034 и не клиента
Litterkeep.
`LIST n` и `UIDL n` возвращают одиночный `+OK` с номером и данными; без аргумента
обе команды используют multiline frame. LIST octet size считает CRLF-представление
сообщения и не включает дублированные dot-stuffing октеты. Независимый Linux
клиент проверил обе формы, сравнил LIST размер с RETR после удаления dot-stuffing;
unit-тест отдельно подтвердил LF-only raw message нормализацию.
POP3 нормализует CRLF только как пару: одиночный CR в сохранённом RFC822
сообщении остаётся частью данных и учитывается в LIST size. Регрессия проверяет
байты RETR/TOP и размер для CR перед обычным символом и в конце последней строки.
После DELE отмеченное сообщение исключается из STAT totals и многострочных
LIST/UIDL, а одиночные LIST n/UIDL n для него возвращают `-ERR`; номера других
сообщений не перенумеровываются в текущем snapshot. RSET восстанавливает
видимость, успешный QUIT durable удаляет только отмеченный UID. Повторный
`DELE n` до RSET также возвращает `-ERR`, как требует RFC 1939 §5. Unit и
двухсообщенческий SMTP->POP3 black-box проверяют RFC 1939 §§5-6.
RETR/TOP отдельно ограничивают status, сохраняя допустимые длинные строки тела:
проверено точное чтение 700-октетной строки. Это ограниченный набор проверок,
а не сертификация остальных требований RFC 1939/2449 или совместимости клиента.
Независимый Linux STARTTLS acceptance проверяет RFC 1939 TOP: `TOP 1 0`
возвращает headers с разделяющей пустой строкой без body, `TOP 1 1` ровно одну
строку, а запрос больше длины короткого body возвращает всё тело. Сценарий
передаёт `.first` и `..second` через SMTP; byte-exact POP3 результат одновременно
проверяет снятие SMTP и POP3 dot-transparency по одному октету на границе.

`AUTH PLAIN` поддержан только после TLS: до STLS механизм отсутствует в CAPA и
команда AUTH получает отказ; после STLS сервер рекламирует `SASL PLAIN` и убирает
`STLS`. Поддержаны initial response и continuation, отмена continuation через
`*`, строгая Base64-проверка и сохранение SASL capability после входа. Независимый
socket acceptance проверяет оба пути аутентификации, отказ от SCRAM, обновление
CAPA и запрет повторного AUTH после входа. Это ограниченный профиль RFC 5034,
не полная реализация: SCRAM и прочие SASL-механизмы не поддержаны, а
взаимодействие с litterkeep не проверялось.
Во время незавершённого `AUTH PLAIN` единственная команда отмены SASL exchange
это строка `*`. Любой иной клиентский ответ обязан быть Base64; обычный SMTP
текст `NOOP` и повторный `AUTH` не исполняются как вложенные команды, получают
501, завершают неудачный AUTH и оставляют соединение доступным для следующей
команды. Независимый STARTTLS peer проверяет обе строки, последующий `NOOP 250`
и успешный новый continuation login. См. [RFC 4954 §4](https://www.rfc-editor.org/rfc/rfc4954.html#section-4).
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
`UTF8` принимается в RFC-состоянии AUTHORIZATION согласно §2.1, а `LANG` в
AUTHORIZATION и TRANSACTION согласно §3.1. Положительный `USER` оставляет POP3
сессию в AUTHORIZATION до успешного PASS/AUTH; внутренняя фаза `UserAccepted` не
является отдельным wire state. Поэтому обе команды допустимы между USER и PASS.
Они сохраняют pending USER, что проверяется последующим корректно сформированным
PASS до backend-аутентификации. Успешная команда UTF8 включает raw UTF-8 message
mode и запрещает последующий STLS.
До переключения `LIST`, `TOP` и `RETR` возвращают
ошибку для любого high-bit octet и не отправляют такое сообщение как ASCII;
`[UTF8]` используется, когда обнаружена корректная non-ASCII UTF-8
последовательность, а непрозрачные high-bit octets получают обычный `-ERR`.
Независимый Linux SMTP->POP3 socket acceptance отправил 8BITMIME UTF-8 письмо,
получил `[UTF8]` от LIST/TOP/RETR в ASCII mode, затем включил UTF8 до USER и
сравнил точные байты RETR. Отдельный IMAP APPEND с единственным `0xFF` проверил
обычный отказ для LIST/TOP/RETR до UTF8; unit-тест подтвердил, что этот октет
не выдаётся и после переключения UTF8, поскольку содержимое не является
валидным UTF-8 и legacy charset conversion не реализован. Новый independent
STARTTLS сценарий после переключения получает `-ERR` для opaque octets и
byte-check-ит RETR валидного UTF-8 письма. Acceptance также проверил CAPA
до/после TLS и отказ STLS после UTF8. MIME-дерево
рекурсивно обнаруживает `Content-Transfer-Encoding: binary`, включая вложенные
части всех `multipart/*`; POP3 `TOP` и `RETR` отклоняют такое сообщение даже в
UTF8 mode. Независимый Linux acceptance сохранил fixture через IMAP APPEND и
подтвердил отказ обеих команд. Если ограниченный MIME parser не может проверить
структуру, `TOP` и `RETR` также завершаются `-ERR`, а сообщение сохраняется в
maildrop. Unit и независимый Linux acceptance проверяют оба отказа для обеих
команд и сохранение сессии. Это fail-closed политика для malformed или
неподдержанной MIME-структуры и она может ограничить POP3-доступность таких
сообщений; IMAP и файловое хранение не затрагиваются. Остальные ограничения RFC
6856 сохраняются: legacy charset conversion и UTF-8 credentials/SASLprep не
поддержаны. Повреждённые raw octets сами по себе не считаются UTF-8
строками. `LANG` объявляется и реализует listing единственной локали `en`, выбор
`en` и серверной локали через `*`; корректный, но неподдерживаемый basic language
range получает `-ERR`. Так как поддержана только английская локаль, выбор языка
не меняет тексты ответов. Многоязычная локализация, остальные RFC 6856 сценарии
и интеграция с реальным клиентом не проверялись.
Независимый STARTTLS peer проверил RFC 6856 `LANG` в AUTHORIZATION и
TRANSACTION, обе выдачи CAPA, listing, `EN`, `*`, отказ `de` и возможность
продолжить transaction-команды после неуспешного выбора языка.

## Sieve и ManageSieve

Актуальный проверенный профиль и его ограничения описаны отдельно в
[sieve-coverage.md](sieve-coverage.md), чтобы сохранять компактность этой
матрицы.
В частности, независимый Linux socket acceptance проверяет RFC 5804
capability transitions ManageSieve вокруг STARTTLS и AUTHENTICATE; implicit-TLS
проверен unit-тестом, а совместимость с Litterkeep остаётся непроверенной.

## Outbound SMTP

Маршрут задаётся явно как IP:port и typed TLS policy. Scheduler durable,
single-flight, применяет bounded retry/backoff и DSN. Pool повторно выдаёт только
чистое соединение после success либо подтверждённого RSET.
После EHLO клиент разбирает multiline reply и typed extension keywords. Для
сообщения с high-bit octets в MIME body отправляется `BODY=8BITMIME`, если peer
рекламировал `8BITMIME`; без capability доставка завершается permanent failure
до `MAIL`. Для валидных UTF-8 header values и Unicode local-part в sender или
recipient envelope клиент требует от peer `SMTPUTF8` вместе с `8BITMIME` и
добавляет `SMTPUTF8` к MAIL. При нескольких адресатах флаг устанавливается,
если хотя бы один forward-path интернационализирован. Только UTF-8 заголовки
дополнительно требуют `BODY=8BITMIME`; ASCII message с Unicode envelope не
получает ненужный BODY параметр. RFC 6531 extension не отправляется, если peer
не объявил обе capabilities. Targeted builder tests проверяют envelope-only
случай и отказ peer без SMTPUTF8; прежний независимый Linux SMTP peer проверял
wire параметры для UTF-8 headers. Сквозной outbound MTA и Litterkeep app не
проверялись.
Outbound SMTP ограничивает каждую строку greeting и reply, включая каждую
строку multiline EHLO, значением RFC 5321 §4.5.3.1.5: не более 512 октетов с
CRLF. Reader не накапливает сверх лимита и закрывает соединение при превышении
или неверном окончании строки. Независимый peer подтвердил приём точной
границы 512 и отказ для 513 октетов в greeting и EHLO до `MAIL`.
Как SMTP SIZE client outbound transport разбирает EHLO `SIZE`, `SIZE 0` и
числовой maximum. При объявленном расширении `MAIL FROM` содержит точное число
передаваемых message octets, включая добавленный terminal CRLF, но без
terminator и dot-stuffing; без объявления параметр не отправляется. Размер
выше фиксированного maximum отвергается до `MAIL`. Независимый peer проверяет
wire command и все три исхода. Implicit-TLS unit peer проверяет совместное
использование `SIZE` и `BODY=8BITMIME`.
Для валидного MIME-дерева с `Content-Transfer-Encoding: binary` relay никогда
не подменяет payload командой DATA/8BITMIME. Если peer не объявляет одновременно
`BINARYMIME` и `CHUNKING`, команда MAIL не отправляется, и job получает
permanent failure согласно допустимой политике [RFC 3030 §3](https://www.rfc-editor.org/rfc/rfc3030.html#section-3).
При наличии обеих extensions используется точное BDAT framing; автоматическое
преобразование в Base64 пока не выполняется.
По RFC 1870 EHLO numeric `SIZE` не ограничен 20 цифрами; это ограничение
применяется к value команды MAIL. Peer limit выше диапазона `u64` сохраняется
отдельным typed случаем, поэтому корректное capability не теряется. Rust и
независимый peer тесты проверяют 21-значный предел и точное MAIL declaration;
см. [RFC 1870 §3](https://www.rfc-editor.org/rfc/rfc1870.html#section-3).
Outbound reply handling теперь привязано к фазе SMTP: только greeting 220,
`DATA` continuation 354, `QUIT` 221 и AUTH success 235 принимаются в этих
контекстах; команда `MAIL`, `RCPT`, финальная DATA completion и reset требуют
positive completion 2yz, при этом 4yz остаётся deferred, а 5yz permanent.
Неожиданный 3yz не продвигает состояние команды. Код каждой строки multiline
reply обязан совпадать с первой. Независимый peer подтвердил, что 354 и 221 в
ответ на MAIL не приводят к отправке RCPT; unit tests проверяют известные
фазовые ответы, будущий 2yz код и смешанные коды multiline.
RFC 5321 §4.2 разрешает клиенту обрабатывать reply code без текста: outbound
reader принимает `250\r\n` и `250 \r\n`, сохраняя трёхзначный numeric code,
CRLF и multiline separator; для будущих числовых кодов исход клиентской
операции выбирается по первому разряду. Отдельный TCP peer прошёл полную отправку с bare
220/250/354/221 ответами; Litterkeep и внешние MTA в этом тесте не участвовали.
Ответ `421` трактуется отдельно от прочих временных отказов: очередь получает
`Deferred`, но соединение считается закрываемым. Plaintext и implicit-TLS pools
не отправляют `RSET` и не переиспользуют такой socket. Независимые SMTP test peers
проверили закрытие после `421` на RCPT и успешную следующую доставку через новое
соединение. Остальные 4yz по-прежнему могут вернуть соединение в pool только
после успешного `RSET`.
Исходящий клиент начинает с `EHLO`; при однострочном `500` «command not
recognized» повторяет идентификацию через `HELO` и работает без ESMTP
capabilities. Policy-отказ `550` не вызывает downgrade. Независимые plaintext и
implicit-TLS peers проверили wire-порядок `EHLO` -> `HELO` -> `MAIL` и доставку
без не объявленных `SIZE`/`8BITMIME` параметров.
Исходящий SMTP использует отдельные socket timeouts по RFC 5321 §4.5.3.2:
300 секунд для greeting/MAIL/RCPT и обычных команд, 120 секунд для ответа на
DATA, 180 секунд на каждую системную отправку DATA и 600 секунд на финальный
ответ после terminator. Пять настроек принимаются в положительных секундах через
`LITTERKEEP_SMTP_*_TIMEOUT_SECONDS`; отсутствие значения оставляет нормативный
default, некорректные значения отклоняются при старте outbound worker-а.
Unit-тест сверяет таблицу фаз и валидирует partial overrides/ошибки; тестовый
transport может задать короткий единый предел, а wire-тест подтверждает timeout
зависшего greeting. Дополнительный независимый TCP peer с короткими phase-specific
overrides удерживает соединение открытым до timeout для greeting, команд EHLO,
MAIL и RCPT, ожидания `354` на DATA и финального ответа после фактически
полученного DATA terminator. Это проверяет шесть реальных точек ожидания. QUIT,
BDAT и нормативные минуты ожидания на wall clock не проверялись; Litterkeep app
не проверялось. Отдельный rustls unit peer проверяет переход greeting -> EHLO
для implicit TLS при различающихся greeting/command deadlines.
DATA передаётся потоково без дополнительной копии полного message: RFC-ограниченная
строка формирует bounded fragment, dot-stuffing применяется до записи, а каждый
фрагмент отправляется через `write_all`. Unit writer с partial writes проверяет
границу фрагмента и точные octets; независимый Linux SMTP peer сверил около 2 MiB
payload после dot-unstuffing. Это не измерение peak RSS и не тест максимального
25 MiB сообщения.
