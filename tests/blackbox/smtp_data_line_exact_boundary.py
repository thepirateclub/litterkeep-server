"""Проверяет RFC 5321 DATA line limit на точной STARTTLS wire-границе."""

import argparse
from enum import Enum, IntEnum
from email import policy
from email.parser import BytesParser
from pathlib import Path
import imaplib
import smtplib
import ssl
import uuid


class ReplyCode(IntEnum):
    """Коды успешной передачи и отказа для граничных DATA-сообщений."""

    GREETING = 220
    OK = 250
    STARTTLS = 220
    AUTHENTICATED = 235
    DATA = 354
    TOO_LARGE = 552


class BodyMode(Enum):
    """Профили тела MAIL для проверки ограничителя строки DATA."""

    SEVEN_BIT = ()
    EIGHT_BIT_MIME = ("BODY=8BITMIME",)


class ImapStatus(Enum):
    """Закрытый набор исходов IMAP-команд в acceptance-тесте."""

    OK = "OK"
    FAILED = "FAILED"

    @classmethod
    def parse(cls, value: str) -> "ImapStatus":
        """Преобразует статус стандартной библиотеки в типизированный исход."""
        match value:
            case "OK":
                return cls.OK
            case _:
                return cls.FAILED


def arguments() -> argparse.Namespace:
    """Читает listener и пути к тестовым сертификату и учётным данным."""
    parser = argparse.ArgumentParser()
    parser.add_argument("host")
    parser.add_argument("smtp_port", type=int)
    parser.add_argument("ca_file", type=Path)
    parser.add_argument("credentials", type=Path)
    parser.add_argument("--imap-port", type=int, default=1143)
    return parser.parse_args()


def password(path: Path, username: str) -> str:
    """Читает пароль выбранного аккаунта без раскрытия в отчёте."""
    for line in path.read_text(encoding="utf-8").splitlines():
        match line.split("\t"):
            case [account, secret] if account == username:
                return secret
    raise AssertionError(f"в acceptance credentials отсутствует {username}")


def require_code(actual: int, expected: ReplyCode, phase: str, response: bytes) -> None:
    """Сверяет код ответа и оставляет диагностический текст доступным."""
    if actual != expected:
        raise AssertionError(f"{phase}: SMTP {actual} {response!r}")


def begin_data(client: smtplib.SMTP, subject: str, mode: BodyMode) -> None:
    """Открывает DATA-транзакцию с отдельной темой письма."""
    code, response = client.mail("alice@litterkeep.test", options=mode.value)
    require_code(code, ReplyCode.OK, "MAIL", response)
    code, response = client.rcpt("bob@litterkeep.test")
    require_code(code, ReplyCode.OK, "RCPT", response)
    code, response = client.docmd("DATA")
    require_code(code, ReplyCode.DATA, subject, response)


def send_message(
    client: smtplib.SMTP, subject: str, body_line: bytes, mode: BodyMode
) -> tuple[int, bytes]:
    """Передаёт исходные DATA-октеты без изменения длины строки клиентом."""
    begin_data(client, subject, mode)
    wire_message = (
        f"Subject: {subject}\r\n\r\n".encode("ascii") + body_line + b".\r\n"
    )
    client.send(wire_message)
    return client.getreply()


def overlong_body_line(mode: BodyMode) -> bytes:
    """Создаёт тело из 1001 октета, корректное для выбранного BODY-профиля."""
    match mode:
        case BodyMode.SEVEN_BIT:
            return b"x" * 999 + b"\r\n"
        case BodyMode.EIGHT_BIT_MIME:
            return b"\xff" + b"x" * 998 + b"\r\n"


def require_imap(status: str, operation: str, detail: object) -> None:
    """Требует успешный IMAP-статус и сохраняет диагностический ответ."""
    if ImapStatus.parse(status) is not ImapStatus.OK:
        raise AssertionError(f"IMAP {operation} failed: {detail!r}")


def fetched_message(records: list[object]) -> bytes:
    """Извлекает RFC822 literal без позиционного доступа к ответу imaplib."""
    for record in records:
        match record:
            case (_metadata, payload) if isinstance(payload, bytes):
                return payload
            case _:
                continue
    raise AssertionError(f"FETCH response omitted RFC822 literal: {records!r}")


