"""Проверяет bounded SMTP DATA input при отсутствии CRLF в длинной строке."""

import argparse
from enum import Enum
from pathlib import Path
import re
import socket
import ssl


class ReplyCode(Enum):
    """Коды ответов, необходимые этому сетевому acceptance-клиенту."""

    GREETING = 220
    OK = 250
    STARTTLS = 220
    AUTHENTICATED = 235
    DATA = 354
    TOO_LARGE = 552
    QUIT = 221


REPLY_PATTERN = re.compile(rb"\A([0-9]{3})([- ])")
MAX_MESSAGE_SIZE = 25 * 1024 * 1024
OLD_INPUT_LIMIT = MAX_MESSAGE_SIZE + 1024 * 1024
CHUNK_SIZE = 64 * 1024
TLS_CONTEXT = ssl.create_default_context()
TLS_CONTEXT.check_hostname = False
TLS_CONTEXT.verify_mode = ssl.CERT_NONE


def read_reply(stream):
    """Считывает все строки SMTP reply и проверяет общий numeric code."""
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
    return int(status), b"".join(lines)


def expect_reply(stream, expected, context):
    """Проверяет числовой SMTP-код, не зависящий от текста сервера."""
    actual, lines = read_reply(stream)
    if actual != expected.value:
        raise AssertionError(
            f"{context}: expected {expected.value}, received {lines!r}"
        )


def password_from(path, username):
    """Читает сгенерированный bootstrap секрет, не печатая его."""
    for line in Path(path).read_text(encoding="utf-8").splitlines():
        fields = line.split("\t")
        match fields:
            case [account, password] if account == username:
                return password
            case _:
                continue
    raise ValueError(f"acceptance account is absent: {username}")


def open_secure_session(host, port, password):
    """Выполняет STARTTLS и SMTP AUTH PLAIN независимым socket peer."""
    connection = socket.create_connection((host, port), timeout=30)
    stream = connection.makefile("rwb", buffering=0)
    expect_reply(stream, ReplyCode.GREETING, "SMTP greeting")
    stream.write(b"EHLO data-line-probe.test\r\n")
    expect_reply(stream, ReplyCode.OK, "pre-TLS EHLO")
    stream.write(b"STARTTLS\r\n")
    expect_reply(stream, ReplyCode.STARTTLS, "STARTTLS")
    stream.close()
    secure = TLS_CONTEXT.wrap_socket(connection, server_hostname=host)
    secure_stream = secure.makefile("rwb", buffering=0)
    secure_stream.write(b"EHLO data-line-probe.test\r\n")
    expect_reply(secure_stream, ReplyCode.OK, "post-TLS EHLO")
    import base64

    credentials = b"\x00alice\x00" + password.encode("utf-8")
    encoded = base64.b64encode(credentials)
    secure_stream.write(b"AUTH PLAIN " + encoded + b"\r\n")
    expect_reply(secure_stream, ReplyCode.AUTHENTICATED, "AUTH PLAIN")
    return secure, secure_stream


def send_unterminated_data(stream):
    """Превышает старый session cap без CRLF, затем закрывает DATA строку."""
    stream.write(b"MAIL FROM:<alice@test>\r\n")
    expect_reply(stream, ReplyCode.OK, "MAIL")
    stream.write(b"RCPT TO:<bob@test>\r\n")
    expect_reply(stream, ReplyCode.OK, "RCPT")
    stream.write(b"DATA\r\n")
    expect_reply(stream, ReplyCode.DATA, "DATA")
    remaining = OLD_INPUT_LIMIT + CHUNK_SIZE
    chunk = b"x" * CHUNK_SIZE
    while remaining > 0:
        write_size = min(remaining, len(chunk))
        stream.write(chunk[:write_size])
        remaining -= write_size
    stream.write(b"\r\n.\r\n")
    expect_reply(stream, ReplyCode.TOO_LARGE, "oversized unterminated DATA line")
    stream.write(b"NOOP\r\n")
    expect_reply(stream, ReplyCode.OK, "NOOP after DATA rejection")
    stream.write(b"QUIT\r\n")
    expect_reply(stream, ReplyCode.QUIT, "QUIT after DATA rejection")


def main():
    """Запускает один SMTP line-bound сценарий по параметрам окружения."""
    parser = argparse.ArgumentParser()
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=1587)
    parser.add_argument("--credentials", required=True)
    options = parser.parse_args()
    password = password_from(options.credentials, "alice")
    connection, stream = open_secure_session(options.host, options.port, password)
    try:
        send_unterminated_data(stream)
    finally:
        stream.close()
        connection.close()
    print("PASS smtp-data-line-bound: overlong fragment bounded; DATA and NOOP synchronized")


if __name__ == "__main__":
    main()
