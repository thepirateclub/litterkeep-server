"""Проверяет APPEND и SELECT через долговечный внутренний ID named folder."""

import argparse
import imaplib
import ssl


def arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--port", type=int, required=True)
    parser.add_argument("--ca", required=True)
    parser.add_argument("--verify-existing", action="store_true")
    return parser.parse_args()


def message_bytes(fetch_result: list[object]) -> bytes:
    for item in fetch_result:
        if isinstance(item, tuple):
            payload = item[1]
            if isinstance(payload, bytes):
                return payload
    raise AssertionError(f"FETCH literal missing: {fetch_result!r}")


def main() -> None:
    options = arguments()
    context = ssl.create_default_context(cafile=options.ca)
    client = imaplib.IMAP4("127.0.0.1", options.port)
    client.starttls(ssl_context=context)
    client.login("bob", "bob-password")
    raw = b"From: bob@litterkeep.test\r\nSubject: named folder\r\n\r\nbody\x00bytes"
    if not options.verify_existing:
        status, result = client.create("Projects/2026")
        if status != "OK":
            raise AssertionError(f"CREATE failed: {result!r}")
        status, result = client.append("Missing", None, None, raw)
        if status != "NO" or not any(b"TRYCREATE" in item for item in result):
            raise AssertionError(f"missing folder accepted: {status!r} {result!r}")
        status, result = client.append("Projects/2026", "(\\Draft)", None, raw)
        if status != "OK" or not any(b"APPENDUID 1 1" in item for item in result):
            raise AssertionError(f"named APPEND failed: {status!r} {result!r}")
    status, count = client.select("Projects/2026")
    if status != "OK" or count != [b"1"]:
        raise AssertionError(f"named SELECT failed: {status!r} {count!r}")
    status, fetched = client.fetch("1", "RFC822")
    if status != "OK" or message_bytes(fetched) != raw:
        raise AssertionError("named folder bytes differ")
    status, flags = client.fetch("1", "FLAGS")
    if status != "OK" or b"\\Draft" not in b" ".join(item for item in flags if isinstance(item, bytes)):
        raise AssertionError(f"named folder flags lost: {flags!r}")
    client.logout()
    phase = "recovered" if options.verify_existing else "stored"
    print(f"IMAP named folder passed: message {phase} by stable folder ID")


if __name__ == "__main__":
    main()
