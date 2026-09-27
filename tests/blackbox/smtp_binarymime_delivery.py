"""Проверяет RFC 3030 BINARYMIME приём, RSET и сохранение payload через IMAP."""

import argparse
import imaplib
import smtplib
import ssl
from enum import Enum, IntEnum
from pathlib import Path


PAYLOAD = b"opaque\x00bytes\xffwith\nraw\rcharacters\r\n"
UTF8_PAYLOAD = b"utf8-header-profile\x00\xff"
MESSAGE_PREFIX = (
    b"MIME-Version: 1.0\r\n"
    b"From: Alice <alice@litterkeep.test>\r\n"
    b"To: Bob <bob@litterkeep.test>\r\n"
    b"Subject: RFC 3030 inbound BINARYMIME\r\n"
    b"Content-Type: application/octet-stream\r\n"
    b"Content-Transfer-Encoding: binary\r\n\r\n"
)


class ReplyCode(IntEnum):
    """Точные ожидаемые SMTP коды профиля BINARYMIME."""

    ACCEPTED = 250
    BAD_SEQUENCE = 503
    INVALID_CONTENT = 552


class ImapStatus(Enum):
    """Единственный статус успешной IMAP операции в этом тесте."""

    OK = "OK"


class SmtpCapability(Enum):
    """Расширения EHLO, необходимые разным частям данного acceptance."""

    CHUNKING = "chunking"
    BINARYMIME = "binarymime"
    EIGHT_BIT_MIME = "8bitmime"
    SMTP_UTF8 = "smtputf8"


def arguments() -> argparse.Namespace:
    """Читает listener endpoints и локальные acceptance credentials."""
    parser = argparse.ArgumentParser()
    parser.add_argument("host")
    parser.add_argument("smtp_port", type=int)
    parser.add_argument("imap_port", type=int)
    parser.add_argument("ca_file", type=Path)
    parser.add_argument("credentials", type=Path)
    return parser.parse_args()


def password(path: Path, account: str) -> str:
    """Находит пароль без вывода секретов в acceptance output."""
    for line in path.read_text(encoding="utf-8").splitlines():
        match line.split("\t"):
            case [name, secret] if name == account:
                return secret
    raise AssertionError(f"acceptance credentials omit {account}")


def require_reply(client: smtplib.SMTP, expected: ReplyCode, phase: str) -> bytes:
    """Проверяет код и diagnostic text, не скрывая wire mismatch."""
    code, response = client.getreply()
    if code != expected:
        raise AssertionError(f"{phase}: SMTP {code} {response!r}")
    return response


def check_data_is_forbidden(client: smtplib.SMTP) -> None:
    """RFC 3030 запрещает DATA для BODY=BINARYMIME до обязательного RSET."""
    client.putcmd("MAIL", "FROM:<alice@litterkeep.test> BODY=BINARYMIME")
    require_reply(client, ReplyCode.ACCEPTED, "binary MAIL")
    client.putcmd("DATA")
    require_reply(client, ReplyCode.BAD_SEQUENCE, "DATA after BINARYMIME MAIL")
    code, response = client.rset()
    if code != ReplyCode.ACCEPTED:
        raise AssertionError(f"RSET after forbidden DATA: SMTP {code} {response!r}")


def check_noncanonical_text_is_rejected(client: smtplib.SMTP) -> None:
    """Отклоняет bare LF внутри text/* и восстанавливает session через RSET."""
    invalid = (
        b"Content-Type: text/plain\r\n"
        b"Content-Transfer-Encoding: binary\r\n\r\nline one\nline two\r\n"
    )
    client.putcmd("MAIL", "FROM:<alice@litterkeep.test> BODY=BINARYMIME")
    require_reply(client, ReplyCode.ACCEPTED, "invalid-text MAIL")
    client.putcmd("RCPT", "TO:<bob@litterkeep.test>")
    require_reply(client, ReplyCode.ACCEPTED, "invalid-text RCPT")
    client.putcmd("BDAT", f"{len(invalid)} LAST")
    client.send(invalid)
    require_reply(client, ReplyCode.INVALID_CONTENT, "noncanonical text body")
    code, response = client.rset()
    if code != ReplyCode.ACCEPTED:
        raise AssertionError(f"RSET after invalid text: SMTP {code} {response!r}")


def transmit_binary_message(client: smtplib.SMTP) -> bytes:
    """Отправляет opaque payload точным octet-counted BDAT LAST."""
    message = MESSAGE_PREFIX + PAYLOAD
    client.putcmd("MAIL", "FROM:<alice@litterkeep.test> BODY=BINARYMIME")
    require_reply(client, ReplyCode.ACCEPTED, "BINARYMIME MAIL")
    client.putcmd("RCPT", "TO:<bob@litterkeep.test>")
    require_reply(client, ReplyCode.ACCEPTED, "RCPT")
    client.putcmd("BDAT", f"{len(message)} LAST")
    client.send(message)
    require_reply(client, ReplyCode.ACCEPTED, "BDAT LAST")
    return message


