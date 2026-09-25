# Sieve и ManageSieve

Есть bounded parser/executor для обязательного профиля, fileinto, redirect,
keep/discard, header/address/size tests и diagnostics. ManageSieve поддерживает
STARTTLS, SASL PLAIN, literals, CHECKSCRIPT, PUTSCRIPT, LISTSCRIPTS, GETSCRIPT,
SETACTIVE, DELETESCRIPT и RENAMESCRIPT.

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
`fileinto :copy`: полные RFC822 bytes в INBOX и `Invoices` совпадают. Для
`redirect :copy` проверен только Sieve DeliveryPlan; внешний SMTP acceptance
redirect остаётся непроверенным. Правило сверено с RFC 3894 §3.

Повторный `fileinto` в одну папку не считается ошибкой и не создаёт второго
экземпляра письма согласно RFC 5228 §2.10.3. Store-level regression проверяет
сохранение единственной ссылки после restart; независимый ManageSieve -> SMTP
-> IMAP acceptance исполняет два одинаковых `fileinto :copy`, читает обе
папки и сверяет точные RFC822 bytes.

## RFC 5230 vacation: базовый профиль, оставшиеся RFC-пробелы

Parser/executor поддерживают единственный `vacation` action и его RFC-параметры;
ManageSieve объявляет capability. Store создаёт отдельный RFC 5322 response с
`MAIL FROM:<>`, durable suppression claim, loop/list/automated-message guards,
default `:days 7` с диапазоном 1..365, RFC 2047 Subject и quoted-printable UTF-8
body. `:mime` валидируется с учётом текущего 7BIT-only SMTP transport. Unit и
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

Отдельный `vacation_mime` validator применяет ограничения RFC 2045 §§2.7, 6.7
и 6.8: 7bit body не может содержать NUL, bare CR/LF и строки длиннее 998
октетов; Base64/QP требуют CRLF, разрешённые encoded octets и строки максимум
76 символов. QP также запрещает незакодированные завершающие пробелы и табы.
Unit cases проверяют обе границы строк, NUL, bare LF и пробел перед soft break.
Сетевой клиент также проверяет успешную передачу CRLF в Sieve MIME string; он
не подтверждает все граничные длины строк, которые остаются unit-only.

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
некорректные token-ы. Эти новые проверки пока unit-only, без независимого
сетевого MIME клиента.

Для параметров Content-Type/Content-Disposition проверяются RFC 2045 §5.1
attribute=value, token/quoted-string и список tspecials. RFC 2231 attribute
грамматика исключает зарезервированные символы, а continuation номера должны
начинаться с секции 0 и далее идти без ведущих нулей. Unit cases покрывают
пробелы/tspecials в unquoted значениях, повреждённые имена, внутреннюю
неэкранированную кавычку, корректный escaped DQUOTE, пустой quoted value и
filename continuation. Сетевой MIME acceptance этих новых границ не выполнялся.

RFC 2231 extended payload decoder принимает только attribute-char или
percent-encoded octets с uppercase hex digits; проверяются mixed encoded и
unencoded continuation segments. Charset декодируется явным allowlist из
восьми профилей: US-ASCII, UTF-8, ISO-8859-1, Windows-1251, Windows-1252,
UTF-16, UTF-16BE и UTF-16LE. Три UTF-16 профиля используются в RFC 2047 и
RFC 2231, но исключены из декодирования MIME `text/*` при поиске согласно
RFC 2781. RFC 2231 начальная секция может опускать charset и language:
непомеченный ASCII payload принимается, а high-bit octets без charset
отвергаются, поскольку Unicode API не может безопасно угадать их кодировку.
Языковой tag проверяется на well-formed BCP 47 структуру RFC 5646 и сохраняется
без изменения регистра в `StructuredValue` и каждом MIME tree entity. Проверка
не доказывает членство subtags в датированной IANA Language Subtag Registry.
RFC 2231 charset/language regressions unit-only.

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
Resent-To) и сравнивает mailbox внутри групп, не имя группы. SMTP -> Sieve ->
IMAP acceptance доставляет письмо с group-form `To`; произвольные пользовательские
address-list headers этим не подтверждены.

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
проверяют принятие и отказ. Исходящая SMTP-доставка redirect не проверена.

Sieve `header` декодирует RFC 2047 encoded-word общей MIME-декодировкой до
сравнения в UTF-8 и игнорирует ведущие/завершающие SP и HTAB по RFC 5228 §5.7.
Unit покрывает ISO-8859-1 и UTF-8 encoded-word и краевые пробелы; независимая
ManageSieve -> SMTP -> IMAP acceptance отправляет encoded UTF-8 Subject и
проверяет его Unicode-сопоставление при выборе папки. RFC 2047 decoder также
требует непустой `encoded-text` из печатных ASCII octets без SP
и `?`; Q-escape `=01` остаётся допустимым encoded octet. Unit tests покрывают
raw whitespace/control/non-ASCII и сохранение повреждённого слова в header
projection; новые grammar cases отдельно по сети не проверялись. Общий header
parser распознаёт целый encoded-word только в начале field-body или после
SP/HTAB и оставляет embedded/слитный токен обычным текстом по RFC 2047 §6.1.
Эта граница пока покрыта unit-only. Header parser также игнорирует SP/HTAB
между field-name и `:`, как требует §2.4.2.2;
сетевой тест подтверждает, что `Subject \t:` маршрутизируется по Sieve, а IMAP
FETCH возвращает исходный байтовый заголовок без переписывания. Header matcher
принимает пустой ключ и различает `:contains` (пустая подстрока содержится в
значении существующего поля) от `:is` (пустое значение не равно непустому).
Невалидное имя поля в `header` test принимается как заведомо не совпадающее,
а не как ошибка скрипта; эти случаи проверены unit-тестом и ManageSieve CHECKSCRIPT.
