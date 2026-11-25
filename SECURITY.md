# Security policy

## Scope

`litterkeep-server` is a self-hosted mail server that implements its own SMTP,
IMAP, POP3 and ManageSieve protocol handling, its own MIME/Sieve parsing and
its own filesystem store. It is a research-grade implementation with a
documented verification matrix, not a hardened internet-facing product.

The project does not claim full RFC conformance and does not claim
interoperability with any real-world mail client or server beyond the
black-box checks recorded in `docs/acceptance.tsv`.

## Reporting a vulnerability

Report suspected vulnerabilities privately through GitHub Security Advisories
(the "Report a vulnerability" button on the Security tab of the repository).
Please include:

1. the affected component (SMTP, IMAP, POP3, ManageSieve, MIME, Sieve, store);
2. the exact input or command sequence that triggers the problem;
3. the observed effect and the expected one.

Do not open a public issue for a vulnerability before a fix exists.

## Known exposure

The following are known and accepted properties of the current checkpoint, not
undisclosed bugs:

- The server speaks STARTTLS and implicit TLS, but it does not enforce
  certificate trust for incoming sessions by default.
- `init` creates local acceptance accounts with generated passwords in
  `test-accounts.tsv`. This is a test bootstrap, not account provisioning.
- The filesystem store keeps message blobs and journals on disk with a
  `litterkeep-store-v2` marker. There is no automatic migration from earlier
  development formats.
- The outbound scheduler accepts only an explicit numeric `IP:port` route.

## Deployment guidance

Do not expose this server to the public internet. Bind it to loopback or to a
private network, and put a reverse proxy with TLS termination and real
anti-abuse policy in front of it before any external exposure.
