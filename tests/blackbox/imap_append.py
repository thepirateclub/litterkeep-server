"""Проверяет APPEND через стандартный независимый IMAP-клиент."""

import argparse
import imaplib
import ssl


def arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--port", type=int, required=True)
    parser.add_argument("--ca", required=True)
    return parser.parse_args()


def main() -> None:
    options = arguments()
    raw = (
        b"From: bob@litterkeep.test\r\n"
        b"To: bob@litterkeep.test\r\n"
        b"Subject: IMAP APPEND\r\n"
        b"\r\n"
        b"draft bytes\x00preserved"
    )
    context = ssl.create_default_context(cafile=options.ca)
    client = imaplib.IMAP4("127.0.0.1", options.port)
    client.starttls(ssl_context=context)
    client.login("bob", "bob-password")
    status, append_data = client.append("INBOX", None, None, raw)
    if status != "OK" or not any(b"APPENDUID 1 1" in item for item in append_data):
        raise AssertionError(f"APPENDUID is absent: {status!r} {append_data!r}")
    status, counts = client.select("INBOX", readonly=True)
    if status != "OK" or counts != [b"1"]:
        raise AssertionError(f"APPEND is not visible: {status!r} {counts!r}")
    status, payloads = client.fetch("1", "(RFC822)")
    stored = next(
        value
        for item in payloads
        if isinstance(item, tuple)
        for value in reversed(item)
        if isinstance(value, bytes)
    )
    if status != "OK" or stored != raw:
        raise AssertionError("APPEND bytes were not preserved exactly")
    client.logout()
    print(f"IMAP APPEND passed: octets={len(raw)}")


if __name__ == "__main__":
    main()