def verify_mailbox(
    host: str,
    port: int,
    context: ssl.SSLContext,
    bob_password: str,
    accepted: list[tuple[str, bytes]],
    rejected: list[str],
) -> None:
    """Проверяет exact body сохранённых строк и отсутствие строк > 1000."""
    mailbox = imaplib.IMAP4(host, port, timeout=20)
    mailbox.starttls(ssl_context=context)
    status, detail = mailbox.login("bob", bob_password)
    require_imap(status, "LOGIN", detail)
    try:
        status, detail = mailbox.select("INBOX", readonly=True)
        require_imap(status, "SELECT", detail)
        for subject, expected_body in accepted:
            status, records = mailbox.search(None, "SUBJECT", subject)
            require_imap(status, "accepted message SEARCH", records)
            match records:
                case [uids] if len(uids.split()) == 1:
                    status, fetched = mailbox.fetch(uids, "(RFC822)")
                case _:
                    raise AssertionError(f"expected one message for {subject}: {records!r}")
            require_imap(status, "accepted message FETCH", fetched)
            raw = fetched_message(fetched)
            parsed = BytesParser(policy=policy.default).parsebytes(raw)
            actual_body = parsed.get_payload(decode=True)
            if actual_body != expected_body:
                raise AssertionError(f"stored line octets changed for {subject}")
        for subject in rejected:
            status, records = mailbox.search(None, "SUBJECT", subject)
            require_imap(status, "rejected message SEARCH", records)
            match records:
                case [uids] if not uids.split():
                    continue
                case _:
                    raise AssertionError(f"overlong DATA line was stored: {subject}")
    finally:
        mailbox.logout()


def main() -> None:
    """Проверяет границу 1000/1001 octet для 7BIT и 8BITMIME."""
    options = arguments()
    context = ssl.create_default_context(cafile=str(options.ca_file))
    marker = uuid.uuid4().hex
    accepted_messages: list[tuple[str, bytes]] = []
    rejected_messages: list[str] = []
    with smtplib.SMTP(options.host, options.smtp_port, timeout=20) as client:
        code, response = client.ehlo()
        require_code(code, ReplyCode.OK, "EHLO", response)
        code, response = client.starttls(context=context)
        require_code(code, ReplyCode.STARTTLS, "STARTTLS", response)
        code, response = client.ehlo()
        require_code(code, ReplyCode.OK, "EHLO after TLS", response)
        code, response = client.login("alice", password(options.credentials, "alice"))
        require_code(code, ReplyCode.AUTHENTICATED, "AUTH", response)

        seven_subject = f"line-1000-seven-bit-{marker}"
        accepted_seven_bit = b".." + b"x" * 997 + b"\r\n"
        code, response = send_message(
            client, seven_subject, accepted_seven_bit, BodyMode.SEVEN_BIT
        )
        require_code(code, ReplyCode.OK, "1000-octet 7BIT line", response)
        accepted_messages.append((seven_subject, b"." + b"x" * 997 + b"\r\n"))

        eight_subject = f"line-1000-eight-bit-{marker}"
        accepted_eight_bit = b".." + b"\xff" + b"x" * 996 + b"\r\n"
        code, response = send_message(
            client, eight_subject, accepted_eight_bit, BodyMode.EIGHT_BIT_MIME
        )
        require_code(code, ReplyCode.OK, "1000-octet 8BITMIME line", response)
        accepted_messages.append((eight_subject, b"." + b"\xff" + b"x" * 996 + b"\r\n"))

        for mode in BodyMode:
            rejected_line = overlong_body_line(mode)
            rejected_subject = f"line-1001-{mode.name}-{marker}"
            code, response = send_message(
                client, rejected_subject, rejected_line, mode
            )
            require_code(code, ReplyCode.TOO_LARGE, f"1001-octet {mode.name} line", response)
            rejected_messages.append(rejected_subject)
            code, response = client.noop()
            require_code(code, ReplyCode.OK, f"NOOP after {mode.name} overflow", response)

    verify_mailbox(
        options.host,
        options.imap_port,
        context,
        password(options.credentials, "bob"),
        accepted_messages,
        rejected_messages,
    )
    print("PASS SMTP DATA line bound: exact 7BIT/8BITMIME bodies and 1001-octet rejection")


if __name__ == "__main__":
    main()
