"""Создаёт реальный Sieve redirect job независимыми ManageSieve и SMTP clients."""
import argparse
import base64
from enum import Enum
import smtplib
import socket
import ssl
from email.message import EmailMessage
from email.policy import SMTP
from managesieve_literal_delivery import authenticate, finish_starttls


class TestAccount(Enum):
    """Имена тестовых аккаунтов fixture."""

    ALICE = "alice"
    BOB = "bob"


def password_for(path: str, account: TestAccount) -> str:
    """Читает пароль нужной записи из локального TSV fixture."""
    with open(path, encoding="utf-8") as credentials_file:
        for line in credentials_file:
            match line.rstrip("\n").split("\t", maxsplit=1):
                case [name, password] if name == account.value:
                    return password
    raise AssertionError(f"credential fixture has no {account.value} account")


def require(value: bool, message: str) -> None:
    if not value:
        raise AssertionError(message)


def read_line(client: socket.socket) -> bytes:
    result = bytearray()
    while not result.endswith(b"\r\n"):
        chunk = client.recv(4096)
        require(chunk != b"", "ManageSieve closed connection")
        result.extend(chunk)
    return bytes(result)


def command(client: socket.socket, wire: bytes) -> bytes:
    client.sendall(wire)
    return read_line(client)


def read_until(client: socket.socket, marker: bytes) -> bytes:
    """Читает reply целиком, пока не получит его завершающий статус."""
    result = bytearray()
    while marker not in result:
        try:
            chunk = client.recv(4096)
        except TimeoutError as error:
            raise AssertionError(
                f"timed out waiting for {marker!r}; received {bytes(result)!r}"
            ) from error
        require(chunk != b"", "ManageSieve closed before reply terminator")
        result.extend(chunk)
    return bytes(result)


def configure(options: argparse.Namespace, context: ssl.SSLContext, bob_password: str) -> None:
    with socket.create_connection((options.host, options.managesieve_port), timeout=10) as plain:
        greeting = read_until(plain, b'OK "ready"\r\n')
        require(b'"STARTTLS"\r\n' in greeting, "STARTTLS missing")
        require(
            command(plain, b"STARTTLS\r\n") == b'OK "begin TLS"\r\n',
            "STARTTLS",
        )
        with context.wrap_socket(plain, server_hostname="litterkeep.test") as client:
            finish_starttls(client)
            credentials = base64.b64encode(b"\x00bob\x00" + bob_password.encode())
            wire = b'AUTHENTICATE "PLAIN" "' + credentials + b'"\r\n'
            require(
                authenticate(client, wire) == b'OK "authenticated"\r\n',
                "AUTH",
            )
            source = b'redirect "remote@peer.test";'
            marker = f'PUTSCRIPT "forward" {{{len(source)}}}\r\n'.encode()
            client.sendall(marker)
            require(
                read_until(client, b'OK "ready for literal data"\r\n').endswith(
                    b'OK "ready for literal data"\r\n'
                ),
                "PUTSCRIPT literal continuation",
            )
            client.sendall(source + b"\r\n")
            require(read_line(client) == b'OK "completed"\r\n', "PUTSCRIPT")
            require(command(client, b'SETACTIVE "forward"\r\n') == b'OK "completed"\r\n', "SETACTIVE")


def deliver(options: argparse.Namespace, context: ssl.SSLContext, alice_password: str) -> None:
    message = EmailMessage()
    message["From"] = "alice@litterkeep.test"
    message["To"] = "bob@litterkeep.test"
    message["Subject"] = options.subject
    message.set_content("queue body")
    with smtplib.SMTP(options.host, options.smtp_port, timeout=10) as client:
        client.ehlo()
        client.starttls(context=context)
        client.ehlo()
        client.login("alice", alice_password)
        client.sendmail("alice@litterkeep.test", ["bob@litterkeep.test"], message.as_bytes(policy=SMTP))


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--smtp-port", type=int, required=True)
    parser.add_argument("--managesieve-port", type=int, required=True)
    parser.add_argument("--ca", required=True)
    parser.add_argument("--credentials", required=True)
    parser.add_argument("--subject", default="queue redirect peer")
    parser.add_argument("--skip-configure", action="store_true")
    options = parser.parse_args()
    context = ssl.create_default_context(cafile=options.ca)
    alice_password = password_for(options.credentials, TestAccount.ALICE)
    bob_password = password_for(options.credentials, TestAccount.BOB)
    if not options.skip_configure:
        configure(options, context, bob_password)
    deliver(options, context, alice_password)
    print("queue redirect seed: PASS")


if __name__ == "__main__":
    main()
