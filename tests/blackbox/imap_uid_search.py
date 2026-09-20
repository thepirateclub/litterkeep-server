"""Проверяет SEARCH и UID SEARCH без смешения sequence number и UID."""

import argparse
import imaplib
import ssl


def arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--port", type=int, required=True)
    parser.add_argument("--ca", required=True)
    parser.add_argument("--mailbox", default="SearchFresh")
    parser.add_argument("--verify-existing", action="store_true")
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
    if not options.verify_existing:
        require(client.create(options.mailbox), "OK")
        require(
            client.append(
            options.mailbox,
            "(\\Seen \\Answered \\Flagged)",
            '"01-Feb-2026 12:00:00 +0000"',
            b"From: alice@example.test\r\nDate: Mon, 2 Feb 2026 12:00:00 +0000\r\nSubject: seen report\r\n\r\nfirst token",
            ),
            "OK",
        )
        require(
            client.append(
            options.mailbox,
            None,
            '"02-Feb-2026 12:00:00 +0000"',
            b"From: bob@example.test\r\nDate: Tue, 3 Feb 2026 12:00:00 +0000\r\nSubject: unseen\r\n\r\nsecond token",
            ),
            "OK",
        )
        require(
            client.append(
            options.mailbox,
            "(\\Deleted \\Draft)",
            '"03-Feb-2026 12:00:00 +0000"',
            b"Subject: deleted draft\r\n\r\nthird",
            ),
            "OK",
        )
    require(client.select(options.mailbox), "OK")
    require_numbers(require(client.search(None, "UNSEEN"), "OK"), [b"2 3"])
    require_numbers(require(client.uid("SEARCH", None, "SEEN"), "OK"), [b"1"])
    require_numbers(require(client.uid("SEARCH", None, "DELETED"), "OK"), [b"3"])
    require_numbers(require(client.search(None, "ANSWERED"), "OK"), [b"1"])
    require_numbers(require(client.search(None, "UNANSWERED"), "OK"), [b"2 3"])
    require_numbers(require(client.search(None, "FLAGGED"), "OK"), [b"1"])
    require_numbers(require(client.search(None, "DRAFT"), "OK"), [b"3"])
    recent = [b"1 2 3"]
    new = [b"2 3"]
    old = [b""]
    if options.verify_existing:
        recent = [b""]
        new = [b""]
        old = [b"1 2 3"]
    require_numbers(require(client.search(None, "RECENT"), "OK"), recent)
    require_numbers(require(client.search(None, "NEW"), "OK"), new)
    require_numbers(require(client.search(None, "OLD"), "OK"), old)
    require_numbers(require(client.search(None, "2:3"), "OK"), [b"2 3"])
    require_numbers(require(client.uid("SEARCH", None, "UID", "2:3"), "OK"), [b"2 3"])
    require_numbers(require(client.search(None, "OR", "ANSWERED", "DRAFT"), "OK"), [b"1 3"])
    require_numbers(require(client.search(None, "NOT", "SEEN"), "OK"), [b"2 3"])
    require_numbers(require(client.search(None, "TEXT", "TOKEN"), "OK"), [b"1 2"])
    require_numbers(require(client.search(None, "SINCE", "2-Feb-2026"), "OK"), [b"2 3"])
    require_numbers(require(client.search(None, "SENTON", "2-Feb-2026"), "OK"), [b"1"])
    require_numbers(require(client.search(None, "SENTSINCE", "3-Feb-2026"), "OK"), [b"2"])
    observer = imaplib.IMAP4("127.0.0.1", options.port)
    observer.starttls(ssl_context=context)
    require(observer.login("bob", "bob-password"), "OK")
    require(observer.select(options.mailbox), "OK")
    require_numbers(require(observer.search(None, "OR", "ANSWERED", "DRAFT"), "OK"), [b"1 3"])
    observer.logout()
    client.logout()
    phase = "recovered" if options.verify_existing else "created"
    print(f"IMAP SEARCH matrix passed: {phase}, concurrent session checked")


if __name__ == "__main__":
    main()
