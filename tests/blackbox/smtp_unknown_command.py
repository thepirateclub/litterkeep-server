"""Проверяет RFC 5321 коды неизвестной команды и неверного аргумента."""

import os
import socket


HOST = os.environ.get("LITTERKEEP_SMTP_HOST", "127.0.0.1")
PORT = int(os.environ.get("LITTERKEEP_SMTP_PORT", "2525"))


def read_reply(stream, expected_code):
    """Сверяет числовой код ответа, не завися от текста сервера."""
    response = stream.readline()
    if not response:
        raise AssertionError("сервер закрыл соединение до ответа")
    expected_prefix = str(expected_code).encode("ascii")
    if not response.startswith(expected_prefix + b" "):
        raise AssertionError(f"ожидался код {expected_code}, получено {response!r}")
    return response.rstrip(b"\r\n")


def main():
    """Выполняет неизвестные и синтаксически неверные команды на одном сокете."""
    with socket.create_connection((HOST, PORT), timeout=10) as connection:
        stream = connection.makefile("rwb", buffering=0)
        read_reply(stream, 220)
        cases = (
            (b"WUT\r\n", 500),
            (b"WUT extra\r\n", 500),
            (b"DATA unexpected\r\n", 501),
            (b"DATA\tunexpected\r\n", 501),
            (b"VRFY \xc2\xa0alice@example.test\r\n", 501),
            (b"NOOP \xc2\xa0\r\n", 501),
            (b"HELP \xc2\xa0\r\n", 501),
            (b"NOOP\r\n", 250),
            (b"QUIT\r\n", 221),
        )
        for command, expected_code in cases:
            stream.write(command)
            response = read_reply(stream, expected_code)
            print(f"{command.rstrip()!r} => {response.decode('ascii')}")
        stream.close()
    print("PASS smtp-unknown-command: syntax classification and session recovery")


if __name__ == "__main__":
    main()
