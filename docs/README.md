# litterkeep-server documentation

Navigation over the project documentation. Every document answers a single
question and links to the others; you do not have to read them in order.

**The Russian version of the documentation — [ru/README.md](ru/README.md).**

## Where to start

| You want to | Read |
|---|---|
| Understand how ready the project is | [../STATUS.md](../STATUS.md) — readiness summary, what is done, what is planned |
| Run the server locally | [../README.md](../README.md) — start-up commands and verified profiles |
| Connect a mail client | [client-setup.md](client-setup.md) |
| Understand how the server is built | [architecture.md](architecture.md) → [runtime-lifecycle.md](runtime-lifecycle.md) → [storage-format.md](storage-format.md) |
| Work through a specific protocol | [protocol-coverage.md](protocol-coverage.md), then the specific profile: [imap-profile.md](imap-profile.md) or [sieve-coverage.md](sieve-coverage.md) |
| See what has actually been verified | [acceptance.md](acceptance.md) and [acceptance.tsv](acceptance.tsv) |
| Understand why it is built this way | [decisions.md](decisions.md) |
| Assess the risks | [security.md](security.md) |

## Documents

### About the project as a whole

| Document | Contents |
|---|---|
| [architecture.md](architecture.md) | Process and resource ownership, write durability, protocols and identity, platform limits |
| [decisions.md](decisions.md) | Decisions taken: execution and waiting, the unsafe boundary, resource ownership, fixing the storage and the queue, the IMAP profile, cancellation and the queue |
| [runtime-lifecycle.md](runtime-lifecycle.md) | Task lifecycle: ownership, waiting and wake-up, cancellation and cleanup, observability |
| [storage-format.md](storage-format.md) | Storage format: journals, blob publication, UIDs and crash recovery |
| [security.md](security.md) | Passwords, TLS, account canonicalisation, size limits and what never reaches the logs |
| [dependencies.md](dependencies.md) | Production dependencies, the allowlist and the project's own primitives instead of crates |
| [unicode-data.md](unicode-data.md) | Pinned Unicode data version, NFC, IDNA2008, SASLprep, data sources and licence |

### Protocols

| Document | Contents |
|---|---|
| [protocol-coverage.md](protocol-coverage.md) | TLS listener composition, SMTP, IMAP, POP3, Sieve and ManageSieve, outbound SMTP |
| [imap-profile.md](imap-profile.md) | Line-by-line matrix of IMAP4rev1 and the target IMAP4rev2, with the status of every command group and its verifiable boundary |
| [sieve-coverage.md](sieve-coverage.md) | Sieve and ManageSieve profile, RFC 5229 and RFC 5228 |
| [sieve-extension-audit.md](sieve-extension-audit.md) | Inventory of modern Sieve extensions: what is advertised, what is implemented internally, what has not been started |

### Acceptance and operation

| Document | Contents |
|---|---|
| [acceptance.md](acceptance.md) | Final acceptance per task: what was confirmed and how |
| [acceptance.tsv](acceptance.tsv) | Machine-readable acceptance matrix |
| [client-setup.md](client-setup.md) | Connecting a mail client, the verified profile and the actual result on a device |

### Development journal

| Document | Contents |
|---|---|
| [../GOAL.md](../GOAL.md) | Chronological journal of the development stages: what changed, which commands verified it, which limits were found. Kept in its original Russian, as a historical record |

## Conventions

The documents use one convention, borrowed from [imap-profile.md](imap-profile.md):

- **PASS** — there is a typed parser, a state transition and a profile test.
- **PARTIAL** — deliberately limited semantics. `CAPABILITY` does not present it
  as a broader extension, and the boundary is named explicitly.
- **NOT STARTED** — not implemented and not advertised.

The wording "verified" always refers to a concrete observation: an independent
wire client, a Linux Docker run or a profile test. It does not mean
compatibility with production clients, and it does not mean full RFC
conformance. Where that matters, the document names the exception directly.

## Editorial rules

1. A fact without a stated verification method is not a fact. Every claim about
   working functionality comes with a reference to the check.
2. Negative statements are worded explicitly ("not implemented", "not
   advertised") so they cannot be read as a default.
3. Protocol profiles never claim more than what is actually implemented: the
   advertised capability is the source of truth.
4. Large documents (`protocol-coverage.md`, `imap-profile.md`,
   `sieve-coverage.md`) carry a table of contents so they can be navigated
   without being read end to end.
5. The Russian version under `ru/` is maintained as a parallel copy. When the
   English version changes, the Russian version must change with it.
