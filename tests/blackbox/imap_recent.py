"""Проверяет durable назначение IMAP \\Recent одной SELECT-сессии."""

import argparse
import imaplib
import ssl


def arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--port", type=int, required=True)
    parser.add_argument("--ca", required=True)
    parser.add_argument("--verify-restart", action="store_true")
    return parser.parse_args()


def connect(port: int, context: ssl.SSLContext) -> imaplib.IMAP4:
    client = imaplib.IMAP4("127.0.0.1", port)
    client.starttls(ssl_context=context)
    client.login("bob", "bob-password")
    return client


def contains(data: list[object], expected: bytes) -> bool:
    return any(expected in value for value in data if isinstance(value, bytes))


def status_recent(client: imaplib.IMAP4) -> tuple[str, list[object]]:
    tag = client._new_tag()
    client.send(tag + b" STATUS INBOX (RECENT)\r\n")
    status, _ = client._get_tagged_response(tag)
    return status, client.untagged_responses.pop("STATUS", [])


def require_ok(status: str, values: list[object]) -> None:
    if status != "OK":
        raise AssertionError(f"command failed: {values!r}")


def require_contains(values: list[object], expected: bytes) -> None:
    if not contains(values, expected):
        raise AssertionError(f"missing {expected!r}: {values!r}")


def verify_restart(options: argparse.Namespace, context: ssl.SSLContext) -> None:
    client = connect(options.port, context)
    status, values = status_recent(client)
    require_ok(status, values)
    require_contains(values, b"RECENT 0")
    status, _ = client.select("INBOX")
    require_ok(status, [])
    status, values = client.uid("FETCH", "1", "(FLAGS)")
    require_ok(status, values)
    if contains(values, b"\\Recent"):
        raise AssertionError(f"restart restored claimed recent: {values!r}")
    client.logout()
    print("IMAP RECENT restart replay passed")


def main() -> None:
    options = arguments()
    context = ssl.create_default_context(cafile=options.ca)
    if options.verify_restart:
        verify_restart(options, context)
        return
    producer = connect(options.port, context)
    body = b"From: qa@litterkeep.test\r\nSubject: recent\r\n\r\nbody"
    status, _ = producer.append("INBOX", None, None, body)
    require_ok(status, [])
    status, values = status_recent(producer)
    require_ok(status, values)
    require_contains(values, b"RECENT 1")
    producer.logout()

    first = connect(options.port, context)
    status, values = first.select("INBOX")
    require_ok(status, values)
    require_contains(values, b"1")
    status, values = first.uid("FETCH", "1", "(FLAGS)")
    require_ok(status, values)
    require_contains(values, b"\\Recent")
    first.logout()

    second = connect(options.port, context)
    status, values = status_recent(second)
    require_ok(status, values)
    require_contains(values, b"RECENT 0")
    status, _ = second.select("INBOX")
    if status != "OK":
        raise AssertionError("second SELECT failed")
    status, values = second.uid("FETCH", "1", "(FLAGS)")
    if status != "OK":
        raise AssertionError("second FETCH failed")
    if contains(values, b"\\Recent"):
        raise AssertionError(f"recent leaked to second session: {values!r}")
    second.logout()
    print("IMAP RECENT passed: durable delivery, single SELECT assignment")


if __name__ == "__main__":
    main()
