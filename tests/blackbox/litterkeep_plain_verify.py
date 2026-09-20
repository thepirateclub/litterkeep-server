"""Независимо подтверждает письмо, отправленное Android-клиентом litterkeep."""

from __future__ import annotations

import argparse
import imaplib
import ssl


def arguments() -> argparse.Namespace:
    """Читает только сетевые реквизиты и ожидаемую тему acceptance-письма."""
    parser = argparse.ArgumentParser()
    parser.add_argument("--host", required=True)
    parser.add_argument("--port", required=True, type=int)
    parser.add_argument("--ca", required=True)
    parser.add_argument("--login", required=True)
    parser.add_argument("--password", required=True)
    parser.add_argument("--subject", required=True)
    return parser.parse_args()


def main() -> None:
    """Выполняет STARTTLS, LOGIN и точный SUBJECT SEARCH через стандартный IMAP."""
    options = arguments()
    context = ssl.create_default_context(cafile=options.ca)
    with imaplib.IMAP4(options.host, options.port, timeout=10) as client:
        status, _ = client.starttls(context)
        require(status == "OK", "STARTTLS")
        status, _ = client.login(options.login, options.password)
        require(status == "OK", "LOGIN")
        status, _ = client.select("INBOX", readonly=True)
        require(status == "OK", "SELECT")
        status, values = client.search(None, "SUBJECT", f'"{options.subject}"')
        require(status == "OK", "SEARCH")
        identifiers = next(iter(values), b"").split()
        require(len(identifiers) == 1, f"message count: {len(identifiers)}")
    print("LITTERKEEP plain message ok count=1")


def require(condition: bool, operation: str) -> None:
    """Завершает независимый сценарий на первой неверной wire-семантике."""
    if not condition:
        raise AssertionError(operation)


if __name__ == "__main__":
    main()
