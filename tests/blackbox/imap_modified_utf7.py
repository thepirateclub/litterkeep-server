"""Проверяет RFC 3501 modified UTF-7 имена папок через независимый IMAP-клиент."""

import argparse
import imaplib
import ssl


def arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--port", type=int, required=True)
    parser.add_argument("--ca", required=True)
    parser.add_argument("--wire-name", default="&ZeVnLIqe-")
    parser.add_argument("--verify-existing", action="store_true")
    return parser.parse_args()


def require(result: tuple[str, object], expected: str) -> object:
    status, detail = result
    if status != expected:
        raise AssertionError(f"expected {expected}, got {status}: {detail!r}")
    return detail


def require_listed(records: object, wire_name: str) -> None:
    if not isinstance(records, list):
        raise AssertionError(f"unexpected LIST data: {records!r}")
    expected = wire_name.encode()
    for record in records:
        if isinstance(record, bytes):
            if expected in record:
                return
    raise AssertionError(f"modified UTF-7 name is absent: {records!r}")


def main() -> None:
    options = arguments()
    context = ssl.create_default_context(cafile=options.ca)
    client = imaplib.IMAP4("127.0.0.1", options.port)
    client.starttls(ssl_context=context)
    require(client.login("bob", "bob-password"), "OK")

    if not options.verify_existing:
        require(client._simple_command("CREATE", options.wire_name), "OK")

    require(client.select(options.wire_name), "OK")
    require_listed(require(client.list("", options.wire_name), "OK"), options.wire_name)
    client.logout()
    phase = "replayed" if options.verify_existing else "created"
    print(f"IMAP modified UTF-7 passed: {phase}")


if __name__ == "__main__":
    main()
