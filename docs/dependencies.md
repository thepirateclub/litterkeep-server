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

Это только криптографическая основа, а не реализация SCRAM. Account-файл пока
содержит только Argon2id PHC verifier, из которого нельзя восстановить пароль
или SCRAM salted password. Следующие обязательные работы: versioned SCRAM
verifier при регистрации/смене пароля, безопасный формат миграции существующих
аккаунтов, bounded iteration policy, SASL state machine, constant-time proof
validation и независимый ManageSieve/POP3 wire acceptance. Пока они не готовы,
сервер не рекламирует SCRAM и матрица считает его незакрытым RFC-пробелом.

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
