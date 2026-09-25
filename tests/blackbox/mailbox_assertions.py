"""Общие независимые IMAP assertions для black-box acceptance клиентов."""

import imaplib


def fetch_single_message(client: imaplib.IMAP4, mailbox: str) -> bytes:
    """Читает единственное тестовое сообщение RFC822 без индексации ответов."""
    status, count = client.select(mailbox)
    if status != "OK" or count == [b"0"]:
        raise AssertionError(f"{mailbox}: ожидается доставленное сообщение")
    status, search_result = client.uid("SEARCH", None, "ALL")
    if status != "OK":
        raise AssertionError(f"{mailbox}: IMAP UID SEARCH завершился ошибкой")
    identifier, separator, remainder = next(iter(search_result), b"").strip().partition(b" ")
    if identifier == b"":
        raise AssertionError(f"{mailbox}: не найден UID доставленного письма")
    if separator != b"":
        raise AssertionError(f"{mailbox}: найдено несколько сообщений")
    if remainder != b"":
        raise AssertionError(f"{mailbox}: ожидалось ровно одно сообщение")
    status, fetched = client.uid("FETCH", identifier, "RFC822")
    if status != "OK":
        raise AssertionError(f"{mailbox}: IMAP UID FETCH завершился ошибкой")
    payloads = [
        payload
        for item in fetched
        if isinstance(item, tuple)
        for _, payload in [item]
        if isinstance(payload, bytes)
    ]
    if not payloads:
        raise AssertionError(f"{mailbox}: FETCH не вернул байты RFC822")
    return b"".join(payloads)
