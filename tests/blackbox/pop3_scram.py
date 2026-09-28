"""Проверяет RFC 5034 POP3 SCRAM-SHA-1 через независимый TLS peer."""

from __future__ import annotations

import argparse
import base64
import hashlib
import hmac
import secrets
import socket
import ssl
from pathlib import Path
from typing import BinaryIO


def read_line(stream: BinaryIO) -> bytes:
    """Читает одну bounded POP3 line и требует точное CRLF завершение."""
    line = stream.readline(8193)
    if not line.endswith(b"\r\n") or len(line) > 8192:
        raise AssertionError(f"invalid POP3 line: {line!r}")
    return line


def expect(stream: BinaryIO, prefix: bytes, context: str) -> bytes:
    """Проверяет status prefix, не выводя пароль и SCRAM transcript."""
    line = read_line(stream)
    if not line.startswith(prefix):
        raise AssertionError(f"{context}: unexpected response {line!r}")
    return line


def password_from(path: Path) -> str:
    """Читает локальный fixture, не передавая credential через argv."""
    for line in path.read_text(encoding="utf-8").splitlines():
        fields = line.split("\t", maxsplit=1)
        match fields:
            case ["bob", password]:
                return password
            case _:
                continue
    raise AssertionError("bob fixture password is missing")


def challenge(stream: BinaryIO) -> dict[str, str]:
    """Декодирует RFC 5034 Base64 challenge и парсит SCRAM attributes."""
    line = expect(stream, b"+ ", "SCRAM challenge")
    encoded = line.removeprefix(b"+ ").removesuffix(b"\r\n")
    message = base64.b64decode(encoded, validate=True).decode("ascii")
    return dict(field.split("=", maxsplit=1) for field in message.split(","))


def authenticate(
    stream: BinaryIO,
    password: str,
    no_initial: bool,
    invalid: bool,
    expected_completion: bytes = b"+OK",
) -> bytes:
    """Проводит full SCRAM proof, проверяет server-final и client acknowledgement."""
    nonce = secrets.token_urlsafe(18)
    first_bare = f"n=bob,r={nonce}"
    initial = base64.b64encode(f"n,,{first_bare}".encode("ascii"))
    if no_initial:
        stream.write(b"AUTH SCRAM-SHA-1\r\n")
        if read_line(stream) != b"+ \r\n":
            raise AssertionError("AUTH omitted the RFC 5034 empty challenge")
        stream.write(initial + b"\r\n")
    else:
        stream.write(b"AUTH SCRAM-SHA-1 " + initial + b"\r\n")
    fields = challenge(stream)
    combined_nonce = fields.get("r")
    salt_text = fields.get("s")
    iterations_text = fields.get("i")
    if combined_nonce is None or not combined_nonce.startswith(nonce):
        raise AssertionError(f"server nonce did not extend client nonce: {fields!r}")
    if salt_text is None or iterations_text is None:
        raise AssertionError(f"server-first lacks verifier parameters: {fields!r}")
    salt = base64.b64decode(salt_text, validate=True)
    salted = hashlib.pbkdf2_hmac("sha1", password.encode("utf-8"), salt, int(iterations_text))
    unsigned = f"c=biws,r={combined_nonce}"
    server_first = ",".join(f"{key}={value}" for key, value in fields.items())
    auth_message = f"{first_bare},{server_first},{unsigned}".encode("ascii")
    client_key = hmac.digest(salted, b"Client Key", "sha1")
    stored_key = hashlib.sha1(client_key).digest()
    client_signature = hmac.digest(stored_key, auth_message, "sha1")
    server_key = hmac.digest(salted, b"Server Key", "sha1")
    expected_server_signature = base64.b64encode(
        hmac.digest(server_key, auth_message, "sha1")
    ).decode("ascii")
    proof = bytes(left ^ right for left, right in zip(client_key, client_signature, strict=True))
    if invalid:
        proof = bytes(len(proof))
    final = f"{unsigned},p={base64.b64encode(proof).decode('ascii')}"
    stream.write(base64.b64encode(final.encode("ascii")) + b"\r\n")
    final_fields = challenge(stream)
    if invalid:
        if final_fields.get("e") != "invalid-proof":
            raise AssertionError(f"server did not expose SCRAM proof failure: {final_fields!r}")
        stream.write(b"\r\n")
        return expect(stream, b"-ERR authentication failed", "invalid proof")
    signature = final_fields.get("v")
    if signature != expected_server_signature:
        raise AssertionError("server-final signature did not match the SCRAM transcript")
    stream.write(b"\r\n")
    return expect(stream, expected_completion, "SCRAM success")


