"""Проверяет RFC 9394 PARTIAL SEARCH и UID FETCH по STARTTLS."""

import argparse
from enum import Enum
import imaplib
from pathlib import Path
import re
import ssl
import uuid


class ImapStatus(Enum):
    """Допустимый успешный tagged completion для сценария."""

    OK = "OK"
    BYE = "BYE"


def arguments() -> argparse.Namespace:
    """Читает endpoint и локальный acceptance-файл с секретом Bob."""
    parser = argparse.ArgumentParser()
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, required=True)
    parser.add_argument("--ca", required=True, type=Path)
    parser.add_argument("--credentials", required=True, type=Path)
    return parser.parse_args()


def bob_password(path: Path) -> str:
    """Извлекает пароль Bob без помещения секрета в аргументы процесса."""
    for record in path.read_text(encoding="ascii").splitlines():
        account, separator, password = record.partition("\t")
        match (account == "bob", bool(separator), bool(password)):
            case (True, True, True):
                return password
            case _:
                continue
    raise AssertionError("Bob отсутствует в acceptance credentials")


def require_ok(result: tuple[str, object], operation: str) -> object:
    """Проверяет typed статус вместо принятия неизвестного ответа."""
    status, detail = result
    try:
        parsed = ImapStatus(status)
    except ValueError as error:
        raise AssertionError(f"{operation}: {status!r} {detail!r}") from error
    if parsed is not ImapStatus.OK:
        raise AssertionError(f"{operation}: {status!r} {detail!r}")
    return detail


def append_messages(client: imaplib.IMAP4, mailbox: str) -> None:
    """Создаёт пять сообщений с уникальным UID-набором для ранжирования."""
    require_ok(client.create(mailbox), "CREATE")
    for number in range(1, 6):
        message = f"Subject: partial {number}\r\n\r\nbody {number}".encode("ascii")
        require_ok(client.append(mailbox, None, None, message), "APPEND")
    require_ok(client.select(mailbox), "SELECT")


def search_partial(client: imaplib.IMAP4, mailbox: str) -> None:
    """Проверяет ESEARCH, отрицательный ранг, NIL и сохранённый UID-набор."""
    require_ok(client.uid("SEARCH", "RETURN", "(PARTIAL 2:3 COUNT)", "ALL"), "SEARCH")
    response_type, response = client.response("ESEARCH")
    wire = b" ".join(item for item in response if isinstance(item, bytes))
    if response_type != "ESEARCH" or b"PARTIAL (2:3 2:3)" not in wire:
        raise AssertionError(f"неверный ESEARCH PARTIAL: {response_type!r} {wire!r}")
    require_ok(client.uid("SEARCH", "RETURN", "(PARTIAL -1:-2)", "ALL"), "SEARCH newest")
    _, newest = client.response("ESEARCH")
    newest_wire = b" ".join(item for item in newest if isinstance(item, bytes))
    if b"PARTIAL (-1:-2 4:5)" not in newest_wire:
        raise AssertionError(f"неверный отрицательный диапазон: {newest_wire!r}")
    require_ok(client.uid("SEARCH", "RETURN", "(PARTIAL 20:30)", "ALL"), "SEARCH NIL")
    _, absent = client.response("ESEARCH")
    absent_wire = b" ".join(item for item in absent if isinstance(item, bytes))
    if b"PARTIAL (20:30 NIL)" not in absent_wire:
        raise AssertionError(f"отсутствует NIL: {absent_wire!r}")
    require_ok(client.uid("SEARCH", "RETURN", "(SAVE PARTIAL 2:3)", "ALL"), "SAVE")
    _, saved = client.uid("FETCH", "$", "(UID)")
    saved_wire = b" ".join(item for item in saved if isinstance(item, bytes))
    if not {b"UID 2", b"UID 3"}.issubset(set(re.findall(rb"UID \d+", saved_wire))):
        raise AssertionError(f"SAVE не сохранил выбранные UID: {saved_wire!r}")


def fetch_partial_seen(client: imaplib.IMAP4) -> None:
    r"""Меняет \Seen только на сообщениях выбранной страницы UID FETCH."""
    tag = client._command("UID", "FETCH", "1:5", "(UID BODY[])", "(PARTIAL 2:2)")
    require_ok(client._command_complete("UID FETCH", tag), "UID FETCH PARTIAL")
    _, fetched = client.uid("FETCH", "1:5", "(UID FLAGS)")
    wire = b" ".join(item for item in fetched if isinstance(item, bytes))
    records = re.findall(rb"\d+ \(UID (\d+) FLAGS \(([^)]*)\)", wire)
    seen = {int(uid) for uid, flags in records if b"\\Seen" in flags}
    if seen != {2}:
        raise AssertionError(f"PARTIAL FETCH изменил лишние Seen flags: {records!r}; {wire!r}")


def main() -> None:
    """Запускает изолированную проверку без изменения пользовательских папок."""
    options = arguments()
    context = ssl.create_default_context(cafile=options.ca)
    client = imaplib.IMAP4(options.host, options.port, timeout=10)
    client.starttls(ssl_context=context)
    capabilities = b" ".join(require_ok(client.capability(), "CAPABILITY"))
    if b"PARTIAL" not in capabilities:
        raise AssertionError(f"сервер не объявил PARTIAL: {capabilities!r}")
    require_ok(client.login("bob", bob_password(options.credentials)), "LOGIN")
    mailbox = "Partial-" + uuid.uuid4().hex
    append_messages(client, mailbox)
    search_partial(client, mailbox)
    fetch_partial_seen(client)
    logout_status, _ = client.logout()
    if ImapStatus(logout_status) is not ImapStatus.BYE:
        raise AssertionError(f"неожиданный completion LOGOUT: {logout_status!r}")
    print("IMAP RFC 9394 PARTIAL acceptance passed")


if __name__ == "__main__":
    main()
