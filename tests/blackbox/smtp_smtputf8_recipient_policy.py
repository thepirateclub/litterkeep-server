"""Проверяет SMTPUTF8 mailbox policy через независимый TCP peer."""

from __future__ import annotations

import argparse
import socket
import ssl
from enum import Enum, IntEnum


class ReplyCode(IntEnum):
    """Коды ответов, различающие синтаксис, capability и route policy."""

    TLS_READY = 220
    GREETING = 220
    SUCCESS = 250
    QUIT = 221
    SYNTAX_ERROR = 501
    EXPANSION_UNSUPPORTED = 502
    VERIFICATION_UNAVAILABLE = 252
    SENDER_UTF8_NOT_PERMITTED = 550
    RCPT_UTF8_REQUIRED = 553
    UNKNOWN_LOCAL_RECIPIENT = 550

    @classmethod
    def parse(cls, value: int) -> ReplyCode | int:
        """Преобразует известный wire-код в enum, сохраняя неизвестный код."""
        try:
            return cls(value)
        except ValueError:
            return value


class EnhancedStatusCode(Enum):
    """RFC 3463 статусы, уточняющие одинаковые трёхзначные SMTP-коды."""

    SENDER_UTF8_NOT_PERMITTED = b"5.6.7"
    RECIPIENT_UTF8_NOT_PERMITTED = b"5.6.7"


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
    enhanced_status: EnhancedStatusCode | None = None,
) -> bytes:
    """Отправляет одну wire-команду и проверяет её итоговый трехзначный код."""
    stream.write(command + b"\r\n")
    stream.flush()
    actual, response = read_reply(stream)
    if actual != expected:
        raise AssertionError(
            f"{command!r}: ожидался {expected.value}, получен {actual}: {response!r}"
        )
    if enhanced_status is not None and enhanced_status.value not in response:
        raise AssertionError(
            f"{command!r}: ожидался enhanced status {enhanced_status.name}: {response!r}"
        )
    return response


def require_ascii_response(response: bytes) -> None:
    """Не разрешает ответам VRFY/EXPN раскрывать UTF-8 без нужды."""
    if any(octet > 127 for octet in response):
        raise AssertionError(f"ответ должен оставаться ASCII: {response!r}")


