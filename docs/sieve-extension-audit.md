# Инвентарь современных расширений Sieve

Дата сверки: 2026-09-29. Инвентарь ограничен расширениями, заметно меняющими
совместимость с современными Sieve clients. Он не является полным перечнем
IANA registry и не заменяет отдельные acceptance для каждого расширения.

Реальный `SIEVE` capability из `src/managesieve/session.rs`:
`fileinto subaddress envelope copy vacation vacation-seconds`.
Проверка capability unit-тестом подтверждает именно этот список. Расширение
считается поддержанным только когда оно рекламируется и имеет parser,
валидацию, typed execution semantics и профильные тесты.

| Capability | RFC | Статус в litterkeep | Следующая необходимая граница |
|---|---|---|---|
| `fileinto` | RFC 5228 | Реализовано, рекламируется | Сценарии `:create` и современные mailbox selectors остаются за пределами базовой поддержки. |
| `subaddress` | RFC 3598 | Реализовано, рекламируется | Проверить все допустимые recipient-part варианты и межклиентские адресные соглашения. |
| `copy` | RFC 3894 | Реализовано, рекламируется | Проверить комбинации с actions и доставкой на независимом профиле. |
| `vacation-seconds` | RFC 6131 | Реализовано, рекламируется | Остальные границы vacation и взаимодействие с расширениями требуют отдельного профиля. |
| `variables` | [RFC 5229](https://www.rfc-editor.org/rfc/rfc5229.html) | Есть внутренняя capture-aware matcher основа и per-script byte capture state, extension не реализовано и не рекламируется | Typed string templates и expansion в момент выполнения; `set` с проверкой имён; `string` test над source/key списками; полный набор modifier-ов; проверяемые лимиты и validation `${0}`..`${9}`. |
| `body` | [RFC 5173](https://www.rfc-editor.org/rfc/rfc5173.html) | Не реализовано, не рекламируется | MIME-aware `:raw`, `:content`, `:text`; не допускать совпадений через границы частей и ограничить CPU/память. |
| `reject`, `ereject` | [RFC 5429](https://www.rfc-editor.org/rfc/rfc5429.html) | Не реализовано, не рекламируется | Нужна Sieve evaluation до SMTP принятия DATA для protocol-level refusal; post-accept DSN не эквивалентен `ereject`. |
| `imap4flags` | [RFC 5232](https://www.rfc-editor.org/rfc/rfc5232.html) | Не реализовано, не рекламируется | Реализовать `setflag`, `addflag`, `removeflag`, `hasflag` и `:flags` с durable mailbox flag mutation. |
| IMAP events | [RFC 6785](https://www.rfc-editor.org/rfc/rfc6785.html) | Не реализовано, не рекламируется | Требуются IMAP flag support и определённый event context для Sieve execution. |
| `mailboxid` | [RFC 9042](https://www.rfc-editor.org/rfc/rfc9042.html) | Не реализовано, не рекламируется | Требуются устойчивые mailbox OBJECTID, `:mailboxid` у `fileinto` и `mailboxidexists`. |
| Mailbox metadata | [RFC 5490](https://www.rfc-editor.org/rfc/rfc5490.html) | Не реализовано, не рекламируется | `mailboxexists`, metadata tests и metadata-based processing. |
| Special-use delivery | [RFC 8579](https://www.rfc-editor.org/rfc/rfc8579.html) | Не реализовано, не рекламируется | Требуются `specialuse`, `:specialuse` и согласование с typed special-use каталогом. |

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

Это ещё не RFC 5229 implementation. До включения capability должны быть
реализованы интерпретация variable-ref за один проход (неизвестная переменная
становится пустой строкой, некорректная ссылка остаётся как есть), case-
insensitive имена, расширение только после обработки quoting/encoded-character,
runtime expansion на момент достижения команды, `set` только с константным
валидным именем и запретом изменения match variables, `string` test без
неявного trim, и все шесть modifier-ов с RFC precedence и запретом двух
modifier-ов одного precedence. Runtime должен поддержать минимум 128 distinct
переменных, 32 ASCII-символа имени и 4000 символов значения, а также `${0}`
и `${1}`..`${9}`; больший match-index должен отклоняться при validation.
Нужно отдельно зафиксировать поведение Unicode character length и bounded
memory expansion. Пока эти требования не проверены совместно, `variables`
должно оставаться не рекламируемым.

## Базовый RFC 5228: адресные списки

Sieve `address` использует общий RFC 5322 mailbox parser. До T424 проекция
ограничивалась первыми 64 mailboxes и молча игнорировала остальную часть
валидного заголовка, что противоречило правилу «любой адрес/key match». Лимит
удалён; стоимость ограничивается максимальным размером принятого сообщения
(25 MiB). Регрессионный тест проверяет сохранение 65-го адреса. Это устраняет
только скрытое усечение и не является заявлением полной совместимости
address-test с RFC 5228.
