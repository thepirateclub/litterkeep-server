# Architectural decisions

> **About this document.** Why the server is built the way it is: the decisions taken, their rationale and their cost.
>
> Documentation navigation — [docs/README.md](README.md), project status — [STATUS.md](../STATUS.md).

## Execution and waiting

The server uses a single Linux `poll(2)` reactor for listeners and sessions. A
protocol session is a resource of the `Orchestrator`: `ResourceLease<Session>`
holds the real socket and state, while the registry stores only verifiable
metadata. A storage completion wakes the poll loop through a Unix socket, not
through a separate thread per waiting session. The checks
`exclusive_resource_cannot_be_double_issued` and
`signal_between_observation_and_park_is_not_lost` confirm the exclusive lease
and the absence of a lost wake between observation and parking.

## The unsafe boundary

Production unsafe boundaries are limited to two Linux FFI modules, each of which
lifts the unsafe ban for itself only:

- `src/platform/linux_poll.rs` — the `poll(2)` call. Before the call a live
  contiguous `repr(C)` array of descriptors is created, and the function borrows
  them only for the duration of the syscall.
- `src/platform/linux_lock.rs` — the `flock(2)` call for the advisory writer
  lock. The descriptor belongs to a live `File`, and `flock` takes no ownership
  of it.

The rest of the crate forbids unsafe through `#![deny(unsafe_code)]` in
`src/lib.rs`, and both modules are marked `#![allow(unsafe_code)]` locally.

## Resource ownership

The Store is opened by a single `StorageWorker`; a protocol session hands it a
bounded request and never receives a `Store` lease. The outgoing SMTP worker
owns one bounded `ConnectionPool` in a single thread. Its completion channel is
the source of truth, while the Linux wake socket carries only a coalesced hint.
A full wake socket therefore never loses a completion.

## Fixing the storage and the queue

A blob, mailbox links and an owned outbound job are published by one durable
Store transaction. The queue sees the job only after the global commit ID.
Before an outgoing SMTP attempt the job must receive a durable `InFlight`
transition; recovery moves an unfinished `InFlight` into `Deferred`. The retry
deadline is stored as a Unix epoch, separately from the monotonic deadlines of
the reactor.

## The IMAP profile

IMAP uses a typed parser and a state machine. `\Recent` is stored as a
session-local projection: a durable delivery records a candidate UID, and
SELECT atomically claims it for one session. The marker therefore never turns
into a persistent message flag. All already selected sessions receive the marker
on a live notification; this is a permissible implementation-defined side of the
RFC 3501 ambiguity for multiple open connections. The durable candidate stays
until the next future SELECT, survives a restart before the claim, and after the
claim is no longer issued to the next session.

## Cancellation and the queue

Closing an incoming session releases its lifecycle lease but does not delete the
committed message. A committed delivery belongs to the durable queue, not to the
sender's TCP connection. A timeout starts request cancellation, but it is not
proof that a live resource has been released: the lease returns only after a
confirmed completion or a quarantine/close.

---

[Documentation navigation](README.md) · [Project status](../STATUS.md) · [README](../README.md)
