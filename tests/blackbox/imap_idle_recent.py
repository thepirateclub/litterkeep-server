"""Проверяет \\Recent для message, доставленного в уже selected IDLE mailbox."""

import argparse
import imaplib
import ssl


def options() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--port", type=int, required=True)
    parser.add_argument("--ca", required=True)
    parser.add_argument("--mailbox", default="IdleRecentFresh")
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


def main() -> None:
    args = options()
    context = ssl.create_default_context(cafile=args.ca)
    waiting = connect(args.port, context)
    producer = connect(args.port, context)
    require(waiting.create(args.mailbox))
    require(waiting.select(args.mailbox))
    require(producer.select(args.mailbox))
    message = b"From: qa@litterkeep.test\r\nSubject: idle recent\r\n\r\nbody"
    with waiting.idle(duration=5) as idler:
        require(producer.append(args.mailbox, None, None, message))
        kind, data = next(idler)
    if kind != "EXISTS":
        raise AssertionError(f"missing EXISTS event: {kind!r} {data!r}")
    if data != [b"1"]:
        raise AssertionError(f"unexpected EXISTS count: {data!r}")
    require(waiting.noop())
    status, values = waiting.uid("FETCH", "1", "(FLAGS)")
    if status != "OK":
        raise AssertionError(f"FETCH failed: {values!r}")
    if not any(b"\\Recent" in value for value in values if isinstance(value, bytes)):
        raise AssertionError(f"selected session lost recent: {values!r}")
    waiting.logout()
    producer.logout()
    print("IMAP IDLE RECENT passed: durable delivery into selected mailbox")


if __name__ == "__main__":
    main()
