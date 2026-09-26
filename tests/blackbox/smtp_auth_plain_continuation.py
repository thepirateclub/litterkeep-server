"""Проверяет RFC 4954 SASL PLAIN challenge, cancel и initial-response flows."""

import argparse
import base64
from pathlib import Path
import smtplib
import ssl
from enum import Enum


PARSER = argparse.ArgumentParser()
PARSER.add_argument("host", nargs="?", default="127.0.0.1")
PARSER.add_argument("port", nargs="?", type=int, default=1587)
PARSER.add_argument("ca_file")
PARSER.add_argument("credentials_file")
OPTIONS = PARSER.parse_args()


class Account(Enum):
    ALICE = "alice"


def alice_password():
    for record in Path(OPTIONS.credentials_file).read_text(encoding="utf-8").splitlines():
        account, separator, password = record.partition("\t")
        if account != Account.ALICE.value:
            continue
        if separator != "\t":
            continue
        return password
    raise AssertionError("Alice credentials are absent from the fixture file")


def plain_payload(password):
    return f"\x00{Account.ALICE.value}\x00{password}"


password = alice_password()


context = ssl.create_default_context(cafile=OPTIONS.ca_file)
with smtplib.SMTP(OPTIONS.host, OPTIONS.port, timeout=10) as client:
    client.ehlo()
    client.starttls(context=context)
    client.ehlo()

    initial = base64.b64encode(plain_payload(password).encode("ascii")).decode("ascii")
    code, response = client.docmd("AUTH", f"PLAIN\t{initial}")
    if code != 501:
        raise AssertionError(f"expected 501 for HTAB separator, received {(code, response)!r}")
    code, challenge = client.docmd("AUTH", "PLAIN")
    if code != 334:
        raise AssertionError(f"expected empty 334 challenge, received {(code, challenge)!r}")
    if challenge:
        raise AssertionError(f"expected empty 334 challenge, received {(code, challenge)!r}")
    code, response = client.docmd("*")
    if code != 501:
        raise AssertionError(f"expected 501 cancellation, received {(code, response)!r}")
    code, _ = client.docmd("AUTH", "PLAIN")
    if code != 334:
        raise AssertionError(f"expected retry challenge, received {code}")
    code, response = client.docmd("%%%")
    if code != 501:
        raise AssertionError(f"expected 501 invalid response, received {(code, response)!r}")
    code, response = client.noop()
    if code != 250:
        raise AssertionError(f"session did not recover after invalid response: {(code, response)!r}")
    code, response = client.auth(
        "PLAIN",
        lambda _challenge: plain_payload(password),
        initial_response_ok=False,
    )
    if code != 235:
        raise AssertionError(f"continuation authentication failed: {(code, response)!r}")
    code, response = client.docmd("AUTH", f"PLAIN {initial}")
    if code != 503:
        raise AssertionError(f"reauthentication should be rejected: {(code, response)!r}")

with smtplib.SMTP(OPTIONS.host, OPTIONS.port, timeout=10) as client:
    client.ehlo()
    client.starttls(context=context)
    client.ehlo()
    code, response = client.docmd("AUTH", f"PLAIN {initial}")
    if code != 235:
        raise AssertionError(f"initial-response authentication failed: {(code, response)!r}")

print("PASS smtp-auth-plain-continuation: empty challenge, cancel, response, and reauth gate")
