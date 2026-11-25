# Runtime lifecycle

> **О документе.** Жизненный цикл задач: владение, ожидание и пробуждение, отмена и очистка, наблюдаемость.
>
> Навигация по документации — [docs/ru/README.md](README.md), сводная готовность — [STATUS.md](./STATUS.md).

## Владение

`runtime::Orchestrator` хранит generation-protected идентификаторы задач,
exclusive leases ресурсов и реальные регистрации ожиданий. Reactor владеет
сетевыми session lease, Store работает в одном writer thread, outbound transport
имеет bounded worker и отдельный connection pool. Значение ресурса не копируется
при transfer и не попадает в control diagnostics.

## Ожидание и пробуждение

Задача сначала наблюдает условие, затем регистрирует versioned wait. Сигнал между
наблюдением и парковкой сохраняется поколением события. Worker completion будит
reactor socket, после чего reactor повторно проверяет состояние. Дубликаты wake
не создают неограниченную ready queue.

## Отмена и очистка

Timeout переводит задачу в `CancelRequested`, но не освобождает живой lease.
Ресурс возвращается только после фактического завершения владельца и cleanup.
Stale completion со старым generation не действует на новую выдачу того же slot
или FD. Control-команда `runtime cancel` использует тот же cooperative path.

## Наблюдаемость

`runtime stats`, `tasks`, `resources`, `inspect` и `cancel` работают через
owner-only locator. Они возвращают IDs, состояния и счётчики, но не пароли,
ключи, тела сообщений или значения ресурсов.

---

[Навигация по документации](README.md) · [Статус проекта](./STATUS.md) · [README](../../README.md)
