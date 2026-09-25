#!/usr/bin/env python3
"""Проверяет синтаксис Sieve redirect через независимое соединение."""

import argparse
import base64
import socket
import ssl

from managesieve_literal_delivery import authenticate, command, finish_starttls, require, read_until


def check_script(client: socket.socket, source: bytes, expected: bytes) -> None:
    """Передаёт literal и проверяет нормативный ответ CHECKSCRIPT."""
    marker = f"CHECKSCRIPT {{{len(source)}}}\r\n".encode()
    require(command(client, marker) == b'OK "ready for literal data"\r\n', "нет literal continuation")
    client.sendall(source)
    require(read_until(client, b"\r\n") == expected, f"неверный ответ для {source!r}")


def main() -> None:
    """Подключается к ManageSieve и проверяет допустимый и запрещённые адреса."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, required=True)
    parser.add_argument("--ca", required=True)
    parser.add_argument("--username", default="bob")
    parser.add_argument("--password", default="bob-password")
    options = parser.parse_args()
    context = ssl.create_default_context(cafile=options.ca)
    with socket.create_connection((options.host, options.port), timeout=10) as raw:
        greeting = read_until(raw, b'OK "ready"\r\n')
        require(b'"STARTTLS"' in greeting, "ManageSieve не объявил STARTTLS")
        require(command(raw, b"STARTTLS\r\n") == b'OK "begin TLS"\r\n', "STARTTLS отклонён")
        with context.wrap_socket(raw, server_hostname="localhost") as client:
            finish_starttls(client)
            credentials = ("\0" + options.username + "\0" + options.password).encode()
            auth = b'AUTHENTICATE "PLAIN" "' + base64.b64encode(credentials) + b'"\r\n'
            require(authenticate(client, auth) == b'OK "authenticated"\r\n', "AUTHENTICATE отклонён")
            check_script(client, b'redirect "valid@example.test";', b'OK "completed"\r\n')
            check_script(client, b'redirect "Team: valid@example.test;";', b'NO "operation failed"\r\n')
            check_script(client, b'redirect "<@relay.example:valid@example.test>";', b'NO "operation failed"\r\n')


if __name__ == "__main__":
    main()
