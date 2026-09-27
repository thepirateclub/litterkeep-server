"""Проверяет фактический RFC 5321 пятиминутный SMTP command timeout."""

import argparse
from enum import Enum, IntEnum
from pathlib import Path
import smtplib
import ssl
import time


class ReplyCode(IntEnum):
    """Ожидаемые ответы SMTP до начала idle периода."""

    GREETING = 220
    OK = 250
    STARTTLS = 220
    AUTHENTICATED = 235


class DisconnectKind(Enum):
    """Способы закрытия transport-а, подтверждающие timeout session-а."""

    TLS_CLOSE_NOTIFY = "TLS close_notify"
    TLS_EOF = "TLS EOF"
    TCP_EOF = "TCP EOF"
    TCP_RESET = "TCP reset"


def arguments() -> argparse.Namespace:
    """Читает STARTTLS endpoint и локальный acceptance credential store."""
    parser = argparse.ArgumentParser()
    parser.add_argument("host")
    parser.add_argument("smtp_port", type=int)
    parser.add_argument("ca_file", type=Path)
    parser.add_argument("credentials", type=Path)
    return parser.parse_args()


def password(path: Path) -> str:
    """Читает пароль Alice, не печатая его в отчёт acceptance."""
    for line in path.read_text(encoding="utf-8").splitlines():
        match line.split("\t"):
            case [account, secret] if account == "alice":
                return secret
    raise AssertionError("acceptance credentials omit alice")


def require_code(actual: int, expected: ReplyCode, phase: str, response: bytes) -> None:
    """Проверяет numeric reply и сохраняет diagnostic при mismatch."""
    if actual != expected:
        raise AssertionError(f"{phase}: SMTP {actual} {response!r}")


def authenticate(client: smtplib.SMTP, context: ssl.SSLContext, secret: str) -> None:
    """Устанавливает TLS SMTP session и оставляет её в command state."""
    code, response = client.ehlo()
    require_code(code, ReplyCode.OK, "EHLO", response)
    code, response = client.starttls(context=context)
    require_code(code, ReplyCode.STARTTLS, "STARTTLS", response)
    code, response = client.ehlo()
    require_code(code, ReplyCode.OK, "EHLO after TLS", response)
    code, response = client.login("alice", secret)
    require_code(code, ReplyCode.AUTHENTICATED, "AUTH", response)


def observe_disconnect(client: smtplib.SMTP) -> DisconnectKind:
    """Блокирует на сокете до фактического закрытия idle SMTP session."""
    if client.sock is None:
        raise AssertionError("SMTP socket vanished before idle wait")
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
            raise AssertionError(f"unexpected SMTP bytes during command idle: {received!r}")


def main() -> None:
    """Измеряет командную неактивность и требует срок около 300 секунд."""
    options = arguments()
    context = ssl.create_default_context(cafile=str(options.ca_file))
    client = smtplib.SMTP(options.host, options.smtp_port, timeout=335)
    try:
        authenticate(client, context, password(options.credentials))
        if client.sock is None:
            raise AssertionError("SMTP socket is absent after AUTH")
        client.sock.settimeout(330)
        started = time.monotonic()
        reason = observe_disconnect(client)
        elapsed = time.monotonic() - started
    finally:
        client.close()
    match elapsed:
        case value if value < 285:
            raise AssertionError(f"command idle timeout fired too early: {value:.1f}s")
        case value if value > 330:
            raise AssertionError(f"command idle timeout fired too late: {value:.1f}s")
        case _:
            pass
    print(f"PASS SMTP command idle timeout: {elapsed:.1f}s, closure={reason.value}")


if __name__ == "__main__":
    main()
