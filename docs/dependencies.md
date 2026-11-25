# Dependencies

> **About this document.** Production dependencies, the allowlist of the technical specification and the project's own primitives instead of third-party crates.
>
> Documentation navigation — [docs/README.md](README.md), project status — [STATUS.md](../STATUS.md).

## Production dependencies

The server binary has exactly two direct production dependencies from the
allowlist of the technical specification. The mail protocols, MIME, Sieve, the
file Store, the queue, the reactor and the lifecycle orchestration are
implemented in this repository.

| Dependency | Version | Features | Purpose |
|---|---:|---|---|
| `rustls` | 0.23.45 | `std`, `ring`, `tls12` | TLS server/client state and the cryptographic verification of certificates. `ring` is the only chosen backend. |
| `argon2` | 0.6.0 | `alloc`, `password-hash`, `getrandom` | Argon2id PHC password records and cryptographically secure salts. |

`rcgen 0.14.10` is only a dev-dependency. It is available to
`examples/make_test_pki.rs` for the one-time issuance of a test CA and does not
end up in the ordinary runtime server binary.

## The project's own primitives

### SCRAM-SHA-1: hash, HMAC and PBKDF2

RFC 5804 §2.1 requires SCRAM-SHA-1. The allowlist of production dependencies
still permits only `rustls` and `argon2`; the hash/HMAC API of the pinned
rustls backend is not the public API of the application, and the direct use of a
transitive crate is inadmissible. In `src/sasl/sha1.rs`,
`src/sasl/hmac_sha1.rs` and `src/sasl/pbkdf2_sha1.rs` the own safe-Rust
primitives are added without a new dependency. Their narrow
checks are compared with the known answers of RFC 3174, RFC 2202 and RFC 6070.
SHA-1 is applied only as a legacy SCRAM compatibility, not for signatures, new
password storage or protection against collision attacks.

### SCRAM-SHA-1 verifier and sidecar record

The type `ScramSha1Verifier` binds PBKDF2, ClientKey/StoredKey and ServerKey,
verifies a non-empty salt of up to 64 octets and limits the iteration count to
the range 4096..=200000. An end-to-end test reproduces the client proof and the
server signature from the complete RFC 5802 §5.1 example. The RFC 4013
SASLprep is already implemented; the new Argon2 credentials keep a normalized
`.phc2`, and a successful login with a legacy `.phc` publishes a `.phc2`
keeping the old file. The SMTP TLS black-box acceptance separately confirmed
the wire authentication and the precedence of the new record.

The creation of a new account now writes a separate versioned
`litterkeep-scram-sha1-v1` sidecar with a cryptographically random salt, a
limited iteration count, the StoredKey and the ServerKey. The sidecar is
created after the durable publication of the Argon2 credential; on an error the
creation of the Argon2 file is rolled back. A strict parser rejects unknown
versions, extra fields, an incorrect key length and an excessive cost. The
reload test checks the proof through the restored record. The already existing
Argon2 accounts do not have a SCRAM sidecar yet, but a successful verification
of `.phc2` now safely creates it during the backfill; the legacy `.phc`
migration publishes the sidecar before the new record. An unsuccessful login
does not change the files. T327 added a bounded-worker path for reading the
sidecar by the single Accounts owner; the full exchange is wired in T328.
### SCRAM protocol: attributes, client and server

