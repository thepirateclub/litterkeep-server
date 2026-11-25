# Архитектурные решения

> **О документе.** Почему сервер устроен именно так: принятые решения, их обоснование и цена.
>
> Навигация по документации — [docs/ru/README.md](README.md), сводная готовность — [STATUS.md](./STATUS.md).

## Исполнение и ожидания

Сервер использует один Linux `poll(2)` reactor для listener-ов и сессий.
Протокольная сессия является ресурсом `Orchestrator`: `ResourceLease<Session>`
содержит реальный socket/state, а registry хранит только проверяемую metadata.
Storage completion будит poll-loop Unix socket-ом, не отдельным thread на
ожидающую сессию. Проверки `exclusive_resource_cannot_be_double_issued` и
`signal_between_observation_and_park_is_not_lost` подтверждают exclusive lease
и отсутствие lost wake между observation и parking.

## Граница unsafe

Производственные unsafe-границы ограничены двумя Linux FFI-модулями, каждый из
которых снимает запрет unsafe только для себя:

- `src/platform/linux_poll.rs` — вызов `poll(2)`. Перед вызовом создаётся живой
  contiguous `repr(C)` массив дескрипторов, а функция заимствует их только на
  время syscall.
- `src/platform/linux_lock.rs` — вызов `flock(2)` для advisory writer lock.
  Дескриптор принадлежит live `File`, а `flock` не получает им владение.

Остальной crate запрещает unsafe через `#![deny(unsafe_code)]` в `src/lib.rs`,
и оба модуля помечены `#![allow(unsafe_code)]` локально.

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
persistent message flag. Все уже selected sessions получают marker при live
уведомлении; это допустимая определённая реализацией сторона неоднозначности
RFC 3501 для нескольких открытых соединений. Durable кандидат остаётся до
первого будущего SELECT, переживает restart до claim и после claim больше не
выдаётся следующей сессии.

## Отмена и очередь

Закрытие входящей сессии освобождает её lifecycle lease, но не удаляет
зафиксированное письмо. Уже committed delivery принадлежит durable queue, а
не TCP-соединению отправителя. Таймаут запускает request cancellation, однако
не является доказательством освобождения живого ресурса: lease возвращается
только после подтверждённого completion либо quarantine/close.

---

[Навигация по документации](README.md) · [Статус проекта](./STATUS.md) · [README](../../README.md)
