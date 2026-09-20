# Security profile

Пароли создаются OS/TLS CSPRNG и хранятся как Argon2id PHC. Bootstrap secrets
не передаются в argv и не печатаются; на Linux `test-accounts.tsv` имеет `0600`.
TLS реализован rustls, неизвестный CA и неверный SAN отклоняются.

Каждый внешний account login канонизируется через local-domain enum policy.
Полный адрес внешнего домена не становится путём credential или Store owner.
Mailbox, script и message operations всегда получают owner из authenticated
session, а не произвольный owner ID клиента.

Protocol sizes, literals, DATA, AST, diagnostic journal, Store quota, connection
admission, worker channels и downstream output ограничены. Stalled stage имеет
deadline; slow reader теряет только собственную session lease.

Server logs и runtime control не содержат passwords, TLS keys или mail body.
Тестовый CA предназначен только для локальной acceptance-среды.

