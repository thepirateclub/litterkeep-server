"""Проверяет RFC 3516 BINARY advertisement вокруг STARTTLS и LOGIN."""

import argparse
import base64
from enum import Enum
import os
import re
import socket
import ssl


class ImapCompletion(Enum):
    """Типизированные IMAP completion atoms из tagged ответа."""

    OK = "OK"
    NO = "NO"
    BAD = "BAD"


def options():
    """Разбирает адрес независимого тестового IMAP-сервера."""
    parser = argparse.ArgumentParser()
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=2143)
    parser.add_argument("--ca", default="data/pki/ca.pem")
    return parser.parse_args()


class ImapWire:
    """Минимальная tagged-сессия для проверки точного capability token."""

    def __init__(self, host: str, port: int, context: ssl.SSLContext):
        self.socket = socket.create_connection((host, port), timeout=10)
        self.stream = self.socket.makefile("rb")
        self.counter = 0
        greeting = self.stream.readline()
        if not greeting.startswith(b"* OK"):
            raise AssertionError(f"unexpected greeting: {greeting!r}")
        self.context = context
        self.host = host

    def command(self, value: bytes) -> bytes:
        """Считывает untagged-строки до tagged завершения команды."""
        self.counter += 1
        tag = f"C{self.counter:03}".encode("ascii")
        self.socket.sendall(tag + b" " + value + b"\r\n")
        response = bytearray()
        while True:
            line = self.stream.readline()
            if not line:
                raise AssertionError("server closed during IMAP response")
            response.extend(line)
            if line.startswith(tag + b" "):
                return bytes(response)

    def starttls(self) -> None:
        """Переключает единственный socket на TLS после tagged OK."""
        response = self.command(b"STARTTLS")
        require_ok(response, "STARTTLS")
        self.stream.close()
        self.socket = self.context.wrap_socket(self.socket, server_hostname=self.host)
        self.stream = self.socket.makefile("rb")

    def close(self) -> None:
        """Освобождает сокет независимо от исхода проверки."""
        self.stream.close()
        self.socket.close()


def require_binary(client: ImapWire, phase: str) -> None:
    """Требует ровно capability atom BINARY, а не похожую подстроку."""
    response = client.command(b"CAPABILITY")
    require_ok(response, f"{phase} CAPABILITY")
    lines = [line for line in response.splitlines() if line.startswith(b"* CAPABILITY ")]
    tokens = b" ".join(lines).upper().split()
    if b"BINARY" not in tokens:
        raise AssertionError(f"{phase} CAPABILITY omitted BINARY: {tokens!r}")


def authenticate(client: ImapWire) -> None:
    """Выполняет PLAIN SASL-IR после перехода в TLS."""
    password = os.environ["LITTERKEEP_BOB_PASSWORD"]
    payload = base64.b64encode(b"\x00bob\x00" + password.encode("utf-8"))
    response = client.command(b"AUTHENTICATE PLAIN " + payload)
    require_ok(response, "AUTHENTICATE PLAIN")


def require_ok(response: bytes, operation: str) -> None:
    """Проверяет tagged completion через enum, не сравнивая status-строки."""
    tagged = next(
        (line for line in response.splitlines() if re.match(rb"^C[0-9]+ ", line)),
        None,
    )
    if tagged is None:
        raise AssertionError(f"{operation}: tagged completion missing: {response!r}")
    atom = tagged.split(maxsplit=2)
    try:
        status = ImapCompletion(atom[1].decode("ascii"))
    except (IndexError, UnicodeDecodeError, ValueError) as error:
        raise AssertionError(f"{operation}: invalid completion: {tagged!r}") from error
    if status is not ImapCompletion.OK:
        raise AssertionError(f"{operation}: {status.name}: {tagged!r}")


def main() -> None:
    """Проверяет объявление BINARY до TLS и в обоих защищённых состояниях."""
    selected = options()
    context = ssl.create_default_context(cafile=selected.ca)
    client = ImapWire(selected.host, selected.port, context)
    try:
        require_binary(client, "pre-STARTTLS")
        client.starttls()
        require_binary(client, "post-STARTTLS")
        authenticate(client)
        require_binary(client, "authenticated")
    finally:
        client.close()
    print("PASS IMAP BINARY capability before STARTTLS, after STARTTLS and LOGIN")


if __name__ == "__main__":
    main()
