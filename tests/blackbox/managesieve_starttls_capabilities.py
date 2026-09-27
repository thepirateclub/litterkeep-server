"""Проверяет ManageSieve SASL и CAPABILITY transitions вокруг STARTTLS."""

from __future__ import annotations

import argparse
import base64
import socket
import ssl
from pathlib import Path


def read_until(stream: socket.socket, marker: bytes) -> bytes:
    """Читает bounded capability block до точной завершающей строки."""
    received = bytearray()
    while marker not in received:
        block = stream.recv(4096)
        if not block:
            raise AssertionError(f"server closed before {marker!r}")
        received.extend(block)
        if len(received) > 65536:
            raise AssertionError("ManageSieve response exceeded test bound")
    return bytes(received)


def require_capabilities(
    block: bytes,
    sasl: bytes,
    starttls: bool,
    ending: bytes,
    owner: bytes | None = None,
) -> None:
    """Проверяет SASL list, STARTTLS и заключительную response line."""
    require_line(block, b'"IMPLEMENTATION" "litterkeep-server"\r\n')
    require_line(block, b'"VERSION" "1.0"\r\n')
    require_line(block, b'"SASL" "' + sasl + b'"\r\n')
    require_line(
        block,
        b'"SIEVE" "fileinto subaddress envelope copy vacation vacation-seconds"\r\n',
    )
    require_line(block, b'"UNAUTHENTICATE"\r\n')
    if owner is not None:
        require_line(block, b'"OWNER" "' + owner + b'"\r\n')
    has_starttls = b'"STARTTLS"\r\n' in block
    if has_starttls is not starttls:
        raise AssertionError(f"wrong STARTTLS capability state: {block!r}")
    if not block.endswith(ending):
        raise AssertionError(f"wrong capability terminator: {block!r}")


def require_line(block: bytes, expected: bytes) -> None:
    """Требует точную capability line без нормализации регистра."""
    if expected not in block:
        raise AssertionError(f"missing {expected!r} in {block!r}")


def main() -> None:
    """Проходит plaintext, STARTTLS upgrade и повторные capabilities."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--port", type=int, required=True)
    parser.add_argument("--ca", required=True)
    parser.add_argument("--credentials", type=Path, required=True)
    options = parser.parse_args()
    context = ssl.create_default_context(cafile=options.ca)
    with socket.create_connection(("127.0.0.1", options.port), timeout=10) as raw:
        greeting = read_until(raw, b'OK "ready"\r\n')
        require_capabilities(greeting, b"", True, b'OK "ready"\r\n')
        raw.sendall(b"CAPABILITY\r\n")
        plaintext_capabilities = read_until(raw, b'OK "capability"\r\n')
        require_capabilities(
            plaintext_capabilities, b"", True, b'OK "capability"\r\n'
        )
        raw.sendall(b"STARTTLS\r\n")
        starttls_reply = read_until(raw, b'OK "begin TLS"\r\n')
        if starttls_reply != b'OK "begin TLS"\r\n':
            raise AssertionError(f"unexpected STARTTLS reply: {starttls_reply!r}")
        with context.wrap_socket(raw, server_hostname="localhost") as encrypted:
            post_tls = read_until(encrypted, b'OK "ready"\r\n')
            require_capabilities(post_tls, b"PLAIN", False, b'OK "ready"\r\n')
            encrypted.sendall(b"CAPABILITY\r\n")
            current = read_until(encrypted, b'OK "capability"\r\n')
            require_capabilities(current, b"PLAIN", False, b'OK "capability"\r\n')
            password = password_for(options.credentials, "bob")
            sasl = base64.b64encode(b"\x00bob\x00" + password.encode("utf-8"))
            encrypted.sendall(b'AUTHENTICATE "PLAIN" "' + sasl + b'"\r\n')
            authenticated = read_until(encrypted, b'OK "authenticated"\r\n')
            require_capabilities(
                authenticated,
                b"PLAIN",
                False,
                b'OK "authenticated"\r\n',
                owner=b"bob",
            )
            encrypted.sendall(b"CAPABILITY\r\n")
            owner_capabilities = read_until(encrypted, b'OK "capability"\r\n')
            require_capabilities(
                owner_capabilities,
                b"PLAIN",
                False,
                b'OK "capability"\r\n',
                owner=b"bob",
            )
            encrypted.sendall(b"STARTTLS\r\n")
            repeated = read_until(encrypted, b"\r\n")
            if repeated != b'NO "TLS is unavailable or already active"\r\n':
                raise AssertionError(f"repeated STARTTLS was not rejected: {repeated!r}")
            encrypted.sendall(b"UNAUTHENTICATE\r\n")
            unauthenticated = read_until(encrypted, b'OK "unauthenticated"\r\n')
            require_capabilities(
                unauthenticated,
                b"PLAIN",
                False,
                b'OK "unauthenticated"\r\n',
            )
            encrypted.sendall(b"LISTSCRIPTS\r\n")
            denied = read_until(encrypted, b"\r\n")
            if denied != b'NO "invalid command for state"\r\n':
                raise AssertionError(f"authenticated command survived UNAUTHENTICATE: {denied!r}")
            encrypted.sendall(b"CAPABILITY\r\n")
            after_unauthenticate = read_until(encrypted, b'OK "capability"\r\n')
            require_capabilities(
                after_unauthenticate,
                b"PLAIN",
                False,
                b'OK "capability"\r\n',
            )
            encrypted.sendall(b'AUTHENTICATE "PLAIN" "' + sasl + b'"\r\n')
            reauthenticated = read_until(encrypted, b'OK "authenticated"\r\n')
            require_capabilities(
                reauthenticated,
                b"PLAIN",
                False,
                b'OK "authenticated"\r\n',
                owner=b"bob",
            )
            print("PASS ManageSieve SASL/STARTTLS capability transitions")


def password_for(path: Path, username: str) -> str:
    """Читает credential из отдельного локального bootstrap-файла."""
    for line in path.read_text(encoding="utf-8").splitlines():
        match line.split("\t", maxsplit=1):
            case [account, password] if account == username:
                return password
            case _:
                continue
    raise AssertionError(f"missing test account: {username}")


if __name__ == "__main__":
    main()