def verify_policy(host: str, port: int) -> None:
    """Доказывает отдельные SMTPUTF8 маркеры MAIL и RCPT на сетевом пути."""
    with socket.create_connection((host, port), timeout=10) as connection:
        stream = connection.makefile("rwb", buffering=0)
        greeting, banner = read_reply(stream)
        if greeting != ReplyCode.GREETING:
            raise AssertionError(f"неожиданный SMTP greeting: {banner!r}")

        capabilities = expect_reply(stream, b"EHLO independent-peer.test", ReplyCode.SUCCESS)
        if b"SMTPUTF8" not in capabilities.upper():
            raise AssertionError("сервер не объявил SMTPUTF8 capability")
        if b"8BITMIME" not in capabilities.upper():
            raise AssertionError("SMTPUTF8 объявлена без обязательной 8BITMIME")

        ascii_verify = expect_reply(
            stream, b"VRFY alice@example.test SMTPUTF8", ReplyCode.VERIFICATION_UNAVAILABLE
        )
        unmarked_utf8_verify = expect_reply(
            stream,
            "VRFY почта@example.test".encode("utf-8"),
            ReplyCode.VERIFICATION_UNAVAILABLE,
        )
        utf8_verify = expect_reply(
            stream,
            "VRFY почта@example.test SMTPUTF8".encode("utf-8"),
            ReplyCode.VERIFICATION_UNAVAILABLE,
        )
        utf8_expn = expect_reply(
            stream,
            "EXPN список@example.test SMTPUTF8".encode("utf-8"),
            ReplyCode.EXPANSION_UNSUPPORTED,
        )
        for response in (ascii_verify, unmarked_utf8_verify, utf8_verify, utf8_expn):
            require_ascii_response(response)
        expect_reply(
            stream,
            b"VRFY alice@example.test SMTPUTF8=YES",
            ReplyCode.SYNTAX_ERROR,
        )
        expect_reply(stream, b"NOOP", ReplyCode.SUCCESS)

        expect_reply(stream, b"MAIL FROM:<alice@test>", ReplyCode.SUCCESS)
        expect_reply(
            stream,
            "RCPT TO:<получатель@litterkeep.test>".encode("utf-8"),
            ReplyCode.RCPT_UTF8_REQUIRED,
            EnhancedStatusCode.RECIPIENT_UTF8_NOT_PERMITTED,
        )
        expect_reply(stream, b"RSET", ReplyCode.SUCCESS)

        expect_reply(
            stream,
            "MAIL FROM:<alice@bücher.test>".encode("utf-8"),
            ReplyCode.SENDER_UTF8_NOT_PERMITTED,
            EnhancedStatusCode.SENDER_UTF8_NOT_PERMITTED,
        )
        expect_reply(stream, b"MAIL FROM:<alice@test>", ReplyCode.SUCCESS)
        expect_reply(
            stream,
            "RCPT TO:<bob@bücher.test>".encode("utf-8"),
            ReplyCode.RCPT_UTF8_REQUIRED,
            EnhancedStatusCode.RECIPIENT_UTF8_NOT_PERMITTED,
        )
        expect_reply(stream, b"RSET", ReplyCode.SUCCESS)

        expect_reply(
            stream,
            b"MAIL FROM:<alice@test> SMTPUTF8",
            ReplyCode.SUCCESS,
        )
        expect_reply(
            stream,
            "RCPT TO:<bob@bücher.test>".encode("utf-8"),
            ReplyCode.SUCCESS,
        )
        expect_reply(stream, b"RSET", ReplyCode.SUCCESS)

        expect_reply(
            stream,
            b"MAIL FROM:<alice@test> SMTPUTF8",
            ReplyCode.SUCCESS,
        )
        expect_reply(stream, b"RSET", ReplyCode.SUCCESS)
        expect_reply(stream, b"MAIL FROM:<alice@test>", ReplyCode.SUCCESS)
        expect_reply(
            stream,
            "RCPT TO:<bob@bücher.test>".encode("utf-8"),
            ReplyCode.RCPT_UTF8_REQUIRED,
            EnhancedStatusCode.RECIPIENT_UTF8_NOT_PERMITTED,
        )
        expect_reply(stream, b"RSET", ReplyCode.SUCCESS)

        expect_reply(
            stream,
            "MAIL FROM:<алиса@litterkeep.test>".encode("utf-8"),
            ReplyCode.SENDER_UTF8_NOT_PERMITTED,
            EnhancedStatusCode.SENDER_UTF8_NOT_PERMITTED,
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


def verify_unauthenticated_rset(host: str, port: int) -> None:
    """Проверяет RSET сразу после TLS до EHLO и AUTH пользователя."""
    with socket.create_connection((host, port), timeout=10) as connection:
        stream = connection.makefile("rwb", buffering=0)
        greeting, banner = read_reply(stream)
        if greeting != ReplyCode.GREETING:
            raise AssertionError(f"неожиданный SMTP greeting: {banner!r}")
        capabilities = expect_reply(
            stream, b"EHLO independent-peer.test", ReplyCode.SUCCESS
        )
        if b"STARTTLS" not in capabilities.upper():
            raise AssertionError("listener не объявил STARTTLS")
        expect_reply(stream, b"STARTTLS", ReplyCode.TLS_READY)

        context = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
        context.check_hostname = False
        context.verify_mode = ssl.CERT_NONE
        with context.wrap_socket(connection, server_hostname=host) as tls_connection:
            tls_stream = tls_connection.makefile("rwb", buffering=0)
            expect_reply(tls_stream, b"RSET", ReplyCode.SUCCESS)
            expect_reply(
                tls_stream, b"EHLO independent-peer.test", ReplyCode.SUCCESS
            )
            expect_reply(tls_stream, b"NOOP", ReplyCode.SUCCESS)
            expect_reply(tls_stream, b"QUIT", ReplyCode.QUIT)


def main() -> None:
    """Разбирает адрес тестового listener и запускает socket acceptance."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, required=True)
    options = parser.parse_args()
    verify_policy(options.host, options.port)
    verify_unauthenticated_rset(options.host, options.port)
    print("PASS smtp-policy: UTF-8 guards and unauthenticated RSET over TCP/TLS")


if __name__ == "__main__":
    main()
