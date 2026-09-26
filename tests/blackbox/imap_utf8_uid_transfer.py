"""Проверяет UID COPY/MOVE с UTF-8 mailbox names через независимый IMAP-клиент."""

import argparse
import imaplib
import ssl
from pathlib import Path


def arguments() -> argparse.Namespace:
    """Читает endpoint и тестовые учётные данные из локальных fixtures."""
    parser = argparse.ArgumentParser()
    parser.add_argument("--port", type=int, required=True)
    parser.add_argument("--ca", required=True)
    parser.add_argument("--credentials", required=True)
    return parser.parse_args()


def account_password(path: Path, account: str) -> str:
    """Извлекает пароль тестовой учётной записи из TSV без вывода секрета."""
    for line in path.read_text(encoding="utf-8").splitlines():
        identity, separator, password = line.partition("\t")
        if separator and identity == account and password:
            return password
    raise AssertionError(f"acceptance credential for {account} is missing")


def require_ok(status: str, response: object, operation: str) -> object:
    """Останавливает acceptance при любом ответе, отличном от tagged OK."""
    if status != "OK":
        raise AssertionError(f"{operation} failed: {status!r} {response!r}")
    return response


def enable_utf8(client: imaplib.IMAP4) -> None:
    """Включает RFC 9755 режим и настраивает UTF-8 у независимого клиента."""
    status, _ = client._simple_command("ENABLE", "UTF8=ACCEPT")
    response_type, response = client.response("ENABLED")
    if status != "OK" or response_type != "ENABLED" or not any(
        b"UTF8=ACCEPT" in item for item in response
    ):
        raise AssertionError(f"ENABLE failed: {status!r} {response!r}")
    client._encoding = "utf-8"


def append_source(client: imaplib.IMAP4, source: str, message: bytes) -> None:
    """Создаёт Unicode source mailbox и публикует один тестовый RFC822 literal."""
    require_ok(*client.create(source), "CREATE source")
    require_ok(*client.append(source, None, None, message), "APPEND source")
    require_ok(*client.select(source), "SELECT source")
    require_message_count(client, b"1", "source before transfer")


def require_message_count(client: imaplib.IMAP4, expected: bytes, mailbox: str) -> None:
    """Сверяет EXISTS после SELECT выбранного mailbox."""
    status, counts = client.response("EXISTS")
    if status != "EXISTS" or counts != [expected]:
        raise AssertionError(f"{mailbox}: expected {expected!r}, got {status!r} {counts!r}")


def uid_transfer(
    client: imaplib.IMAP4,
    verb: str,
    mailbox: str,
) -> None:
    """Выполняет UID-команду с quoted UTF-8 destination и требует COPYUID."""
    status, response = client._simple_command("UID", verb, "1", f'"{mailbox}"')
    require_ok(status, response, f"UID {verb} {mailbox}")
    if not any(b"COPYUID" in item for item in response):
        raise AssertionError(f"UID {verb} omitted COPYUID: {response!r}")


def require_exact_message(client: imaplib.IMAP4, expected: bytes, mailbox: str) -> None:
    """Подтверждает точное сохранение всех RFC822 octets после transfer."""
    status, response = client.fetch("1", "(RFC822)")
    require_ok(status, response, f"FETCH {mailbox}")
    literals = [
        item
        for response_item in response
        if isinstance(response_item, tuple)
        for item in response_item
        if isinstance(item, bytes) and item.startswith(b"From:")
    ]
    if literals != [expected]:
        raise AssertionError(f"{mailbox}: transferred RFC822 octets differ")


def main() -> None:
    """Проверяет UID COPY и UID MOVE на реальном enabled IMAP соединении."""
    options = arguments()
    password = account_password(Path(options.credentials), "bob")
    context = ssl.create_default_context(cafile=options.ca)
    client = imaplib.IMAP4("127.0.0.1", options.port, timeout=10)
    client.starttls(ssl_context=context)
    require_ok(*client.login("bob", password), "LOGIN")
    enable_utf8(client)

    source = "UID исходник"
    copied = "UID копия"
    moved = "UID перемещение"
    message = "From: bob@litterkeep.test\r\nSubject: UID transfer\r\n\r\nbody\x00bytes".encode()
    append_source(client, source, message)
    require_ok(*client.create(copied), "CREATE copy destination")
    require_ok(*client.create(moved), "CREATE move destination")

    uid_transfer(client, "COPY", copied)
    require_ok(*client.select(copied), "SELECT copy destination")
    require_message_count(client, b"1", "UID COPY destination")
    require_exact_message(client, message, "UID COPY destination")

    require_ok(*client.select(source), "SELECT source before UID MOVE")
    uid_transfer(client, "MOVE", moved)
    require_ok(*client.select(source), "RESELECT source after UID MOVE")
    require_message_count(client, b"0", "source after UID MOVE")
    require_ok(*client.select(moved), "SELECT UID MOVE destination")
    require_message_count(client, b"1", "UID MOVE destination")
    require_exact_message(client, message, "UID MOVE destination")
    client.logout()
    print("RFC 9755 UID COPY/MOVE passed: Unicode destinations, COPYUID and exact RFC822 bytes")


if __name__ == "__main__":
    main()
