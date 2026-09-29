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
| `variables` | [RFC 5229](https://www.rfc-editor.org/rfc/rfc5229.html) | Не реализовано, не рекламируется | Typed string templates, scopes, match captures, modifiers, expansion limits и контроль рекурсивного expansion. |
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
