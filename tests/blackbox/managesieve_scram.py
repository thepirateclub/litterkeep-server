"""Проверяет независимым TLS peer полный ManageSieve SCRAM-SHA-1 exchange."""

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
    """Читает одну bounded CRLF строку, не смешивая SASL frame-ы."""
    line = stream.readline(8193)
    if not line.endswith(b"\r\n") or len(line) > 8192:
        raise AssertionError(f"invalid ManageSieve line: {line!r}")
    return line


def read_result(stream: BinaryIO) -> tuple[list[bytes], bytes]:
    """Собирает capability block до его терминальной OK/NO строки."""
    lines: list[bytes] = []
    while True:
        line = read_line(stream)
        if line.startswith((b"OK", b"NO", b"BYE")):
            return lines, line
        lines.append(line)


def credentials(path: Path) -> str:
    """Загружает fixture-пароль локально, не выводя его в журнал."""
    for line in path.read_text(encoding="utf-8").splitlines():
        fields = line.split("\t", maxsplit=1)
        match fields:
            case ["bob", password]:
                return password
            case _:
                continue
    raise AssertionError("bob fixture credential is missing")


def open_tls(port: int, ca: Path) -> ssl.SSLSocket:
    """Открывает implicit TLS ManageSieve соединение с проверкой CA/SAN."""
    context = ssl.create_default_context(cafile=ca)
    raw = socket.create_connection(("127.0.0.1", port), timeout=10)
    raw.settimeout(10)
    return context.wrap_socket(raw, server_hostname="localhost")


def authenticate(
    stream: BinaryIO, password: str, no_initial: bool, invalid: bool
) -> tuple[bytes, str]:
    """Проходит client-first, verifier challenge, proof и читает финальный ответ."""
    nonce = secrets.token_urlsafe(18)
    first_bare = f"n=bob,r={nonce}"
    first = f"n,,{first_bare}".encode("utf-8")
    initial = base64.b64encode(first)
    if no_initial:
        stream.write(b'AUTHENTICATE "SCRAM-SHA-1"\r\n')
        if read_line(stream) != b'""\r\n':
            raise AssertionError("SCRAM without initial response missed empty challenge")
        stream.write(b'"' + initial + b'"\r\n')
    else:
        stream.write(b'AUTHENTICATE "SCRAM-SHA-1" "' + initial + b'"\r\n')
    server_first_line = read_line(stream)
    server_first = decode_quoted(server_first_line)
    server_fields = parse_fields(server_first)
    combined_nonce = server_fields.get("r")
    salt_text = server_fields.get("s")
    iterations_text = server_fields.get("i")
    if combined_nonce is None or not combined_nonce.startswith(nonce):
        raise AssertionError(f"server nonce does not extend the client nonce: {server_first!r}")
    if salt_text is None or iterations_text is None:
        raise AssertionError(f"server challenge lacks verifier parameters: {server_first!r}")
    salt = base64.b64decode(salt_text, validate=True)
    iterations = int(iterations_text)
    salted = hashlib.pbkdf2_hmac("sha1", password.encode("utf-8"), salt, iterations)
    unsigned = f"c=biws,r={combined_nonce}"
    auth_message = f"{first_bare},{server_first},{unsigned}".encode("utf-8")
    client_key = hmac.digest(salted, b"Client Key", "sha1")
    stored_key = hashlib.sha1(client_key).digest()
    signature = hmac.digest(stored_key, auth_message, "sha1")
    server_key = hmac.digest(salted, b"Server Key", "sha1")
    expected_server_signature = base64.b64encode(
        hmac.digest(server_key, auth_message, "sha1")
    ).decode("ascii")
    proof = bytes(left ^ right for left, right in zip(client_key, signature, strict=True))
    if invalid:
        proof = bytes(len(proof))
    final = f"{unsigned},p={base64.b64encode(proof).decode('ascii')}".encode("ascii")
    stream.write(b'"' + base64.b64encode(final) + b'"\r\n')
    _, completion = read_result(stream)
    return completion, expected_server_signature


