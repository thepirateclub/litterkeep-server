"""Проверяет MOVE notifications в source и destination через три IMAP-сессии."""

import argparse
import imaplib
import ssl


def arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--port", type=int, required=True)
    parser.add_argument("--ca", required=True)
    parser.add_argument("--source", default="IdleMoveSource")
    parser.add_argument("--destination", default="IdleMoveDestination")
    return parser.parse_args()


def require(result: tuple[str, object], expected: str) -> object:
    status, detail = result
    if status != expected:
        raise AssertionError(f"expected {expected}, got {status}: {detail!r}")
    return detail


def connect(port: int, context: ssl.SSLContext) -> imaplib.IMAP4:
    client = imaplib.IMAP4("127.0.0.1", port)
    require(client.starttls(ssl_context=context), "OK")
    require(client.login("bob", "bob-password"), "OK")
    return client


def main() -> None:
    options = arguments()
    context = ssl.create_default_context(cafile=options.ca)
    source = connect(options.port, context)
    destination = connect(options.port, context)
    producer = connect(options.port, context)
    require(producer.create(options.source), "OK")
    require(producer.create(options.destination), "OK")
    for value in range(1, 3):
        message = f"Subject: move-{value}\r\n\r\n{value}".encode()
        require(producer.append(options.source, None, None, message), "OK")
    require(source.select(options.source), "OK")
    require(destination.select(options.destination), "OK")
    require(producer.select(options.source), "OK")

    with source.idle(duration=5) as source_idle:
        with destination.idle(duration=5) as destination_idle:
            require(producer._simple_command("MOVE", "1,2", options.destination), "OK")
            source_events = [next(source_idle), next(source_idle)]
            destination_event = next(destination_idle)

    expunged = [detail for kind, detail in source_events if kind == "EXPUNGE"]
    if expunged != [[b"1"], [b"1"]]:
        raise AssertionError(f"unexpected source events: {source_events!r}")
    if destination_event != ("EXISTS", [b"2"]):
        raise AssertionError(f"unexpected destination event: {destination_event!r}")
    if require(source.select(options.source), "OK") != [b"0"]:
        raise AssertionError("source projection was not synchronized")
    if require(destination.select(options.destination), "OK") != [b"2"]:
        raise AssertionError("destination projection was not synchronized")
    flags = require(destination.uid("FETCH", "1", "(FLAGS)"), "OK")
    if b"\\Recent" not in b" ".join(part for part in flags if isinstance(part, bytes)):
        raise AssertionError(f"destination UID did not retain session Recent: {flags!r}")
    for client in (source, destination, producer):
        client.logout()
    print("IMAP IDLE MOVE passed: source EXPUNGE and destination EXISTS/Recent")


if __name__ == "__main__":
    main()
