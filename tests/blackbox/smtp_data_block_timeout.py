"""Проверяет реальное RFC 5321 трёхминутное DATA block timeout."""

import argparse
from enum import Enum, IntEnum
from pathlib import Path
import smtplib
import ssl
import time


class ReplyCode(IntEnum):
    """Ожидаемые ответы до начала DATA body."""

    GREETING = 220
    OK = 250
    STARTTLS = 220
    AUTHENTICATED = 235
    DATA = 354


class DisconnectKind(Enum):
    """Допустимые способы закрытия TCP/TLS после истечения срока."""

    TLS_CLOSE_NOTIFY = "TLS close_notify"
    TLS_EOF = "TLS EOF"
    TCP_EOF = "TCP EOF"
    TCP_RESET = "TCP reset"


def arguments() -> argparse.Namespace:
    """Читает STARTTLS endpoint и локальные тестовые credentials."""
    parser = argparse.ArgumentParser()
    parser.add_argument("host")
    parser.add_argument("smtp_port", type=int)
    parser.add_argument("ca_file", type=Path)
    parser.add_argument("credentials", type=Path)
    return parser.parse_args()


def password(path: Path) -> str:
    """Читает пароль Alice, не печатая секрет в acceptance output."""
    for line in path.read_text(encoding="utf-8").splitlines():
        match line.split("\t"):
            case [account, secret] if account == "alice":
                return secret
    raise AssertionError("acceptance credentials omit alice")


def require_code(actual: int, expected: ReplyCode, phase: str, response: bytes) -> None:
    """Сверяет SMTP numeric reply и сохраняет diagnostic при отказе."""
    if actual != expected:
        raise AssertionError(f"{phase}: SMTP {actual} {response!r}")


def start_incomplete_data(
    client: smtplib.SMTP,
    context: ssl.SSLContext,
    secret: str,
) -> None:
    """Аутентифицирует Alice и оставляет SMTP сессию внутри DATA body."""
    code, response = client.ehlo()
    require_code(code, ReplyCode.OK, "EHLO", response)
    code, response = client.starttls(context=context)
    require_code(code, ReplyCode.STARTTLS, "STARTTLS", response)
    code, response = client.ehlo()
    require_code(code, ReplyCode.OK, "EHLO after TLS", response)
    code, response = client.login("alice", secret)
    require_code(code, ReplyCode.AUTHENTICATED, "AUTH", response)
    code, response = client.mail("alice@litterkeep.test")
    require_code(code, ReplyCode.OK, "MAIL", response)
    code, response = client.rcpt("bob@litterkeep.test")
    require_code(code, ReplyCode.OK, "RCPT", response)
    code, response = client.docmd("DATA")
    require_code(code, ReplyCode.DATA, "DATA", response)
    client.send(b"Subject: incomplete timeout probe\r\n\r\npartial-block")


def observe_disconnect(client: smtplib.SMTP) -> DisconnectKind:
    """Дожидается закрытия уже согласованной TLS SMTP сессии."""
    if client.sock is None:
        raise AssertionError("SMTP socket vanished before the timeout observation")
    try:
        received = client.sock.recv(1)
    except ssl.SSLZeroReturnError:
        return DisconnectKind.TLS_CLOSE_NOTIFY
    except ssl.SSLEOFError:
        return DisconnectKind.TLS_EOF
    except ConnectionResetError:
        return DisconnectKind.TCP_RESET
    match received:
        case b"":
            return DisconnectKind.TCP_EOF
        case _:
            raise AssertionError(f"unexpected SMTP bytes while DATA was incomplete: {received!r}")


def main() -> None:
    """Измеряет idle body close и требует интервал около 180 секунд."""
    options = arguments()
    context = ssl.create_default_context(cafile=str(options.ca_file))
    client = smtplib.SMTP(options.host, options.smtp_port, timeout=210)
    try:
        start_incomplete_data(client, context, password(options.credentials))
        if client.sock is None:
            raise AssertionError("SMTP socket is absent after DATA 354")
        client.sock.settimeout(205)
        started = time.monotonic()
        reason = observe_disconnect(client)
        elapsed = time.monotonic() - started
    finally:
        client.close()
    match elapsed:
        case value if value < 165:
            raise AssertionError(f"DATA block timeout fired too early: {value:.1f}s")
        case value if value > 205:
            raise AssertionError(f"DATA block timeout fired too late: {value:.1f}s")
        case _:
            pass
    print(f"PASS SMTP DATA timeout: {elapsed:.1f}s, closure={reason.value}")


if __name__ == "__main__":
    main()
