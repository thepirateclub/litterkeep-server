"""Проверяет implicit TLS, greeting и auth на четырёх mail listener-ах."""

from __future__ import annotations

import argparse
import base64
from enum import Enum
from pathlib import Path
import socket
import ssl


class Protocol(Enum):
    """Поддерживаемые endpoint-ы и их ожидаемые TLS greeting-ы."""

    SMTP = "smtp"
    IMAP = "imap"
    POP3 = "pop3"
    SIEVE = "managesieve"

    @property
    def greeting(self) -> bytes:
        """Возвращает типизированный префикс greeting-а endpoint-а."""
        match self:
            case Protocol.SMTP:
                return b"220 "
            case Protocol.IMAP:
                return b"* OK "
            case Protocol.POP3:
                return b"+OK "
            case Protocol.SIEVE:
                return b'"IMPLEMENTATION"'


def line(connection: ssl.SSLSocket) -> bytes:
    """Читает одну ограниченную CRLF-строку ответа."""
    received = bytearray()
    while not received.endswith(b"\r\n"):
        block = connection.recv(1)
        if not block:
            raise AssertionError("сервер закрыл TLS stream до ответа")
        received.extend(block)
        if len(received) > 8192:
            raise AssertionError("строка ответа превысила лимит")
    return bytes(received)


def block(connection: ssl.SSLSocket, terminator: bytes) -> bytes:
    """Читает строки до типизированного протокольного terminal marker."""
    response = bytearray()
    while not response.endswith(terminator):
        response.extend(line(connection))
        if len(response) > 65536:
            raise AssertionError("ответ протокола превысил лимит")
    return bytes(response)


def smtp_block(connection: ssl.SSLSocket) -> bytes:
    """Читает многострочный SMTP reply до строки с разделителем SPACE."""
    response = bytearray()
    while True:
        current = line(connection)
        response.extend(current)
        if current.startswith(b"250 "):
            return bytes(response)
        if len(response) > 65536:
            raise AssertionError("SMTP capabilities exceeded limit")


def password(credentials: Path, account: str) -> str:
    """Читает пароль из init-файла, не передавая секрет через argv."""
    for row in credentials.read_text(encoding="utf-8").splitlines():
        match row.split("\t", maxsplit=1):
            case [name, secret] if name == account:
                if secret:
                    return secret
            case _:
                continue
    raise AssertionError(f"credentials file omits {account}")


def sasl_plain(account: str, secret: str) -> bytes:
    """Кодирует стандартный SASL PLAIN initial response."""
    return base64.b64encode(b"\x00" + account.encode() + b"\x00" + secret.encode())


def connect(context: ssl.SSLContext, host: str, port: int) -> ssl.SSLSocket:
    """Завершает TLS handshake прежде, чем клиент читает greeting."""
    raw = socket.create_connection((host, port), timeout=5)
    secure = context.wrap_socket(raw, server_hostname="litterkeep.test")
    secure.settimeout(5)
    return secure


def check_smtp(context: ssl.SSLContext, host: str, port: int, secret: str) -> None:
    """Проверяет SMTP greeting, отсутствие STARTTLS и AUTH PLAIN."""
    with connect(context, host, port) as peer:
        if not line(peer).startswith(Protocol.SMTP.greeting):
            raise AssertionError("SMTP greeting is malformed")
        peer.sendall(b"EHLO acceptance.test\r\n")
        capabilities = smtp_block(peer)
        if b"STARTTLS" in capabilities:
            raise AssertionError(f"implicit TLS SMTP must not offer STARTTLS: {capabilities!r}")
        if b"AUTH PLAIN" not in capabilities:
            raise AssertionError(f"unexpected SMTP capabilities: {capabilities!r}")
        payload = sasl_plain("alice", secret)
        peer.sendall(b"AUTH PLAIN " + payload + b"\r\n")
        if not line(peer).startswith(b"235 "):
            raise AssertionError("SMTP AUTH PLAIN failed")
        peer.sendall(b"QUIT\r\n")
        line(peer)


def check_imap(context: ssl.SSLContext, host: str, port: int, secret: str) -> None:
    """Проверяет IMAP greeting, TLS state capability и SASL PLAIN."""
    with connect(context, host, port) as peer:
        if not line(peer).startswith(Protocol.IMAP.greeting):
            raise AssertionError("IMAP greeting is malformed")
        peer.sendall(b"A1 CAPABILITY\r\n")
        capabilities = block(peer, b"A1 OK CAPABILITY completed\r\n")
        if b"STARTTLS" in capabilities:
            raise AssertionError(f"implicit TLS IMAP must not offer STARTTLS: {capabilities!r}")
        if b"AUTH=PLAIN" not in capabilities:
            raise AssertionError(f"unexpected IMAP capabilities: {capabilities!r}")
        payload = sasl_plain("bob", secret)
        peer.sendall(b"A2 AUTHENTICATE PLAIN " + payload + b"\r\n")
        if not line(peer).startswith(b"A2 OK "):
            raise AssertionError("IMAP AUTHENTICATE PLAIN failed")
        peer.sendall(b"A3 LOGOUT\r\n")
        block(peer, b"A3 OK LOGOUT completed\r\n")