def open_tls(port: int) -> tuple[ssl.SSLSocket, BinaryIO]:
    """Открывает новую POP3 TLS сессию, выполняя обязательный STLS upgrade."""
    raw = socket.create_connection(("127.0.0.1", port), timeout=10)
    raw.settimeout(10)
    clear = raw.makefile("rwb", buffering=0)
    expect(clear, b"+OK", "second greeting")
    clear.write(b"STLS\r\n")
    expect(clear, b"+OK", "second STLS")
    clear.close()
    context = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
    context.check_hostname = False
    context.verify_mode = ssl.CERT_NONE
    secure = context.wrap_socket(raw, server_hostname="localhost")
    return secure, secure.makefile("rwb", buffering=0)


def run(port: int, credentials: Path) -> None:
    """Проверяет TLS-only advertisement, both SCRAM flows, recovery and maildrop."""
    password = password_from(credentials)
    raw = socket.create_connection(("127.0.0.1", port), timeout=10)
    raw.settimeout(10)
    stream = raw.makefile("rwb", buffering=0)
    expect(stream, b"+OK", "greeting")
    stream.write(b"CAPA\r\n")
    clear = bytearray()
    while True:
        line = read_line(stream)
        if line == b".\r\n":
            break
        clear.extend(line)
    if b"SASL" in clear:
        raise AssertionError("plaintext CAPA exposed SASL mechanisms")
    stream.write(b"STLS\r\n")
    expect(stream, b"+OK", "STLS")
    stream.close()
    context = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
    context.check_hostname = False
    context.verify_mode = ssl.CERT_NONE
    with context.wrap_socket(raw, server_hostname="localhost") as secure:
        tls = secure.makefile("rwb", buffering=0)
        tls.write(b"CAPA\r\n")
        capability = bytearray()
        while True:
            line = read_line(tls)
            if line == b".\r\n":
                break
            capability.extend(line)
        if b"SASL PLAIN SCRAM-SHA-1\r\n" not in capability:
            raise AssertionError("TLS CAPA omitted SCRAM-SHA-1")
        invalid_reply = authenticate(tls, password, no_initial=False, invalid=True)
        if not invalid_reply.startswith(b"-ERR"):
            raise AssertionError("invalid proof unexpectedly authenticated")
        success = authenticate(tls, password, no_initial=True, invalid=False)
        if not success.startswith(b"+OK"):
            raise AssertionError("valid SCRAM proof was rejected")
        tls.write(b"STAT\r\n")
        expect(tls, b"+OK", "STAT after SCRAM")

        second_socket, second = open_tls(port)
        denied = authenticate(
            second,
            password,
            no_initial=False,
            invalid=False,
            expected_completion=b"-ERR [IN-USE]",
        )
        if not denied.startswith(b"-ERR [IN-USE]"):
            raise AssertionError("second SCRAM session acquired an owned maildrop")
        second.write(b"AUTH SCRAM-SHA-1\r\n")
        if read_line(second) != b"+ \r\n":
            raise AssertionError("maildrop conflict left SCRAM session unusable")
        second.write(b"*\r\n")
        expect(second, b"-ERR authentication cancelled", "retry after maildrop conflict")
        second.write(b"QUIT\r\n")
        expect(second, b"+OK", "second QUIT")
        second.close()
        second_socket.close()

        tls.write(b"QUIT\r\n")
        expect(tls, b"+OK", "QUIT")
        tls.close()
        secure.close()

        third_socket, third = open_tls(port)
        authenticate(third, password, no_initial=False, invalid=False)
        third.write(b"QUIT\r\n")
        expect(third, b"+OK", "QUIT after lease reuse")
        third.close()
        third_socket.close()


def main() -> None:
    """Разбирает адрес запуска и выполняет один независимый STARTTLS сценарий."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--port", type=int, required=True)
    parser.add_argument("--credentials", type=Path, required=True)
    options = parser.parse_args()
    run(options.port, options.credentials)
    print("PASS POP3 RFC 5034 SCRAM-SHA-1 STARTTLS exchange")


if __name__ == "__main__":
    main()
