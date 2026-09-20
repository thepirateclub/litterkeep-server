# Итоговая приёмка

Машиночитаемая матрица находится в `docs/acceptance.tsv`. Статус G12 не
завышается: 44 проверки имеют PASS, T23 имеет NOT RUN, T25 имеет BLOCKED.

## Чистый G12 прогон

На новом container-local `/data` bootstrap создал четыре аккаунта. Повтор не
изменил passwords, PHC или CA. Seed дал `created=35 existing=0`, повтор и повтор
после process restart дали `created=0 existing=35`. Независимый Python-клиент
подтвердил 35 fixture IDs и SHA-256 восьми decoded attachments.

После Android-отправки независимый STARTTLS IMAP-клиент с полным login Bob нашёл
ровно одно письмо `G12-Litterkeep-UI`. После остановки writer `check` сообщил
5 mailboxes, 37 links, 36 referenced blobs, 0 orphans и 0 pending deliveries.
Финальный backup скопировал 63 файла и 14,381,601 bytes; восстановленная копия
прошла checker. Предыдущий clean backup также подтвердил одинаковые SHA-256
secret-файла и CA.

## Слои evidence

- `server-protocol`: Rust tests и независимые Python wire clients.
- `client-integration`: физический TECNO CH6i и установленный litterkeep.
- `manual-device`: UI-наблюдение без очистки существующих app data/cache.

Litterkeep действительно проверил TLS/CA, подключил Alice, получил каталог
папок и отправил plain message Bob. Attachment UI отсутствовал, а Bob setup не
завершился, поэтому получение и скачивание вложения приложением не заявлены.

## Не выполнено

T23 не запускался, потому что отдельный third-party MTA не был настроен. T25
заблокирован текущим клиентским UI/вторым account setup, а не серверным Python
тестом. Сырые локальные измерения сохранены под `data/g12-clean` и не входят в
Git, поскольку содержат локальный PKI и runtime artifacts.
