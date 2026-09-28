# Зависимости

Серверный бинарник имеет ровно две прямые production-зависимости из allowlist
технического задания. Почтовые протоколы, MIME, Sieve, файловый Store, очередь,
reactor и lifecycle orchestration реализованы в этом репозитории.

| Зависимость | Версия | Features | Назначение |
|---|---:|---|---|
| `rustls` | 0.23.45 | `std`, `ring`, `tls12` | TLS server/client state и криптографическая проверка сертификатов. `ring` является единственным выбранным backend. |
| `argon2` | 0.6.0 | `alloc`, `password-hash`, `getrandom` | Argon2id PHC password records и криптографически безопасные соли. |

`rcgen 0.14.10` является только dev-dependency. Она доступна
`examples/make_test_pki.rs` для однократного выпуска test CA и не попадает в
обычный runtime server binary.

### Собственные примитивы для SCRAM-SHA-1 и оставшийся парольный verifier

RFC 5804 §2.1 требует SCRAM-SHA-1. Allowlist production dependencies по-прежнему
разрешает только `rustls` и `argon2`; hash/HMAC API закреплённого rustls backend
не является публичным API приложения, а прямое использование транзитивного
crate недопустимо. В `src/sasl/sha1.rs`, `hmac_sha1.rs` и `pbkdf2_sha1.rs`
добавлены собственные safe-Rust примитивы без новой зависимости. Их узкие
проверки сравниваются с RFC 3174, RFC 2202 и RFC 6070 известными ответами.
SHA-1 применяется только как legacy совместимость SCRAM, не для подписей,
нового хранения паролей или защиты от collision атак.

Тип `ScramSha1Verifier` связывает PBKDF2, ClientKey/StoredKey и ServerKey,
проверяет непустой salt длиной до 64 октетов и ограничивает iteration count
диапазоном 4096..=200000. Сквозной тест воспроизводит client proof и server
signature из полного RFC 5802 §5.1 примера. RFC 4013 SASLprep уже реализован;
новые Argon2 credentials сохраняют нормализованный `.phc2`, а успешный вход
по legacy `.phc` публикует `.phc2` с сохранением старого файла. SMTP TLS
black-box acceptance отдельно подтвердил wire-аутентификацию и приоритет
новой записи.