def decode_quoted(line: bytes) -> str:
    """Проверяет quoted SASL response и декодирует его Base64 payload."""
    if not line.startswith(b'"') or not line.endswith(b'"\r\n'):
        raise AssertionError(f"expected quoted SASL data: {line!r}")
    encoded = line.removeprefix(b'"').removesuffix(b'"\r\n')
    return base64.b64decode(encoded, validate=True).decode("utf-8")


def parse_fields(message: str) -> dict[str, str]:
    """Разбирает SCRAM challenge fields после Base64 framing проверки."""
    return dict(field.split("=", maxsplit=1) for field in message.split(","))


def verify_success(stream: BinaryIO, password: str) -> None:
    """Сверяет mutual server signature и проверяет logged-in NOOP state."""
    completion, expected_signature = authenticate(
        stream, password, no_initial=False, invalid=False
    )
    prefix = b'OK (SASL "'
    if not completion.startswith(prefix) or b'"authenticated"\r\n' not in completion:
        raise AssertionError(f"SCRAM authentication failed: {completion!r}")
    encoded = first_segment(completion.removeprefix(prefix), b'"')
    if encoded is None:
        raise AssertionError("successful SCRAM response omitted server-final data")
    server_final = decode_sasl_final(encoded)
    fields = parse_fields(server_final)
    signature_text = fields.get("v")
    if signature_text is None or signature_text != expected_signature:
        raise AssertionError(f"server-final lacks signature: {server_final!r}")
    stream.write(b"NOOP\r\n")
    if read_line(stream) != b'OK "noop"\r\n':
        raise AssertionError("authenticated SCRAM session did not accept NOOP")


def decode_sasl_final(encoded: bytes) -> str:
    """Проверяет Base64 response-code payload и возвращает server-final text."""
    return base64.b64decode(encoded, validate=True).decode("utf-8")


def first_segment(value: bytes, separator: bytes) -> bytes | None:
    """Извлекает первую разделённую часть без числового доступа к массиву."""
    return next(iter(value.split(separator, maxsplit=1)), None)


def verify_rejection(stream: BinaryIO, password: str) -> None:
    """Проверяет client proof отказ и восстановление неаутентифицированного FSM."""
    completion, _ = authenticate(stream, password, no_initial=True, invalid=True)
    prefix = b'NO (SASL "'
    if not completion.startswith(prefix):
        raise AssertionError(f"bad proof missed SASL server-final error: {completion!r}")
    encoded = first_segment(completion.removeprefix(prefix), b'"')
    if encoded is None or decode_sasl_final(encoded) != "e=invalid-proof":
        raise AssertionError(f"bad proof did not map to invalid-proof: {completion!r}")
    stream.write(b"NOOP\r\n")
    if read_line(stream) != b'OK "noop"\r\n':
        raise AssertionError("failed SCRAM exchange corrupted unauthenticated session")


def main() -> None:
    """Проверяет TLS capabilities и положительный/отрицательный SCRAM flows."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--port", type=int, required=True)
    parser.add_argument("--ca", type=Path, required=True)
    parser.add_argument("--credentials", type=Path, required=True)
    options = parser.parse_args()
    password = credentials(options.credentials)
    with open_tls(options.port, options.ca) as success:
        with success.makefile("rwb", buffering=0) as stream:
            greeting, terminal = read_result(stream)
            if terminal != b'OK "ready"\r\n':
                raise AssertionError(f"unexpected server greeting: {terminal!r}")
            if b'"SASL" "PLAIN SCRAM-SHA-1 SCRAM-SHA-256"\r\n' not in greeting:
                raise AssertionError(f"SCRAM capabilities missing: {greeting!r}")
            verify_success(stream, password)
    with open_tls(options.port, options.ca) as rejection:
        with rejection.makefile("rwb", buffering=0) as stream:
            _, terminal = read_result(stream)
            if terminal != b'OK "ready"\r\n':
                raise AssertionError(f"unexpected rejection greeting: {terminal!r}")
            verify_rejection(stream, password)
    print("PASS ManageSieve SCRAM-SHA-1 independent TLS exchange")


if __name__ == "__main__":
    main()
