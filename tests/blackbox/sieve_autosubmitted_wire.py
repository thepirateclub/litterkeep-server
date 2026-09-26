#!/usr/bin/env python3
"""Проверяет RFC 3834 Auto-Submitted policy через реальные mail wire paths."""

import argparse
import base64
from collections import Counter
from dataclasses import dataclass
from email import policy
from email.parser import BytesParser
from enum import Enum
from pathlib import Path
import imaplib
import smtplib
import socket
import ssl
import threading
import time

from sieve_vacation_peer import CaptureServer


class Result(Enum):
    OK = "OK"


class Account(Enum):
    BOB = "bob"


@dataclass(frozen=True)
class PolicyCase:
    sender: str
    fields: tuple[str, ...]


class Case(Enum):
    VALID_CWFS = PolicyCase("human-comments@peer.test", ("no (human) ; x-source=manual",))
    VALID_PARAMETER = PolicyCase("human-parameter@peer.test", ('no; x-label="human; authored"',))
    AUTOMATED_GENERATED = PolicyCase("generated@peer.test", ("auto-generated; x-source=cron",))
    AUTOMATED_REPLIED = PolicyCase("replied@peer.test", ("auto-replied",))
    EXTENSION = PolicyCase("extension@peer.test", ("x-automation",))
    MALFORMED_PARAMETER = PolicyCase("malformed@peer.test", ("no; x-broken=",))
    DUPLICATE = PolicyCase("duplicate@peer.test", ("no", "no"))
    UNICODE_WSP = PolicyCase("unicode-wsp@peer.test", ("\u00a0no",))


def require(condition: bool, message: str) -> None:
    """Останавливает acceptance на первом несовпадении wire поведения."""
    if not condition:
        raise AssertionError(message)


def credentials(path: Path) -> dict[Account, str]:
    """Загружает только Alice/Bob пароли и не выводит их в журнал."""
    result: dict[Account, str] = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        username, separator, password = line.partition("\t")
        if separator:
            try:
                result[Account(username)] = password
            except ValueError:
                continue
    require(Account.BOB in result, "Bob credentials missing")
    return result


def configure_vacation(options: argparse.Namespace, passwords: dict[Account, str]) -> None:
    """Сохраняет активный RFC 5230 script через независимый ManageSieve TLS клиент."""
    context = ssl.create_default_context(cafile=options.ca)
    with socket.create_connection((options.host, options.sieve_port), timeout=10) as raw:
        greeting = read_until(raw, b'OK "ready"\r\n')
        require(b'"STARTTLS"' in greeting, "ManageSieve omitted STARTTLS")
        raw.sendall(b"STARTTLS\r\n")
        require(read_line(raw) == b'OK "begin TLS"\r\n', "ManageSieve STARTTLS failed")
        with context.wrap_socket(raw, server_hostname="localhost") as client:
            read_until(client, b'OK "ready"\r\n')
            plain = b"\x00bob\x00" + passwords[Account.BOB].encode()
            auth = base64.b64encode(plain)
            client.sendall(b'AUTHENTICATE "PLAIN" "' + auth + b'"\r\n')
            authenticated = read_until(client, b'OK "authenticated"\r\n')
            require(authenticated.endswith(b'OK "authenticated"\r\n'), "Sieve auth")
            script = b'require "vacation"; vacation :days 1 :subject "Policy" "Away";'
            marker = f'PUTSCRIPT "policy" {{{len(script)}+}}\r\n'.encode("ascii")
            client.sendall(marker + script)
            require(read_line(client) == b'OK "completed"\r\n', "PUTSCRIPT")
            client.sendall(b'SETACTIVE "policy"\r\n')
            require(read_line(client) == b'OK "completed"\r\n', "SETACTIVE")


def send_policy_cases(options: argparse.Namespace) -> None:
    """Передаёт external messages distinct senders через входящий SMTP listener."""
    for case in Case:
        headers = [
            b"From: " + case.value.sender.encode("ascii"),
            b"To: bob@litterkeep.test",
            b"Subject: Auto-Submitted policy acceptance",
        ]
        headers.extend(b"Auto-Submitted: " + value.encode("utf-8") for value in case.value.fields)
        message = b"\r\n".join(headers) + b"\r\n\r\npolicy body\r\n"
        with smtplib.SMTP(options.host, options.smtp_port, timeout=10) as client:
            client.ehlo()
            send_forced_smtputf8(client, case.value.sender, message)


def send_forced_smtputf8(client: smtplib.SMTP, sender: str, message: bytes) -> None:
    """Передаёт RFC 6531 заголовок, явно форсируя не рекламируемый extension."""
    code, response = client.docmd("MAIL", f"FROM:<{sender}> SMTPUTF8")
    require(code == 250, f"forced SMTPUTF8 MAIL failed: {response!r}")
    code, response = client.rcpt("bob@litterkeep.test")
    require(code == 250, f"RCPT failed: {response!r}")
    code, response = client.data(message)
    require(code == 250, f"DATA failed: {response!r}")