Создание нового account теперь пишет отдельный versioned
`litterkeep-scram-sha1-v1` sidecar с криптографически случайной солью,
ограниченным числом итераций, StoredKey и ServerKey. Sidecar создаётся после
durable публикации Argon2 credential; при ошибке создание Argon2 файла
откатывается. Строгий parser отвергает неизвестные версии, лишние поля,
неверную длину ключей и чрезмерную стоимость. Reload-тест проверяет proof
через восстановленную запись. Уже существующие Argon2 аккаунты пока не имеют
SCRAM sidecar, но успешная проверка `.phc2` теперь безопасно создаёт его при
backfill; legacy `.phc` миграция публикует sidecar до новой записи. Неуспешный
вход не меняет файлы. T327 добавил bounded-worker путь для чтения sidecar
единственным Accounts owner-ом; полный exchange подключён в T328.
Для подготовки протокольного слоя модуль `scram_attributes` разбирает только
ограниченный синтаксис уникальных односимвольных атрибутов, отклоняет
неподдержанные mandatory extension `m=` и ограничивает сообщение 16 KiB и
26 атрибутами. T323 исправил чрезмерное ASCII-only ограничение T322: по RFC
5802 §7 обычные `value-char` включают UTF-8 и пробельные/control октеты,
кроме NUL и разделителя comma; отдельный nonce профиль ограничен printable
ASCII без comma. `scram_client_first` поддерживает только GS2 `n,,`, username
SASLprep/escaping и сохраняет точный transcript; optional extensions
игнорируются как требует RFC.
`scram_server_first` формирует `r,s,i`, добавляя 18 криптографически случайных
октетов Rustls к client nonce; соль и iteration count берутся из сохранённой
verifier-записи; он подключён к SASL lifecycle.
`scram_client_final` проверяет фиксированные `c,r,[extensions],p`, exact
GS2 `biws`, полный nonce и canonical 20-octet ClientProof. Он формирует
AuthMessage из исходных SCRAM transcript строк; полный RFC 5802 §5.1
known-answer проходит через StoredKey verifier и совпадает с published
ServerSignature.
`scram_server_final` завершает тот же crypto transcript типизированным
`v=<signature>` или `e=invalid-proof`; все RFC 5802 §7 server-error значения
закрыты enum. ManageSieve exchange связывает final data с protocol-level
success, а независимый TLS peer проверяет end-to-end transcript, включая
серверную подпись и отказ неверного proof.
[`RFC 5804 §2.1`](https://www.rfc-editor.org/rfc/rfc5804.html#section-2.1)
прямо требует SCRAM-SHA-1 вместе с PLAIN over TLS; оба механизма рекламируются
только после TLS. Независимый TLS peer подтвердил полный ManageSieve exchange,
но STARTTLS и приложение Litterkeep не проверялись. Синтаксическая основа сверялась
по [RFC 5802 §7](https://www.rfc-editor.org/rfc/rfc5802.html#section-7).
[RFC 7677](https://www.rfc-editor.org/rfc/rfc7677.html) регистрирует
SCRAM-SHA-256, более сильный алгоритм семейства, и отмечает связанные с
SHA-1 и TLS channel binding требования безопасности. SCRAM-SHA-256 теперь
реализован для ManageSieve TLS; PLUS/channel binding остаются отдельным пробелом.

Для SCRAM-SHA-256 добавлены собственные safe-Rust примитивы `sha256`,
`hmac_sha256`, одноблочный `pbkdf2_sha256`, `ScramSha256Verifier` и отдельный
versioned `ScramSha256Record`. Они не добавляют прямых production dependencies.
SHA-256 сверяется с RFC 6234, HMAC с RFC 4231, PBKDF2 с RFC 7914 §11, а полный
client proof/server signature — с примером RFC 7677 §3; iteration count
ограничен значением 200000. Аккаунты сохраняют SHA-256 verifier в отдельном
sidecar и предоставляют его через типизированный запрос единственного Accounts
worker; запрос не выводит секрет в Debug. POP3 и ManageSieve используют
отдельный SHA-256 verifier lookup через bounded Accounts owner. POP3 принимает
SCRAM-SHA-256 только внутри TLS и выдаёт maildrop после mutual proof.
ManageSieve state machine принимает
initial-response и continuation, а capability рекламирует механизм только
после TLS. Независимые Linux peers подтвердили mutual proof и server signature
по implicit TLS и STARTTLS; отдельный Python peer также завершил POP3
SCRAM-SHA-256 по STARTTLS и implicit TLS с независимым proof. TLS 1.3 tls-exporter проверен
между rustls endpoints и подключён к ManageSieve SCRAM-SHA-256-PLUS с точной
GS2/c= binding-проверкой и live wire acceptance. Litterkeep app interoperability
для этих механизмов пока не проверялась.

IMAP mailbox validation использует собственные generated ranges Unicode
17.0.0, а не crate. Официальный источник, версия toolchain, SHA-256 и лицензия
зафиксированы в [`unicode-data.md`](unicode-data.md); compile-time assertion
сверяет таблицу с `char::UNICODE_VERSION`.

Проверяемый граф зависимостей строится из закреплённого `Cargo.lock`:

```powershell
cargo tree --locked --edges normal
cargo tree --locked --edges build
cargo tree --locked --edges dev
cargo metadata --locked --format-version 1
```

Транзитивные crypto, certificate и build dependencies видны в этих командах.
Они не означают подключение готового MTA, базы данных, runtime, actor framework
или mail parser. Новую прямую production dependency нельзя добавлять без
обновления этого файла, `Cargo.toml`, lockfile и проверки allowlist.
