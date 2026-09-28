"""Проверяет RFC 5228 token case-fold через независимый ManageSieve peer."""

from __future__ import annotations

import argparse
import base64
import socket
import ssl
from pathlib import Path


def read_until(connection: socket.socket, ending: bytes) -> bytes:
    """Читает ответ до заданной строки с ограничением на объём протокола."""
    received = bytearray()
    while not received.endswith(ending):
        block = connection.recv(4096)
        if not block:
            raise AssertionError("сервер завершил ManageSieve соединение")
        received.extend(block)
        if len(received) > 65536:
            raise AssertionError("ответ ManageSieve превысил тестовый лимит")
    return bytes(received)


def password_for(path: Path, username: str) -> str:
    """Получает пароль указанного тестового аккаунта из локального fixture."""
    for line in path.read_text(encoding="utf-8").splitlines():
        match line.split("\t", maxsplit=1):
            case [account, password] if account == username:
                return password
            case _:
                continue
    raise AssertionError(f"нет credentials для тестового аккаунта {username!r}")


def send_checkscript(connection: ssl.SSLSocket, source: bytes, expected: bytes) -> None:
    """Передаёт synchronizing literal и проверяет точный CHECKSCRIPT outcome."""
    marker = b"CHECKSCRIPT {" + str(len(source)).encode("ascii") + b"}\r\n"
    connection.sendall(marker)
    if read_until(connection, b"\r\n") != b'OK "ready for literal data"\r\n':
        raise AssertionError("CHECKSCRIPT не запросил synchronizing literal")
    connection.sendall(source + b"\r\n")
    actual = read_until(connection, b"\r\n")
    if actual != expected:
        raise AssertionError(f"CHECKSCRIPT вернул {actual!r}, ожидался {expected!r}")


def main() -> None:
    """Поднимает независимое TLS соединение, входит и проверяет два скрипта."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--port", type=int, required=True)
    parser.add_argument("--ca", type=Path, required=True)
    parser.add_argument("--credentials", type=Path, required=True)
    options = parser.parse_args()
    context = ssl.create_default_context(cafile=options.ca)
    with socket.create_connection(("127.0.0.1", options.port), timeout=10) as plain:
        greeting = read_until(plain, b'OK "ready"\r\n')
        if b'"STARTTLS"\r\n' not in greeting:
            raise AssertionError("сервер не объявил STARTTLS")
        plain.sendall(b"STARTTLS\r\n")
        if read_until(plain, b"\r\n") != b'OK "begin TLS"\r\n':
            raise AssertionError("ManageSieve STARTTLS не принят")
        with context.wrap_socket(plain, server_hostname="litterkeep.test") as secure:
            read_until(secure, b'OK "ready"\r\n')
            password = password_for(options.credentials, "bob")
            proof = base64.b64encode(b"\x00bob\x00" + password.encode("utf-8"))
            secure.sendall(b'AUTHENTICATE "PLAIN" "' + proof + b'"\r\n')
            authenticated = read_until(secure, b'OK "authenticated"\r\n')
            if not authenticated.endswith(b'OK "authenticated"\r\n'):
                raise AssertionError("ManageSieve authentication failed")
            valid = (
                b'REQUIRE "FiLeInTo"; IF HEADER :IS :COMPARATOR '
                b'"I;ASCII-CASEMAP" "Subject" "invoice" '
                b'{ FILEINTO "Invoices/2026"; } eLsE { KEEP; }'
            )
            send_checkscript(secure, valid, b'OK "completed"\r\n')
            unsupported = b'REQUIRE "Unknown-Extension"; KEEP;'
            send_checkscript(secure, unsupported, b'NO "operation failed"\r\n')
    print("PASS ManageSieve RFC 5228 token case-insensitivity")


if __name__ == "__main__":
    main()
