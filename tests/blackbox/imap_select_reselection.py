"""Проверяет RFC 9051 CLOSED и reset после неудачного SELECT."""

import argparse
from enum import Enum
import imaplib
from pathlib import Path
import ssl
import uuid


class ImapStatus(Enum):
    """Только tagged outcomes, необходимые acceptance-сценарию."""

    OK = "OK"
    NO = "NO"


def arguments() -> argparse.Namespace:
    """Читает адрес STARTTLS listener и локальные acceptance fixtures."""
    parser = argparse.ArgumentParser()
    parser.add_argument("host")
    parser.add_argument("port", type=int)
    parser.add_argument("--ca", required=True, type=Path)
    parser.add_argument("--credentials", required=True, type=Path)
    return parser.parse_args()


def password_for(path: Path, account_name: str) -> str:
    """Находит пароль fixture без печати секрета."""
    for line in path.read_text(encoding="utf-8").splitlines():
        match line.split("\t"):
            case [account, password] if account == account_name:
                return password
            case _:
                continue
    raise AssertionError(f"acceptance credentials omit {account_name}")


def command_lines(
    client: imaplib.IMAP4, tag: bytes, command: bytes
) -> list[bytes]:
    """Отправляет raw команду и дочитывает только её tagged completion."""
    client.send(tag + b" " + command + b"\r\n")
    responses = []
    for _ in range(32):
        response = client.readline()
        responses.append(response)
        if response.startswith(tag + b" "):
            return responses
    raise AssertionError(f"tagged completion missing for {command!r}: {responses!r}")


def require_closed_first(
    responses: list[bytes],
    tag: bytes,
    expected_status: ImapStatus,
    expected_mailbox: bytes | None,
) -> None:
    """Проверяет CLOSED как границу старой и новой mailbox проекций."""
    first = next(iter(responses), b"")
    completion = next((line for line in responses if line.startswith(tag + b" ")), b"")
    if not first.startswith(b"* OK [CLOSED]"):
        raise AssertionError(f"SELECT omitted CLOSED boundary: {responses!r}")
    if not completion.startswith(tag + b" " + expected_status.value.encode("ascii")):
        raise AssertionError(f"unexpected SELECT completion: {responses!r}")
    if expected_mailbox is not None:
        expected_list = b'* LIST () "/" "' + expected_mailbox + b'"'
        if not any(line.startswith(expected_list) for line in responses):
            raise AssertionError(f"SELECT omitted selected-mailbox LIST: {responses!r}")
        list_position = next(
            position for position, line in enumerate(responses)
            if line.startswith(expected_list)
        )
        flags_position = next(
            position for position, line in enumerate(responses)
            if line.startswith(b"* FLAGS ")
        )
        if list_position > flags_position:
            raise AssertionError(f"SELECT LIST must precede FLAGS: {responses!r}")


def require_no_selected_mailbox(client: imaplib.IMAP4) -> None:
    """После неудачного SELECT отвергает FETCH до новой SELECT команды."""
    responses = command_lines(client, b"R3", b"FETCH 1 FLAGS")
    completion = next(iter(responses), b"")
    if not completion.startswith(b"R3 NO select mailbox first"):
        raise AssertionError(f"failed SELECT retained a mailbox: {responses!r}")


def add_deleted_message(client: imaplib.IMAP4) -> str:
    """Создаёт уникальное письмо с флагом, который SELECT не должен удалять."""
    marker = uuid.uuid4().hex
    message = f"Subject: {marker}\r\n\r\nselection boundary\r\n".encode("ascii")
    status, detail = client.append("INBOX", "(\\Deleted)", None, message)
    if ImapStatus(status) is not ImapStatus.OK:
        raise AssertionError(f"APPEND deleted fixture failed: {status} {detail!r}")
    return marker


def require_deleted_message(client: imaplib.IMAP4, marker: str) -> None:
    """Подтверждает после повторного SELECT, что письмо и флаг сохранились."""
    status, search_result = client.search(None, "SUBJECT", marker)
    if ImapStatus(status) is not ImapStatus.OK:
        raise AssertionError(f"SEARCH deleted fixture failed: {status} {search_result!r}")
    match search_result:
        case [sequences] if len(sequences.split()) == 1:
            status, records = client.fetch(sequences, "(FLAGS)")
        case _:
            raise AssertionError(f"deleted fixture missing or duplicated: {search_result!r}")
    if ImapStatus(status) is not ImapStatus.OK:
        raise AssertionError(f"FETCH deleted fixture failed: {status} {records!r}")
    encoded_records = repr(records).encode("ascii", errors="backslashreplace")
    if b"\\Deleted" not in encoded_records:
        raise AssertionError(f"implicit SELECT expunged or changed fixture: {records!r}")


def verify(host: str, port: int, ca_file: Path, credentials: Path) -> None:
    """Переключает выбранную папку, затем проверяет failed-select reset."""
    context = ssl.create_default_context(cafile=str(ca_file))
    client = imaplib.IMAP4(host, port, timeout=15)
    client.starttls(ssl_context=context)
    status, detail = client.login("bob", password_for(credentials, "bob"))
    if ImapStatus(status) is not ImapStatus.OK:
        raise AssertionError(f"LOGIN failed: {status} {detail!r}")
    marker = add_deleted_message(client)
    status, detail = client.select("INBOX")
    if ImapStatus(status) is not ImapStatus.OK:
        raise AssertionError(f"initial SELECT failed: {status} {detail!r}")
    try:
        switched = command_lines(client, b"R1", b"SELECT INBOX")
        require_closed_first(switched, b"R1", ImapStatus.OK, b"INBOX")
        folder_name = "Invoices-" + uuid.uuid4().hex
        status, detail = client.create(folder_name)
        if ImapStatus(status) is not ImapStatus.OK:
            raise AssertionError(f"CREATE named mailbox failed: {status} {detail!r}")
        named_command = f'SELECT "{folder_name}"'.encode("ascii")
        named = command_lines(client, b"R4", named_command)
        require_closed_first(named, b"R4", ImapStatus.OK, folder_name.encode("ascii"))
        examined = command_lines(client, b"R5", b"EXAMINE INBOX")
        require_closed_first(examined, b"R5", ImapStatus.OK, b"INBOX")
        completion = next(reversed(examined), b"")
        if not completion.startswith(b"R5 OK [READ-ONLY]"):
            raise AssertionError(f"EXAMINE was not read-only: {examined!r}")
        failed = command_lines(client, b"R2", b"SELECT no-such-mailbox")
        require_closed_first(failed, b"R2", ImapStatus.NO, None)
        require_no_selected_mailbox(client)
        status, detail = client.noop()
        if ImapStatus(status) is not ImapStatus.OK:
            raise AssertionError(f"NOOP after failed SELECT: {status} {detail!r}")
        status, detail = client.select("INBOX")
        if ImapStatus(status) is not ImapStatus.OK:
            raise AssertionError(f"SELECT recovery failed: {status} {detail!r}")
        require_deleted_message(client, marker)
    finally:
        client.logout()


def main() -> None:
    """Запускает независимый STARTTLS сценарий по параметрам CLI."""
    options = arguments()
    verify(options.host, options.port, options.ca, options.credentials)
    print("PASS imap-select-reselection: CLOSED boundary and failed-select reset")


if __name__ == "__main__":
    main()
