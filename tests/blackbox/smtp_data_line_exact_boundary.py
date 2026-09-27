"""Проверяет RFC 5321 DATA line limit на точной STARTTLS wire-границе."""

import argparse
from enum import IntEnum
from pathlib import Path
import smtplib
import ssl


class ReplyCode(IntEnum):
    """Коды успешной передачи и отказа для граничных DATA-сообщений."""

    GREETING = 220
    OK = 250
    STARTTLS = 220
    AUTHENTICATED = 235
    DATA = 354
    TOO_LARGE = 552


def arguments() -> argparse.Namespace:
    """Читает listener endpoints и локальный acceptance credential store."""
    parser = argparse.ArgumentParser()
    parser.add_argument("host")
    parser.add_argument("smtp_port", type=int)
    parser.add_argument("ca_file", type=Path)
    parser.add_argument("credentials", type=Path)
    return parser.parse_args()


def password(path: Path) -> str:
    """Читает пароль Alice, не выводя его в результат acceptance."""
    for line in path.read_text(encoding="utf-8").splitlines():
        match line.split("\t"):
            case [account, secret] if account == "alice":
                return secret
    raise AssertionError("acceptance credentials omit alice")


def require_code(actual: int, expected: ReplyCode, phase: str, response: bytes) -> None:
    """Сверяет numeric reply и оставляет диагностический текст видимым."""
    if actual != expected:
        raise AssertionError(f"{phase}: SMTP {actual} {response!r}")


def begin_data(client: smtplib.SMTP, subject: str) -> None:
    """Открывает DATA transaction с отдельной уникальной темой."""
    code, response = client.mail("alice@litterkeep.test")
    require_code(code, ReplyCode.OK, "MAIL", response)
    code, response = client.rcpt("bob@litterkeep.test")
    require_code(code, ReplyCode.OK, "RCPT", response)
    code, response = client.docmd("DATA")
    require_code(code, ReplyCode.DATA, subject, response)


def send_message(client: smtplib.SMTP, subject: str, body_line: bytes) -> tuple[int, bytes]:
    """Передаёт raw DATA bytes, не меняя длину строки SMTP-клиентом."""
    begin_data(client, subject)
    wire_message = (
        f"Subject: {subject}\r\n\r\n".encode("ascii") + body_line + b".\r\n"
    )
    client.send(wire_message)
    return client.getreply()


def main() -> None:
    """Принимает 1000 octets и отвергает 1001, включая CRLF в обоих случаях."""
    options = arguments()
    context = ssl.create_default_context(cafile=str(options.ca_file))
    with smtplib.SMTP(options.host, options.smtp_port, timeout=20) as client:
        code, response = client.ehlo()
        require_code(code, ReplyCode.OK, "EHLO", response)
        code, response = client.starttls(context=context)
        require_code(code, ReplyCode.STARTTLS, "STARTTLS", response)
        code, response = client.ehlo()
        require_code(code, ReplyCode.OK, "EHLO after TLS", response)
        code, response = client.login("alice", password(options.credentials))
        require_code(code, ReplyCode.AUTHENTICATED, "AUTH", response)

        accepted_line = b".." + b"x" * 997 + b"\r\n"
        code, response = send_message(client, "line-1000-dot-stuffed", accepted_line)
        require_code(code, ReplyCode.OK, "1000-octet unstuffed line", response)

        rejected_line = b"x" * 999 + b"\r\n"
        code, response = send_message(client, "line-1001", rejected_line)
        require_code(code, ReplyCode.TOO_LARGE, "1001-octet DATA line", response)
        code, response = client.noop()
        require_code(code, ReplyCode.OK, "NOOP after overlong line", response)

    print("PASS SMTP DATA line octet boundary: 1000 accepted, 1001 rejected")


if __name__ == "__main__":
    main()
