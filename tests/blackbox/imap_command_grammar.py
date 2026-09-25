"""Проверяет сетевые ответы BAD на лишние аргументы безаргументных команд."""

import argparse
from enum import Enum
import imaplib
import os
import ssl
from pathlib import Path


class ImapStatus(Enum):
    """Статусы, используемые независимым IMAP acceptance-клиентом."""

    OK = "OK"
    NO = "NO"
    BAD = "BAD"


def main() -> None:
    """До и после TLS проверяет parser rejection без потери сессии."""
    parser = argparse.ArgumentParser()
    parser.add_argument("host", nargs="?", default="127.0.0.1")
    parser.add_argument("port", nargs="?", type=int, default=2146)
    parser.add_argument("--ca", default="data/pki/ca.pem")
    parser.add_argument("--credentials", type=Path)
    options = parser.parse_args()
    client = imaplib.IMAP4(options.host, options.port, timeout=10)
    for verb in ("CAPABILITY", "NOOP", "LOGOUT", "STARTTLS"):
        require_bad(client, verb)
    context = ssl.create_default_context(cafile=options.ca)
    client.starttls(ssl_context=context)
    require_bad_quoted_pair(client)
    require_bad_utf8_quoted_string(client)
    require_bad_utf8_atom(client)
    require_bad_malformed_utf8(client)
    require_untagged_bad_malformed_tag(client)
    require_tagged_bad_response_special_tag(client)
    require_untagged_bad_plus_tag(client)
    require_bad_tab_separator(client)
    require_bad_quote_inside_atom(client)
    require_bad_structural_command_verbs(client)
    require(client.noop(), ImapStatus.OK)
    if options.credentials is not None:
        password = password_for(options.credentials, "bob")
    else:
        password = os.environ["LITTERKEEP_BOB_PASSWORD"]
    require(client.login("bob", password), ImapStatus.OK)
    require_append_limit(client)
    require_bad(client, "IDLE")
    require(client.select("INBOX"), ImapStatus.OK)
    require_bad(client, "EXPUNGE")
    require(client.noop(), ImapStatus.OK)
    client.logout()
    print("PASS imap-command-grammar")


def require_bad_quoted_pair(client: imaplib.IMAP4) -> None:
    """Проверяет RFC quoted-pair rejection и корректный tagged BAD response."""
    tag = client._command("LOGIN", b'"bob\\q" "unused"')
    status, detail = client._get_tagged_response(tag)
    response_text = b" ".join(item for item in (detail or []) if item is not None)
    if ImapStatus(status) is not ImapStatus.BAD:
        raise AssertionError(f"invalid quoted-pair status: {status} {detail!r}")
    if b"invalid command syntax" not in response_text:
        raise AssertionError(f"invalid quoted-pair: {status} {detail!r}")


def require_bad_utf8_quoted_string(client: imaplib.IMAP4) -> None:
    """Отвергает UTF-8 в кавычках до согласования UTF8=ACCEPT."""
    tag = client._command("LOGIN", b'"b\xc3\xb8b" "unused"')
    status, detail = client._get_tagged_response(tag)
    response_text = b" ".join(item for item in (detail or []) if item is not None)
    if ImapStatus(status) is not ImapStatus.BAD:
        raise AssertionError(f"unnegotiated UTF-8 quote status: {status} {detail!r}")
    if b"invalid command syntax" not in response_text:
        raise AssertionError(f"unnegotiated UTF-8 quote: {status} {detail!r}")


def require_bad_utf8_atom(client: imaplib.IMAP4) -> None:
    """До UTF8=ACCEPT отвергает high-bit octets внутри wire atom."""
    tag = client._command("LOGIN", b"b\xc3\xb8b unused")
    status, detail = client._get_tagged_response(tag)
    if ImapStatus(status) is not ImapStatus.BAD:
        raise AssertionError(f"unnegotiated UTF-8 atom status: {status} {detail!r}")


def require_bad_malformed_utf8(client: imaplib.IMAP4) -> None:
    """Сохраняет исходный tag при некорректной UTF-8 последовательности."""
    tag = client._command("LOGIN", b'"b\xffb" "unused"')
    status, detail = client._get_tagged_response(tag)
    response_text = b" ".join(item for item in (detail or []) if item is not None)
    if ImapStatus(status) is not ImapStatus.BAD:
        raise AssertionError(f"malformed UTF-8 quote status: {status} {detail!r}")
    if b"invalid command syntax" not in response_text:
        raise AssertionError(f"malformed UTF-8 quote: {status} {detail!r}")


