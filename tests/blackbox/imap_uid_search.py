"""Проверяет SEARCH и UID SEARCH без смешения sequence number и UID."""

import argparse
import imaplib
import ssl


def arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--port", type=int, required=True)
    parser.add_argument("--ca", required=True)
    parser.add_argument("--mailbox", default="SearchFresh")
    return parser.parse_args()


def require(result: tuple[str, object], expected: str) -> object:
    status, detail = result
    if status != expected:
        raise AssertionError(f"expected {expected}, got {status}: {detail!r}")
    return detail


def require_numbers(value: object, expected: list[bytes]) -> None:
    if value != expected:
        raise AssertionError(f"expected {expected!r}, got {value!r}")


def main() -> None:
    options = arguments()
    context = ssl.create_default_context(cafile=options.ca)
    client = imaplib.IMAP4("127.0.0.1", options.port)
    client.starttls(ssl_context=context)
    require(client.login("bob", "bob-password"), "OK")
    require(client.create(options.mailbox), "OK")
    require(client.append(options.mailbox, "(\\Seen)", None, b"Subject: seen\r\n\r\n1"), "OK")
    require(client.append(options.mailbox, None, None, b"Subject: unseen\r\n\r\n2"), "OK")
    require(client.append(options.mailbox, "(\\Deleted)", None, b"Subject: deleted\r\n\r\n3"), "OK")
    require(client.select(options.mailbox), "OK")
    require_numbers(require(client.search(None, "UNSEEN"), "OK"), [b"2 3"])
    require_numbers(require(client.uid("SEARCH", None, "SEEN"), "OK"), [b"1"])
    require_numbers(require(client.uid("SEARCH", None, "DELETED"), "OK"), [b"3"])
    client.logout()
    print("IMAP UID SEARCH passed: sequence and UID spaces are distinct")


if __name__ == "__main__":
    main()
