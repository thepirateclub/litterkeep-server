#!/usr/bin/env python3
"""Независимо проверяет RFC 5230 :mime через ManageSieve CHECKSCRIPT."""

import argparse
import base64
import os
import socket
import ssl


def require(condition: bool, message: str) -> None:
    """Завершает acceptance с понятной причиной при неверном wire-ответе."""
    if not condition:
        raise AssertionError(message)


def read_line(client: socket.socket) -> bytes:
    """Читает одну CRLF-строку, не предполагая границы TCP-пакетов."""
    received = bytearray()
    while not received.endswith(b"\r\n"):
        chunk = client.recv(4096)
        require(chunk != b"", "ManageSieve закрыл соединение до ответа")
        received.extend(chunk)
    return bytes(received)


def read_until(client: socket.socket, marker: bytes) -> bytes:
    """Читает многострочный greeting до его завершающей RFC 5804 строки."""
    received = bytearray()
    while marker not in received:
        chunk = client.recv(4096)
        require(chunk != b"", "ManageSieve закрыл соединение до greeting")
        received.extend(chunk)
    return bytes(received)


def command(client: socket.socket, wire: bytes) -> bytes:
    """Отправляет одну команду RFC 5804 и читает её строковый ответ."""
    client.sendall(wire)
    return read_line(client)


def check_script(client: socket.socket, script: bytes, accepted: bool) -> None:
    """Проверяет literal framing и ожидаемое решение валидатора скрипта."""
    declaration = f"CHECKSCRIPT {{{len(script)}}}\r\n".encode("ascii")
    require(
        command(client, declaration) == b'OK "ready for literal data"\r\n',
        "CHECKSCRIPT не запросил literal",
    )
    client.sendall(script)
    expected = b'OK "completed"\r\n'
    if not accepted:
        expected = b'NO "operation failed"\r\n'
    require(read_line(client) == expected, f"неверный CHECKSCRIPT для {script!r}")


def vacation_script(entity: bytes) -> bytes:
    """Встраивает MIME entity в Sieve quoted-string с сохранением CRLF."""
    prefix = b'require "vacation"; vacation :mime "'
    suffix = b'";'
    return prefix + entity + suffix


def run(options: argparse.Namespace) -> None:
    """Выполняет TLS-аутентификацию и набор положительных/отрицательных проб."""
    context = ssl.create_default_context(cafile=options.ca)
    with socket.create_connection((options.host, options.port), timeout=10) as raw:
        greeting = read_until(raw, b'OK "ready"\r\n')
        require(b'"STARTTLS"' in greeting, "ManageSieve не объявил STARTTLS")
        require(command(raw, b"STARTTLS\r\n") == b'OK "begin TLS"\r\n', "STARTTLS отклонён")
        with context.wrap_socket(raw, server_hostname="localhost") as client:
            read_until(client, b'OK "ready"\r\n')
            credentials = b"\x00" + options.username.encode() + b"\x00" + options.password.encode()
            encoded = base64.b64encode(credentials)
            authentication = b'AUTHENTICATE "PLAIN" "' + encoded + b'"\r\n'
            client.sendall(authentication)
            auth_reply = read_until(client, b'OK "authenticated"\r\n')
            require(auth_reply.endswith(b'OK "authenticated"\r\n'), "AUTHENTICATE PLAIN отклонён")
            check_script(
                client,
                vacation_script(b"Content-Type: text/plain\r\n\r\nAway\r\n"),
                True,
            )
            check_script(
                client,
                vacation_script(
                    b"MIME-Version: (before) 1.(inside)0 (after)\r\n"
                    b"Content-Type: text/plain\r\n\r\nAway\r\n"
                ),
                True,
            )
            check_script(
                client,
                vacation_script(
                    b"MIME-Version: 2.0\r\n"
                    b"Content-Type: text/plain\r\n\r\nAway\r\n"
                ),
                False,
            )
            check_script(
                client,
                vacation_script(
                    b"MIME-Version: 1.0\r\nMIME-Version: 1.0\r\n"
                    b"Content-Type: text/plain\r\n\r\nAway\r\n"
                ),
                False,
            )
            check_script(
                client,
                vacation_script(
                    b"MIME-Version: 1 . 0\r\n"
                    b"Content-Type: text/plain\r\n\r\nAway\r\n"
                ),
                False,
            )
            check_script(
                client,
                vacation_script(
                    b"Content-Type: text/plain\r\n"
                    b"Content-Transfer-Encoding: quoted-printable\r\n\r\nOl=E1\r\n"
                ),
                True,
            )
            check_script(
                client,
                vacation_script(
                    b"Content-Type: text/plain\r\n"
                    b"Content-Transfer-Encoding: base64\r\n\r\nSGVsbG8=\r\n"
                ),
                True,
            )
            check_script(
                client,
                vacation_script(
                    b"Content-Type: text/plain\r\n"
                    b"Content-Transfer-Encoding: base64\r\n"
                    b"Content-Transfer-Encoding: 7bit\r\n\r\nSGVsbG8=\r\n"
                ),
                False,
            )
            check_script(
                client,
                vacation_script(
                    b"Content-Type: text/plain\r\n"
                    b"Content-Transfer-Encoding: base64\r\n\r\n%%%\r\n"
                ),
                False,
            )
            check_script(
                client,
                vacation_script(
                    b"Content-Type: multipart/mixed; boundary=x\r\n"
                    b"Content-Transfer-Encoding: base64\r\n\r\nSGVsbG8=\r\n"
                ),
                False,
            )
            print("PASS managesieve-vacation-mime: MIME-Version and CTE validation")


def main() -> None:
    """Разбирает параметры независимого тестового клиента."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, required=True)
    parser.add_argument("--ca", required=True)
    parser.add_argument("--username", default="bob")
    parser.add_argument("--password", default=os.environ.get("LITTERKEEP_BOB_PASSWORD", "bob-password"))
    run(parser.parse_args())


if __name__ == "__main__":
    main()
