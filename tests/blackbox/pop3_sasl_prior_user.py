"""Проверяет сохранение POP3 AUTHORIZATION после каждого отказа AUTH."""

import argparse
import base64
from enum import Enum
import socket
import ssl
from pathlib import Path


class FixtureAccount(Enum):
    """Типизированные названия аккаунтов acceptance-fixture."""

    BOB = "bob"


def read_line(stream: socket.SocketIO, label: str) -> bytes:
    """Читает одну строку POP3 и отвергает оборванную границу CRLF."""
    response = stream.readline()
    if not response.endswith(b"\r\n"):
        raise AssertionError(f"{label}: сервер оборвал ответ: {response!r}")
    return response


def exchange(stream: socket.SocketIO, command: bytes, prefix: bytes) -> bytes:
    """Отправляет команду и проверяет ожидаемый статусный префикс."""
    stream.write(command + b"\r\n")
    stream.flush()
    response = read_line(stream, command.decode("ascii"))
    if not response.startswith(prefix):
        raise AssertionError(
            f"{command!r}: ожидался {prefix!r}, получен {response!r}"
        )
    return response


def verify(host: str, port: int, ca: str, password: str) -> None:
    """Проверяет отказные ветви AUTH до успешного USER/PASS входа."""
    context = ssl.create_default_context(cafile=ca)
    with socket.create_connection((host, port), timeout=10) as connection:
        stream = connection.makefile("rwb", buffering=0)
        greeting = read_line(stream, "greeting")
        if not greeting.startswith(b"+OK"):
            raise AssertionError(f"неожиданный greeting: {greeting!r}")
        exchange(stream, b"STLS", b"+OK")
        stream.close()
        with context.wrap_socket(connection, server_hostname="localhost") as secured:
            protected = secured.makefile("rwb", buffering=0)
            exchange(protected, b"USER bob", b"+OK")
            verify_unsupported_mechanism(protected)
            rejected_commands(protected, password)
            exchange(
                protected,
                b"PASS " + b"x" * 40,
                b"-ERR authentication failed",
            )
            exchange(protected, b"USER bob", b"+OK")
            response = base64.b64encode(b"\x00bob\x00" + password.encode("ascii"))
            exchange(protected, b"AUTH PLAIN " + response, b"+OK")
            exchange(protected, b"STAT", b"+OK")
            exchange(protected, b"QUIT", b"+OK")


def read_bob_password(credentials: Path) -> str:
    """Читает пароль Bob из локального fixture, не передавая его в argv."""
    for record in credentials.read_text(encoding="ascii").splitlines():
        account_name, separator, password = record.partition("\t")
        try:
            account = FixtureAccount(account_name)
        except ValueError:
            continue
        if account is FixtureAccount.BOB and separator:
            return password
    raise ValueError(f"Bob отсутствует в fixture {credentials}")


def rejected_commands(stream: socket.SocketIO, password: str) -> None:
    """Перебирает SASL отказы, не меняя выбранного USER bob."""
    exchange(stream, b"AUTH PLAIN =", b"-ERR invalid AUTH PLAIN credentials")
    exchange(stream, b"AUTH PLAIN !!!", b"-ERR invalid AUTH PLAIN credentials")
    exchange(stream, b"AUTH PLAIN", b"+ ")
    long_password = b"x" * 256
    long_response = base64.b64encode(b"\x00bob\x00" + long_password)
    if len(long_response) <= 255:
        raise AssertionError("test SASL response must exceed the POP3 command limit")
    exchange(stream, long_response, b"-ERR authentication failed")
    exchange(stream, b"AUTH PLAIN", b"+ ")
    exchange(stream, b"*", b"-ERR authentication cancelled")
    exchange(stream, b"AUTH PLAIN", b"+ ")
    exchange(stream, b"!!!", b"-ERR invalid AUTH PLAIN credentials")
    invalid = base64.b64encode(b"\x00bob\x00wrong-password")
    exchange(stream, b"AUTH PLAIN " + invalid, b"-ERR authentication failed")
    valid = base64.b64encode(b"\x00missing-account\x00" + password.encode("ascii"))
    exchange(stream, b"AUTH PLAIN " + valid, b"-ERR authentication failed")


def verify_unsupported_mechanism(stream: socket.SocketIO) -> None:
    """Проверяет, что не объявленный механизм с initial-response отвергается."""
    exchange(stream, b"CAPA", b"+OK")
    capabilities = []
    while True:
        line = read_line(stream, "SASL capability listing")
        if line == b".\r\n":
            break
        capabilities.append(line)
    sasl_lines = [line for line in capabilities if line.startswith(b"SASL ")]
    if len(sasl_lines) != 1:
        raise AssertionError(f"CAPA advertises unexpected SASL mechanisms: {sasl_lines!r}")
    if b"DIGEST-MD5" in sasl_lines[0]:
        raise AssertionError("CAPA unexpectedly advertises DIGEST-MD5")
    exchange(
        stream,
        b"AUTH DIGEST-MD5 ZHVtbXk=",
        b"-ERR unsupported SASL mechanism",
    )


def main() -> None:
    """Запускает независимую RFC 5034 STARTTLS-проверку."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, required=True)
    parser.add_argument("--ca", required=True)
    parser.add_argument("--credentials", type=Path, required=True)
    options = parser.parse_args()
    verify(options.host, options.port, options.ca, read_bob_password(options.credentials))
    print("PASS POP3 RFC 5034: failed AUTH preserves the prior USER state")


if __name__ == "__main__":
    main()
