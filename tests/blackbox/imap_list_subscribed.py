"""Проверяет RFC LIST (SUBSCRIBED) через независимый TLS IMAP-клиент."""

import argparse
import imaplib
import os
import ssl
import uuid


def require(status: str, detail: object, expected: str, operation: str) -> object:
    if status != expected:
        raise AssertionError(f"{operation}: {status} {detail!r}")
    return detail


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("host", nargs="?", default="127.0.0.1")
    parser.add_argument("port", nargs="?", type=int, default=2149)
    parser.add_argument("--ca", default="data/pki/ca.pem")
    options = parser.parse_args()
    trust = ssl.create_default_context(cafile=options.ca)
    client = imaplib.IMAP4(options.host, options.port, timeout=10)
    client.starttls(ssl_context=trust)
    password = os.environ["LITTERKEEP_BOB_PASSWORD"]
    require(*client.login("bob", password), "OK", "LOGIN")
    suffix = uuid.uuid4().hex
    present_name = f"SubscribedPresent{suffix}"
    missing_name = f"SubscribedMissing{suffix}"
    require(*client.create(present_name), "OK", "CREATE")
    require(*client.subscribe(present_name), "OK", "SUBSCRIBE existing")
    require(*client.subscribe(missing_name), "OK", "SUBSCRIBE missing")

    tag = client._new_tag()
    client.send(tag + b' LIST (SUBSCRIBED) "" "*"\r\n')
    status, detail = client._get_tagged_response(tag)
    require(status, detail, "OK", "LIST (SUBSCRIBED)")
    listing = client.untagged_responses.pop("LIST", [])
    records = b"\n".join(value for value in listing if isinstance(value, bytes))
    present = f'(\\Subscribed) "/" "{present_name}"'.encode()
    missing = f'(\\NonExistent \\Subscribed) "/" "{missing_name}"'.encode()
    if present not in records or missing not in records:
        raise AssertionError(f"LIST (SUBSCRIBED) records are wrong: {records!r}")
    client.logout()
    print("PASS imap-list-subscribed: existing and missing subscriptions")


if __name__ == "__main__":
    main()
