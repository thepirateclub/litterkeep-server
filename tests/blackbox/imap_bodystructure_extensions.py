"""Проверяет extension fields BODYSTRUCTURE через независимый IMAP-клиент."""

import argparse
import enum
import imaplib
import os
import ssl
import uuid


class ReplyStatus(enum.Enum):
    """Ответы верхнего уровня IMAP, используемые acceptance-клиентом."""

    OK = "OK"


def require_ok(response: tuple[str, list[object]], operation: str) -> list[object]:
    """Проверяет tagged status на границе стандартной библиотеки клиента."""
    status, data = response
    if ReplyStatus(status) is not ReplyStatus.OK:
        raise AssertionError(f"{operation} failed: {status!r} {data!r}")
    return data


def fetch_payload(records: list[object]) -> bytes:
    """Собирает текстовые FETCH metadata без предположения о позиции tuple."""
    values = [record for record in records if isinstance(record, bytes)]
    return b" ".join(values)


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=2146)
    parser.add_argument("--ca", default="data/pki/ca.pem")
    parser.add_argument("--password", default=os.environ.get("LITTERKEEP_BOB_PASSWORD"))
    options = parser.parse_args()
    if options.password is None:
        raise ValueError("provide --password or LITTERKEEP_BOB_PASSWORD")

    context = ssl.create_default_context(cafile=options.ca)
    context.check_hostname = False
    mailbox = f"BodyExtensions-{uuid.uuid4().hex}"
    raw = (
        b"From: alice@example.test\r\n"
        b"To: bob@example.test\r\n"
        b"Content-Type: multipart/mixed; boundary=meta-boundary\r\n"
        b"Content-Language: en-US, fr\r\n"
        b"Content-Location: https://example.test/mail/\r\n\r\n"
        b"--meta-boundary\r\n"
        b"Content-Type: application/octet-stream\r\n"
        b"Content-Disposition: attachment; filename*=UTF-8''r%C3%A9sum%C3%A9.bin\r\n"
        b"Content-Language: ru\r\n"
        b"Content-Location: ../files/a%20b.bin\r\n\r\n"
        b"attachment-octets\r\n--meta-boundary--\r\n"
    )
    with imaplib.IMAP4(options.host, options.port, timeout=10) as client:
        client.starttls(ssl_context=context)
        require_ok(client.login("bob", options.password), "LOGIN")
        require_ok(client.create(mailbox), "CREATE")
        require_ok(client.append(mailbox, None, None, raw), "APPEND")
        require_ok(client.select(mailbox, readonly=True), "SELECT")
        basic = fetch_payload(require_ok(client.fetch("1", "(BODY)"), "BODY FETCH"))
        extended = fetch_payload(
            require_ok(client.fetch("1", "(BODYSTRUCTURE)"), "BODYSTRUCTURE FETCH")
        )

    for forbidden in (b'"attachment"', b'"ru"', b'"../files/a%20b.bin"'):
        if forbidden in basic:
            raise AssertionError(f"basic BODY included extension {forbidden!r}: {basic!r}")
    for required in (
        b'("en-US" "fr")',
        b'"https://example.test/mail/"',
        '("attachment" ("FILENAME" "résumé.bin"))'.encode("utf-8"),
        b'"ru"',
        b'"../files/a%20b.bin"',
    ):
        if required not in extended:
            raise AssertionError(f"BODYSTRUCTURE lacks {required!r}: {extended!r}")
    print("PASS imap-bodystructure-extensions: BODY profile and MIME metadata")


if __name__ == "__main__":
    main()
