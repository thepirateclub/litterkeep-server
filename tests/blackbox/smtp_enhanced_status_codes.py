"""Проверяет объявление и применение SMTP enhanced status codes RFC 2034."""

import argparse
import base64
from enum import Enum
from pathlib import Path
import re
import socket
import ssl


class ReplyCode(Enum):
    """Типизированные числовые коды ответов тестируемого сценария."""

    GREETING = 220
    OK = 250
    STARTTLS = 220
    AUTHENTICATED = 235
    HELP = 214
    BAD_COMMAND = 500
    UNKNOWN_RECIPIENT = 550
    TEMPORARY_FAILURE = 451
    DATA = 354
    QUEUED = 250
    QUIT = 221


REPLY_PATTERN = re.compile(rb"\A([0-9]{3})([- ])")
ENHANCED_PATTERN = re.compile(
    rb"\A([245])[0-9]{2}[- ]([245]\.[0-9]{1,3}\.[0-9]{1,3}) "
)
TLS_CONTEXT = ssl.create_default_context()
TLS_CONTEXT.check_hostname = False
TLS_CONTEXT.verify_mode = ssl.CERT_NONE


def read_reply(stream):
    """Читает многострочный ответ и требует единый основной код RFC 5321."""
    lines = []
    status = None
    separator = None
    while separator != b" ":
        line = stream.readline()
        match = REPLY_PATTERN.match(line)
        if match is None:
            raise AssertionError(f"malformed SMTP response: {line!r}")
        current_status, separator = match.groups()
        if status is None:
            status = current_status
        if current_status != status:
            raise AssertionError(f"mixed SMTP response codes: {lines!r} {line!r}")
        lines.append(line)
    return int(status), tuple(lines)


def expect_reply(stream, expected, context):
    """Проверяет основной код и возвращает все строки ответа."""
    actual, lines = read_reply(stream)
    if actual != expected.value:
        raise AssertionError(f"{context}: expected {expected.value}, received {lines!r}")
    return lines


def expect_enhanced_reply(stream, expected, context):
    """Требует enhanced status code на каждой строке обычного ответа."""
    lines = expect_reply(stream, expected, context)
    response_status = None
    for line in lines:
        match = ENHANCED_PATTERN.match(line)
        if match is None:
            raise AssertionError(f"{context}: enhanced code absent: {line!r}")
        primary_class, enhanced_status = match.groups()
        enhanced_class = enhanced_status.split(b".", 1)[0]
        expected_class = str(expected.value // 100).encode("ascii")
        if primary_class != enhanced_class or primary_class != expected_class:
            raise AssertionError(f"{context}: status classes disagree: {line!r}")
        if response_status is None:
            response_status = enhanced_status
        if enhanced_status != response_status:
            raise AssertionError(f"{context}: multiline status codes disagree: {lines!r}")


def require_capability(lines):
    """Проверяет EHLO keyword как отдельный атом, а не подстроку."""
    capabilities = {
        line[4:].split(b" ", 1)[0].strip().upper()
        for line in lines
        if line.startswith((b"250-", b"250 "))
    }
    if b"ENHANCEDSTATUSCODES" not in capabilities:
        raise AssertionError(f"EHLO did not advertise the extension: {capabilities!r}")


def password_from(path, username):
    """Получает bootstrap пароль, не выводя секрет в stdout или ошибку."""
    for line in Path(path).read_text(encoding="utf-8").splitlines():
        match line.split("\t"):
            case [account, password] if account == username:
                return password
            case _:
                continue
    raise ValueError(f"acceptance account is absent: {username}")


def establish_tls(host, port):
    """Проходит greeting, команды до TLS и повторный EHLO в TLS-состоянии."""
    connection = socket.create_connection((host, port), timeout=30)
    stream = connection.makefile("rwb", buffering=0)
    expect_reply(stream, ReplyCode.GREETING, "initial greeting exception")
    stream.write(b"HELO enhanced-status.test\r\n")
    expect_reply(stream, ReplyCode.OK, "HELO exception")
    stream.write(b"UNKNOWN\r\n")
    expect_enhanced_reply(stream, ReplyCode.BAD_COMMAND, "unknown command after HELO")
    stream.write(b"NOOP\r\n")
    expect_enhanced_reply(stream, ReplyCode.OK, "NOOP after HELO")
    stream.write(b"EHLO enhanced-status.test\r\n")
    require_capability(expect_reply(stream, ReplyCode.OK, "plaintext EHLO exception"))
    stream.write(b"STARTTLS\r\n")
    expect_enhanced_reply(stream, ReplyCode.STARTTLS, "STARTTLS response")
    stream.close()
    secure = TLS_CONTEXT.wrap_socket(connection, server_hostname=host)
    secure_stream = secure.makefile("rwb", buffering=0)
    secure_stream.write(b"EHLO enhanced-status.test\r\n")
    require_capability(expect_reply(secure_stream, ReplyCode.OK, "TLS EHLO exception"))
    return secure, secure_stream


def exercise_transaction(stream, password):
    """Проверяет AUTH, HELP, ошибки envelope, DATA и подтверждение доставки."""
    credentials = base64.b64encode(b"\x00alice\x00" + password.encode("utf-8"))
    stream.write(b"AUTH PLAIN " + credentials + b"\r\n")
    expect_enhanced_reply(stream, ReplyCode.AUTHENTICATED, "AUTH PLAIN")
    stream.write(b"HELP\r\n")
    expect_enhanced_reply(stream, ReplyCode.HELP, "multiline HELP")
    stream.write(b"MAIL FROM:<alice@test>\r\n")
    expect_enhanced_reply(stream, ReplyCode.OK, "MAIL FROM")
    stream.write(b"RCPT TO:<missing@test>\r\n")
    expect_enhanced_reply(stream, ReplyCode.UNKNOWN_RECIPIENT, "unknown recipient")
    stream.write(b"RSET\r\n")
    expect_enhanced_reply(stream, ReplyCode.OK, "RSET")
    stream.write(b"MAIL FROM:<alice@test>\r\n")
    expect_enhanced_reply(stream, ReplyCode.OK, "MAIL FROM for delivery")
    stream.write(b"RCPT TO:<bob@test>\r\n")
    expect_enhanced_reply(stream, ReplyCode.OK, "valid RCPT")
    stream.write(b"DATA\r\n")
    expect_reply(stream, ReplyCode.DATA, "3xx continuation exception")
    stream.write(b"From: alice@test\r\nTo: bob@test\r\nSubject: RFC 2034\r\n\r\nprobe\r\n.\r\n")
    expect_enhanced_reply(stream, ReplyCode.QUEUED, "accepted DATA")
    stream.write(b"NOOP\r\n")
    expect_enhanced_reply(stream, ReplyCode.OK, "NOOP after delivery")
    stream.write(b"QUIT\r\n")
    expect_enhanced_reply(stream, ReplyCode.QUIT, "QUIT")


def main():
    """Запускает один независимый RFC 2034 сценарий через raw socket peer."""
    parser = argparse.ArgumentParser()
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=1587)
    parser.add_argument("--credentials", required=True)
    options = parser.parse_args()
    password = password_from(options.credentials, "alice")
    connection, stream = establish_tls(options.host, options.port)
    try:
        exercise_transaction(stream, password)
    finally:
        stream.close()
        connection.close()
    print("PASS smtp-enhanced-status-codes: EHLO, ordinary replies and multiline HELP")


if __name__ == "__main__":
    main()
