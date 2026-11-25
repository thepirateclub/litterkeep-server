# Runtime lifecycle

> **About this document.** Task lifecycle: ownership, waiting and wake-up, cancellation and cleanup, observability.
>
> Documentation navigation — [docs/README.md](README.md), project status — [STATUS.md](../STATUS.md).

## Ownership

`runtime::Orchestrator` holds generation-protected task identifiers, exclusive
resource leases and real wait registrations. The reactor owns network session
leases, the Store runs in a single writer thread, and the outbound transport has
a bounded worker and a separate connection pool. A resource value is never
copied on transfer and never appears in control diagnostics.

## Waiting and wake-up

A task first observes the condition and then registers a versioned wait. A
signal between the observation and the parking is preserved by the generation of
the event. A worker completion wakes the reactor socket, after which the reactor
re-checks the state. Duplicate wake-ups do not create an unbounded ready queue.

## Cancellation and cleanup

A timeout moves the task to `CancelRequested` but does not release a live lease.
The resource returns only after the owner has actually finished and cleanup has
run. A stale completion carrying an old generation does not act on a new issue
of the same slot or file descriptor. The `runtime cancel` control command uses
the same cooperative path.

## Observability

`runtime stats`, `tasks`, `resources`, `inspect` and `cancel` work through an
owner-only locator. They return IDs, states and counters, but never passwords,
keys, message bodies or resource values.

---

[Documentation navigation](README.md) · [Project status](../STATUS.md) · [README](../README.md)
