"""Проверяет UID метаданные после перезапуска независимым IMAP-клиентом."""

import argparse
import imaplib
import ssl


def arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--port", required=True, type=int)
    parser.add_argument("--ca", required=True)
    parser.add_argument("--messages", required=True, type=int)
    parser.add_argument("--uid-next", required=True, type=int)
    return parser.parse_args()


def main() -> None:
    options = arguments()
    context = ssl.create_default_context(cafile=options.ca)
    client = imaplib.IMAP4("127.0.0.1", options.port)
    client.starttls(ssl_context=context)
    client.login("bob", "bob-password")
    status, counts = client.select("INBOX", readonly=True)
    if status != "OK" or counts != [str(options.messages).encode("ascii")]:
        raise AssertionError(f"unexpected SELECT count: {status!r} {counts!r}")
    if client.response("UIDVALIDITY") != ("UIDVALIDITY", [b"1"]):
        raise AssertionError("UIDVALIDITY changed after restart")
    expected_next = str(options.uid_next).encode("ascii")
    if client.response("UIDNEXT") != ("UIDNEXT", [expected_next]):
        raise AssertionError("UIDNEXT changed after restart")
    status, values = client.uid("FETCH", "1:*", "(RFC822.SIZE)")
    wire = b" ".join(value for value in values if isinstance(value, bytes))
    if status != "OK" or b"UID 1 RFC822.SIZE" not in wire:
        raise AssertionError(f"UID FETCH failed after restart: {status!r} {wire!r}")
    client.logout()
    print(f"IMAP UID restart passed: messages={options.messages} uid_next={options.uid_next}")


if __name__ == "__main__":
    main()
