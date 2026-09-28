"""Проверяет RFC 5804 Unicode, quoted specials и границы script names."""

from __future__ import annotations

import argparse
import base64
import socket
import ssl
from enum import Enum
from pathlib import Path


class Reply(bytes, Enum):
    """Терминальные ответы ManageSieve в acceptance-сценарии."""

    READY = b'OK "ready"\r\n'
    STARTTLS = b'OK "begin TLS"\r\n'
    AUTHENTICATED = b'OK "authenticated"\r\n'
    CONTINUATION = b'OK "ready for literal data"\r\n'
    COMPLETED = b'OK "completed"\r\n'
    REJECTED = b'NO "operation failed"\r\n'
    LISTED = b'OK "list completed"\r\n'
    GETSCRIPT = b'OK "get completed"\r\n'
    LOGOUT = b'OK "bye"\r\n'


def read_line(connection: socket.socket) -> bytes:
    """Читает ровно одну CRLF-строку с ограничением размера ответа."""
    received = bytearray()
    while not received.endswith(b"\r\n"):
        block = connection.recv(1)
        if not block:
            raise AssertionError("ManageSieve server closed before CRLF")
        received.extend(block)
        if len(received) > 4096:
            raise AssertionError("ManageSieve response line exceeded limit")
    return bytes(received)


def read_block(connection: socket.socket, ending: Reply) -> bytes:
    """Собирает строки до конкретного protocol completion."""
    received = bytearray()
    while not received.endswith(ending.value):
        received.extend(read_line(connection))
        if len(received) > 65536:
            raise AssertionError("ManageSieve response exceeded limit")
    return bytes(received)


def password_for(path: Path) -> str:
    """Находит Bob credential без вывода секрета в аргументы процесса."""
    for line in path.read_text(encoding="utf-8").splitlines():
        match line.split("\t", maxsplit=1):
            case [account, secret] if account == "bob":
                return secret
            case _:
                continue
    raise AssertionError("acceptance credentials omit Bob")


def command(connection: socket.socket, wire: bytes, ending: Reply) -> bytes:
    """Отправляет команду и ожидает её заданный терминальный ответ."""
    connection.sendall(wire)
    return read_block(connection, ending)


def authenticate(connection: ssl.SSLSocket, credentials: Path) -> None:
    """Аутентифицирует Bob через PLAIN только после TLS upgrade."""
    payload = base64.b64encode(
        b"\x00bob\x00" + password_for(credentials).encode("utf-8")
    )
    response = command(
        connection,
        b'AUTHENTICATE "PLAIN" "' + payload + b'"\r\n',
        Reply.AUTHENTICATED,
    )
    if not response.endswith(Reply.AUTHENTICATED.value):
        raise AssertionError(f"ManageSieve authentication failed: {response!r}")


def put_script(connection: socket.socket, name: bytes, source: bytes) -> Reply:
    """Передаёт name LITERAL+ и source synchronizing literal ровно по length."""
    marker = b"PUTSCRIPT {" + str(len(name)).encode() + b"+}\r\n"
    connection.sendall(marker + name + b" {" + str(len(source)).encode() + b"}\r\n")
    continuation = read_line(connection)
    if continuation != Reply.CONTINUATION.value:
        raise AssertionError(f"unexpected source continuation: {continuation!r}")
    connection.sendall(source + b"\r\n")
    response = read_line(connection)
    match response:
        case Reply.COMPLETED.value:
            return Reply.COMPLETED
        case Reply.REJECTED.value:
            return Reply.REJECTED
        case _:
            raise AssertionError(f"unexpected PUTSCRIPT completion: {response!r}")


