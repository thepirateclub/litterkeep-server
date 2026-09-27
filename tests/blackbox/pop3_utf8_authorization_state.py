"""Проверяет RFC 6856 UTF8 только в POP3 AUTHORIZATION до USER."""

from __future__ import annotations

import argparse
import os
import socket
import ssl


def read_line(stream: socket.SocketIO, label: str) -> bytes:
    """Читает один CRLF-framed POP3 response и диагностирует обрыв."""
    line = stream.readline()
    if not line.endswith(b"\r\n"):
        raise AssertionError(f"{label}: сервер оборвал строку: {line!r}")
    return line


def exchange(stream: socket.SocketIO, command: bytes, expected: bytes) -> bytes:
    """Проверяет знак и начало ответа одной независимой POP3-команды."""
    stream.write(command + b"\r\n")
    stream.flush()
    response = read_line(stream, command.decode("ascii"))
    if not response.startswith(expected):
        raise AssertionError(f"{command!r}: ожидался {expected!r}, получен {response!r}")
    return response


def verify(host: str, port: int, password: str) -> None:
    """Проверяет STLS, USER, запрещённый UTF8, PASS и TRANSACTION."""
    with socket.create_connection((host, port), timeout=10) as connection:
        stream = connection.makefile("rwb", buffering=0)
        greeting = read_line(stream, "greeting")
        if not greeting.startswith(b"+OK"):
            raise AssertionError(f"неожиданный greeting: {greeting!r}")
        exchange(stream, b"STLS", b"+OK")
        stream.close()

        context = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
        context.check_hostname = False
        context.verify_mode = ssl.CERT_NONE
        with context.wrap_socket(connection, server_hostname=host) as tls:
            tls_stream = tls.makefile("rwb", buffering=0)
            exchange(tls_stream, b"USER bob", b"+OK")
            exchange(tls_stream, b"UTF8", b"-ERR")
            exchange(tls_stream, f"PASS {password}".encode("ascii"), b"+OK")
            exchange(tls_stream, b"STAT", b"+OK")
            exchange(tls_stream, b"QUIT", b"+OK")


def main() -> None:
    """Запускает независимую STARTTLS-проверку тестовой учётной записи."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, required=True)
    parser.add_argument("--password", default=os.environ.get("LITTERKEEP_BOB_PASSWORD"))
    options = parser.parse_args()
    if options.password is None:
        raise SystemExit("укажите --password или LITTERKEEP_BOB_PASSWORD")
    verify(options.host, options.port, options.password)
    print("PASS POP3 RFC 6856: UTF8 after USER rejected, PASS state preserved")


if __name__ == "__main__":
    main()
