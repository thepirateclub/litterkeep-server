"""Проверяет выборочное durable удаление командой UID EXPUNGE по STARTTLS."""

import argparse
import imaplib
import ssl


def arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--port", type=int, required=True)
    parser.add_argument("--ca", required=True)
    parser.add_argument("--mailbox", default="UidExpungeFresh")
    parser.add_argument("--verify-existing", action="store_true")
    return parser.parse_args()


def require(result: tuple[str, object], expected: str) -> object:
    status, detail = result
    if status != expected:
        raise AssertionError(f"expected {expected}, got {status}: {detail!r}")
    return detail


def require_count(client: imaplib.IMAP4, mailbox: str, expected: bytes) -> None:
    count = require(client.select(mailbox), "OK")
    if count != [expected]:
        raise AssertionError(f"expected {expected!r} message, got {count!r}")


def require_surviving_uid(client: imaplib.IMAP4) -> None:
    records = require(client.uid("FETCH", "2", "RFC822"), "OK")
    if not isinstance(records, list):
        raise AssertionError(f"unexpected UID FETCH data: {records!r}")
    for record in records:
        if not isinstance(record, tuple):
            continue
        _, body = record
        if isinstance(body, bytes):
            if body == b"Subject: message 2\r\n\r\n2":
                return
    raise AssertionError(f"UID 2 message did not survive: {records!r}")


def main() -> None:
    options = arguments()
    context = ssl.create_default_context(cafile=options.ca)
    client = imaplib.IMAP4("127.0.0.1", options.port)
    client.starttls(ssl_context=context)
    require(client.login("bob", "bob-password"), "OK")

    if not options.verify_existing:
        require(client.create(options.mailbox), "OK")
        for uid in range(1, 4):
            message = f"Subject: message {uid}\r\n\r\n{uid}".encode()
            require(client.append(options.mailbox, None, None, message), "OK")
        require(client.select(options.mailbox), "OK")
        require(client.store("1,2", "+FLAGS", "(\\Deleted)"), "OK")
        require(client.uid("EXPUNGE", "1"), "OK")

    require_count(client, options.mailbox, b"2")
    require_surviving_uid(client)
    client.logout()
    phase = "replayed" if options.verify_existing else "committed"
    print(f"IMAP UID EXPUNGE passed: selective tombstone {phase}")


if __name__ == "__main__":
    main()
