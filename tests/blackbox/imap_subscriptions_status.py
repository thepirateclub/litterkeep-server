"""Проверяет durable subscriptions, LSUB и snapshot-based STATUS."""

import argparse
import imaplib
import ssl


def arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--port", type=int, required=True)
    parser.add_argument("--ca", required=True)
    parser.add_argument("--verify-existing", action="store_true")
    return parser.parse_args()


def require(result: tuple[str, object], expected: str) -> object:
    status, detail = result
    if status != expected:
        raise AssertionError(f"expected {expected}, got {status}: {detail!r}")
    return detail


def joined(values: object) -> bytes:
    if not isinstance(values, list):
        raise AssertionError(f"unexpected IMAP response: {values!r}")
    return b"\n".join(value for value in values if isinstance(value, bytes))


def main() -> None:
    options = arguments()
    context = ssl.create_default_context(cafile=options.ca)
    client = imaplib.IMAP4("127.0.0.1", options.port)
    client.starttls(ssl_context=context)
    client.login("bob", "bob-password")

    if not options.verify_existing:
        require(client.create("Metrics"), "OK")
        require(client.append("Metrics", "(\\Seen)", None, b"Subject: seen\r\n\r\na"), "OK")
        require(client.append("Metrics", None, None, b"Subject: unseen\r\n\r\nb"), "OK")
        require(client.subscribe("Metrics"), "OK")
        require(client.subscribe("Ghost"), "OK")
        lsub = joined(require(client.lsub("", "*"), "OK"))
        if b'"Metrics"' not in lsub or b'\\Noselect' not in lsub or b'"Ghost"' not in lsub:
            raise AssertionError(f"LSUB projection is wrong: {lsub!r}")
        require(client.unsubscribe("Ghost"), "OK")

    lsub = joined(require(client.lsub("", "*"), "OK"))
    if b'"Metrics"' not in lsub or b'"Ghost"' in lsub:
        raise AssertionError(f"durable subscriptions are wrong: {lsub!r}")
    status = joined(
        require(client.status("Metrics", "(MESSAGES UIDNEXT UIDVALIDITY UNSEEN)"), "OK")
    )
    for marker in (b"MESSAGES 2", b"UIDNEXT 3", b"UIDVALIDITY 1", b"UNSEEN 1"):
        if marker not in status:
            raise AssertionError(f"STATUS lacks {marker!r}: {status!r}")
    client.logout()
    phase = "replayed" if options.verify_existing else "committed"
    print(f"IMAP subscriptions/status passed: {phase}")


if __name__ == "__main__":
    main()
