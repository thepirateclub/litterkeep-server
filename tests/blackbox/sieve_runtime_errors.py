"""Проверяет сетевой implicit keep при bounded Sieve runtime errors."""

import argparse
import base64
from enum import Enum
import imaplib
import smtplib
import socket
import ssl
from email.message import EmailMessage
from email.policy import SMTP
from pathlib import Path


class Status(Enum):
    """Статусы стандартных клиентов, значимые для сценария."""

    OK = "OK"


class Account(Enum):
    """Локальные тестовые аккаунты из durable bootstrap-файла."""

    ALICE = "alice"
    BOB = "bob"


def require(condition: bool, message: str) -> None:
    """Останавливает сценарий на первой нарушенной границе."""
    if not condition:
        raise AssertionError(message)


def password(options: argparse.Namespace, account: Account) -> str:
    """Читает секрет из существующего bootstrap-файла, не печатая его."""
    rows = Path(options.credentials).read_text(encoding="utf-8").splitlines()
    for row in rows:
        match row.split("\t"):
            case [account_name, secret]:
                try:
                    row_account = Account(account_name)
                except ValueError:
                    continue
                if row_account is account:
                    return secret
            case _:
                continue
    raise AssertionError(f"credential missing for {account.value}")


def read_until(client: socket.socket, marker: bytes) -> bytes:
    """Читает bounded протокольный ответ до известного завершающего marker."""
    result = bytearray()
    while marker not in result:
        chunk = client.recv(4096)
        require(chunk != b"", "ManageSieve closed connection")
        result.extend(chunk)
        require(len(result) <= 65_536, "ManageSieve response exceeded limit")
    return bytes(result)


def command(client: socket.socket, wire: bytes) -> bytes:
    """Посылает одну команду и получает её однострочное завершение."""
    client.sendall(wire)
    return read_until(client, b"\r\n")


def activate(
    options: argparse.Namespace,
    context: ssl.SSLContext,
    name: str,
    source: bytes,
) -> None:
    """Загружает и атомарно активирует отдельную версию Sieve script."""
    with socket.create_connection((options.host, options.sieve_port), timeout=10) as plain:
        greeting = read_until(plain, b'OK "ready"\r\n')
        require(b'"STARTTLS"' in greeting, "STARTTLS capability missing")
        require(command(plain, b"STARTTLS\r\n") == b'OK "begin TLS"\r\n', "STARTTLS")
        with context.wrap_socket(plain, server_hostname="litterkeep.test") as client:
            encoded = base64.b64encode(
                b"\x00bob\x00" + password(options, Account.BOB).encode()
            )
            authentication = b'AUTHENTICATE "PLAIN" "' + encoded + b'"\r\n'
            require(command(client, authentication) == b'OK "authenticated"\r\n', "AUTH")
            marker = f'PUTSCRIPT "{name}" {{{len(source)}+}}\r\n'.encode()
            client.sendall(marker + source)
            require(read_until(client, b"\r\n") == b'OK "completed"\r\n', "PUTSCRIPT")
            active = f'SETACTIVE "{name}"\r\n'.encode()
            require(command(client, active) == b'OK "completed"\r\n', "SETACTIVE")


def deliver(
    options: argparse.Namespace,
    context: ssl.SSLContext,
    subject: str,
    received_hops: int,
) -> None:
    """Передаёт письмо через реальный submission listener Alice -> Bob."""
    message = EmailMessage()
    message["From"] = "alice@litterkeep.test"
    message["To"] = "bob@litterkeep.test"
    message["Subject"] = subject
    for hop in range(received_hops):
        message["Received"] = f"from hop-{hop}.peer.test by relay.peer.test"
    message.set_content("runtime error must fall back to implicit keep")
    with smtplib.SMTP(options.host, options.smtp_port, timeout=10) as client:
        client.ehlo()
        client.starttls(context=context)
        client.ehlo()
        client.login("alice", password(options, Account.ALICE))
        client.send_message(message)


def assert_kept(
    options: argparse.Namespace,
    context: ssl.SSLContext,
    subject: str,
) -> None:
    """Доказывает через IMAP, что runtime error не потерял принятое письмо."""
    with imaplib.IMAP4(options.host, options.imap_port, timeout=10) as client:
        tls_status, _ = client.starttls(ssl_context=context)
        login_status, _ = client.login("bob", password(options, Account.BOB))
        select_status, _ = client.select("INBOX")
        require(Status(tls_status) is Status.OK, "IMAP TLS")
        require(Status(login_status) is Status.OK, "IMAP LOGIN")
        require(Status(select_status) is Status.OK, "IMAP SELECT")
        status, values = client.search(None, "SUBJECT", f'"{subject}"')
        require(Status(status) is Status.OK, "IMAP SEARCH")
        require(next(iter(values), b"").strip() != b"", f"message not kept: {subject}")


def arguments() -> argparse.Namespace:
    """Разбирает endpoints независимого тестового клиента."""
    parser = argparse.ArgumentParser()
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--smtp-port", type=int, required=True)
    parser.add_argument("--imap-port", type=int, required=True)
    parser.add_argument("--sieve-port", type=int, required=True)
    parser.add_argument("--ca", required=True)
    parser.add_argument("--credentials", required=True)
    parser.add_argument("--verify-existing", action="store_true")
    return parser.parse_args()


def main() -> None:
    """Проверяет redirect-limit и redirect-loop двумя сетевыми транзакциями."""
    options = arguments()
    context = ssl.create_default_context(cafile=options.ca)
    if not options.verify_existing:
        redirects = b" ".join(b'redirect "remote@peer.test";' for _ in range(17))
        activate(options, context, "runtime-limit", redirects)
        deliver(options, context, "runtime redirect limit", 0)
        activate(options, context, "runtime-loop", b'redirect "remote@peer.test";')
        deliver(options, context, "runtime redirect loop", 7)
    assert_kept(options, context, "runtime redirect limit")
    assert_kept(options, context, "runtime redirect loop")
    phase = "recovered" if options.verify_existing else "created"
    print(f"Sieve runtime-error network matrix: PASS ({phase})")


if __name__ == "__main__":
    main()
