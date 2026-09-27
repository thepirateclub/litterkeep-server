"""Проверяет RFC 5804 RENAMESCRIPT response и активную переадресацию."""

from __future__ import annotations

import argparse
import base64
import socket
import ssl
from enum import Enum
from pathlib import Path


class Reply(bytes, Enum):
    """Точные completion строки сценария ManageSieve."""

    READY = b'OK "ready"\r\n'
    STARTTLS = b'OK "begin TLS"\r\n'
    AUTHENTICATED = b'OK "authenticated"\r\n'
    COMPLETED = b'OK "completed"\r\n'
    ALREADY_EXISTS = b'NO (ALREADYEXISTS) "script already exists"\r\n'
    LISTED = b'OK "list completed"\r\n'
    GETSCRIPT = b'OK "get completed"\r\n'
    LOGOUT = b'OK "bye"\r\n'


def read_line(connection: socket.socket) -> bytes:
    """Читает одну bounded CRLF-строку, включая literal content line."""
    received = bytearray()
    while not received.endswith(b"\r\n"):
        block = connection.recv(1)
        if not block:
            raise AssertionError("ManageSieve connection закрыт до конца строки")
        received.extend(block)
        if len(received) > 4096:
            raise AssertionError("ManageSieve response line превысила лимит")
    return bytes(received)


def read_block(connection: socket.socket, ending: Reply) -> bytes:
    """Читает строки до completion с общим ограничением ответа."""
    received = bytearray()
    while not received.endswith(ending.value):
        received.extend(read_line(connection))
        if len(received) > 65536:
            raise AssertionError("ManageSieve response превысил лимит теста")
    return bytes(received)


def command(connection: socket.socket, wire: bytes, ending: Reply) -> bytes:
    """Отправляет одну команду и собирает её полный completion response."""
    connection.sendall(wire)
    return read_block(connection, ending)


def password_for(path: Path) -> str:
    """Находит локальный bootstrap password для Bob без индексирования списка."""
    for line in path.read_text(encoding="utf-8").splitlines():
        match line.split("\t", maxsplit=1):
            case [account, password] if account == "bob":
                return password
            case _:
                continue
    raise AssertionError("acceptance account bob отсутствует")


def authenticate(connection: ssl.SSLSocket, credentials: Path) -> None:
    """Выполняет однокруговый SASL PLAIN после TLS upgrade."""
    secret = password_for(credentials)
    payload = base64.b64encode(b"\x00bob\x00" + secret.encode("utf-8"))
    reply = command(
        connection,
        b'AUTHENTICATE "PLAIN" "' + payload + b'"\r\n',
        Reply.AUTHENTICATED,
    )
    if not reply.endswith(Reply.AUTHENTICATED.value):
        raise AssertionError(f"SASL PLAIN rejected: {reply!r}")


def put_script(connection: socket.socket, name: str, source: bytes) -> None:
    """Передаёт UTF-8 source как точный non-synchronizing literal."""
    encoded_name = name.encode("ascii")
    marker = b'PUTSCRIPT "' + encoded_name + b'" {' + str(len(source)).encode() + b'+}\r\n'
    connection.sendall(marker + source)
    if read_line(connection) != Reply.COMPLETED.value:
        raise AssertionError(f"PUTSCRIPT did not complete for {name!r}")


def script_lines(connection: socket.socket) -> set[bytes]:
    """Возвращает catalog entries без completion строки."""
    listing = command(connection, b"LISTSCRIPTS\r\n", Reply.LISTED)
    return {line for line in listing.splitlines() if line.startswith(b'"')}


def verify_collision_is_atomic(connection: socket.socket) -> None:
    """Проверяет ALREADYEXISTS и неизменность обоих конфликтующих scripts."""
    reply = command(
        connection,
        b'RENAMESCRIPT "route" "target"\r\n',
        Reply.ALREADY_EXISTS,
    )
    if not reply.endswith(Reply.ALREADY_EXISTS.value):
        raise AssertionError(f"collision response is wrong: {reply!r}")
    expected = {b'"route"', b'"target"'}
    if script_lines(connection) != expected:
        raise AssertionError("failed rename changed the script catalog")
    source = command(connection, b'GETSCRIPT "route"\r\n', Reply.GETSCRIPT)
    if b"{5}\r\nkeep;\r\n" not in source:
        raise AssertionError(f"failed rename changed source script: {source!r}")


def verify_active_rename(connection: socket.socket) -> None:
    """Проверяет, что rename активного script переносит ACTIVE projection."""
    require_reply(connection, b'SETACTIVE "route"\r\n', Reply.COMPLETED)
    require_reply(
        connection,
        b'RENAMESCRIPT "route" "route-renamed"\r\n',
        Reply.COMPLETED,
    )
    expected = {b'"route-renamed" ACTIVE', b'"target"'}
    if script_lines(connection) != expected:
        raise AssertionError("RENAMESCRIPT did not keep the renamed script active")
    source = command(connection, b'GETSCRIPT "route-renamed"\r\n', Reply.GETSCRIPT)
    if b"{5}\r\nkeep;\r\n" not in source:
        raise AssertionError("renamed active script changed its source")


def require_reply(connection: socket.socket, wire: bytes, expected: Reply) -> None:
    """Требует точный terminal reply для одной команды."""
    actual = command(connection, wire, expected)
    if not actual.endswith(expected.value):
        raise AssertionError(f"{wire!r}: unexpected response {actual!r}")


def main() -> None:
    """Проходит ManageSieve TLS, collision, сохранность и active rename."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--port", type=int, default=4195)
    parser.add_argument("--ca", type=Path, required=True)
    parser.add_argument("--credentials", type=Path, required=True)
    options = parser.parse_args()
    tls_context = ssl.create_default_context(cafile=options.ca)
    with socket.create_connection(("127.0.0.1", options.port), timeout=10) as raw:
        greeting = read_block(raw, Reply.READY)
        if b'"STARTTLS"\r\n' not in greeting:
            raise AssertionError("ManageSieve did not advertise STARTTLS")
        if command(raw, b"STARTTLS\r\n", Reply.STARTTLS) != Reply.STARTTLS.value:
            raise AssertionError("STARTTLS rejected")
        with tls_context.wrap_socket(raw, server_hostname="localhost") as encrypted:
            read_block(encrypted, Reply.READY)
            authenticate(encrypted, options.credentials)
            put_script(encrypted, "route", b"keep;")
            put_script(encrypted, "target", b"discard;")
            verify_collision_is_atomic(encrypted)
            verify_active_rename(encrypted)
            require_reply(encrypted, b"LOGOUT\r\n", Reply.LOGOUT)
    print("PASS ManageSieve ALREADYEXISTS and active RENAMESCRIPT")


if __name__ == "__main__":
    main()
