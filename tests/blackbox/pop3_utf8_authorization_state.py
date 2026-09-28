"""Проверяет RFC 6856 UTF8 в POP3 AUTHORIZATION до и после USER."""

from __future__ import annotations

import argparse
import base64
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
    label = command.decode("utf-8", errors="backslashreplace")
    response = read_line(stream, label)
    if not response.startswith(expected):
        raise AssertionError(f"{command!r}: ожидался {expected!r}, получен {response!r}")
    return response


def capabilities(stream: socket.SocketIO) -> set[bytes]:
    """Читает полный CAPA блок и возвращает строки расширений."""
    stream.write(b"CAPA\r\n")
    stream.flush()
    status = read_line(stream, "CAPA status")
    if not status.startswith(b"+OK"):
        raise AssertionError(f"CAPA failed: {status!r}")
    rows = set()
    while True:
        row = read_line(stream, "CAPA row")
        match row:
            case b".\r\n":
                return rows
            case _:
                rows.add(row.rstrip(b"\r\n"))


def verify(host: str, port: int, password: str) -> None:
    """Проверяет UTF8 USER/PASS и SASL повтор после отказа в одной сессии."""
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
            advertised = capabilities(tls_stream)
            if b"UTF8 USER" not in advertised:
                raise AssertionError(f"UTF8 USER capability absent: {advertised!r}")
            exchange(tls_stream, b"USER bob", b"+OK")
            rejected_password = exchange(
                tls_stream, "PASS неверный🔒".encode("utf-8"), b"-ERR"
            )
            if not rejected_password.startswith(b"-ERR authentication failed"):
                raise AssertionError(f"UTF-8 PASS не дошёл до auth: {rejected_password!r}")
            exchange(tls_stream, "USER тестовый".encode("utf-8"), b"+OK")
            rejected_user = exchange(tls_stream, b"PASS wrong", b"-ERR")
            if not rejected_user.startswith(b"-ERR authentication failed"):
                raise AssertionError(f"UTF-8 USER не сохранился до PASS: {rejected_user!r}")
            exchange(tls_stream, b"USER bob", b"+OK")
            exchange(tls_stream, b"UTF8", b"+OK")
            exchange(tls_stream, b"PASS wrong", b"-ERR")
            sasl = base64.b64encode(b"\x00bob\x00" + password.encode("ascii"))
            exchange(tls_stream, b"AUTH PLAIN " + sasl, b"+OK")
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
    print("PASS POP3 RFC 6856: UTF8 valid after USER; PASS exchange preserved")


if __name__ == "__main__":
    main()
