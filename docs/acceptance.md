# Final acceptance

> **About this document.** Final acceptance per task: what was confirmed and how. The machine-readable matrix is in acceptance.tsv.
>
> Documentation navigation — [docs/README.md](README.md), project status — [STATUS.md](../STATUS.md).

## The matrix format

The full acceptance is in [`acceptance.tsv`](acceptance.tsv) — it is a TSV
without escaping, 439 data rows plus the column header:

| Column | Value |
|---|---|
| `id` | Task identifier, for example `T383` |
| `status` | Task result, see the list of values below |
| `layer` | Verification layer: RFC profile, wire acceptance, storage, lifecycle |
| `evidence` | What exactly confirms the result: commands, versions, the extent of the runs and the boundaries that remained unchecked |

The `status` values and their actual count in the current matrix:

| Status | Count | Value |
|---|---:|---|
| `PASS` | 349 | The check was performed and confirmed by an independent observation |
| `PARTIAL` | 79 | Only part of the boundary is confirmed, the rest is named explicitly in `evidence` |
| `SUPERSEDED` | 7 | An earlier checkpoint replaced by a later check |
| `BLOCKED` | 1 | The check could not be performed for an external reason |
| `FAIL` | 1 | The check was performed and did not pass |
| `GAP` | 1 | A recorded gap without an attempt to check |
| `NOT RUN` | 1 | Prepared but never executed |

The `evidence` column always contains both the confirmation and the explicitly
named limitations. A row with the `PARTIAL` status does not mean that the task
is not done: it means that the performed part is confirmed, and the unconfirmed
boundary is listed explicitly.

The sections below break the acceptance down by topic.

## RFC 3461 SMTP DSN negotiation

T405: after enabling the RFC 3461 wire capability independent Python raw-socket
clients check `250-DSN` in EHLO, the acceptance of `MAIL RET=HDRS ENVID=...` and
`RCPT NOTIFY=FAILURE ORCPT=...`, a completed DATA transaction, as well as the
refusal of the same parameters after HELO. The runs were
`tests/blackbox/smtp_dsn_negotiation.py` and
`tests/blackbox/smtp_dsn_refusal.py` against a Linux Rust 1.97.1 release server
in a separate Store. This proves the ESMTP negotiation and the envelope
acceptance, not the sending of the final DSN over the network, not a downstream
relay and not the interaction with the Litterkeep app. The earlier T349/T350
record the old refusal state before enabling the capability and do not describe
the current server.

T406: the own implementation now publishes a failure DSN to the external
original sender through a durable outbound job with an empty reverse-path. The
Linux-only test checks a permanent RCPT refusal, the preservation of the job
after the reopen, a second SMTP connection, the multipart/report and the absence
of the original body with RET=HDRS. The Windows full suite (1053 library + 1
CLI, 4 ignored), strict Clippy and rustfmt passed. The Linux test itself has
not been compiled yet: the available Rust container does not contain a working
`cc` linker. Therefore T406 remains PARTIAL and not a network PASS; the
Litterkeep app was not connected.

The machine-readable matrix is in `docs/acceptance.tsv`. The G12 status is not
inflated: 44 G12 checks have PASS, T23 has NOT RUN, T25 has BLOCKED. A separate
G17 T28 was added to the same matrix and does not change the G12 assessment.

The G17 vacation acceptance used a separate Linux Store
`data/g17-sieve-vacation-rfc45`. An independent client installed an active
vacation script through ManageSieve, transferred three original messages over
SMTP and checked after the restart through IMAP that all three remained in the
Bob INBOX. The Linux `queue-drain` transferred exactly one auto-reply to an
independent SMTP peer: the second replying address was suppressed by the
history, and the letter with a foreign To did not trigger a reply. The peer
checked `MAIL FROM:<>`, RCPT and Auto-Submitted/To/Subject. This is the
server-side protocol acceptance, not a check of the Litterkeep app and not
third-party MTA interoperability.

T82 checks the obsolete form of the SMTP source-route from RFC 5321 §4.1.2
separately for MAIL FROM and RCPT TO. An independent STARTTLS peer transferred
quoted local addresses with route elements, and an independent IMAP STARTTLS
peer confirmed both messages in the Bob INBOX. The check proves the local
delivery to the final mailbox; the external forwarding by the source-route is
not performed. This is not a check of the interaction with the Litterkeep app.