def verify_names(connection: socket.socket) -> None:
    """Проверяет quoted escaping и отказ от строкового ответа через CRLF."""
    name = 'review"\\draft/папка'.encode("utf-8")
    if put_script(connection, name, b"keep;") is not Reply.COMPLETED:
        raise AssertionError("valid Unicode name was rejected")
    long_name = b"a" * 128
    if put_script(connection, long_name, b"keep;") is not Reply.COMPLETED:
        raise AssertionError("RFC 5804 maximum 128-character name was rejected")
    decomposed_name = "Cafe\u0301".encode("utf-8")
    if put_script(connection, decomposed_name, b"keep;") is not Reply.COMPLETED:
        raise AssertionError("decomposed Unicode name was rejected")
    fetched = command(
        connection,
        b'GETSCRIPT "Cafe\xcc\x81"\r\n',
        Reply.GETSCRIPT,
    )
    if b"keep;" not in fetched:
        raise AssertionError(f"NFC alias did not resolve stored script: {fetched!r}")

    injected = b'broken\r\nOK "forged'
    if put_script(connection, injected, b"keep;") is not Reply.REJECTED:
        raise AssertionError("ManageSieve accepted a control-bearing name")
    if put_script(connection, b"a" * 129, b"keep;") is not Reply.REJECTED:
        raise AssertionError("ManageSieve accepted a 129-character name")

    listing = command(connection, b"LISTSCRIPTS\r\n", Reply.LISTED)
    expected_unicode = b'"review\\"\\\\draft/\xd0\xbf\xd0\xb0\xd0\xbf\xd0\xba\xd0\xb0"'
    expected_long = b'"' + long_name + b'"'
    entries = {line for line in listing.splitlines() if line.startswith(b'"')}
    if entries != {expected_unicode, expected_long, b'"Caf\xc3\xa9"'}:
        raise AssertionError(f"unsafe or malformed LISTSCRIPTS data: {listing!r}")
    if b'OK "forged"' in listing:
        raise AssertionError("script name injected a ManageSieve response line")


def verify_reopened_name(connection: socket.socket) -> None:
    """Проверяет NFC имя и decomposed alias после перезапуска Store owner."""
    fetched = command(
        connection,
        b'GETSCRIPT "Cafe\xcc\x81"\r\n',
        Reply.GETSCRIPT,
    )
    if b"keep;" not in fetched:
        raise AssertionError(f"reopened NFC alias lost its script: {fetched!r}")
    listing = command(connection, b"LISTSCRIPTS\r\n", Reply.LISTED)
    entries = {line for line in listing.splitlines() if line.startswith(b'"')}
    if b'"Caf\xc3\xa9"' not in entries:
        raise AssertionError(f"reopened script name is not NFC: {listing!r}")


def run(options: argparse.Namespace) -> None:
    """Проходит независимый STARTTLS peer до точного LISTSCRIPTS ответа."""
    context = ssl.create_default_context(cafile=options.ca)
    with socket.create_connection((options.host, options.port), timeout=10) as raw:
        greeting = read_block(raw, Reply.READY)
        if b'"STARTTLS"' not in greeting:
            raise AssertionError("ManageSieve greeting omitted STARTTLS")
        command(raw, b"STARTTLS\r\n", Reply.STARTTLS)
        with context.wrap_socket(raw, server_hostname=options.server_name) as encrypted:
            read_block(encrypted, Reply.READY)
            authenticate(encrypted, options.credentials)
            match options.verify_reopened:
                case True:
                    verify_reopened_name(encrypted)
                case False:
                    verify_names(encrypted)
            command(encrypted, b"LOGOUT\r\n", Reply.LOGOUT)


def main() -> None:
    """Принимает только адрес сервера и пути локальных acceptance-файлов."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=4195)
    parser.add_argument("--server-name", default="litterkeep.test")
    parser.add_argument("--ca", type=Path, required=True)
    parser.add_argument("--credentials", type=Path, required=True)
    parser.add_argument("--verify-reopened", action="store_true")
    run(parser.parse_args())
    print("PASS ManageSieve RFC 5804 script-name wire profile")


if __name__ == "__main__":
    main()
