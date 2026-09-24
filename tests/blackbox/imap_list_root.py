"""Проверяет RFC delimiter discovery через пустой базовый LIST pattern."""

import argparse
import imaplib
import os
import ssl


def main() -> None:
    """Под STARTTLS сверяет root LIST item, который использует hierarchy browser."""
    parser = argparse.ArgumentParser()
    parser.add_argument("host", nargs="?", default="127.0.0.1")
    parser.add_argument("port", nargs="?", type=int, default=2146)
    parser.add_argument("--ca", default="data/pki/ca.pem")
    options = parser.parse_args()
    client = imaplib.IMAP4(options.host, options.port, timeout=10)
    trust = ssl.create_default_context(cafile=options.ca)
    client.starttls(ssl_context=trust)
    client.login("bob", os.environ["LITTERKEEP_BOB_PASSWORD"])
    status, entries = client.list("", "")
    if status != "OK" or entries != [b'(\\Noselect) "/" ""']:
        raise AssertionError(f"root LIST: {status} {entries!r}")
    client.logout()
    print("PASS imap-list-root")


if __name__ == "__main__":
    main()
