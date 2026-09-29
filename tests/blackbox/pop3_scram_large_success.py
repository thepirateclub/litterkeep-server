"""Проверяет POP3 SCRAM-SHA-256 transcript больше прежних входных caps."""

import argparse
import base64
from enum import Enum
import hashlib
import hmac
import socket
import ssl
from pathlib import Path
from typing import BinaryIO


SCRAM_NONCE_SIZE = 2 * 1024 * 1024
MAX_POP3_LINE = 4 * 1024 * 1024


class FixtureAccount(Enum):
    """Типизированная учётная запись из изолированного acceptance fixture."""

    BOB = "bob"


def read_line(stream: BinaryIO, label: str) -> bytes:
    """Читает POP3 строку в известном транспортном бюджете и требует CRLF."""
    line = stream.readline(MAX_POP3_LINE + 1)
    if len(line) > MAX_POP3_LINE or not line.endswith(b"\r\n"):
        raise AssertionError(f"{label}: oversized or incomplete response")
    return line


def expect(stream: BinaryIO, prefix: bytes, label: str) -> bytes:
    """Проверяет статус без вывода чувствительного SASL transcript-а."""
    response = read_line(stream, label)
    if not response.startswith(prefix):
        raise AssertionError(f"{label}: unexpected POP3 status {response[:80]!r}")
    return response


def credentials(path: Path) -> str:
    """Берёт пароль Bob из локального fixture без передачи его в argv."""
    for line in path.read_text(encoding="utf-8").splitlines():
        fields = line.split("\t", maxsplit=1)
        match fields:
            case [account_name, password]:
                try:
                    account = FixtureAccount(account_name)
                except ValueError:
                    continue
                if account is FixtureAccount.BOB:
                    return password
            case _:
                continue
    raise ValueError("fixture does not contain Bob")


def tls_session(host: str, port: int, ca: Path) -> tuple[ssl.SSLSocket, BinaryIO]:
    """Выполняет POP3 STLS и валидирует сертификат независимым TLS клиентом."""
    raw = socket.create_connection((host, port), timeout=30)
    raw.settimeout(30)
    clear = raw.makefile("rwb", buffering=0)
    expect(clear, b"+OK", "greeting")
    clear.write(b"STLS\r\n")
    expect(clear, b"+OK", "STLS")
    clear.close()
    context = ssl.create_default_context(cafile=str(ca))
    secured = context.wrap_socket(raw, server_hostname="localhost")
    return secured, secured.makefile("rwb", buffering=0)


def provision_verifier(host: str, port: int, ca: Path, password: str) -> None:
    """Создаёт SHA-256 sidecar штатным успешным USER/PASS входом."""
    secured, stream = tls_session(host, port, ca)
    with secured:
        stream.write(b"USER bob\r\n")
        expect(stream, b"+OK", "USER")
        stream.write(f"PASS {password}\r\n".encode("utf-8"))
        expect(stream, b"+OK", "PASS")
        stream.write(b"QUIT\r\n")
        expect(stream, b"+OK", "provisioning QUIT")
        stream.close()


def response_fields(line: bytes) -> dict[bytes, bytes]:
    """Декодирует SCRAM challenge/status line и отвергает повторные поля."""
    if not line.startswith(b"+ ") or not line.endswith(b"\r\n"):
        raise AssertionError("malformed SCRAM challenge framing")
    encoded = line.removeprefix(b"+ ").removesuffix(b"\r\n")
    message = base64.b64decode(encoded, validate=True)
    return parse_fields(message)


def parse_fields(message: bytes) -> dict[bytes, bytes]:
    """Разбирает атрибуты SCRAM без потери байтового wire transcript-а."""
    fields: dict[bytes, bytes] = {}
    for item in message.split(b","):
        name, separator, value = item.partition(b"=")
        if not separator or name in fields:
            raise AssertionError("malformed SCRAM server-first challenge")
        fields[name] = value
    return fields


