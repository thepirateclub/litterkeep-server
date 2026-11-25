# Инвентарь современных расширений Sieve

> **О документе.** Инвентарь современных расширений Sieve: что рекламируется, что реализовано внутри, что не начато.
>
> Навигация по документации — [docs/ru/README.md](README.md), сводная готовность — [STATUS.md](./STATUS.md).

## Оглавление

- [Область инвентаря и рекламируемый capability](#область-инвентаря-и-рекламируемый-capability)
- [Статус расширений и следующие необходимые границы](#статус-расширений-и-следующие-необходимые-границы)
- [Позиция сервера и история реализации](#позиция-сервера-и-история-реализации)
- [Текущий остаток RFC 5229 Variables](#текущий-остаток-rfc-5229-variables)
- [Состояние RFC 5228 encoded-character](#состояние-rfc-5228-encoded-character)
- [Историческая запись T421-T429](#историческая-запись-t421-t429)
- [Развитие VariableStore и matcher backtracking](#развитие-variablestore-и-matcher-backtracking)
- [Базовый RFC 5228: адресные списки](#базовый-rfc-5228-адресные-списки)

### Область инвентаря и рекламируемый capability

Дата сверки: 2026-09-29. Инвентарь ограничен расширениями, заметно меняющими
совместимость с современными Sieve clients. Он не является полным перечнем
IANA registry и не заменяет отдельные acceptance для каждого расширения.

Реальный `SIEVE` capability из `src/managesieve/session.rs`:
`fileinto subaddress envelope copy vacation vacation-seconds`.
Проверка capability unit-тестом подтверждает именно этот список. Расширение
считается поддержанным только когда оно рекламируется и имеет parser,
валидацию, typed execution semantics и профильные тесты.

### Статус расширений и следующие необходимые границы

| Capability | RFC | Статус в litterkeep | Следующая необходимая граница |
|---|---|---|---|
| `fileinto` | RFC 5228 | Реализовано, рекламируется | Сценарии `:create` и современные mailbox selectors остаются за пределами базовой поддержки. |
| `subaddress` | RFC 3598 | Реализовано, рекламируется | Проверить все допустимые recipient-part варианты и межклиентские адресные соглашения. |
| `copy` | RFC 3894 | Реализовано, рекламируется | Проверить комбинации с actions и доставкой на независимом профиле. |
| `vacation-seconds` | RFC 6131 | Реализовано, рекламируется | Остальные границы vacation и взаимодействие с расширениями требуют отдельного профиля. |
| `variables` | [RFC 5229](https://www.rfc-editor.org/rfc/rfc5229.html) | Частично: `set` AST/parser/runtime, per-script store, modifiers, capture references и `string` test работают; common condition/action strings, comparator и envelope-part arguments раскрываются во время выполнения; capability не рекламируется | Переменные в произвольных byte strings, совместный acceptance профиль и ManageSieve discovery/interoperability. |
| `encoded-character` | [RFC 5228 §2.4.2.4](https://www.rfc-editor.org/rfc/rfc5228.html#section-2.4.2.4) | Частично: `parse_octets` декодирует `${hex:...}`/`${unicode:...}` во всех строковых token-ах после quoted escape и dot-unstuffing; matcher и fileinto/redirect actions хранят byte-valued аргументы; capability не рекламируется | Провести октеты через vacation/set и прочие AST/runtime границы; тестировать raw-byte ManageSieve scripts и полную RFC матрицу; только затем решать об advertisement. |
| `body` | [RFC 5173](https://www.rfc-editor.org/rfc/rfc5173.html) | Не реализовано, не рекламируется | MIME-aware `:raw`, `:content`, `:text`; не допускать совпадений через границы частей и ограничить CPU/память. |
| `reject`, `ereject` | [RFC 5429](https://www.rfc-editor.org/rfc/rfc5429.html) | Не реализовано, не рекламируется | Нужна Sieve evaluation до SMTP принятия DATA для protocol-level refusal; post-accept DSN не эквивалентен `ereject`. |
| `imap4flags` | [RFC 5232](https://www.rfc-editor.org/rfc/rfc5232.html) | Не реализовано, не рекламируется | Реализовать `setflag`, `addflag`, `removeflag`, `hasflag` и `:flags` с durable mailbox flag mutation. |
| IMAP events | [RFC 6785](https://www.rfc-editor.org/rfc/rfc6785.html) | Не реализовано, не рекламируется | Требуются IMAP flag support и определённый event context для Sieve execution. |
| `mailboxid` | [RFC 9042](https://www.rfc-editor.org/rfc/rfc9042.html) | Не реализовано, не рекламируется | Требуются устойчивые mailbox OBJECTID, `:mailboxid` у `fileinto` и `mailboxidexists`. |
| Mailbox metadata | [RFC 5490](https://www.rfc-editor.org/rfc/rfc5490.html) | Не реализовано, не рекламируется | `mailboxexists`, metadata tests и metadata-based processing. |
| Special-use delivery | [RFC 8579](https://www.rfc-editor.org/rfc/rfc8579.html) | Не реализовано, не рекламируется | Требуются `specialuse`, `:specialuse` и согласование с typed special-use каталогом. |

### Позиция сервера и история реализации

RFC Editor указывает для RFC 5228 обновления RFC 5229, 5429, 6785 и 9042.
Таблица включает также RFC 5173 и базовые широко используемые extensions;
прочие стандарты Sieve, их dependencies и errata требуют дальнейшего аудита.
На дату этой записи корректная позиция сервера - частичная реализация с узким
списком рекламируемых extensions, не полный современный Sieve профиль.

T421 добавил в `src/sieve/matching/glob.rs` helper, который возвращает byte
captures для `*` и `?` по мере выполнения существующего whole-string matcher.
T422 подключил эти результаты к per-script execution state: capture zero
содержит совпавший вход, последующие captures следуют порядку wildcard-ов;
ошибочное сопоставление не перезаписывает состояние, а составные тесты
сохраняют обязательные порядок слева направо и short-circuit.

### Текущий остаток RFC 5229 Variables

**Текущий остаток RFC 5229 после T445.** `set`, `string`, modifier-ы, один
проход раскрытия, numeric captures с ведущими нулями, неизвестные имена и
per-script scope теперь связаны с parser/runtime. `string` сохраняет пробелы,
сравнивает оба списка, обновляет captures успешного `:matches` и участвует в
short-circuit. Условия `header`, `address`, `envelope` и `exists` раскрывают
свои string arguments непосредственно перед оценкой, только если требуется
`variables`. `fileinto`, `redirect` и все string-поля `vacation` раскрываются
в точке action и повторно проходят проверки path, mailbox и MIME; redirect по-
прежнему проходит Store allowlist/hop-limit. RFC 5229 backslash quoting уже
снимается лексером до expansion; регрессия покрывает `\${foo}`, `\\${foo}` и
literal-dollar single-pass образец. Compile-time проход AST отвергает namespace,
если его capability не требуется; `${variables.unknown}` считается неизвестным
именем и раскрывается в пустую строку. Comparator string arguments также
раскрываются в точке условия и повторно разрешаются только в поддерживаемые
enum-значения; неизвестный результат вызывает runtime failure. Envelope-part
string list также раскрывается в точке `envelope` test-а и повторно разбирается
в `from`/`to`. Расширение всё ещё не рекламируется: RFC 5228
`encoded-character` expansion (раздел 2.4.2.4) частично реализован после T446,
но не рекламируется из-за неполного byte-valued action path. Production
`parse_octets(&[u8])` сохраняет quoted/multiline string bytes; `StringMatch`
keys и `Condition::String.sources` в AST также byte-valued. `is`, `contains` и
`matches` сравнивают октеты без lossy UTF-8, а подстрочный поиск использует
линейный KMP. Parser-to-runtime regression подтверждает literal с октетами
`FF 80`. После T447 аргументы `fileinto` и `redirect` также сохраняются как
октеты в AST; runtime преобразует их в UTF-8 при проверке mailbox/address,
поэтому бинарный target синтаксически принимается, но завершается typed runtime
error. Vacation/set AST и `VariableStore` остаются текстовыми: byte literal с
interpolation marker-ом пока требует UTF-8. Поэтому произвольный `hex` octet
ещё не проходит весь script путь, а переменная не может безопасно
интерполироваться в произвольный binary literal.
Нормативный порядок:
quoted-string escape и dot-unstuffing, затем `encoded-character`, затем RFC 5229
variable expansion. Нельзя применять lossy UTF-8 к `hex` или объявлять Variables
доступным до byte-exact tests для всего профиля. T443 добавил
ограниченный ManageSieve -> SMTP -> IMAP wire acceptance для dynamic selectors,
однако тест вручную использует `require "variables"`, которое не рекламируется.
Это не подтверждает совместимость клиента, полную RFC conformance матрицу или
готовность включить Variables capability. T445 проверяет parser-to-condition
runtime path на unit-уровне, но не ManageSieve wire acceptance с raw bytes и не
полный Sieve script path через actions.

### Состояние RFC 5228 encoded-character

**RFC 5228 encoded-character после T447.** Начальный `require
"encoded-character"` включает замену `${hex:...}` на octets и `${unicode:...}`
на UTF-8 scalar values после лексического unescaping и dot-unstuffing. Имена
sequence case-insensitive; malformed input остаётся буквальным, nested
sequence обрабатывается по RFC, well-formed недопустимая Unicode scalar
sequence даёт parse error. Парсер передаёт результат в byte-valued matcher
AST; fileinto/redirect сохраняют байты до проверки UTF-8 на runtime boundary.
Vacation/set аргументы и несколько других AST полей пока требуют UTF-8,
поэтому расширение распознаётся для явно требующих его scripts, но не
публикуется в ManageSieve capability list. Это не полный extension profile.

### Историческая запись T421-T429

Историческая запись T421-T429 ниже описывает состояние на даты тех этапов;
её выводы о полном отсутствии `set`/`string` superseded текущим статусом выше.
До включения capability требовалось:
реализованы интерпретация variable-ref за один проход (неизвестная переменная
становится пустой строкой, некорректная ссылка остаётся как есть), case-
insensitive имена, расширение только после обработки quoting/encoded-character,
runtime expansion на момент достижения команды, `set` только с константным
валидным именем и запретом изменения match variables, `string` test без
неявного trim, и все шесть modifier-ов с RFC precedence и запретом двух
modifier-ов одного precedence. Runtime должен поддержать минимум 128 distinct
переменных, 32 ASCII-символа имени и 4000 символов значения, а также `${0}`
и decimal match-index-ы; leading zeroes допустимы, отсутствующий index
раскрывается в пустую строку.
Нужно отдельно зафиксировать поведение Unicode character length и bounded
memory expansion. Пока эти требования не проверены совместно, `variables`
должно оставаться не рекламируемым.

### Развитие VariableStore и matcher backtracking

T425 добавил самостоятельный `VariableStore`: ASCII-регистронезависимые имена,
до 128 переменных, имена до 32 символов, сохранённые значения обрезаются до
4000 Unicode scalar values. `expand` выполняет один проход, раскрывает numeric
captures и именованные ссылки, оставляет некорректные ссылки и возвращает пустую
строку для неизвестных валидных имён. Numeric references принимают любое
decimal-представление индекса, включая leading zeroes; недоступный или
переполняющий машинный тип индекс раскрывается в пустую строку. Отдельный
локальный предел результата -
65 536 символов; его превышение возвращает typed error. Это протестированное
ядро библиотеки, но не часть AST/executor pipeline. В частности, lexer пока не
передаёт provenance escaping/encoded-character, parser не разбирает и не
валидирует variable references, а engine не подключён к typed ошибкам parser-а.
T426 добавил typed errors в
engine для валидной namespace-ссылки без включённого namespace extension;
parser-time diagnostics ещё отсутствуют. Предыдущие T426/T427 неверно
трактовали numeric references как одиночную цифру или syntax error; RFC 5229
разрешает leading zeroes и раскрывает недоступный index в пустую строку.
Расхождение исправлено в T428. `set` и `string` отсутствуют, поэтому
capability остаётся скрытым.

T428 также заменил greedy glob capture matching на backtracking, чтобы
соблюдать non-greedy распределение wildcard captures и сбрасывать значения
неудачных ветвей. Поиск ограничен одним миллионом шагов. T429 исправил путь
исчерпания бюджета: matcher возвращает typed work-limit error, evaluator
передаёт его как `ExecutionError::MatcherLimit`, а delivery planner выбирает
implicit keep и пишет diagnostic вместо ложного `NoMatch`. Store integration
test проверяет, что частичный `discard` не применяется, письмо остаётся в
INBOX, а typed diagnostic читается после переоткрытия Store.

## Базовый RFC 5228: адресные списки

Sieve `address` использует общий RFC 5322 mailbox parser. До T424 проекция
ограничивалась первыми 64 mailboxes и молча игнорировала остальную часть
валидного заголовка, что противоречило правилу «любой адрес/key match». Лимит
удалён; стоимость ограничивается максимальным размером принятого сообщения
(25 MiB). Регрессионный тест проверяет сохранение 65-го адреса. Это устраняет
только скрытое усечение и не является заявлением полной совместимости
address-test с RFC 5228.

---

[Навигация по документации](README.md) · [Статус проекта](./STATUS.md) · [README](../../README.md)