def require_untagged_bad_malformed_tag(client: imaplib.IMAP4) -> None:
    """Не отражает некорректный tag и оставляет соединение пригодным."""
    client.send(b'A\xff7 LOGIN "bob" "unused"\r\n')
    response = client.readline()
    if not response.startswith(b"* BAD "):
        raise AssertionError(f"malformed command tag response: {response!r}")


def require_tagged_bad_response_special_tag(client: imaplib.IMAP4) -> None:
    """RFC 9051 tag использует ASTRING-CHAR и допускает закрывающую скобку."""
    client.send(b'A]7 LOGIN "b\xffb" "unused"\r\n')
    response = client.readline()
    if not response.startswith(b"A]7 BAD "):
        raise AssertionError(f"response-special command tag: {response!r}")
    client.send(b"A]8 NOOP\r\n")
    response = client.readline()
    if not response.startswith(b"A]8 OK "):
        raise AssertionError(f"valid response-special command tag: {response!r}")


def require_untagged_bad_plus_tag(client: imaplib.IMAP4) -> None:
    """Tag исключает плюс и не отражает его в error response."""
    client.send(b'A+8 LOGIN "b\xffb" "unused"\r\n')
    response = client.readline()
    if not response.startswith(b"* BAD "):
        raise AssertionError(f"invalid plus command tag: {response!r}")


def require_bad_tab_separator(client: imaplib.IMAP4) -> None:
    """RFC command grammar requires SP, not HTAB, between tag and command."""
    client.send(b"A17\tNOOP\r\n")
    response = client.readline()
    if not response.startswith(b"A17 BAD "):
        raise AssertionError(f"HTAB separator response: {response!r}")


def require_bad_quote_inside_atom(client: imaplib.IMAP4) -> None:
    """Кавычка внутри atom должна отклоняться, а соединение сохраняться."""
    client.send(b'A18 LOGIN bo"b "unused"\r\n')
    response = client.readline()
    if not response.startswith(b"A18 BAD "):
        raise AssertionError(f'quote inside atom response: {response!r}')


def require_bad_structural_command_verbs(client: imaplib.IMAP4) -> None:
    """Проверяет обязательный atom verb без пропуска скобок и quoted-string."""
    for command, expected in (
        (b"A19 (NOOP)\r\n", b"A19 BAD "),
        (b'A20 "NOOP"\r\n', b"A20 BAD "),
    ):
        client.send(command)
        response = client.readline()
        if not response.startswith(expected):
            raise AssertionError(f"invalid command verb form: {response!r}")


def require_bad(client: imaplib.IMAP4, verb: str) -> None:
    """Сырой запрос с лишним atom обязан завершаться tagged BAD."""
    tag = client._command(verb, b"EXTRA")
    status, detail = client._get_tagged_response(tag)
    response_text = b" ".join(item for item in (detail or []) if item is not None)
    if ImapStatus(status) is not ImapStatus.BAD:
        raise AssertionError(f"{verb} with extra atom status: {status} {detail!r}")
    if b"invalid command syntax" not in response_text:
        raise AssertionError(f"{verb} with extra atom: {status} {detail!r}")


def require_append_limit(client: imaplib.IMAP4) -> None:
    """Проверяет объявленный предел и отказ TOOBIG до приёма payload."""
    _, capabilities = client.capability()
    capability_tokens = b" ".join(capabilities or []).split()
    if b"APPENDLIMIT=26214400" not in capability_tokens:
        raise AssertionError(f"APPENDLIMIT missing from CAPABILITY: {capabilities!r}")
    tag = client._command("APPEND", b"INBOX {26214401}")
    status, detail = client._get_tagged_response(tag)
    response_text = b" ".join(item for item in (detail or []) if item is not None)
    if ImapStatus(status) is not ImapStatus.NO:
        raise AssertionError(f"oversize APPEND status: {status} {detail!r}")
    if b"TOOBIG" not in response_text:
        raise AssertionError(f"oversize APPEND missing TOOBIG: {status} {detail!r}")


def require(result: tuple[str, object], expected: ImapStatus) -> object:
    """Не позволяет последующему тесту продолжиться после неожиданного статуса."""
    status, detail = result
    if ImapStatus(status) is not expected:
        raise AssertionError(f"expected {expected}, got {status}: {detail!r}")
    return detail


def password_for(path: Path, username: str) -> str:
    """Читает credential из локального acceptance-файла без argv-секрета."""
    for line in path.read_text(encoding="utf-8").splitlines():
        match line.split("\t", maxsplit=1):
            case [account, password] if account == username:
                return password
            case _:
                continue
    raise AssertionError(f"missing test account: {username}")


if __name__ == "__main__":
    main()
