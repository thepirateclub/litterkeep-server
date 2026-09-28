"""Независимо проверяет TLS wire exchange ManageSieve SCRAM-SHA-256."""

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


def line(stream: BinaryIO) -> bytes:
    """Читает ровно одну bounded CRLF строку протокола."""
    value = stream.readline(8193)
    match (len(value) <= 8192, value.endswith(b"\r\n")):
        case (True, True):
            pass
        case _:
            raise AssertionError(f"invalid line {value!r}")
    return value


def result(stream: BinaryIO) -> tuple[list[bytes], bytes]:
    """Читает строки capability до terminal result."""
    lines: list[bytes] = []
    while True:
        value = line(stream)
        if value.startswith((b"OK", b"NO", b"BYE")):
            return lines, value
        lines.append(value)


def password(path: Path) -> str:
    """Загружает пароль bob из локальной acceptance fixture."""
    for row in path.read_text(encoding="utf-8").splitlines():
        fields = row.split("\t", maxsplit=1)
        match fields:
            case ["bob", value]:
                return value
            case _:
                continue
    raise AssertionError("bob credential is missing")


def decode_quoted(value: bytes) -> str:
    """Декодирует quoted Base64 ответ сервера с проверкой каноничности."""
    match (value.startswith(b'"'), value.endswith(b'"\r\n')):
        case (True, True):
            pass
        case _:
            raise AssertionError(f"expected quoted response: {value!r}")
    encoded = value.removeprefix(b'"').removesuffix(b'"\r\n')
    return base64.b64decode(encoded, validate=True).decode("ascii")


def authenticate(stream: BinaryIO, secret: str, invalid: bool) -> str:
    """Выполняет SHA-256 exchange, возвращая terminal SASL result."""
    nonce = secrets.token_urlsafe(20)
    bare = f"n=bob,r={nonce}"
    first = base64.b64encode(f"n,,{bare}".encode("ascii"))
    stream.write(b'AUTHENTICATE "SCRAM-SHA-256"\r\n')
    if line(stream) != b'""\r\n':
        raise AssertionError("missing initial SASL challenge")
    stream.write(b'"' + first + b'"\r\n')
    server_text = decode_quoted(line(stream))
    server_fields = dict(field.split("=", maxsplit=1) for field in server_text.split(","))
    server_nonce = server_fields.get("r")
    salt_text = server_fields.get("s")
    iterations_text = server_fields.get("i")
    match (server_nonce, salt_text, iterations_text):
        case (str(server_nonce), str(salt_text), str(iterations_text)):
            pass
        case _:
            raise AssertionError("server-first omitted SCRAM verifier parameters")
    if not server_nonce.startswith(nonce):
        raise AssertionError("server nonce does not extend client nonce")
    salt = base64.b64decode(salt_text, validate=True)
    salted = hashlib.pbkdf2_hmac("sha256", secret.encode(), salt, int(iterations_text))
    unsigned = f"c=biws,r={server_nonce}"
    transcript = f"{bare},{server_text},{unsigned}".encode("ascii")
    client_key = hmac.digest(salted, b"Client Key", "sha256")
    stored_key = hashlib.sha256(client_key).digest()
    signature = hmac.digest(stored_key, transcript, "sha256")
    server_key = hmac.digest(salted, b"Server Key", "sha256")
    expected = base64.b64encode(hmac.digest(server_key, transcript, "sha256")).decode()
    proof = bytes(left ^ right for left, right in zip(client_key, signature, strict=True))
    if invalid:
        proof = bytes(32)
    final = f"{unsigned},p={base64.b64encode(proof).decode()}".encode("ascii")
    stream.write(b'"' + base64.b64encode(final) + b'"\r\n')
    _, completion = result(stream)
    if completion.startswith(b'OK (SASL "'):
        remainder = completion.removeprefix(b'OK (SASL "')
        encoded = next(iter(remainder.split(b'"', maxsplit=1)), b"")
        server_final = base64.b64decode(encoded, validate=True).decode("ascii")
        if server_final != f"v={expected}":
            raise AssertionError("mutual server signature mismatch")
    return completion.decode("ascii")


def connect(port: int, ca: Path, starttls: bool) -> tuple[ssl.SSLSocket, list[bytes], bytes]:
    """Подключает независимого peer через implicit TLS либо STARTTLS."""
    context = ssl.create_default_context(cafile=ca)
    peer = socket.create_connection(("127.0.0.1", port), timeout=10)
    peer.settimeout(10)
    if starttls:
        with peer.makefile("rwb", buffering=0) as clear:
            greeting, terminal = result(clear)
            match (
                terminal,
                any(item == b'"SASL" ""\r\n' for item in greeting),
                any(item == b'"STARTTLS"\r\n' for item in greeting),
            ):
                case (b'OK "ready"\r\n', True, True):
                    pass
                case _:
                    raise AssertionError("unexpected pre-TLS ManageSieve greeting")
            clear.write(b"STARTTLS\r\n")
            if line(clear) != b'OK "begin TLS"\r\n':
                raise AssertionError("STARTTLS upgrade was rejected")
        peer = context.wrap_socket(peer, server_hostname="localhost")
    else:
        peer = context.wrap_socket(peer, server_hostname="localhost")
    stream = peer.makefile("rwb", buffering=0)
    greeting, terminal = result(stream)
    if terminal != b'OK "ready"\r\n':
        stream.close()
        peer.close()
        raise AssertionError("unexpected secure ManageSieve greeting")
    stream.close()
    return peer, greeting, terminal


def main() -> None:
    """Проверяет capability, mutual success и invalid-proof recovery."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--port", type=int, required=True)
    parser.add_argument("--ca", type=Path, required=True)
    parser.add_argument("--credentials", type=Path, required=True)
    parser.add_argument("--starttls", action="store_true")
    options = parser.parse_args()
    secret = password(options.credentials)
    peer, greeting, terminal = connect(options.port, options.ca, options.starttls)
    with peer, peer.makefile("rwb", buffering=0) as stream:
        capability = b'"SASL" "PLAIN SCRAM-SHA-1 SCRAM-SHA-256 SCRAM-SHA-256-PLUS"\r\n'
        match (terminal == b'OK "ready"\r\n', capability in greeting):
            case (True, True):
                pass
            case _:
                raise AssertionError(f"SHA-256 capability missing: {greeting!r}")
        if not authenticate(stream, secret, invalid=False).startswith("OK (SASL "):
            raise AssertionError("valid proof was rejected")
        stream.write(b"UNAUTHENTICATE\r\n")
        _, reset = result(stream)
        if reset != b'OK "unauthenticated"\r\n':
            raise AssertionError("UNAUTHENTICATE did not reset the owner")
        rejection = authenticate(stream, secret, invalid=True)
        encoded_error = rejection.removeprefix('NO (SASL "').split('"', maxsplit=1)
        error_data = next(iter(encoded_error), "")
        if not rejection.startswith('NO (SASL "'):
            raise AssertionError("invalid proof did not use a SASL response code")
        if base64.b64decode(error_data, validate=True).decode("ascii") != "e=invalid-proof":
            raise AssertionError("invalid proof did not return invalid-proof")
        stream.write(b"NOOP\r\n")
        if line(stream) != b'OK "noop"\r\n':
            raise AssertionError("failed exchange did not recover the session")
    print("PASS independent ManageSieve SCRAM-SHA-256 TLS exchange")


if __name__ == "__main__":
    main()
