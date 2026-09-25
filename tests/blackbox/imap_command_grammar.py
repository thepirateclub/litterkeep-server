"""Проверяет сетевые ответы BAD на лишние аргументы безаргументных команд."""

import argparse
import imaplib
import os
import ssl
from pathlib import Path


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
    require(client.noop(), "OK")
    if options.credentials is not None:
        password = password_for(options.credentials, "bob")
    else:
        password = os.environ["LITTERKEEP_BOB_PASSWORD"]
    require(client.login("bob", password), "OK")
    require_bad(client, "IDLE")
    require(client.select("INBOX"), "OK")
    require_bad(client, "EXPUNGE")
    require(client.noop(), "OK")
    client.logout()
    print("PASS imap-command-grammar")


def require_bad_quoted_pair(client: imaplib.IMAP4) -> None:
    """Проверяет RFC quoted-pair rejection и корректный tagged BAD response."""
    tag = client._command("LOGIN", b'"bob\\q" "unused"')
    status, detail = client._get_tagged_response(tag)
    response_text = b" ".join(item for item in (detail or []) if item is not None)
    if status != "BAD" or b"invalid command syntax" not in response_text:
        raise AssertionError(f"invalid quoted-pair: {status} {detail!r}")


def require_bad_utf8_quoted_string(client: imaplib.IMAP4) -> None:
    """Отвергает UTF-8 в кавычках до согласования UTF8=ACCEPT."""
    tag = client._command("LOGIN", b'"b\xc3\xb8b" "unused"')
    status, detail = client._get_tagged_response(tag)
    response_text = b" ".join(item for item in (detail or []) if item is not None)
    if status != "BAD" or b"invalid command syntax" not in response_text:
        raise AssertionError(f"unnegotiated UTF-8 quote: {status} {detail!r}")


def require_bad(client: imaplib.IMAP4, verb: str) -> None:
    """Сырой запрос с лишним atom обязан завершаться tagged BAD."""
    tag = client._command(verb, b"EXTRA")
    status, detail = client._get_tagged_response(tag)
    response_text = b" ".join(item for item in (detail or []) if item is not None)
    if status != "BAD" or b"invalid command syntax" not in response_text:
        raise AssertionError(f"{verb} with extra atom: {status} {detail!r}")


def require(result: tuple[str, object], expected: str) -> object:
    """Не позволяет последующему тесту продолжиться после неожиданного статуса."""
    status, detail = result
    if status != expected:
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
