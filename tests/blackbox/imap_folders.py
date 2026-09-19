"""Проверяет default folders, CREATE и LIST до и после restart."""

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
    status, initial = client.list("", "*")
    if status != "OK":
        raise AssertionError(f"initial LIST failed: {initial!r}")
    required = [
        b'"INBOX"',
        b'\\Sent',
        b'\\Drafts',
        b'\\Trash',
        b'\\Junk',
        b'\\Archive',
        b'"Invoices"',
        b'"Invoices/2026"',
    ]
    joined = b"\n".join(initial)
    missing = [marker for marker in required if marker not in joined]
    if missing:
        raise AssertionError(f"default folders missing: {missing!r} from {initial!r}")
    if not options.verify_existing:
        status, result = client.create("Projects/2026")
        if status != "OK":
            raise AssertionError(f"CREATE failed: {result!r}")
    status, folders = client.list("", "*")
    if status != "OK" or not any(b'"Projects/2026"' in row for row in folders):
        raise AssertionError(f"created folder missing: {folders!r}")
    status, shallow = client.list("", "%")
    if status != "OK" or any(b'"Projects/2026"' in row for row in shallow):
        raise AssertionError(f"one-level wildcard leaked child: {shallow!r}")
    client.logout()
    phase = "recovered" if options.verify_existing else "created"
    print(f"IMAP folders passed: defaults and nested folder {phase}")


if __name__ == "__main__":
    main()
