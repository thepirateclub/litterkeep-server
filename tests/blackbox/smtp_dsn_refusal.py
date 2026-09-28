"""Проверяет, что RFC 3461 параметры требуют EHLO, а не HELO."""

import argparse
import socket
from enum import IntEnum


class ReplyCode(IntEnum):
    """Числовые исходы команд ограничения DSN по greeting mode."""

    GREETING = 220
    SUCCESS = 250
    PARAMETER_REQUIRES_EHLO = 555
    BYE = 221


def read_reply(stream):
    """Считывает многострочный ответ до строки с завершающим SP-разделителем."""
    lines = []
    while True:
        line = stream.readline()
        if not line:
            raise AssertionError("SMTP peer closed before completing its reply")
        lines.append(line)
        if len(line) < 4 or line[3:4] == b" ":
            return b"".join(lines)


def exchange(stream, request, expected):
    """Проверяет числовой код ответа на одну команду."""
    stream.write(request)
    response = read_reply(stream)
    actual = ReplyCode(int(response[:3]))
    if actual != expected:
        raise AssertionError(f"{request!r}: expected {expected}, got {response!r}")
    return response


def verify(host, port):
    """Сверяет отказ в HELO-режиме и поддержку параметров после EHLO."""
    with socket.create_connection((host, port), timeout=10) as connection:
        connection.settimeout(10)
        stream = connection.makefile("rwb", buffering=0)
        greeting = read_reply(stream)
        if int(greeting[:3]) != ReplyCode.GREETING:
            raise AssertionError(f"expected SMTP 220 greeting, got {greeting!r}")
        exchange(stream, b"HELO dsn.test\r\n", ReplyCode.SUCCESS)
        exchange(
            stream,
            b"MAIL FROM:<alice@test> ENVID=queue-9\r\n",
            ReplyCode.PARAMETER_REQUIRES_EHLO,
        )
        exchange(stream, b"MAIL FROM:<alice@test>\r\n", ReplyCode.SUCCESS)
        exchange(
            stream,
            b"RCPT TO:<bob@test> NOTIFY=FAILURE ORCPT=rfc822;bob@test\r\n",
            ReplyCode.PARAMETER_REQUIRES_EHLO,
        )
        exchange(stream, b"RSET\r\n", ReplyCode.SUCCESS)
        ehlo = exchange(stream, b"EHLO dsn.test\r\n", ReplyCode.SUCCESS)
        if b"250-DSN\r\n" not in ehlo:
            raise AssertionError(f"EHLO did not advertise RFC 3461 DSN: {ehlo!r}")
        exchange(
            stream,
            b"MAIL FROM:<alice@test> ENVID=queue-9\r\n",
            ReplyCode.SUCCESS,
        )
        exchange(
            stream,
            b"RCPT TO:<bob@test> NOTIFY=FAILURE ORCPT=rfc822;bob@test\r\n",
            ReplyCode.SUCCESS,
        )
        exchange(stream, b"RSET\r\n", ReplyCode.SUCCESS)
        exchange(stream, b"QUIT\r\n", ReplyCode.BYE)


def main():
    """Разбирает endpoint и запускает raw-socket acceptance."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("host", nargs="?", default="127.0.0.1")
    parser.add_argument("port", nargs="?", type=int, default=2525)
    options = parser.parse_args()
    verify(options.host, options.port)
    print("PASS smtp-dsn-greeting: HELO rejects DSN and EHLO accepts it")


if __name__ == "__main__":
    main()
