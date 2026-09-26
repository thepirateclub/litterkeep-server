"""Проверяет IMAP LOGIN со стандартными synchronizing literals через STARTTLS."""

import argparse
import socket
import ssl
from pathlib import Path


def options() -> argparse.Namespace:
    """Читает адрес сервера, доверенный CA и локальный fixture аккаунтов."""
    parser = argparse.ArgumentParser()
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, required=True)
    parser.add_argument("--ca", required=True)
    parser.add_argument("--credentials", required=True)
    return parser.parse_args()


def password_for(path: Path, account: str) -> str:
    """Читает секрет из TSV, не включая его в диагностические сообщения."""
    for line in path.read_text(encoding="utf-8").splitlines():
        identity, separator, password = line.partition("\t")
        if separator:
            if identity == account and password:
                return password
    raise AssertionError(f"test credential for {account} is missing")


def response(reader: object) -> bytes:
    """Читает ровно одну CRLF-завершённую строку ответа сервера."""
    line = reader.readline(4096)
    if not line.endswith(b"\r\n"):
        raise AssertionError(f"server returned an incomplete response: {line!r}")
    return line


def expect(reader: object, prefix: bytes, phase: str) -> bytes:
    """Проверяет тип ответа, не выдавая credentials и содержимое сессии."""
    line = response(reader)
    if not line.startswith(prefix):
        raise AssertionError(f"{phase} expected {prefix!r}, got {line!r}")
    return line


def literal_marker(value: bytes) -> bytes:
    """Строит sync marker по числу передаваемых UTF-8 октетов."""
    return b"{" + str(len(value)).encode("ascii") + b"}\r\n"


def authenticate(stream: ssl.SSLSocket, reader: object, password: bytes) -> None:
    """Передаёт обе LOGIN строки только после своего continuation ответа."""
    stream.sendall(b"A2 LOGIN {3}\r\n")
    expect(reader, b"+", "username continuation")
    invalid_octet = b"\xff"
    stream.sendall(b"bob " + literal_marker(invalid_octet))
    expect(reader, b"+", "non-UTF-8 password continuation")
    stream.sendall(invalid_octet + b"\r\n")
    expect(reader, b"A2 NO", "non-UTF-8 literal password")

    invalid_password = b"bad\r\npassword"
    stream.sendall(b"A3 LOGIN {3}\r\n")
    expect(reader, b"+", "retry username continuation")
    stream.sendall(b"bob " + literal_marker(invalid_password))
    expect(reader, b"+", "password continuation")
    stream.sendall(invalid_password + b"\r\n")
    expect(reader, b"A3 NO", "CRLF-containing literal password")

    stream.sendall(b"A4 LOGIN {3}\r\n")
    expect(reader, b"+", "final username continuation")
    stream.sendall(b"bob " + literal_marker(password))
    expect(reader, b"+", "final password continuation")
    stream.sendall(password + b"\r\n")
    expect(reader, b"A4 OK", "literal LOGIN after authentication failures")


def tagged_responses(reader: object, tag: bytes) -> list[bytes]:
    """Собирает ограниченный набор untagged строк до заданного tagged OK."""
    responses = []
    while len(responses) < 32:
        line = response(reader)
        responses.append(line)
        if line.startswith(tag + b" OK"):
            return responses
    raise AssertionError(f"tagged response {tag!r} was not received")


def mailbox_literals(stream: ssl.SSLSocket, reader: object) -> None:
    """Проверяет mailbox astring literals в CREATE, LIST и SELECT командах."""
    mailbox = b"LiteralStage"
    marker = literal_marker(mailbox)
    stream.sendall(b"A5 CREATE " + marker)
    expect(reader, b"+", "CREATE continuation")
    stream.sendall(mailbox + b"\r\n")
    expect(reader, b"A5 OK", "literal CREATE")

    stream.sendall(b'A6 LIST "" ' + marker)
    expect(reader, b"+", "LIST continuation")
    stream.sendall(mailbox + b"\r\n")
    listed = tagged_responses(reader, b"A6")
    if not any(mailbox in line for line in listed):
        raise AssertionError("literal LIST did not return the created mailbox")

    stream.sendall(b"A7 SELECT " + marker)
    expect(reader, b"+", "SELECT continuation")
    stream.sendall(mailbox + b"\r\n")
    selected = tagged_responses(reader, b"A7")
    if not any(b"EXISTS" in line for line in selected):
        raise AssertionError("literal SELECT omitted mailbox state")


def verify_authenticated_command(stream: ssl.SSLSocket, reader: object) -> None:
    """Подтверждает переход сессии в authenticated state после LOGIN."""
    stream.sendall(b"A8 NOOP\r\n")
    expect(reader, b"A8 OK", "authenticated NOOP")


def main() -> None:
    """Выполняет независимый raw-wire LOGIN поверх проверенного STARTTLS."""
    arguments = options()
    password = password_for(Path(arguments.credentials), "bob").encode("utf-8")
    context = ssl.create_default_context(cafile=arguments.ca)
    with socket.create_connection((arguments.host, arguments.port), timeout=10) as plain:
        plain.settimeout(10)
        greeting = plain.makefile("rb").readline(4096)
        if not greeting.startswith(b"* OK"):
            raise AssertionError(f"unexpected IMAP greeting: {greeting!r}")
        plain.sendall(b"A1 STARTTLS\r\n")
        expect_plain = plain.makefile("rb").readline(4096)
        if not expect_plain.startswith(b"A1 OK"):
            raise AssertionError(f"STARTTLS failed: {expect_plain!r}")
        with context.wrap_socket(plain, server_hostname="localhost") as secure:
            reader = secure.makefile("rb")
            authenticate(secure, reader, password)
            mailbox_literals(secure, reader)
            verify_authenticated_command(secure, reader)
    print("PASS imap-command-astring-literal")


if __name__ == "__main__":
    main()
