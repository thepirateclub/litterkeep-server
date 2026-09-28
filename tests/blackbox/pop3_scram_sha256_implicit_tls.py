"""Проверяет POP3 SCRAM-SHA-256 поверх RFC 8314 implicit TLS."""

from __future__ import annotations

import argparse
import socket
import ssl
from pathlib import Path
from typing import BinaryIO

from pop3_scram import (
    ScramMechanism,
    authenticate,
    expect,
    password_from,
    read_line,
)


def provision_verifier(port: int, password: str, ca: Path) -> None:
    """Инициализирует старую acceptance-учётку штатной авторизацией."""
    raw = socket.create_connection(("127.0.0.1", port), timeout=10)
    raw.settimeout(10)
    context = ssl.create_default_context(cafile=ca)
    with context.wrap_socket(raw, server_hostname="localhost") as secure:
        stream = secure.makefile("rwb", buffering=0)
        expect(stream, b"+OK", "implicit verifier greeting")
        stream.write(b"USER bob\r\n")
        expect(stream, b"+OK", "implicit verifier USER")
        stream.write(f"PASS {password}\r\n".encode("utf-8"))
        expect(stream, b"+OK", "implicit verifier PASS")
        stream.write(b"QUIT\r\n")
        expect(stream, b"+OK", "implicit verifier QUIT")
        stream.close()


def run(port: int, credentials: Path, ca: Path) -> None:
    """Проверяет CA, implicit greeting, capability, mutual proof и maildrop."""
    password = password_from(credentials)
    provision_verifier(port, password, ca)
    raw = socket.create_connection(("127.0.0.1", port), timeout=10)
    raw.settimeout(10)
    context = ssl.create_default_context(cafile=ca)
    with context.wrap_socket(raw, server_hostname="localhost") as secure:
        stream: BinaryIO = secure.makefile("rwb", buffering=0)
        expect(stream, b"+OK", "implicit-TLS greeting")
        stream.write(b"CAPA\r\n")
        capabilities = bytearray()
        while True:
            line = read_line(stream)
            if line == b".\r\n":
                break
            capabilities.extend(line)
        if b"SASL PLAIN SCRAM-SHA-1 SCRAM-SHA-256\r\n" not in capabilities:
            raise AssertionError("implicit-TLS CAPA omitted SCRAM mechanisms")
        authenticate(
            stream,
            password,
            no_initial=False,
            invalid=False,
            mechanism=ScramMechanism.SHA256,
        )
        stream.write(b"STAT\r\n")
        expect(stream, b"+OK", "STAT after implicit-TLS SCRAM")
        stream.write(b"QUIT\r\n")
        expect(stream, b"+OK", "implicit-TLS QUIT")
        stream.close()


def main() -> None:
    """Разбирает параметры isolated POP3 implicit-TLS acceptance."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--port", type=int, required=True)
    parser.add_argument("--credentials", type=Path, required=True)
    parser.add_argument("--ca", type=Path, required=True)
    options = parser.parse_args()
    run(options.port, options.credentials, options.ca)
    print("PASS POP3 SCRAM-SHA-256 implicit-TLS exchange")


if __name__ == "__main__":
    main()
