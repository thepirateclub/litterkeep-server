"""Проверяет ManageSieve vacation через SMTP и последующее IMAP чтение."""

import argparse
import base64
from enum import Enum
import imaplib
import smtplib
import socket
import ssl
from email.message import EmailMessage
from email.policy import SMTP


class Phase(Enum):
    SEED = "seed"
    VERIFY = "verify"


class Status(Enum):
    OK = "OK"


class Account(Enum):
    ALICE = "alice"
    BOB = "bob"


def require(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


def read_line(client: socket.socket) -> bytes:
    result = bytearray()
    while not result.endswith(b"\r\n"):
        chunk = client.recv(4096)
        require(chunk != b"", "ManageSieve closed the connection")
        result.extend(chunk)
    return bytes(result)


def command(client: socket.socket, wire: bytes) -> bytes:
    client.sendall(wire)
    return read_line(client)


def read_until(client: socket.socket, marker: bytes) -> bytes:
    """Поглощает RFC 5804 capability block до его итогового ответа."""
    received = bytearray()
    while marker not in received:
        chunk = client.recv(4096)
        require(chunk != b"", "ManageSieve closed before capability terminator")
        received.extend(chunk)
    return bytes(received)


def configure(host: str, port: int, context: ssl.SSLContext, passwords: dict[Account, str]) -> None:
    with socket.create_connection((host, port), timeout=10) as plain:
        greeting = read_line(plain)
        require(b"vacation" in greeting.lower(), "CAPABILITY omitted vacation")
        require(command(plain, b"STARTTLS\r\n") == b'OK "begin TLS"\r\n', "STARTTLS")
        with context.wrap_socket(plain, server_hostname="litterkeep.test") as client:
            read_until(client, b'OK "ready"\r\n')
            password = passwords[Account.BOB]
            secret = base64.b64encode(b"\x00bob\x00" + password.encode())
            auth = b'AUTHENTICATE "PLAIN" "' + secret + b'"\r\n'
            client.sendall(auth)
            response = read_until(client, b'OK "authenticated"\r\n')
            require(response == b'OK "authenticated"\r\n', f"ManageSieve AUTH: {response!r}")
            source = (
                b'require "vacation"; '
                b'vacation :days 1 :subject "Out of office" '
                b':addresses ["alternate@litterkeep.test"] "I am away";'
            )
            marker = f'PUTSCRIPT "vacation" {{{len(source)}+}}\r\n'.encode()
            client.sendall(marker + source)
            require(read_line(client) == b'OK "completed"\r\n', "PUTSCRIPT vacation")
            require(command(client, b'SETACTIVE "vacation"\r\n') == b'OK "completed"\r\n', "SETACTIVE")


def submit(host: str, port: int, context: ssl.SSLContext, passwords: dict[Account, str]) -> None:
    password = passwords[Account.ALICE]
    submissions = (
        ("Vacation acceptance one", "alternate@litterkeep.test"),
        ("Vacation acceptance duplicate", "alternate@litterkeep.test"),
        ("Vacation acceptance not personal", "unrelated@peer.test"),
    )
    for subject, header_recipient in submissions:
        message = EmailMessage()
        message["From"] = "alice@peer.test"
        message["To"] = header_recipient
        message["Subject"] = subject
        message.set_content(f"body for {subject}")
        with smtplib.SMTP(host, port, timeout=10) as client:
            client.ehlo()
            client.starttls(context=context)
            client.ehlo()
            client.login("alice", password)
            client.sendmail("alice@peer.test", ["bob@litterkeep.test"], message.as_bytes(policy=SMTP))


def verify_mailbox(host: str, port: int, context: ssl.SSLContext, passwords: dict[Account, str]) -> None:
    client = imaplib.IMAP4(host, port, timeout=10)
    try:
        status, _ = client.starttls(ssl_context=context)
        require(Status(status) is Status.OK, "IMAP STARTTLS")
        password = passwords[Account.BOB]
        require(Status(client.login("bob", password)[0]) is Status.OK, "IMAP LOGIN")
        status, _ = client.select("INBOX", readonly=True)
        require(Status(status) is Status.OK, "IMAP SELECT INBOX")
        status, records = client.uid("SEARCH", None, "ALL")
        require(Status(status) is Status.OK, "IMAP UID SEARCH")
        identifiers = next(iter(records), b"").split()
        require(len(identifiers) == 3, f"expected three original messages, got {len(identifiers)}")
        subjects = []
        for identifier in identifiers:
            status, fetched = client.uid("FETCH", identifier, "RFC822")
            require(Status(status) is Status.OK, "IMAP UID FETCH")
            payload = fetched_payload(fetched)
            require(isinstance(payload, bytes), "FETCH did not return RFC822 bytes")
            subjects.append(payload)
        require(all(b"Vacation acceptance" in item for item in subjects), "original messages missing")
    finally:
        client.shutdown()


def fetched_payload(records: list[object]) -> bytes:
    """Извлекает RFC822 literal из FETCH tuples без привязки к response shape."""
    for record in records:
        if isinstance(record, tuple):
            metadata, payload = record
            if isinstance(metadata, bytes) and isinstance(payload, bytes):
                return payload
    raise AssertionError("FETCH did not return a message literal")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("phase", choices=[phase.value for phase in Phase])
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--smtp-port", type=int, default=1587)
    parser.add_argument("--imap-port", type=int, default=1143)
    parser.add_argument("--managesieve-port", type=int, default=4190)
    parser.add_argument("--ca", required=True)
    parser.add_argument("--accounts-file", required=True)
    options = parser.parse_args()
    phase = Phase(options.phase)
    passwords = read_passwords(options.accounts_file)
    context = ssl.create_default_context(cafile=options.ca)
    match phase:
        case Phase.SEED:
            configure(options.host, options.managesieve_port, context, passwords)
            submit(options.host, options.smtp_port, context, passwords)
        case Phase.VERIFY:
            verify_mailbox(options.host, options.imap_port, context, passwords)
    print(f"Sieve vacation {phase.value}: PASS")


def read_passwords(path: str) -> dict[Account, str]:
    """Загружает случайные пароли acceptance-аккаунтов, не печатая их."""
    result = {}
    with open(path, encoding="utf-8") as account_file:
        for line in account_file:
            username, separator, secret = line.rstrip("\r\n").partition("\t")
            if separator:
                try:
                    result[Account(username)] = secret
                except ValueError:
                    continue
    require(Account.ALICE in result, "Alice secret missing from account file")
    require(Account.BOB in result, "Bob secret missing from account file")
    return result


if __name__ == "__main__":
    main()
