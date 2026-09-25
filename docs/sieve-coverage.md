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

## RFC 5230 vacation: не реализовано

`vacation` пока не входит в поддерживаемый профиль и не объявляется в
ManageSieve capability. Store теперь может независимо фиксировать
сгенерированные RFC822 bytes как committed outbound payload без mailbox link;
очередной журнал кодирует пустой SMTP reverse-path токеном `<>` и сохраняет его
через deferred/reopen. Store test доказывает точное восстановление payload, а
Linux SMTP peer подтверждает реальную передачу отдельного ответа с `MAIL FROM:<>`.
Дополнительно Store принимает typed vacation request и атомарно фиксирует claim
подавления по account, envelope sender и response identity в
`journal/vacation-history.log` вместе с payload/queue transaction. Claims
восстанавливаются только для committed transaction ID; regressions покрывают
reopen, expiration и отказ до commit. Это storage foundation: Sieve parser и
executor её пока не вызывают, response identity normalization и RFC guards
ещё не реализованы.
Это инфраструктурная основа, не реализация `vacation`. RFC 5230 также требует:
deduplication ответов по адресу отправителя и идентичности vacation-ответа,
период `:days`, максимум не менее 1000 записей истории, ограничения
автоответов для daemon/list сообщений, один vacation action на script и
сохранение implicit keep. Генерируемый ответ использует пустой SMTP
reverse-path и заголовки/связи ответа, определённые стандартом.

Обычный Sieve redirect по-прежнему ссылается на blob входящего письма, тогда как
`Store::enqueue_outbound_payload` создаёт отдельную transaction/blob для
сгенерированного сообщения. Extension нельзя принимать или объявлять до
реализации durable suppression history и всех RFC-защит. Acceptance для
будущей реализации должен доказать первое ответное письмо, отсутствие
повторного ответа тому же отправителю в интервале,
повтор после срока, запрет ответов на автоматические/списочные письма,
корректные RFC 5322/MIME поля, пустой MAIL FROM и сохранение исходной локальной
доставки. Требования сверены с RFC 5230 §§4.1-4.7 и 5.1-5.8.

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
проверяет его Unicode-сопоставление при выборе папки. Общий header parser
также игнорирует SP/HTAB между field-name и `:`, как требует §2.4.2.2;
сетевой тест подтверждает, что `Subject \t:` маршрутизируется по Sieve, а IMAP
FETCH возвращает исходный байтовый заголовок без переписывания. Header matcher
принимает пустой ключ и различает `:contains` (пустая подстрока содержится в
значении существующего поля) от `:is` (пустое значение не равно непустому).
Невалидное имя поля в `header` test принимается как заведомо не совпадающее,
а не как ошибка скрипта; эти случаи проверены unit-тестом и ManageSieve CHECKSCRIPT.
