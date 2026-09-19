"""Проверяет EXPUNGE notification и sequence-map двумя IMAP IDLE-сессиями."""
import argparse
import imaplib
import ssl


def options() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--port", type=int, required=True)
    parser.add_argument("--ca", required=True)
    parser.add_argument("--mailbox", default="IdleExpungeFresh")
    return parser.parse_args()


def require(result: tuple[str, object], expected: str) -> object:
    status, data = result
    if status != expected:
        raise AssertionError(f"expected {expected}, got {status}: {data!r}")
    return data


def connect(port: int, context: ssl.SSLContext) -> imaplib.IMAP4:
    client = imaplib.IMAP4("127.0.0.1", port)
    require(client.starttls(ssl_context=context), "OK")
    require(client.login("bob", "bob-password"), "OK")
    return client


def main() -> None:
    args = options()
    context = ssl.create_default_context(cafile=args.ca)
    waiting = connect(args.port, context)
    producer = connect(args.port, context)
    require(waiting.create(args.mailbox), "OK")
    for value in range(1, 4):
        require(waiting.append(args.mailbox, None, None, f"Subject: {value}\r\n\r\n{value}".encode()), "OK")
    require(waiting.select(args.mailbox), "OK")
    require(producer.select(args.mailbox), "OK")
    with waiting.idle(duration=5) as idler:
        require(producer.store("1,2", "+FLAGS", "(\\Deleted)"), "OK")
        require(producer.expunge(), "OK")
        observed = [next(idler), next(idler)]
    values = [data for kind, data in observed if kind == "EXPUNGE"]
    if values != [[b"1"], [b"1"]]:
        raise AssertionError(f"unexpected EXPUNGE events: {observed!r}")
    require(waiting.noop(), "OK")
    count = require(waiting.select(args.mailbox), "OK")
    if count != [b"1"]:
        raise AssertionError(f"expected synchronized mailbox, got {count!r}")
    waiting.logout()
    producer.logout()
    print("IMAP IDLE EXPUNGE passed: two-session sequence map")


if __name__ == "__main__":
    main()
