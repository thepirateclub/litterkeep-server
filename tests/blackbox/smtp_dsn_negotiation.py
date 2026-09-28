"""Независимая проверка RFC 3461 negotiation через raw SMTP socket."""

import argparse
import socket
from enum import IntEnum


class ReplyCode(IntEnum):
    """Коды ответов, существенные для ESMTP DSN acceptance."""

    GREETING = 220
    SUCCESS = 250
    DATA = 354
    PARAMETER_REQUIRES_EHLO = 555
    BYE = 221


def read_reply(stream):
    """Считывает все строки SMTP reply до завершающего SP-кода."""
    lines = []
    while True:
        line = stream.readline()
        if not line:
            raise AssertionError("SMTP peer closed before completing its reply")
        lines.append(line)
        if len(line) < 4 or line[3:4] == b" ":
            break
    return b"".join(lines)


def exchange(stream, request, expected):
    """Отправляет одну команду и проверяет числовой SMTP response code."""
    stream.write(request)
    response = read_reply(stream)
    code = ReplyCode(int(response[:3]))
    if code != expected:
        raise AssertionError(f"{request!r}: expected {expected}, got {response!r}")
    return response


def verify(host, port):
    """Проверяет HELO limitation и ESMTP DSN transaction независимым peer."""
    with socket.create_connection((host, port), timeout=10) as connection:
        connection.settimeout(10)
        stream = connection.makefile("rwb", buffering=0)
        greeting = read_reply(stream)
        if int(greeting[:3]) != ReplyCode.GREETING:
            raise AssertionError(f"expected SMTP 220 greeting, got {greeting!r}")

        exchange(stream, b"HELO dsn.test\r\n", ReplyCode.SUCCESS)
        exchange(
            stream,
            b"MAIL FROM:<alice@test> ENVID=blocked+2B1\r\n",
            ReplyCode.PARAMETER_REQUIRES_EHLO,
        )
        ehlo = exchange(stream, b"EHLO dsn.test\r\n", ReplyCode.SUCCESS)
        if b"250-DSN\r\n" not in ehlo:
            raise AssertionError(f"EHLO did not advertise RFC 3461 DSN: {ehlo!r}")

        exchange(
            stream,
            b"MAIL FROM:<alice@test> RET=HDRS ENVID=wire+2B1\r\n",
            ReplyCode.SUCCESS,
        )
        exchange(
            stream,
            b"RCPT TO:<bob@test> NOTIFY=FAILURE ORCPT=rfc822;bob+40test\r\n",
            ReplyCode.SUCCESS,
        )
        exchange(stream, b"DATA\r\n", ReplyCode.DATA)
        stream.write(
            b"From: alice@test\r\nTo: bob@test\r\nSubject: DSN wire acceptance\r\n"
            b"\r\nRFC 3461 envelope acceptance.\r\n.\r\n"
        )
        accepted = read_reply(stream)
        if int(accepted[:3]) != ReplyCode.SUCCESS:
            raise AssertionError(f"message was not accepted after DSN envelope: {accepted!r}")
        exchange(stream, b"QUIT\r\n", ReplyCode.BYE)


def main():
    """Разбирает endpoint и запускает один независимый SMTP acceptance."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("host", nargs="?", default="127.0.0.1")
    parser.add_argument("port", nargs="?", type=int, default=2525)
    arguments = parser.parse_args()
    verify(arguments.host, arguments.port)
    print("PASS smtp-dsn-negotiation: EHLO capability, typed MAIL/RCPT, DATA")


if __name__ == "__main__":
    main()