def verify_originals(options: argparse.Namespace, passwords: dict[Account, str]) -> None:
    """Подтверждает сохранение всех входных писем через независимый IMAP клиент."""
    context = ssl.create_default_context(cafile=options.ca)
    client = imaplib.IMAP4(options.host, options.imap_port, timeout=10)
    try:
        status, _ = client.starttls(ssl_context=context)
        require(Result(status) is Result.OK, "IMAP STARTTLS")
        status, _ = client.login(Account.BOB.value, passwords[Account.BOB])
        require(Result(status) is Result.OK, "IMAP LOGIN")
        status, _ = client.select("INBOX", readonly=True)
        require(Result(status) is Result.OK, "IMAP SELECT")
        status, rows = client.uid("SEARCH", None, "ALL")
        require(Result(status) is Result.OK, "IMAP SEARCH")
        identifiers = next(iter(rows), b"").split()
        require(len(identifiers) == len(Case), f"expected {len(Case)} originals, got {len(identifiers)}")
        messages = []
        for identifier in identifiers:
            status, records = client.uid("FETCH", identifier.decode("ascii"), "RFC822")
            require(Result(status) is Result.OK, "IMAP UID FETCH")
            payload = next(
                (
                    body
                    for record in records
                    if isinstance(record, tuple)
                    for _metadata, body in [record]
                    if isinstance(body, bytes)
                ),
                None,
            )
            require(payload is not None, "IMAP FETCH omitted message literal")
            messages.append(BytesParser(policy=policy.default).parsebytes(payload))
        actual_fields = Counter(
            tuple(str(value) for value in message.get_all("Auto-Submitted", []))
            for message in messages
        )
        expected_fields = Counter(case.value.fields for case in Case)
        require(actual_fields == expected_fields, "stored Auto-Submitted fields differ from inputs")
    finally:
        client.shutdown()


def verify_replies(peer: CaptureServer) -> None:
    """Ждёт штатный outbound scheduler и проверяет точный набор auto-replies."""
    deadline = time.monotonic() + 30
    while len(peer.payloads) < 2 and time.monotonic() < deadline:
        time.sleep(0.05)
    replies = [BytesParser(policy=policy.default).parsebytes(payload) for payload in peer.payloads]
    require(len(replies) == 2, f"expected exactly two generated replies, got {len(replies)}")
    actual = {str(reply["To"]) for reply in replies}
    expected = {Case.VALID_CWFS.value.sender, Case.VALID_PARAMETER.value.sender}
    require(actual == expected, f"unexpected vacation recipients: {actual!r}")
    require(
        all(
            str(reply["Auto-Submitted"]) == "auto-replied"
            and str(reply["Subject"]) == "Policy"
            for reply in replies
        ),
        "captured message is not the expected vacation response",
    )
    require(
        len(peer.envelope_senders) == len(replies)
        and all(sender == "MAIL FROM:<>" for sender in peer.envelope_senders),
        "generated response did not use null reverse-paths",
    )
    expected_envelopes = {f"RCPT TO:<{recipient}>" for recipient in expected}
    require(len(peer.recipients) == len(expected_envelopes), "unexpected duplicate peer delivery")
    require(set(peer.recipients) == expected_envelopes, f"unexpected peer recipients: {peer.recipients!r}")


def read_line(client: socket.socket) -> bytes:
    """Читает одну CRLF-строку без предположений о границах TCP пакета."""
    data = bytearray()
    while not data.endswith(b"\r\n"):
        chunk = client.recv(4096)
        require(bool(chunk), "ManageSieve closed before line completion")
        data.extend(chunk)
        require(len(data) <= 65536, "ManageSieve response line exceeded bound")
    return bytes(data)


def read_until(client: socket.socket, marker: bytes) -> bytes:
    """Читает bounded capability/auth response до терминальной строки."""
    data = bytearray()
    while marker not in data:
        chunk = client.recv(4096)
        require(bool(chunk), "ManageSieve closed before response terminator")
        data.extend(chunk)
        require(len(data) <= 65536, "ManageSieve response exceeded bound")
    return bytes(data)


def main() -> None:
    """Запускает независимые TLS клиенты для ManageSieve, SMTP, IMAP и peer."""
    parser = argparse.ArgumentParser()
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--smtp-port", type=int, default=1587)
    parser.add_argument("--imap-port", type=int, default=1143)
    parser.add_argument("--sieve-port", type=int, default=4190)
    parser.add_argument("--peer-port", type=int, default=2526)
    parser.add_argument("--ca", required=True)
    parser.add_argument("--accounts", required=True)
    options = parser.parse_args()
    passwords = credentials(Path(options.accounts))
    with CaptureServer(("0.0.0.0", options.peer_port)) as peer:
        worker = threading.Thread(target=peer.serve_forever, daemon=True)
        worker.start()
        try:
            configure_vacation(options, passwords)
            send_policy_cases(options)
            verify_originals(options, passwords)
            verify_replies(peer)
        finally:
            peer.shutdown()
            worker.join(timeout=5)
    print("RFC 3834 Auto-Submitted SMTP/Sieve/queue/peer acceptance: PASS")


if __name__ == "__main__":
    main()