For the preparation of the protocol layer the module `scram_attributes` parses
only the restricted syntax of unique single-character attributes, rejects the
unsupported mandatory extension `m=` and limits the message to 16 KiB and 26
attributes. T323 fixed the excessive ASCII-only restriction of T322: according
to RFC 5802 §7 the ordinary `value-char` include UTF-8 and the
whitespace/control octets, except NUL and the comma separator; the separate
nonce profile is restricted to printable ASCII without a comma.
`scram_client_first` supports only the GS2 `n,,`, the username
SASLprep/escaping and keeps the exact transcript; the optional extensions are
ignored as the RFC requires.
`scram_server_first` forms `r,s,i`, adding 18 cryptographically random
Rustls octets to the client nonce; the salt and the iteration count are taken
from the stored verifier record; it is wired into the SASL lifecycle.
`scram_client_final` checks the fixed `c,r,[extensions],p`, the exact
GS2 `biws`, the full nonce and the canonical 20-octet ClientProof. It forms
the AuthMessage from the original SCRAM transcript lines; the complete
RFC 5802 §5.1 known-answer passes through the StoredKey verifier and matches
the published ServerSignature.
`scram_server_final` completes the same crypto transcript with a typed
`v=<signature>` or `e=invalid-proof`; all RFC 5802 §7 server-error values
are covered by an enum. The ManageSieve exchange binds the final data to the
protocol-level success, and an independent TLS peer checks the end-to-end
transcript, including the server signature and the rejection of an incorrect
proof.
[`RFC 5804 §2.1`](https://www.rfc-editor.org/rfc/rfc5804.html#section-2.1)
directly requires SCRAM-SHA-1 together with PLAIN over TLS; both mechanisms are
advertised only after TLS. An independent TLS peer confirmed the complete
ManageSieve exchange, but STARTTLS and the Litterkeep application were not
checked. The syntactic basis was compared against
[RFC 5802 §7](https://www.rfc-editor.org/rfc/rfc5802.html#section-7).
[RFC 7677](https://www.rfc-editor.org/rfc/rfc7677.html) registers SCRAM-SHA-256,
the stronger algorithm of the family, and notes the security requirements
related to SHA-1 and TLS channel binding. SCRAM-SHA-256 is now implemented for
ManageSieve TLS; PLUS/channel binding remain a separate gap.

### SCRAM-SHA-256 and channel binding

For SCRAM-SHA-256 the own safe-Rust primitives `sha256`, `hmac_sha256`, the
single-block `pbkdf2_sha256`, `ScramSha256Verifier` and a separate versioned
`ScramSha256Record` were added. They do not add direct production dependencies.
SHA-256 is verified against RFC 6234, HMAC against RFC 4231, PBKDF2 against
RFC 7914 §11, and the full client proof/server signature against the example of
RFC 7677 §3; the iteration count is limited to the value 200000. The accounts
store the SHA-256 verifier in a separate sidecar and provide it through a typed
request to the single Accounts worker; the request does not print the secret in
Debug. POP3 and ManageSieve use a separate SHA-256 verifier lookup through the
bounded Accounts owner. POP3 accepts SCRAM-SHA-256 only inside TLS and releases
the maildrop after the mutual proof. The ManageSieve state machine accepts the
initial-response and the continuation, and the capability advertises the
mechanism only after TLS. Independent Linux peers confirmed the mutual proof and
the server signature over implicit TLS and STARTTLS; a separate Python peer
also completed POP3 SCRAM-SHA-256 over STARTTLS and implicit TLS with an
independent proof. The TLS 1.3 tls-exporter is verified between rustls
endpoints and wired into ManageSieve SCRAM-SHA-256-PLUS with an exact GS2/c=
binding check and live wire acceptance. The Litterkeep app interoperability for
these mechanisms has not been checked yet.

## The project's own Unicode tables

The IMAP mailbox validation uses the own generated ranges of Unicode 17.0.0,
not a crate. The official source, the toolchain version, the SHA-256 and the
licence are fixed in [`unicode-data.md`](unicode-data.md); the compile-time
assertion compares the table with `char::UNICODE_VERSION`.

## Verifiable dependency graph

The verifiable dependency graph is built from the pinned `Cargo.lock`:

```powershell
cargo tree --locked --edges normal
cargo tree --locked --edges build
cargo tree --locked --edges dev
cargo metadata --locked --format-version 1
```

The transitive crypto, certificate and build dependencies are visible in these
commands. They do not imply the inclusion of a ready-made MTA, a database, a
runtime, an actor framework or a mail parser. A new direct production
dependency cannot be added without updating this file, `Cargo.toml`, the
lockfile and the allowlist check.

---

[Documentation navigation](README.md) · [Project status](../STATUS.md) · [README](../README.md)
