"""Проверяет перезапуск DATA block timeout новым socket read."""

import argparse
from enum import Enum, IntEnum
from pathlib import Path
import smtplib
import ssl
import time


class ReplyCode(IntEnum):
    """Ожидаемые ответы SMTP до инициализации DATA body."""

    GREETING = 220
    OK = 250
    STARTTLS = 220
    AUTHENTICATED = 235
    DATA = 354


class DisconnectKind(Enum):
    """Допустимые transport closure после истечения DATA срока."""

    TLS_CLOSE_NOTIFY = "TLS close_notify"
    TLS_EOF = "TLS EOF"
    TCP_EOF = "TCP EOF"
    TCP_RESET = "TCP reset"


def arguments() -> argparse.Namespace:
    """Читает listener endpoint и локальные acceptance credentials."""
    parser = argparse.ArgumentParser()
    parser.add_argument("host")
    parser.add_argument("smtp_port", type=int)
    parser.add_argument("ca_file", type=Path)
    parser.add_argument("credentials", type=Path)
    return parser.parse_args()


def password(path: Path) -> str:
    """Читает пароль Alice, не раскрывая его в stdout."""
    for line in path.read_text(encoding="utf-8").splitlines():
        match line.split("\t"):
            case [account, secret] if account == "alice":
                return secret
    raise AssertionError("acceptance credentials omit alice")


def require_code(actual: int, expected: ReplyCode, phase: str, response: bytes) -> None:
    """Сверяет numeric reply и сохраняет diagnostic при ошибке."""
    if actual != expected:
        raise AssertionError(f"{phase}: SMTP {actual} {response!r}")


def begin_incomplete_data(
    client: smtplib.SMTP,
    context: ssl.SSLContext,
    secret: str,
) -> None:
    """Аутентифицирует Alice, начинает DATA и передаёт первый частичный блок."""
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
    client.send(b"Subject: timeout refresh probe\r\n\r\nfirst-block")


def observe_disconnect(client: smtplib.SMTP) -> DisconnectKind:
    """Ожидает серверное закрытие после второго, свежего body блока."""
    if client.sock is None:
        raise AssertionError("SMTP socket vanished before second DATA block")
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
            raise AssertionError(f"unexpected SMTP bytes: {received!r}")


def main() -> None:
    """Сравнивает deadline после нового TCP read с тремя минутами."""
    options = arguments()
    context = ssl.create_default_context(cafile=str(options.ca_file))
    client = smtplib.SMTP(options.host, options.smtp_port, timeout=205)
    try:
        begin_incomplete_data(client, context, password(options.credentials))
        time.sleep(120)
        if client.sock is None:
            raise AssertionError("SMTP socket is absent before second DATA block")
        started = time.monotonic()
        client.sock.sendall(b"second-block")
        client.sock.settimeout(205)
        reason = observe_disconnect(client)
        elapsed = time.monotonic() - started
    finally:
        client.close()
    match elapsed:
        case value if value < 165:
            raise AssertionError(f"fresh DATA block did not reset timeout: {value:.1f}s")
        case value if value > 205:
            raise AssertionError(f"refreshed DATA timeout fired too late: {value:.1f}s")
        case _:
            pass
    print(f"PASS SMTP DATA timeout refresh: {elapsed:.1f}s, closure={reason.value}")


if __name__ == "__main__":
    main()
