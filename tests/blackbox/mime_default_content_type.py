"""Проверяет RFC 2045 default Content-Type через SMTP и IMAP wire."""

import argparse
import enum
import imaplib
import os
import re
import smtplib
import ssl
import uuid


class ImapStatus(enum.Enum):
    """Набор допустимых итоговых состояний для успешного IMAP шага."""

    OK = "OK"


def require_ok(response: tuple[str, list[object]], operation: str) -> list[object]:
    """Сравнивает wire status через enum и возвращает данные успешного ответа."""
    status, data = response
    if ImapStatus(status) is not ImapStatus.OK:
        raise AssertionError(f"{operation} failed: {status!r} {data!r}")
    return data


def options() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--smtp-port", type=int, default=2158)
    parser.add_argument("--imap-port", type=int, default=2146)
    parser.add_argument("--ca", default="data/pki/ca.pem")
    parser.add_argument("--alice-password", default=os.environ.get("LITTERKEEP_ALICE_PASSWORD"))
    parser.add_argument("--bob-password", default=os.environ.get("LITTERKEEP_BOB_PASSWORD"))
    parsed = parser.parse_args()
    if parsed.alice_password is None or parsed.bob_password is None:
        raise ValueError("bootstrap Alice and Bob passwords must be provided through environment")
    return parsed


def fixtures(token: str) -> dict[bytes, bytes]:
    """Создаёт headerless и malformed Content-Type RFC 822 сообщения."""
    base = b"From: alice@litterkeep.test\r\nTo: bob@litterkeep.test\r\nSubject: "
    token_bytes = token.encode("ascii")
    return {
        b"Missing-" + token_bytes: base + b"Missing-" + token_bytes + b"\r\n\r\nplain body " + token_bytes + b"\r\n",
        b"Malformed-" + token_bytes: base + b"Malformed-" + token_bytes
        + b"\r\nContent-Type: text/plain; charset=\"unterminated\r\n\r\nplain body "
        + token_bytes + b"\r\n",
    }


def send_messages(settings: argparse.Namespace, messages: dict[bytes, bytes]) -> None:
    """Передаёт fixtures только через SMTP STARTTLS как независимый клиент."""
    context = ssl.create_default_context(cafile=settings.ca)
    with smtplib.SMTP("127.0.0.1", settings.smtp_port, timeout=10) as client:
        client.ehlo()
        client.starttls(context=context)
        client.ehlo()
        client.login("alice", settings.alice_password)
        for message in messages.values():
            client.sendmail("alice@litterkeep.test", "bob@litterkeep.test", message)


def fetch_messages(settings: argparse.Namespace) -> list[tuple[bytes, bytes]]:
    """Возвращает пару wire BODYSTRUCTURE и исходных RFC822 bytes на каждый UID."""
    context = ssl.create_default_context(cafile=settings.ca)
    output = []
    with imaplib.IMAP4("127.0.0.1", settings.imap_port, timeout=10) as client:
        client.starttls(ssl_context=context)
        client.login("bob", settings.bob_password)
        require_ok(client.select("INBOX", readonly=True), "INBOX SELECT")
        records = require_ok(client.uid("SEARCH", None, "ALL"), "UID SEARCH")
        identifiers = b" ".join(item for item in records if isinstance(item, bytes)).split()
        if len(identifiers) != 2:
            raise AssertionError(f"expected exactly two delivered messages, got {identifiers!r}")
        for identifier in identifiers:
            structure = require_ok(
                client.uid("FETCH", identifier, "(BODYSTRUCTURE)"),
                "BODYSTRUCTURE FETCH",
            )
            source = require_ok(client.uid("FETCH", identifier, "(RFC822)"), "RFC822 FETCH")
            structure_bytes = b" ".join(item for item in structure if isinstance(item, bytes))
            source_bytes = b"".join(
                payload for item in source if isinstance(item, tuple)
                for payload in item if isinstance(payload, bytes)
            )
            output.append((structure_bytes, source_bytes))
    return output


def verify(messages: list[tuple[bytes, bytes]], expected: dict[bytes, bytes]) -> None:
    """Проверяет default text/plain projection и сохранение исходных заголовков."""
    expected_subjects = set(expected)
    seen_subjects = set()
    for structure, source in messages:
        subject = next(
            (candidate for candidate in expected_subjects if b"Subject: " + candidate in source),
            None,
        )
        if subject is None:
            raise AssertionError(f"delivered source has an unexpected Subject: {source!r}")
        seen_subjects.add(subject)
        if not source.endswith(expected[subject]):
            raise AssertionError("original SMTP RFC822 bytes were not preserved in the delivered source")
        if re.search(rb'\("TEXT" "PLAIN" NIL NIL NIL "7BIT" [0-9]+ [0-9]+\)', structure) is None:
            raise AssertionError(f"RFC default type was not projected as text/plain: {structure!r}")
        malformed = subject.startswith(b"Malformed-")
        if malformed and b"Content-Type: text/plain; charset=\"unterminated" not in source:
            raise AssertionError(f"malformed source Content-Type was not preserved: {source!r}")
        if not malformed and b"Content-Type:" in source:
            raise AssertionError(f"headerless fixture unexpectedly gained Content-Type: {source!r}")
    if seen_subjects != expected_subjects:
        raise AssertionError(f"missing delivered fixtures: {expected_subjects - seen_subjects!r}")


def main() -> None:
    settings = options()
    expected = fixtures(uuid.uuid4().hex)
    send_messages(settings, expected)
    verify(fetch_messages(settings), expected)
    print("PASS mime-default-content-type: SMTP delivery and IMAP fallback projection")


if __name__ == "__main__":
    main()
