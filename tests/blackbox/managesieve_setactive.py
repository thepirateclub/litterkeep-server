"""Проверяет RFC 5804 SETACTIVE через ManageSieve TLS и Store owner."""

from __future__ import annotations

import argparse
import base64
import socket
import ssl
from enum import Enum
from pathlib import Path


class Reply(bytes, Enum):
    """Точные completion строки используемых ManageSieve команд."""

    READY = b'OK "ready"\r\n'
    TLS = b'OK "begin TLS"\r\n'
    AUTHENTICATED = b'OK "authenticated"\r\n'
    COMPLETED = b'OK "completed"\r\n'
    NONEXISTENT = b'NO (NONEXISTENT) "script does not exist"\r\n'
    LISTED = b'OK "list completed"\r\n'
    LOGOUT = b'OK "bye"\r\n'


def read_line(connection: socket.socket) -> bytes:
    """Читает ограниченную CRLF-строку без потери wire bytes."""
    received = bytearray()
    while not received.endswith(b"\r\n"):
        block = connection.recv(1)
        if not block:
            raise AssertionError("сервер закрыл соединение до конца строки")
        received.extend(block)
        if len(received) > 4096:
            raise AssertionError("ManageSieve response line превысила лимит")
    return bytes(received)


def read_block(connection: socket.socket, ending: Reply) -> bytes:
    """Читает ограниченный ответ, включая все строки до completion."""
    received = bytearray()
    while not received.endswith(ending.value):
        received.extend(read_line(connection))
        if len(received) > 65536:
            raise AssertionError("ManageSieve response превысил лимит")
    return bytes(received)


def command(connection: socket.socket, wire: bytes, ending: Reply) -> bytes:
    """Отправляет одну команду и возвращает её полный completion block."""
    connection.sendall(wire)
    return read_block(connection, ending)


def password_for(path: Path, username: str) -> str:
    """Читает секрет только для нужной записи stage bootstrap TSV."""
    for line in path.read_text(encoding="utf-8").splitlines():
        match line.split("\t", maxsplit=1):
            case [account, password] if account == username:
                return password
            case _:
                continue
    raise AssertionError(f"missing acceptance account {username!r}")


def authenticate(encrypted: ssl.SSLSocket, credentials: Path) -> None:
    """Выполняет независимый SASL PLAIN exchange уже внутри TLS."""
    password = password_for(credentials, "bob")
    payload = base64.b64encode(b"\x00bob\x00" + password.encode("utf-8"))
    result = command(
        encrypted,
        b'AUTHENTICATE "PLAIN" "' + payload + b'"\r\n',
        Reply.AUTHENTICATED,
    )
    if not result.endswith(Reply.AUTHENTICATED.value):
        raise AssertionError(f"authentication failed: {result!r}")


def require_script_listing(connection: socket.socket, active: bool) -> None:
    """Сверяет единственный script и ACTIVE только в требуемом состоянии."""
    listing = command(connection, b"LISTSCRIPTS\r\n", Reply.LISTED)
    script_lines = [line for line in listing.splitlines() if line.startswith(b'"route"')]
    expected = b'"route" ACTIVE' if active else b'"route"'
    if script_lines != [expected]:
        raise AssertionError(f"unexpected LISTSCRIPTS state: {listing!r}")


def exercise_active_state(connection: socket.socket) -> None:
    """Проверяет no-active, NONEXISTENT, activate и пустое deactivation."""
    require_reply(connection, b'SETACTIVE ""\r\n', Reply.COMPLETED)
    missing = command(connection, b'SETACTIVE "missing"\r\n', Reply.NONEXISTENT)
    if not missing.endswith(Reply.NONEXISTENT.value):
        raise AssertionError(f"missing script lacks NONEXISTENT: {missing!r}")
    body = b"keep;"
    marker = f'PUTSCRIPT "route" {{{len(body)}+}}\r\n'.encode("ascii")
    connection.sendall(marker + body)
    if read_line(connection) != Reply.COMPLETED.value:
        raise AssertionError("PUTSCRIPT route failed")
    require_reply(connection, b'SETACTIVE "route"\r\n', Reply.COMPLETED)
    require_script_listing(connection, active=True)
    require_reply(connection, b'SETACTIVE ""\r\n', Reply.COMPLETED)
    require_script_listing(connection, active=False)
    require_reply(connection, b'SETACTIVE ""\r\n', Reply.COMPLETED)
    require_reply(connection, b'DELETESCRIPT "route"\r\n', Reply.COMPLETED)


def require_reply(connection: socket.socket, wire: bytes, expected: Reply) -> None:
    """Требует заданный completion без разбора локализованного текста."""
    actual = command(connection, wire, expected)
    if not actual.endswith(expected.value):
        raise AssertionError(f"{wire!r}: unexpected response {actual!r}")


def main() -> None:
    """Поднимает TLS к независимому серверу и проходит SETACTIVE сценарий."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--port", type=int, default=4193)
    parser.add_argument("--ca", type=Path, required=True)
    parser.add_argument("--credentials", type=Path, required=True)
    options = parser.parse_args()
    context = ssl.create_default_context(cafile=options.ca)
    with socket.create_connection(("127.0.0.1", options.port), timeout=10) as raw:
        greeting = read_block(raw, Reply.READY)
        if b'"STARTTLS"\r\n' not in greeting:
            raise AssertionError("ManageSieve greeting omitted STARTTLS")
        require_reply(raw, b"STARTTLS\r\n", Reply.TLS)
        with context.wrap_socket(raw, server_hostname="localhost") as encrypted:
            read_block(encrypted, Reply.READY)
            authenticate(encrypted, options.credentials)
            exercise_active_state(encrypted)
            require_reply(encrypted, b"LOGOUT\r\n", Reply.LOGOUT)
    print("PASS ManageSieve SETACTIVE activation and empty-name deactivation")


if __name__ == "__main__":
    main()
