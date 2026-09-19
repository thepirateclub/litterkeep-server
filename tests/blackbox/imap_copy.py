"""Проверяет COPY и UID COPY с durable COPYUID через независимый IMAP-клиент."""

import argparse
import imaplib
import ssl


def arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--port", type=int, required=True)
    parser.add_argument("--ca", required=True)
    parser.add_argument("--source", default="CopySourceFresh")
    parser.add_argument("--destination", default="CopyDestinationFresh")
    parser.add_argument("--uid-destination", default="CopyUidFresh")
    parser.add_argument("--verify-existing", action="store_true")
    return parser.parse_args()


def require(result: tuple[str, object], expected: str) -> object:
    status, detail = result
    if status != expected:
        raise AssertionError(f"expected {expected}, got {status}: {detail!r}")
    return detail


def require_copyuid(detail: object) -> None:
    if not isinstance(detail, list):
        raise AssertionError(f"unexpected COPY result: {detail!r}")
    for value in detail:
        if isinstance(value, bytes):
            if b"COPYUID" in value:
                return
    raise AssertionError(f"COPYUID missing: {detail!r}")


def require_messages(client: imaplib.IMAP4, mailbox: str, expected: bytes) -> None:
    count = require(client.select(mailbox), "OK")
    if count != [expected]:
        raise AssertionError(f"expected {expected!r} messages, got {count!r}")


def require_seen_copy(client: imaplib.IMAP4) -> None:
    records = require(client.fetch("2", "FLAGS"), "OK")
    if not isinstance(records, list):
        raise AssertionError(f"unexpected FLAGS data: {records!r}")
    for record in records:
        if isinstance(record, bytes):
            if b"\\Seen" in record:
                return
    raise AssertionError(f"copied Seen flag is absent: {records!r}")


def main() -> None:
    options = arguments()
    context = ssl.create_default_context(cafile=options.ca)
    client = imaplib.IMAP4("127.0.0.1", options.port)
    client.starttls(ssl_context=context)
    require(client.login("bob", "bob-password"), "OK")

    if not options.verify_existing:
        require(client.create(options.source), "OK")
        require(client.create(options.destination), "OK")
        require(client.create(options.uid_destination), "OK")
        require(client.append(options.source, None, None, b"Subject: first\r\n\r\n1"), "OK")
        require(
            client.append(options.source, "(\\Seen)", None, b"Subject: second\r\n\r\n2"),
            "OK",
        )
        require(client.select(options.source), "OK")
        require_copyuid(require(client.copy("2:1", options.destination), "OK"))
        require_copyuid(
            require(
                client._simple_command("UID", "COPY", "2", options.uid_destination),
                "OK",
            )
        )

    require_messages(client, options.source, b"2")
    require_messages(client, options.destination, b"2")
    require_seen_copy(client)
    require_messages(client, options.uid_destination, b"1")
    client.logout()
    phase = "replayed" if options.verify_existing else "committed"
    print(f"IMAP COPY passed: durable shared blobs {phase}")


if __name__ == "__main__":
    main()
