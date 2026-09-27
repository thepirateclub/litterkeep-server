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

### Ограничение SCRAM-SHA-1 для ManageSieve

RFC 5804 §2.1 требует SCRAM-SHA-1, но текущий production allowlist разрешает
только `rustls` и `argon2`. У закреплённого rustls выбран backend `ring`, однако
его HMAC/hash адаптеры внутри rustls объявлены `pub(crate)` и не являются API
этого приложения; использовать транзитивный crate напрямую также означало бы
добавить третью production-зависимость. Account-файл содержит только Argon2id
PHC verifier, из которого нельзя восстановить пароль или SCRAM salted password.
Следовательно, SCRAM нельзя корректно добавить простым протокольным адаптером:
потребуется явное расширение dependency allowlist либо отдельная криптографически
ревьюируемая реализация требуемых примитивов и формат хранения SCRAM verifier.
Пока ни одно из этих изменений не разрешено, сервер не рекламирует SCRAM и
матрица считает его незакрытым RFC-пробелом.

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
