# Storage format

> **About this document.** Filesystem storage format: journals, blob publication, UIDs and crash recovery.
>
> Documentation navigation — [docs/README.md](README.md), project status — [STATUS.md](../STATUS.md).

The root directory is marked `litterkeep-store-v2`. An unknown or unmarked
directory containing mail data is rejected; there is no automatic guessing of an
older format.

An immutable message blob is published through a temporary file, `fsync`, an
atomic rename and a directory sync. Mailbox links, flags, folders, subscriptions,
Recent, Sieve, diagnostics, the queue and global transaction commits use framed
append-only journals. A fully invalid frame counts as corruption; an incomplete
tail is safely truncated. A mailbox link becomes visible only after the global
commit.

A UID is monotonic within a mailbox and is never reused after EXPUNGE. COPY
shares an immutable blob; MOVE publishes the destination and the source tombstone
in one transaction. A pending outbound delivery stays the owner of the blob until
the terminal transition.

`check` takes the exclusive Store lock, performs recovery and reads every
referenced blob. Orphans are only counted. `backup` requires a stopped writer,
excludes the runtime lock and locator, syncs the copy and runs the checker.

---

[Documentation navigation](README.md) · [Project status](../STATUS.md) · [README](../README.md)
