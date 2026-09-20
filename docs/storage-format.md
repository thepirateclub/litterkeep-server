# Формат файлового хранилища

Корень помечен `litterkeep-store-v2`. Неизвестный или непомеченный каталог с
mail data отклоняется, автоматического угадывания старого формата нет.

Immutable message blob публикуется через temporary file, `fsync`, atomic rename
и directory sync. Mailbox links, flags, folders, subscriptions, Recent, Sieve,
diagnostics, queue и global transaction commits используют framed append-only
journals. Полный неверный frame считается corruption, неполный tail безопасно
обрезается. Mailbox link становится видимым только после global commit.

UID монотонен внутри mailbox и не переиспользуется после EXPUNGE. COPY делит
immutable blob, MOVE публикует destination и source tombstone одной transaction.
Pending outbound delivery остаётся владельцем blob до terminal transition.

`check` берёт exclusive Store lock, выполняет recovery и читает каждый
referenced blob. Orphans только считаются. `backup` требует остановленного
writer, исключает runtime lock/locator, синхронизирует копию и запускает checker.