def proof(password: str, first_bare: bytes, server_first: bytes) -> tuple[bytes, bytes]:
    """Формирует SCRAM client proof и ожидаемую независимую server signature."""
    fields = parse_fields(server_first)
    nonce = fields.get(b"r")
    salt_text = fields.get(b"s")
    iterations_text = fields.get(b"i")
    if nonce is None or salt_text is None or iterations_text is None:
        raise AssertionError("SCRAM server-first omitted mandatory fields")
    client_nonce = first_bare.removeprefix(b"n=bob,r=")
    if not nonce.startswith(client_nonce):
        raise AssertionError("server nonce does not extend the client nonce")
    salt = base64.b64decode(salt_text, validate=True)
    iterations = int(iterations_text)
    salted = hashlib.pbkdf2_hmac("sha256", password.encode("utf-8"), salt, iterations)
    final_without_proof = b"c=biws,r=" + nonce
    auth_message = b",".join((first_bare, server_first, final_without_proof))
    client_key = hmac.digest(salted, b"Client Key", "sha256")
    stored_key = hashlib.sha256(client_key).digest()
    client_signature = hmac.digest(stored_key, auth_message, "sha256")
    client_proof = bytes(left ^ right for left, right in zip(client_key, client_signature, strict=True))
    server_key = hmac.digest(salted, b"Server Key", "sha256")
    expected_signature = base64.b64encode(hmac.digest(server_key, auth_message, "sha256"))
    final = final_without_proof + b",p=" + base64.b64encode(client_proof)
    return base64.b64encode(final), expected_signature


def authenticate_large(stream: BinaryIO, password: str) -> None:
    """Проходит proof с максимальным client nonce и проверяет взаимную подпись."""
    stream.write(b"AUTH SCRAM-SHA-256\r\n")
    expect(stream, b"+ ", "AUTH start")
    prefix = b"n=bob,r="
    client_nonce = b"A" * (SCRAM_NONCE_SIZE - len(prefix))
    first_bare = prefix + client_nonce
    stream.write(base64.b64encode(b"n,," + first_bare) + b"\r\n")
    challenge = expect(stream, b"+ ", "large server-first")
    encoded_challenge = challenge.removeprefix(b"+ ").removesuffix(b"\r\n")
    server_first = base64.b64decode(encoded_challenge, validate=True)
    client_final, expected_signature = proof(password, first_bare, server_first)
    if len(client_final) >= MAX_POP3_LINE:
        raise AssertionError("client-final exceeds the configured POP3 line budget")
    stream.write(client_final + b"\r\n")
    final = expect(stream, b"+ ", "server-final")
    final_fields = response_fields(final)
    if final_fields.get(b"v") != expected_signature:
        raise AssertionError("server-final signature does not match client transcript")
    stream.write(b"\r\n")
    expect(stream, b"+OK", "SCRAM authentication completion")


def run(host: str, port: int, ca: Path, password: str) -> None:
    """Проверяет TLS-only SCRAM и доступ к maildrop после большого proof."""
    provision_verifier(host, port, ca, password)
    secured, stream = tls_session(host, port, ca)
    with secured:
        authenticate_large(stream, password)
        stream.write(b"STAT\r\n")
        expect(stream, b"+OK", "STAT after large SCRAM")
        stream.write(b"QUIT\r\n")
        expect(stream, b"+OK", "QUIT")
        stream.close()


def main() -> None:
    """Запускает независимый POP3 acceptance для большой SCRAM границы."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, required=True)
    parser.add_argument("--ca", type=Path, required=True)
    parser.add_argument("--credentials", type=Path, required=True)
    options = parser.parse_args()
    run(options.host, options.port, options.ca, credentials(options.credentials))
    print("PASS POP3 SCRAM-SHA-256 2 MiB nonce transcript")


if __name__ == "__main__":
    main()
