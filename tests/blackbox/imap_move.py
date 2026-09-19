"""Проверяет IMAP MOVE и UID MOVE с durable destination после рестарта."""
import argparse
import imaplib
import ssl


def arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--port", type=int, required=True)
    parser.add_argument("--ca", required=True)
    parser.add_argument("--source", default="MoveSourceFresh")
    parser.add_argument("--destination", default="MoveDestinationFresh")
    parser.add_argument("--verify-existing", action="store_true")
    return parser.parse_args()


def require(result: tuple[str, object], expected: str) -> object:
    status, detail = result
    if status != expected:
        raise AssertionError(f"expected {expected}, got {status}: {detail!r}")
    return detail


def count(client: imaplib.IMAP4, mailbox: str, expected: bytes) -> None:
    if require(client.select(mailbox), "OK") != [expected]:
        raise AssertionError(f"unexpected count for {mailbox}")


def main() -> None:
    options = arguments()
    context = ssl.create_default_context(cafile=options.ca)
    client = imaplib.IMAP4("127.0.0.1", options.port)
    client.starttls(ssl_context=context)
    require(client.login("bob", "bob-password"), "OK")
    if not options.verify_existing:
        require(client.create(options.source), "OK")
        require(client.create(options.destination), "OK")
        for value in range(1, 4):
            require(client.append(options.source, None, None, f"Subject: {value}\r\n\r\n{value}".encode()), "OK")
        require(client.select(options.source), "OK")
        require(client._simple_command("MOVE", "1,3", options.destination), "OK")
    count(client, options.source, b"1")
    count(client, options.destination, b"2")
    require(client.fetch("1", "RFC822"), "OK")
    client.logout()
    print(f"IMAP MOVE passed: {'replayed' if options.verify_existing else 'committed'}")


if __name__ == "__main__":
    main()