T83 checks IPv4 and the registered tagged IPv6 form in MAIL FROM and RCPT TO.
An independent STARTTLS client distinguishes a malformed literal (`501`) and a
syntactically correct but non-local address (`553` for the sender and `550` for
the recipient); this confirms the passage of the parser without weakening the
local routing or enabling the relay. In the same runs the source-routed
delivery and the reading of an ordinary Bob letter via IMAP STARTTLS remain
working. The scenario: `tests/blackbox/smtp_mailbox_syntax.py`.

T98 checks RFC 5321 §4.2.2: unknown SMTP commands get 500, an argument of a
recognized DATA gets 501; NOOP after the refusals stays synchronized in the
same connection. The repeatable scenario:
`tests/blackbox/smtp_unknown_command.py`. The check by an independent raw-TCP
client, not the Litterkeep application.

The G17 outbound SMTP reply parser accepts a bare reply code and an empty reply
text according to RFC 5321 §4.2. The unit coverage checks the line formats; an
independent TCP peer ran the whole delivery transaction with empty greeting,
EHLO, MAIL, RCPT, DATA, completion and QUIT texts. This is not a test with
Litterkeep or an external MTA.

T100 checks RFC 2045 §6.7: the literal SP/TAB before the terminating `=` soft
break are preserved as the content of the `qp-segment`, and the
transport-padding before the hard CRLF is removed. Independent SMTP STARTTLS ->
IMAP STARTTLS BINARY.PEEK and ManageSieve STARTTLS CHECKSCRIPT confirmed the
exact octets and the acceptance of a correct MIME form. These are the tests of
an independent client, not Litterkeep.

T292 checks the RFC 5322 obs-qp in the domain-literal: the unit test covers the
obs-NO-WS-CTL ranges, the quoted/unquoted NUL/CR/LF; an independent imaplib
client places a test message with a quoted NUL through APPEND and reads the
ENVELOPE. The response uses the IMAP synchronizing literal; its 11 octets
contain the decoded host without the semantically invisible quoting backslash.
The initial check mistakenly counted the backslash as part of the address and
was fixed. This is one control character and one ENVELOPE path without
comments. An additional parser regression checks an escaped closer in an
address with a trailing comment. This is not a complete interoperability proof
of RFC 5322/9051.

## Clean G12 run

On a new container-local `/data` bootstrap created four accounts. The repeat
did not change the passwords, the PHC or the CA. The seed gave
`created=35 existing=0`, the repeat and the repeat after a process restart gave
`created=0 existing=35`. An independent Python client confirmed 35 fixture IDs
and the SHA-256 of eight decoded attachments.

After the Android sending an independent STARTTLS IMAP client with the full
login Bob found exactly one letter `G12-Litterkeep-UI`. After stopping the
writer `check` reported 5 mailboxes, 37 links, 36 referenced blobs, 0 orphans
and 0 pending deliveries. The final backup copied 63 files and 14,381,601
bytes; the restored copy passed the checker. The previous clean backup also
confirmed the identical SHA-256 of the secret file and the CA.

## Evidence layers

- `server-protocol`: Rust tests and independent Python wire clients.
- `client-integration`: a physical TECNO CH6i and the installed litterkeep.
- `manual-device`: UI observation without clearing the existing app data/cache.

Litterkeep indeed checked the TLS/CA, connected Alice, obtained the folder
catalogue and sent a plain message to Bob. The attachment UI was absent, and
the Bob setup did not complete, therefore the receiving and downloading of the
attachment by the application is not claimed.

## Not executed

T23 was not run because a separate third-party MTA was not configured. T25 is
blocked by the current client UI/second account setup, and not by the server
Python test. The raw local measurements are saved under `data/g12-clean` and
are not included in Git, because they contain the local PKI and runtime
artifacts.

## RFC 3461/3464 update, T412-T413

The old T385/T404 rows in `acceptance.tsv` are historical checkpoints. The
current state differs: the SMTP DSN negotiation is independently verified in
T405, and the Store and the failure serializer store and output a separate
recipient group for each eligible original RCPT. T412 checks two such groups
by a unit regression and the full Windows/Linux Rust gate. An independent
network scenario with several report groups has not been run yet; this is not a
check of the Litterkeep application. In T413 independent TCP peers confirmed
the forwarding of the MAIL/RCPT DSN parameters for one source recipient and
the Deferred multi-origin up to MAIL/RCPT. The RFC 6533 UTF-8 ORCPT is encoded
as 7-bit-safe unitext. The reaction to a peer without DSN, the downstream
SUCCESS/DELAY reports and the multi-origin forwarding policy remain incomplete.

---

[Documentation navigation](README.md) · [Project status](../STATUS.md) · [README](../README.md)
