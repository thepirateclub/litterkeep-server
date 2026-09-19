"""Проверяет CHECK и durable CLOSE через независимый стандартный IMAP-клиент."""

import argparse
import imaplib
import ssl


def arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--port", type=int, required=True)
    parser.add_argument("--ca", required=True)
    parser.add_argument("--mailbox", default="CloseFresh")
    return parser.parse_args()


def require(result: tuple[str, object], expected: str) -> object:
    status, detail = result
    if status != expected:
        raise AssertionError(f"expected {expected}, got {status}: {detail!r}")
    return detail


def main() -> None:
    options = arguments()
    context = ssl.create_default_context(cafile=options.ca)
    client = imaplib.IMAP4("127.0.0.1", options.port)
    client.starttls(ssl_context=context)
    require(client.login("bob", "bob-password"), "OK")
    require(client.create(options.mailbox), "OK")
    require(client.append(options.mailbox, None, None, b"Subject: close\r\n\r\nbody"), "OK")
    require(client.select(options.mailbox), "OK")
    require(client._simple_command("CHECK"), "OK")
    require(client.store("1", "+FLAGS", "(\\Deleted)"), "OK")
    require(client.close(), "OK")
    count = require(client.select(options.mailbox), "OK")
    if count != [b"0"]:
        raise AssertionError(f"CLOSE left deleted message visible: {count!r}")
    client.logout()
    print("IMAP CHECK/CLOSE passed: durable close tombstone committed")


if __name__ == "__main__":
    main()
