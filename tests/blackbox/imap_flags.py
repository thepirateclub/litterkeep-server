"""Проверяет IMAP STORE/FETCH FLAGS и их долговечность."""

import argparse
import imaplib
import ssl


def arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--port", type=int, required=True)
    parser.add_argument("--ca", required=True)
    parser.add_argument("--verify-only", action="store_true")
    return parser.parse_args()


def wire(values: list[object]) -> bytes:
    return b" ".join(value for value in values if isinstance(value, bytes))


def main() -> None:
    options = arguments()
    context = ssl.create_default_context(cafile=options.ca)
    client = imaplib.IMAP4("127.0.0.1", options.port)
    client.starttls(ssl_context=context)
    client.login("bob", "bob-password")
    if not options.verify_only:
        for subject in (b"first", b"second"):
            raw = b"From: bob@litterkeep.test\r\nSubject: " + subject + b"\r\n\r\nbody"
            status, _ = client.append("INBOX", None, None, raw)
            if status != "OK":
                raise AssertionError("fixture APPEND failed")
    status, counts = client.select("INBOX")
    if status != "OK" or counts != [b"2"]:
        raise AssertionError(f"unexpected mailbox: {status!r} {counts!r}")
    if not options.verify_only:
        status, changed = client.store("1", "+FLAGS", "(\\Seen \\Flagged)")
        changed_wire = wire(changed)
        if status != "OK" or b"FLAGS (\\Flagged \\Seen)" not in changed_wire:
            raise AssertionError(f"STORE response is wrong: {changed_wire!r}")
        status, silent = client.store("1", "-FLAGS.SILENT", "(\\Flagged)")
        if status != "OK" or wire(silent):
            raise AssertionError(f"silent STORE emitted FETCH: {silent!r}")
        status, _ = client.uid("STORE", "2", "+FLAGS.SILENT", "(\\Draft)")
        if status != "OK":
            raise AssertionError("UID STORE failed")
    status, first = client.fetch("1", "(FLAGS)")
    status_second, second = client.fetch("2", "(FLAGS)")
    first_wire = wire(first)
    second_wire = wire(second)
    if status != "OK" or b"FLAGS (\\Seen)" not in first_wire:
        raise AssertionError(f"first flags differ: {first_wire!r}")
    if status_second != "OK" or b"FLAGS (\\Draft)" not in second_wire:
        raise AssertionError(f"second flags differ: {second_wire!r}")
    client.logout()
    print(f"IMAP FLAGS passed: persisted={options.verify_only}")


if __name__ == "__main__":
    main()
