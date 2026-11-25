# Connecting litterkeep

> **About this document.** Connecting a mail client: the verified profile and the actual result on a device.
>
> Documentation navigation — [docs/README.md](README.md), project status — [STATUS.md](../STATUS.md).

## The verified profile

The server accepts a full local login and only turns it into an owner after the
typed local-domain policy check. For the Alice test account the following values
are used:

| Parameter | Value |
|---|---|
| Email/login | `alice@litterkeep.test` |
| IMAP | `localhost:1143`, STARTTLS |
| SMTP Submission | `localhost:1587`, STARTTLS |
| CA | `ca.pem` from the local test PKI |
| Password | the 40-character hex password from the `alice` line of `test-accounts.tsv` |

The `test-accounts.tsv` file has no header: every line is
`<account-name><TAB><password>`, one line per account. Passwords are generated
cryptographically at the first `init` and are never printed to stdout. On Linux
the file is created with mode `0600`.

The accounts created by `init` are `alice`, `bob`, `qa` and `pop`.

On a physical Android device over USB only the necessary redirections were
performed:

```powershell
adb reverse tcp:1143 tcp:51143
adb reverse tcp:1587 tcp:51587
```

The numbers on the right were Docker G12 host ports. When the binary runs
directly on the host they match the listener ports. The `10.0.2.2` address is
needed for the Android Emulator, but it was not used on the physical device in
G12.

## The actual result

On a TECNO CH6i the application first rejected the unknown CA. After selecting
`litterkeep-g12-ca.pem` through the standard document picker, the application
checked IMAP and SMTP, saved Alice, displayed the folder catalog and sent a
message to Bob. The UI confirmed that the server accepted the message and
accepted the recipient.

The full litterkeep attachment scenario is not confirmed. The current compose
screen had no attachment picker element. Setting up Bob on the second attempt
ended with a generic TLS connection error before the application offered to
choose a trust anchor. The server-side Python attachment tests are therefore
not presented as litterkeep UI evidence.

---

[Documentation navigation](README.md) · [Project status](../STATUS.md) · [README](../README.md)
