"""Негативная приемка границ SMTP AUTH и submission policy."""

import argparse
import base64
from enum import IntEnum
import smtplib
import ssl


class Reply(IntEnum):
    """Ожидаемые типизированные SMTP reply codes проверяемой политики."""

    AUTHENTICATION_FAILED = 535
    ENCRYPTION_REQUIRED = 538
    AUTHENTICATION_REQUIRED = 530
    SENDER_UNAUTHORIZED = 550


def arguments() -> argparse.Namespace:
    """Читает порты отдельного TLS и plaintext listener-а."""
    parser = argparse.ArgumentParser()
    parser.add_argument("host")
    parser.add_argument("tls_port", type=int)
    parser.add_argument("plain_port", type=int)
    parser.add_argument("ca_file")
    return parser.parse_args()


def require(actual: int, expected: Reply, scenario: str) -> None:
    """Сравнивает reply с доменным enum, а не строковым статусом."""
    if actual != expected:
        raise AssertionError(f"{scenario}: expected {expected}, received {actual}")


def reject_wrong_password(
    host: str, port: int, context: ssl.SSLContext
) -> None:
    """Подтверждает отрицательный результат worker-а для неверного секрета."""
    with smtplib.SMTP_SSL(host, port, context=context, timeout=10) as client:
        try:
            client.login("alice", "wrong-password")
        except smtplib.SMTPAuthenticationError as error:
            require(error.smtp_code, Reply.AUTHENTICATION_FAILED, "wrong password")
            return
    raise AssertionError("wrong password was accepted")


def reject_unauthenticated_mail(
    host: str, port: int, context: ssl.SSLContext
) -> None:
    """Подтверждает обязательность входа до создания envelope."""
    with smtplib.SMTP_SSL(host, port, context=context, timeout=10) as client:
        client.ehlo()
        code, _ = client.mail("alice@test")
        require(code, Reply.AUTHENTICATION_REQUIRED, "MAIL before AUTH")


def reject_foreign_sender(host: str, port: int, context: ssl.SSLContext) -> None:
    """Запрещает Alice отправлять envelope от имени Bob."""
    with smtplib.SMTP_SSL(host, port, context=context, timeout=10) as client:
        client.login("alice", "alice-password")
        code, _ = client.mail("bob@test")
        require(code, Reply.SENDER_UNAUTHORIZED, "foreign sender")


def reject_plaintext_auth(host: str, port: int) -> None:
    """Проверяет отсутствие AUTH в EHLO и явный отказ при попытке команды."""
    with smtplib.SMTP(host, port, timeout=10) as client:
        client.ehlo()
        if client.has_extn("auth"):
            raise AssertionError("plaintext listener advertised AUTH")
        response = base64.b64encode(b"\x00alice\x00alice-password").decode("ascii")
        code, _ = client.docmd("AUTH", f"PLAIN {response}")
        require(code, Reply.ENCRYPTION_REQUIRED, "plaintext AUTH")


def main() -> None:
    """Выполняет четыре независимых запретительных сценария."""
    options = arguments()
    context = ssl.create_default_context(cafile=options.ca_file)
    reject_wrong_password(options.host, options.tls_port, context)
    reject_unauthenticated_mail(options.host, options.tls_port, context)
    reject_foreign_sender(options.host, options.tls_port, context)
    reject_plaintext_auth(options.host, options.plain_port)
    print("SMTP AUTH policy rejections passed")


if __name__ == "__main__":
    main()
