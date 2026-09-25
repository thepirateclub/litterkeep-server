"""Проверяет SMTPUTF8 mailbox policy через независимый TCP peer."""

from __future__ import annotations

import argparse
import socket
from enum import IntEnum


class ReplyCode(IntEnum):
    """Коды ответов, различающие синтаксис, capability и route policy."""

    GREETING = 220
    SUCCESS = 250
    QUIT = 221
    SYNTAX_ERROR = 501
    RCPT_UTF8_REQUIRED = 553
    UNKNOWN_LOCAL_RECIPIENT = 550

    @classmethod
    def parse(cls, value: int) -> ReplyCode | int:
        """Преобразует известный wire-код в enum, сохраняя неизвестный код."""
        try:
            return cls(value)
        except ValueError:
            return value


def read_reply(stream: socket.SocketIO) -> tuple[ReplyCode | int, bytes]:
    """Считывает полный SMTP multiline response без предположения о длине."""
    response = bytearray()
    code: ReplyCode | int | None = None
    while True:
        line = stream.readline()
        if not line:
            raise AssertionError("сервер закрыл TCP до завершения SMTP ответа")
        response.extend(line)
        code_text = line.partition(b"-")[0].partition(b" ")[0]
        parsed_code = int(code_text)
        if code is None:
            code = ReplyCode.parse(parsed_code)
        if line.startswith(code_text + b" "):
            return code, bytes(response)


def expect_reply(
    stream: socket.SocketIO,
    command: bytes,
    expected: ReplyCode,
) -> bytes:
    """Отправляет одну wire-команду и проверяет её итоговый трехзначный код."""
    stream.write(command + b"\r\n")
    stream.flush()
    actual, response = read_reply(stream)
    if actual != expected:
        raise AssertionError(
            f"{command!r}: ожидался {expected.value}, получен {actual}: {response!r}"
        )
    return response


def verify_policy(host: str, port: int) -> None:
    """Доказывает отдельные SMTPUTF8 маркеры MAIL и RCPT на сетевом пути."""
    with socket.create_connection((host, port), timeout=10) as connection:
        stream = connection.makefile("rwb", buffering=0)
        greeting, banner = read_reply(stream)
        if greeting != ReplyCode.GREETING:
            raise AssertionError(f"неожиданный SMTP greeting: {banner!r}")

        capabilities = expect_reply(stream, b"EHLO independent-peer.test", ReplyCode.SUCCESS)
        if b"SMTPUTF8" in capabilities.upper():
            raise AssertionError("сервер преждевременно рекламирует неполную SMTPUTF8")

        expect_reply(stream, b"MAIL FROM:<alice@test>", ReplyCode.SUCCESS)
        expect_reply(
            stream,
            "RCPT TO:<получатель@litterkeep.test>".encode("utf-8"),
            ReplyCode.RCPT_UTF8_REQUIRED,
        )
        expect_reply(stream, b"RSET", ReplyCode.SUCCESS)

        expect_reply(
            stream,
            "MAIL FROM:<алиса@litterkeep.test>".encode("utf-8"),
            ReplyCode.SYNTAX_ERROR,
        )
        expect_reply(
            stream,
            "MAIL FROM:<алиса@litterkeep.test> SMTPUTF8".encode("utf-8"),
            ReplyCode.SUCCESS,
        )
        expect_reply(
            stream,
            "RCPT TO:<получатель@litterkeep.test>".encode("utf-8"),
            ReplyCode.UNKNOWN_LOCAL_RECIPIENT,
        )
        expect_reply(stream, b"QUIT", ReplyCode.QUIT)


def main() -> None:
    """Разбирает адрес тестового listener и запускает socket acceptance."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, required=True)
    options = parser.parse_args()
    verify_policy(options.host, options.port)
    print("PASS smtp-smtputf8-recipient-policy: UTF-8 MAIL/RCPT guards over TCP")


if __name__ == "__main__":
    main()
