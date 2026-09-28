"""Проверяет RFC 5804 QUOTA wire response на лимите script count."""

from __future__ import annotations

import argparse
import base64
import socket
import ssl
from enum import Enum
from pathlib import Path


class Reply(bytes, Enum):
    """Точные терминаторы ManageSieve exchange этого acceptance-сценария."""

    READY = b'OK "ready"\r\n'
    TLS = b'OK "begin TLS"\r\n'
    AUTHENTICATED = b'OK "authenticated"\r\n'
    COMPLETED = b'OK "completed"\r\n'
    QUOTA = b'NO (QUOTA) "quota exceeded"\r\n'
    LISTED = b'OK "list completed"\r\n'
    LOGOUT = b'OK "bye"\r\n'


def read_line(connection: socket.socket) -> bytes:
    """Читает одну bounded CRLF-строку ManageSieve."""
    received = bytearray()
    while not received.endswith(b"\r\n"):
        block = connection.recv(1)
        if not block:
            raise AssertionError("сервер закрыл соединение до окончания ответа")
        received.extend(block)
        if len(received) > 4096:
            raise AssertionError("ManageSieve response line превысила лимит теста")
    return bytes(received)


def read_block(connection: socket.socket, ending: Reply) -> bytes:
    """Читает строки до указанного completion и ограничивает весь ответ."""
    received = bytearray()
    while not received.endswith(ending.value):
        received.extend(read_line(connection))
        if len(received) > 65536:
            raise AssertionError("ManageSieve response превысил лимит теста")
    return bytes(received)


def password_for(path: Path, username: str) -> str:
    """Выбирает тестовый секрет по account из локального TSV bootstrap-а."""
    for line in path.read_text(encoding="utf-8").splitlines():
        match line.split("\t", maxsplit=1):
            case [account, password] if account == username:
                return password
            case _:
                continue
    raise AssertionError(f"missing acceptance account {username!r}")


def main() -> None:
    """Создаёт 16 scripts, получает quota на 17-м и проверяет каталог."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--port", type=int, default=4192)
    parser.add_argument("--ca", type=Path, required=True)
    parser.add_argument("--credentials", type=Path, required=True)
    options = parser.parse_args()
    context = ssl.create_default_context(cafile=options.ca)
    with socket.create_connection(("127.0.0.1", options.port), timeout=10) as raw:
        greeting = read_block(raw, Reply.READY)
        if b'"STARTTLS"\r\n' not in greeting:
            raise AssertionError("STARTTLS capability отсутствует")
        raw.sendall(b"STARTTLS\r\n")
        if read_line(raw) != Reply.TLS.value:
            raise AssertionError("STARTTLS negotiation отклонён")
        with context.wrap_socket(raw, server_hostname="localhost") as encrypted:
            post_tls = read_block(encrypted, Reply.READY)
            if b'"SASL" "PLAIN SCRAM-SHA-1"\r\n' not in post_tls:
                raise AssertionError("TLS не рекламирует поддерживаемые SASL mechanisms")
            password = password_for(options.credentials, "bob")
            response = base64.b64encode(b"\x00bob\x00" + password.encode("utf-8"))
            encrypted.sendall(b'AUTHENTICATE "PLAIN" "' + response + b'"\r\n')
            read_block(encrypted, Reply.AUTHENTICATED)
            upload_scripts(encrypted)
            require_quota_rejection(encrypted)
            require_unchanged_catalog(encrypted)
            encrypted.sendall(b"LOGOUT\r\n")
            if read_line(encrypted) != Reply.LOGOUT.value:
                raise AssertionError("ManageSieve LOGOUT завершился неожиданным ответом")
    print("PASS ManageSieve PUTSCRIPT count quota returns NO (QUOTA)")


def upload_scripts(connection: socket.socket) -> None:
    """Публикует ровно 16 валидных скриптов через несинхронные literals."""
    source = b"keep;"
    for number in range(16):
        name = f"quota-{number}".encode("ascii")
        marker = b'PUTSCRIPT "' + name + b'" {5+}\r\n'
        connection.sendall(marker + source)
        if read_line(connection) != Reply.COMPLETED.value:
            raise AssertionError(f"PUTSCRIPT within limit failed for {name!r}")


def require_quota_rejection(connection: socket.socket) -> None:
    """Требует точный response code, а не общий отказ операции."""
    connection.sendall(b'PUTSCRIPT "quota-overflow" {5+}\r\nkeep;')
    actual = read_line(connection)
    if actual != Reply.QUOTA.value:
        raise AssertionError(f"expected RFC QUOTA response, received {actual!r}")


def require_unchanged_catalog(connection: socket.socket) -> None:
    """Подтверждает отказ атомарен и ранее записанные 16 scripts остались."""
    connection.sendall(b"LISTSCRIPTS\r\n")
    listing = read_block(connection, Reply.LISTED)
    scripts = [line for line in listing.splitlines() if line.startswith(b'"quota-')]
    if len(scripts) != 16:
        raise AssertionError(f"quota rejection changed script catalog: {scripts!r}")
    if b'"quota-overflow"' in listing:
        raise AssertionError("over-quota script unexpectedly entered the catalog")


if __name__ == "__main__":
    main()
