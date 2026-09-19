# Architecture checkpoint: G00/G01

The server will be one process with one reactor and bounded helper pools. A
network session is an FSM record, never a thread. SMTP/IMAP/POP3/ManageSieve
decoders feed typed commands, then shared mail-core operations, then protocol
encoders. The storage layer does not produce protocol responses.

`runtime::Orchestrator` is the initial enforcement boundary. It records task
and resource identities separately from OS descriptor numbers. An exclusive
resource is moved into a non-cloneable lease. Only a matching owner can return
it. Cancellation requests task cleanup but cannot reissue a resource that its
old owner still holds. Event-version parking prevents the check/register race.

Linux `poll(2)` and wake socket integration are deferred to G01 completion;
the current portable core is deliberately unit-testable without an OS socket.
