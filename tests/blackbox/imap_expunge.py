"""Проверяет STORE Deleted, EXPUNGE sequence-map и replay tombstone journal."""

import argparse
import imaplib
import ssl


def arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--port", type=int, required=True)
    parser.add_argument("--ca", required=True)
    parser.add_argument("--mailbox", default="Expunge")
    parser.add_argument("--verify-existing", action="store_true")
    return parser.parse_args()


def require(result: tuple[str, object], expected: str) -> object:
    status, detail = result
    if status != expected:
        raise AssertionError(f"expected {expected}, got {status}: {detail!r}")
    return detail


def literal(records: object) -> bytes:
    if not isinstance(records, list):
        raise AssertionError(f"unexpected FETCH data: {records!r}")
    for record in records:
        if isinstance(record, tuple):
            _, payload = record
            if isinstance(payload, bytes):
                return payload
    raise AssertionError(f"literal missing: {records!r}")


def main() -> None:
    options = arguments()
    context = ssl.create_default_context(cafile=options.ca)
    client = imaplib.IMAP4("127.0.0.1", options.port)
    client.starttls(ssl_context=context)
    client.login("bob", "bob-password")
    mailbox = options.mailbox

    if not options.verify_existing:
        require(client.create(mailbox), "OK")
        for number in range(1, 4):
            message = f"Subject: message {number}\r\n\r\n{number}".encode()
            require(client.append(mailbox, None, None, message), "OK")

    require(client.select(mailbox, readonly=True), "OK")
    require(client.expunge(), "NO")

    if not options.verify_existing:
        require(client.select(mailbox), "OK")
        require(client.store("1,3", "+FLAGS", "(\\Deleted)"), "OK")
        sequence_numbers = require(client.expunge(), "OK")
        if sequence_numbers != [b"1", b"2"]:
            raise AssertionError(f"EXPUNGE sequence-map changed: {sequence_numbers!r}")

    count = require(client.select(mailbox), "OK")
    if count != [b"1"]:
        raise AssertionError(f"wrong remaining count: {count!r}")
    remaining = literal(require(client.fetch("1", "RFC822"), "OK"))
    if remaining != b"Subject: message 2\r\n\r\n2":
        raise AssertionError(f"wrong surviving message: {remaining!r}")
    client.logout()
    phase = "replayed" if options.verify_existing else "committed"
    print(f"IMAP EXPUNGE passed: durable tombstone {phase}")


if __name__ == "__main__":
    main()
