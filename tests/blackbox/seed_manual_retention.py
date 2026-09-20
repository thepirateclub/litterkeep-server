"""Создаёт или проверяет ручное письмо вне пространства seed identifiers."""

from __future__ import annotations

import argparse
import imaplib
import socket
import ssl
from pathlib import Path

PROBE = "manual-retention-20260920"


def arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--host", required=True)
    parser.add_argument("--address", required=True)
    parser.add_argument("--port", required=True, type=int)
    parser.add_argument("--ca", required=True)
    parser.add_argument("--password", required=True)
    parser.add_argument("--create", action="store_true")
    return parser.parse_args()


def connect(config: argparse.Namespace) -> imaplib.IMAP4_SSL:
    pem = ssl.DER_cert_to_PEM_cert(Path(config.ca).read_bytes())
    context = ssl.create_default_context(cadata=pem)
    original = socket.getaddrinfo

    def local_address(host: str, port: int, *values: object, **named: object):
        target = config.address if host == config.host else host
        return original(target, port, *values, **named)

    socket.getaddrinfo = local_address
    try:
        client = imaplib.IMAP4_SSL(config.host, config.port, ssl_context=context)
    finally:
        socket.getaddrinfo = original
    status, _ = client.login("bob", config.password)
    require(status == "OK", "LOGIN")
    return client


def count(client: imaplib.IMAP4_SSL) -> int:
    status, _ = client.select("INBOX")
    require(status == "OK", "SELECT")
    status, values = client.search(None, "HEADER", "X-Manual-Probe", PROBE)
    require(status == "OK", "SEARCH")
    return len(next(iter(values), b"").split())


def main() -> None:
    config = arguments()
    client = connect(config)
    try:
        before = count(client)
        if config.create and before == 0:
            raw = (
                b"From: human@litterkeep.test\r\n"
                b"To: bob@litterkeep.test\r\n"
                b"Subject: Manual retention probe\r\n"
                b"X-Manual-Probe: manual-retention-20260920\r\n"
                b"\r\nThis message does not belong to seed.\r\n"
            )
            status, _ = client.append("INBOX", None, None, raw)
            require(status == "OK", "APPEND")
        after = count(client)
        require(after == 1, f"manual message count: {after}")
        print(f"MANUAL ok before={before} after={after}")
    finally:
        client.logout()


def require(condition: bool, context: str) -> None:
    if not condition:
        raise AssertionError(context)


if __name__ == "__main__":
    main()
