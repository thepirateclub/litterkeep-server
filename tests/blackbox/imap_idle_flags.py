"""Проверяет durable FLAGS notification для второй IDLE-сессии."""

import argparse
import imaplib
import ssl


def options() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--port", type=int, required=True)
    parser.add_argument("--ca", required=True)
    parser.add_argument("--mailbox", default="IdleFlagsFresh")
    return parser.parse_args()


def require(result: tuple[str, object]) -> object:
    status, data = result
    if status != "OK":
        raise AssertionError(f"command failed: {status}: {data!r}")
    return data


def connect(port: int, context: ssl.SSLContext) -> imaplib.IMAP4:
    client = imaplib.IMAP4("127.0.0.1", port)
    require(client.starttls(ssl_context=context))
    require(client.login("bob", "bob-password"))
    return client


def contains_seen(data: object) -> bool:
    values = data if isinstance(data, list) else []
    return any(
        b"UID 1 FLAGS (" in value and b"\\Seen" in value
        for value in values
        if isinstance(value, bytes)
    )


def main() -> None:
    args = options()
    context = ssl.create_default_context(cafile=args.ca)
    waiting = connect(args.port, context)
    writer = connect(args.port, context)
    require(waiting.create(args.mailbox))
    message = b"From: qa@litterkeep.test\r\nSubject: flags\r\n\r\nbody"
    require(waiting.append(args.mailbox, None, None, message))
    require(waiting.select(args.mailbox))
    require(writer.select(args.mailbox))
    with waiting.idle(duration=5) as idler:
        require(writer.store("1", "+FLAGS.SILENT", "(\\Seen)"))
        kind, data = next(idler)
    if kind != "FETCH":
        raise AssertionError(f"expected FETCH event, got {kind!r}: {data!r}")
    if not contains_seen(data):
        raise AssertionError(f"missing durable flags event: {data!r}")
    require(waiting.noop())
    status, values = waiting.uid("FETCH", "1", "(FLAGS)")
    if status != "OK":
        raise AssertionError(f"FETCH failed: {values!r}")
    if not contains_seen(values):
        raise AssertionError(f"flags did not persist: {values!r}")
    waiting.logout()
    writer.logout()
    print("IMAP IDLE FLAGS passed: two-session durable FETCH notification")


if __name__ == "__main__":
    main()