def transmit_utf8_header_message(client: smtplib.SMTP) -> bytes:
    """Проверяет MAIL-параметр SMTPUTF8 в сочетании с BINARYMIME."""
    message = (
        "Subject: Привет\r\n"
        "Content-Type: application/octet-stream\r\n"
        "Content-Transfer-Encoding: binary\r\n\r\n"
    ).encode("utf-8") + UTF8_PAYLOAD

    client.putcmd("MAIL", "FROM:<alice@litterkeep.test> BODY=BINARYMIME")
    require_reply(client, ReplyCode.ACCEPTED, "UTF-8 BINARYMIME MAIL without SMTPUTF8 parameter")
    client.putcmd("RCPT", "TO:<bob@litterkeep.test>")
    require_reply(client, ReplyCode.ACCEPTED, "UTF-8 BINARYMIME RCPT without SMTPUTF8 parameter")
    client.putcmd("BDAT", f"{len(message)} LAST")
    client.send(message)
    diagnostic = require_reply(
        client, ReplyCode.INVALID_CONTENT, "UTF-8 headers without SMTPUTF8 parameter"
    )
    if b"negotiated SMTP body profile" not in diagnostic:
        raise AssertionError(f"wrong SMTPUTF8 rejection diagnostic: {diagnostic!r}")
    code, response = client.rset()
    if code != ReplyCode.ACCEPTED:
        raise AssertionError(f"RSET after SMTPUTF8 rejection: SMTP {code} {response!r}")

    client.putcmd("MAIL", "FROM:<alice@litterkeep.test> BODY=BINARYMIME SMTPUTF8")
    require_reply(client, ReplyCode.ACCEPTED, "SMTPUTF8 BINARYMIME MAIL")
    client.putcmd("RCPT", "TO:<bob@litterkeep.test>")
    require_reply(client, ReplyCode.ACCEPTED, "SMTPUTF8 RCPT")
    client.putcmd("BDAT", f"{len(message)} LAST")
    client.send(message)
    require_reply(client, ReplyCode.ACCEPTED, "SMTPUTF8 BINARYMIME LAST")
    return message


def stored_message(
    host: str,
    port: int,
    context: ssl.SSLContext,
    secret: str,
    expected_payload: bytes,
) -> bytes:
    """Находит raw RFC822 literal в Bob INBOX через независимый IMAP peer."""
    with imaplib.IMAP4_SSL(host, port, ssl_context=context, timeout=15) as client:
        client.login("bob", secret)
        status, _ = client.select("INBOX", readonly=True)
        if ImapStatus(status) is not ImapStatus.OK:
            raise AssertionError(f"cannot select INBOX: {status}")
        status, response = client.search(None, "ALL")
        if ImapStatus(status) is not ImapStatus.OK:
            raise AssertionError(f"cannot search INBOX: {status}")
        match response:
            case [bytes() as raw_identifiers]:
                identifiers = raw_identifiers.split()
            case _:
                raise AssertionError("IMAP SEARCH returned malformed identifiers")
        for identifier in identifiers:
            status, fetched = client.fetch(identifier, "(RFC822)")
            if ImapStatus(status) is not ImapStatus.OK:
                continue
            if fetched is None:
                continue
            for item in fetched:
                match item:
                    case (_, bytes() as raw):
                        if raw.endswith(expected_payload):
                            return raw
                    case _:
                        continue
    raise AssertionError("stored BINARYMIME payload was not found via IMAP")


def main() -> None:
    """Сверяет negotiated extension, DATA refusal и долговечные payload bytes."""
    options = arguments()
    context = ssl.create_default_context(cafile=str(options.ca_file))
    alice = password(options.credentials, "alice")
    bob = password(options.credentials, "bob")
    with smtplib.SMTP_SSL(options.host, options.smtp_port, context=context, timeout=20) as client:
        code, response = client.ehlo()
        if code != ReplyCode.ACCEPTED:
            raise AssertionError(f"EHLO failed: SMTP {code} {response!r}")
        for capability in (SmtpCapability.CHUNKING, SmtpCapability.BINARYMIME):
            if not client.has_extn(capability.value):
                raise AssertionError(f"EHLO omitted {capability.name}: {client.esmtp_features!r}")
        for capability in (SmtpCapability.EIGHT_BIT_MIME, SmtpCapability.SMTP_UTF8):
            if not client.has_extn(capability.value):
                raise AssertionError(f"EHLO omitted {capability.name}: {client.esmtp_features!r}")
        client.login("alice", alice)
        check_data_is_forbidden(client)
        check_noncanonical_text_is_rejected(client)
        sent = transmit_binary_message(client)
        utf8_sent = transmit_utf8_header_message(client)
    received = stored_message(options.host, options.imap_port, context, bob, PAYLOAD)
    utf8_received = stored_message(
        options.host, options.imap_port, context, bob, UTF8_PAYLOAD
    )
    if not received.endswith(PAYLOAD):
        raise AssertionError("payload bytes changed between BDAT and IMAP storage")
    if not sent.endswith(PAYLOAD):
        raise AssertionError("test sender payload construction is inconsistent")
    if not utf8_received.endswith(UTF8_PAYLOAD):
        raise AssertionError("received UTF-8 header BINARYMIME payload changed")
    if not utf8_sent.endswith(UTF8_PAYLOAD):
        raise AssertionError("UTF-8 header BINARYMIME payload changed during delivery")
    print("Inbound SMTP BINARYMIME -> IMAP exact payload and SMTPUTF8 guard passed")


if __name__ == "__main__":
    main()