def check_pop3(context: ssl.SSLContext, host: str, port: int, secret: str) -> None:
    """Проверяет POP3 implicit TLS, отсутствие STLS и SASL PLAIN auth."""
    with connect(context, host, port) as peer:
        if not line(peer).startswith(Protocol.POP3.greeting):
            raise AssertionError("POP3 greeting is malformed")
        peer.sendall(b"CAPA\r\n")
        capabilities = block(peer, b".\r\n")
        if b"STLS" in capabilities:
            raise AssertionError(f"unexpected POP3 capabilities: {capabilities!r}")
        response = sasl_plain("bob", secret)
        peer.sendall(b"AUTH PLAIN " + response + b"\r\n")
        if not line(peer).startswith(b"+OK "):
            raise AssertionError("POP3 AUTH PLAIN failed")
        peer.sendall(b"QUIT\r\n")
        if not line(peer).startswith(b"+OK "):
            raise AssertionError("POP3 QUIT failed")


def check_sieve(context: ssl.SSLContext, host: str, port: int, secret: str) -> None:
    """Проверяет ManageSieve greeting и TLS-bound SASL authentication."""
    with connect(context, host, port) as peer:
        greeting = block(peer, b'OK "ready"\r\n')
        if not greeting.startswith(Protocol.SIEVE.greeting):
            raise AssertionError(f"ManageSieve greeting is malformed: {greeting!r}")
        if b'"STARTTLS"' in greeting:
            raise AssertionError(f"unexpected ManageSieve capabilities: {greeting!r}")
        if b'"SASL" "PLAIN"' not in greeting:
            raise AssertionError(f"implicit TLS ManageSieve lacks SASL PLAIN: {greeting!r}")
        peer.sendall(b"AUTHENTICATE {4+}\r\nCRAM {4+}\r\nAAAA\r\n")
        if line(peer) != b'NO "syntax error"\r\n':
            raise AssertionError("unsupported literal mechanism was not rejected")
        peer.sendall(b"NOOP\r\n")
        if line(peer) != b'OK "noop"\r\n':
            raise AssertionError("ManageSieve did not recover after mechanism rejection")
        payload = sasl_plain("bob", secret)
        peer.sendall(b"AUTHENTICATE {5}\r\n")
        if line(peer) != b'OK "ready for literal data"\r\n':
            raise AssertionError("mechanism literal continuation was not requested")
        response_marker = b"PLAIN {" + str(len(payload)).encode() + b"}\r\n"
        peer.sendall(response_marker)
        if line(peer) != b'OK "ready for literal data"\r\n':
            raise AssertionError("SASL response literal continuation was not requested")
        peer.sendall(payload + b"\r\n")
        authenticated = block(peer, b'OK "authenticated"\r\n')
        if not authenticated.endswith(b'OK "authenticated"\r\n'):
            raise AssertionError(f"ManageSieve AUTHENTICATE failed: {authenticated!r}")
        peer.sendall(b"LOGOUT\r\n")
        if line(peer) != b'OK "bye"\r\n':
            raise AssertionError("ManageSieve LOGOUT failed")


def main() -> None:
    """Проверяет четыре независимо TLS-обёрнутых mail listener-а."""
    parser = argparse.ArgumentParser()
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--smtp-port", type=int, default=2587)
    parser.add_argument("--imap-port", type=int, default=21143)
    parser.add_argument("--pop3-port", type=int, default=21110)
    parser.add_argument("--sieve-port", type=int, default=24190)
    parser.add_argument("--ca", required=True, type=Path)
    parser.add_argument("--credentials", required=True, type=Path)
    options = parser.parse_args()
    context = ssl.create_default_context(cafile=str(options.ca))
    context.minimum_version = ssl.TLSVersion.TLSv1_2
    alice = password(options.credentials, "alice")
    bob = password(options.credentials, "bob")
    check_smtp(context, options.host, options.smtp_port, alice)
    check_imap(context, options.host, options.imap_port, bob)
    check_pop3(context, options.host, options.pop3_port, bob)
    check_sieve(context, options.host, options.sieve_port, bob)
    print("PASS: SMTP, IMAP, POP3 and ManageSieve handshake before greeting")


if __name__ == "__main__":
    main()
