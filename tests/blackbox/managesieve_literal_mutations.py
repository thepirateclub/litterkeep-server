"""Проверяет durable Store mutations через ManageSieve literal-аргументы."""

from __future__ import annotations

import argparse
import base64
import socket
import ssl
from enum import Enum
from pathlib import Path


SOURCE = b'require "fileinto"; fileinto "Invoices";'
INITIAL_NAME = b'{7+}\r\ninvoice'
RENAMED_NAME = b'{12+}\r\ninvoice-2026'


class Reply(bytes, Enum):
    """Точные завершения RFC 5804 команд, используемых acceptance."""

    READY = b'OK "ready"\r\n'
    TLS = b'OK "begin TLS"\r\n'
    AUTHENTICATED = b'OK "authenticated"\r\n'
    CONTINUATION = b'OK "ready for literal data"\r\n'
    COMPLETED = b'OK "completed"\r\n'
    LISTED = b'OK "list completed"\r\n'
    GETSCRIPT = b'OK "get completed"\r\n'
    LOGOUT = b'OK "bye"\r\n'


class Phase(str, Enum):
    """Фаза durable проверки: запись перед restart или проверка после него."""

    SEED = "seed"
    VERIFY = "verify"


def read_line(connection: socket.socket) -> bytes:
    """Читает ограниченную CRLF-строку без потери wire octets."""
    received = bytearray()
    while not received.endswith(b"\r\n"):
        block = connection.recv(1)
        if not block:
            raise AssertionError("сервер закрыл соединение до конца ответа")
        received.extend(block)
        if len(received) > 4096:
            raise AssertionError("ManageSieve response line превысила лимит")
    return bytes(received)


def read_block(connection: socket.socket, ending: Reply) -> bytes:
    """Собирает многострочный ответ до его типизированного terminal line."""
    received = bytearray()
    while not received.endswith(ending.value):
        received.extend(read_line(connection))
        if len(received) > 65536:
            raise AssertionError("ManageSieve response превысил лимит")
    return bytes(received)


def command(connection: socket.socket, wire: bytes, ending: Reply) -> bytes:
    """Отправляет одну команду и проверяет полное завершение ответа."""
    connection.sendall(wire)
    return read_block(connection, ending)


def account_password(path: Path) -> str:
    """Читает пароль Bob из stage bootstrap файла, не раскрывая его в argv."""
    for line in path.read_text(encoding="utf-8").splitlines():
        match line.split("\t", maxsplit=1):
            case ["bob", secret] if secret:
                return secret
            case _:
                continue
    raise AssertionError("acceptance credentials omit Bob")


def authenticate(connection: ssl.SSLSocket, credentials: Path) -> None:
    """Выполняет SASL PLAIN внутри TLS, не печатая пароль."""
    secret = account_password(credentials)
    payload = base64.b64encode(b"\x00bob\x00" + secret.encode("utf-8"))
    reply = command(
        connection,
        b'AUTHENTICATE "PLAIN" "' + payload + b'"\r\n',
        Reply.AUTHENTICATED,
    )
    if not reply.endswith(Reply.AUTHENTICATED.value):
        raise AssertionError(f"ManageSieve authentication failed: {reply!r}")


def require_completion(connection: socket.socket, wire: bytes, expected: Reply) -> bytes:
    """Проверяет конечный ответ и возвращает полный блок для content checks."""
    response = command(connection, wire, expected)
    if not response.endswith(expected.value):
        raise AssertionError(f"{wire!r}: unexpected response {response!r}")
    return response


def put_literal(connection: socket.socket, name: bytes, source: bytes) -> None:
    """Передаёт имя как LITERAL- и source как synchronizing literal."""
    marker = b"PUTSCRIPT " + name + b" {" + str(len(source)).encode() + b"}\r\n"
    connection.sendall(marker)
    continuation = read_line(connection)
    if continuation != Reply.CONTINUATION.value:
        raise AssertionError(
            f"PUTSCRIPT source literal continuation differs: {continuation!r}"
        )
    connection.sendall(source + b"\r\n")
    if read_line(connection) != Reply.COMPLETED.value:
        raise AssertionError("PUTSCRIPT literal mutation не завершилась")


def check_listing(connection: socket.socket, expected: bytes) -> None:
    """Сверяет literal script name и ACTIVE projection в LISTSCRIPTS."""
    listing = require_completion(connection, b"LISTSCRIPTS\r\n", Reply.LISTED)
    entries = {line for line in listing.splitlines() if line.startswith(b'"')}
    if entries != {expected}:
        raise AssertionError(f"unexpected ManageSieve catalog: {entries!r}")


def seed(connection: socket.socket) -> None:
    """Создаёт, переименовывает и активирует script через literal names."""
    put_literal(connection, INITIAL_NAME, SOURCE)
    rename = b"RENAMESCRIPT " + INITIAL_NAME + b" " + RENAMED_NAME + b"\r\n"
    require_completion(connection, rename, Reply.COMPLETED)
    activate = b"SETACTIVE " + RENAMED_NAME + b"\r\n"
    require_completion(connection, activate, Reply.COMPLETED)
    check_listing(connection, b'"invoice-2026" ACTIVE')


def verify(connection: socket.socket) -> None:
    """После restart проверяет имя, ACTIVE и неизменность сохранённого source."""
    check_listing(connection, b'"invoice-2026" ACTIVE')
    get_script = b"GETSCRIPT " + RENAMED_NAME + b"\r\n"
    response = require_completion(connection, get_script, Reply.GETSCRIPT)
    if SOURCE not in response:
        raise AssertionError("GETSCRIPT после Store reopen изменил исходные bytes")


def run(options: argparse.Namespace) -> None:
    """Открывает STARTTLS-сессию и исполняет явно заданную acceptance-фазу."""
    context = ssl.create_default_context(cafile=str(options.ca))
    with socket.create_connection((options.host, options.port), timeout=10) as raw:
        greeting = read_block(raw, Reply.READY)
        if b'"STARTTLS"' not in greeting:
            raise AssertionError("ManageSieve greeting omitted STARTTLS")
        command(raw, b"STARTTLS\r\n", Reply.TLS)
        with context.wrap_socket(raw, server_hostname=options.server_name) as encrypted:
            read_block(encrypted, Reply.READY)
            authenticate(encrypted, options.credentials)
            match options.phase:
                case Phase.SEED:
                    seed(encrypted)
                case Phase.VERIFY:
                    verify(encrypted)
            require_completion(encrypted, b"LOGOUT\r\n", Reply.LOGOUT)


def main() -> None:
    """Принимает только endpoint и тестовую учётную запись из аргументов."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("phase", type=Phase)
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=4195)
    parser.add_argument("--server-name", default="litterkeep.test")
    parser.add_argument("--ca", type=Path, required=True)
    parser.add_argument("--credentials", type=Path, required=True)
    options = parser.parse_args()
    run(options)
    print(f"PASS ManageSieve literal Store mutation {options.phase.value}")


if __name__ == "__main__":
    main()
