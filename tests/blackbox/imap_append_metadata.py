"""Проверяет долговечные flags и INTERNALDATE стандартным IMAP-клиентом."""

import argparse
import imaplib
import ssl


def arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--port", type=int, required=True)
    parser.add_argument("--ca", required=True)
    parser.add_argument("--verify-existing", action="store_true")
    return parser.parse_args()


def main() -> None:
    options = arguments()
    context = ssl.create_default_context(cafile=options.ca)
    client = imaplib.IMAP4("127.0.0.1", options.port)
    client.starttls(ssl_context=context)
    client.login("bob", "bob-password")
    raw = b"From: bob@litterkeep.test\r\nSubject: metadata\r\n\r\nbody"
    if not options.verify_existing:
        status, result = client.append(
            "INBOX",
            "(\\Seen \\Draft)",
            '"19-Sep-2026 12:34:56 +0300"',
            raw,
        )
        if status != "OK" or not any(b"APPENDUID 1 1" in item for item in result):
            raise AssertionError(f"APPEND failed: {status!r} {result!r}")
    status, _ = client.select("INBOX")
    if status != "OK":
        raise AssertionError("SELECT failed")
    status, flags = client.fetch("1", "FLAGS")
    joined_flags = b" ".join(item for item in flags if isinstance(item, bytes))
    if status != "OK" or b"\\Seen" not in joined_flags or b"\\Draft" not in joined_flags:
        raise AssertionError(f"initial flags lost: {flags!r}")
    status, dates = client.fetch("1", "INTERNALDATE")
    joined_dates = b" ".join(item for item in dates if isinstance(item, bytes))
    if status != "OK" or b'"19-Sep-2026 09:34:56 +0000"' not in joined_dates:
        raise AssertionError(f"INTERNALDATE lost: {dates!r}")
    client.logout()
    phase = "recovered" if options.verify_existing else "persisted"
    print(f"IMAP APPEND metadata passed: flags and INTERNALDATE {phase}")


if __name__ == "__main__":
    main()
