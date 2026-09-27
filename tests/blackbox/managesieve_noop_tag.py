"""Проверяет RFC 5804 NOOP до аутентификации и эхо response code TAG."""

from __future__ import annotations

import argparse
import socket


def read_line(connection: socket.socket) -> bytes:
    """Читает одну ограниченную CRLF-строку без скрытой нормализации."""
    received = bytearray()
    while not received.endswith(b"\r\n"):
        block = connection.recv(1)
        if not block:
            raise AssertionError("сервер закрыл соединение посреди строки")
        received.extend(block)
        if len(received) > 4096:
            raise AssertionError("строка ManageSieve превысила тестовый предел")
    return bytes(received)


def exchange(connection: socket.socket, command: bytes, expected: bytes) -> None:
    """Сверяет один ответ с точной wire-формой RFC 5804."""
    connection.sendall(command)
    actual = read_line(connection)
    if actual != expected:
        raise AssertionError(f"{command!r}: ожидалось {expected!r}, получено {actual!r}")


def read_greeting(connection: socket.socket) -> bytes:
    """Читает ограниченный capability block и обнаруживает преждевременный EOF."""
    greeting = bytearray()
    while not greeting.endswith(b'OK "ready"\r\n'):
        block = connection.recv(4096)
        if not block:
            raise AssertionError("сервер закрыл соединение до конца greeting")
        greeting.extend(block)
        if len(greeting) > 65536:
            raise AssertionError("ManageSieve greeting превысил тестовый предел")
    return bytes(greeting)


def main() -> None:
    """Проверяет greeting, оба NOOP-варианта и завершение незалогиненной сессии."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=4191)
    options = parser.parse_args()
    with socket.create_connection((options.host, options.port), timeout=10) as connection:
        greeting = read_greeting(connection)
        if greeting.count(b'"VERSION" "1.0"\r\n') != 1:
            raise AssertionError(f"RFC 5804 VERSION capability missing or repeated: {greeting!r}")
        exchange(connection, b"NOOP\r\n", b'OK "noop"\r\n')
        exchange(
            connection,
            b'NOOP "sync-42"\r\n',
            b'OK (TAG "sync-42") "noop"\r\n',
        )
        connection.sendall(b"NOOP {7}\r\n")
        if read_line(connection) != b'OK "ready for literal data"\r\n':
            raise AssertionError("synchronizing NOOP literal was not accepted")
        connection.sendall(b"sync-42")
        if read_line(connection) != b'OK (TAG "sync-42") "noop"\r\n':
            raise AssertionError("synchronizing NOOP literal tag was not echoed")
        connection.sendall(b"NOOP {7+}\r\nsync-42")
        if read_line(connection) != b'OK (TAG "sync-42") "noop"\r\n':
            raise AssertionError("non-synchronizing NOOP literal tag was not echoed")
        connection.sendall(b"NOOP {1+}\r\n\xff")
        if read_line(connection) != b'NO "literal must be UTF-8"\r\n':
            raise AssertionError("invalid UTF-8 literal did not get rejected")
        connection.sendall(b"GETSCRIPT {3}\r\n")
        if read_line(connection) != b'OK "ready for literal data"\r\n':
            raise AssertionError("synchronizing script-name literal was not accepted")
        connection.sendall(b"box")
        if read_line(connection) != b'NO "invalid command for state"\r\n':
            raise AssertionError("GETSCRIPT literal did not reach the state check")
        for wire in (
            b"GETSCRIPT {3+}\r\nbox",
            b"DELETESCRIPT {3+}\r\nbox",
            b"SETACTIVE {3+}\r\nbox",
            b"SETACTIVE {0+}\r\n",
        ):
            connection.sendall(wire)
            if read_line(connection) != b'NO "invalid command for state"\r\n':
                raise AssertionError(f"literal command was not parsed: {wire!r}")
        exchange(
            connection,
            b'NOOP "a \\\"b\\\\c"\r\n',
            b'OK (TAG "a \\\"b\\\\c") "noop"\r\n',
        )
        exchange(connection, b'NOOP "unterminated\r\n', b'NO "syntax error"\r\n')
        exchange(connection, b'NOOP "bad\\q"\r\n', b'NO "syntax error"\r\n')
        exchange(connection, b"SETACTIVE\r\n", b'NO "syntax error"\r\n')
        exchange(connection, b"LOGOUT\r\n", b'OK "bye"\r\n')
    print("PASS ManageSieve RFC 5804 unauthenticated NOOP TAG echo")


if __name__ == "__main__":
    main()
