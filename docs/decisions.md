# Архитектурные решения

## Исполнение и ожидания

Сервер использует один Linux `poll(2)` reactor для listener-ов и сессий.
Протокольная сессия является ресурсом `Orchestrator`: `ResourceLease<Session>`
содержит реальный socket/state, а registry хранит только проверяемую metadata.
Storage completion будит poll-loop Unix socket-ом, не отдельным thread на
ожидающую сессию. Проверки `exclusive_resource_cannot_be_double_issued` и
`signal_between_observation_and_park_is_not_lost` подтверждают exclusive lease
и отсутствие lost wake между observation и parking.

## Граница unsafe

Единственная производственная unsafe-граница находится в
`src/platform/linux_poll.rs`: FFI вызов `poll(2)`. Перед вызовом создаётся
живой contiguous `repr(C)` массив дескрипторов, а функция заимствует их только
на время syscall. Остальной crate запрещает unsafe через `#![deny(unsafe_code)]`.

## Владение ресурсами

Store открывается единственным `StorageWorker`; protocol session передаёт ему
bounded request и никогда не получает `Store` lease. Исходящий SMTP worker
владеет одним bounded `ConnectionPool` в единственном thread. Его completion
channel является источником истины, а Linux wake socket передаёт лишь
coalesced hint. Поэтому полный wake socket не теряет completion.

## Фиксация хранилища и очереди

Blob, mailbox links и owned outbound job публикуются одним durable Store
transaction. Очередь видит job только после global commit ID. Перед исходящей
SMTP попыткой job должен получить durable `InFlight` transition; recovery
переводит незавершённый `InFlight` в `Deferred`. Retry deadline хранится как
Unix epoch, отдельно от монотонных сроков reactor-а.

## Профиль IMAP

IMAP использует typed parser и state machine. `\Recent` хранится как
session-local projection: durable delivery фиксирует кандидат UID, а SELECT
atomарно claim-ит его для одной сессии. Поэтому marker не превращается в
persistent message flag и не выдаётся после restart.

## Отмена и очередь

Закрытие входящей сессии освобождает её lifecycle lease, но не удаляет
зафиксированное письмо. Уже committed delivery принадлежит durable queue, а
не TCP-соединению отправителя. Таймаут запускает request cancellation, однако
не является доказательством освобождения живого ресурса: lease возвращается
только после подтверждённого completion либо quarantine/close.
