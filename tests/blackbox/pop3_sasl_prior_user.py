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
            authenticate_plain(protected, response, b"+OK")
            verify_sasl_capability_after_auth(protected)
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
    long_password = b"x" * 256
    long_response = base64.b64encode(b"\x00bob\x00" + long_password)
    authenticate_plain(stream, long_response, b"-ERR authentication failed")
    maximum_password = b"x" * 4096
    maximum_response = base64.b64encode(b"\x00bob\x00" + maximum_password)
    authenticate_plain(stream, maximum_response, b"-ERR authentication failed")
    verify_maximum_scram_first(stream)
    exchange(stream, b"AUTH PLAIN", b"+ ")
    exchange(stream, b"*", b"-ERR authentication cancelled")
    exchange(stream, b"AUTH PLAIN", b"+ ")
    exchange(stream, b"!!!", b"-ERR invalid AUTH PLAIN credentials")
    invalid = base64.b64encode(b"\x00bob\x00wrong-password")
    exchange(stream, b"AUTH PLAIN " + invalid, b"-ERR authentication failed")
    valid = base64.b64encode(b"\x00missing-account\x00" + password.encode("ascii"))
    exchange(stream, b"AUTH PLAIN " + valid, b"-ERR authentication failed")
    verify_oversized_continuation(stream)


def verify_oversized_continuation(stream: socket.SocketIO) -> None:
    """Проверяет bounded discard, CRLF boundary и следующую POP3 команду."""
    exchange(stream, b"AUTH PLAIN", b"+ ")
    response = b"A" * (1024 * 1024 + 1) + b"\r\nCAPA\r\n"
    write_all(stream, response)
    oversized = read_line(stream, "oversized SASL response")
    if oversized != b"-ERR authentication failed\r\n":
        raise AssertionError(f"неожиданный oversized SASL отказ: {oversized!r}")
    capabilities = read_line(stream, "pipelined CAPA after oversized SASL")
    if not capabilities.startswith(b"+OK"):
        raise AssertionError(f"CAPA не восстановилась после SASL: {capabilities!r}")
    while read_line(stream, "CAPA after oversized SASL") != b".\r\n":
        pass


def write_all(stream: socket.SocketIO, payload: bytes) -> None:
    """Передаёт крупный acceptance payload целиком даже при частичной записи."""
    remaining = memoryview(payload)
    while remaining:
        written = stream.write(remaining)
        match written:
            case None | 0:
                raise ConnectionError("socket перестал принимать POP3 acceptance bytes")
            case count:
                remaining = remaining[count:]


def verify_maximum_scram_first(stream: socket.SocketIO) -> None:
    """Проверяет max client-first и следующий client-final с nonce overhead."""
    exchange(stream, b"AUTH SCRAM-SHA-256", b"+ ")
    username_and_nonce = b"n=bob,r="
    maximum_message_size = 700 * 1024
    client_nonce = b"A" * (maximum_message_size - len(username_and_nonce))
    client_first = username_and_nonce + client_nonce
    sasl_payload = base64.b64encode(b"n,," + client_first)
    write_all(stream, sasl_payload + b"\r\n")
    challenge = read_line(stream, "maximum SCRAM server-first challenge")
    if not challenge.startswith(b"+ "):
        raise AssertionError(f"максимальный SCRAM response отклонён: {challenge!r}")
    encoded_challenge = challenge.removeprefix(b"+ ").removesuffix(b"\r\n")
    server_first = base64.b64decode(encoded_challenge, validate=True)
    nonce_field, separator, _ = server_first.partition(b",")
    match (separator, nonce_field.startswith(b"r=" + client_nonce)):
        case (b",", True):
            pass
        case _:
            raise AssertionError("SCRAM challenge потерял полный client nonce prefix")
    server_nonce = nonce_field.removeprefix(b"r=")
    client_final = b"c=biws,r=" + server_nonce + b",p="
    client_final += base64.b64encode(bytes(32))
    if len(client_final) != maximum_message_size + 72:
        raise AssertionError(f"неожиданная граница SCRAM client-final: {len(client_final)}")
    write_all(stream, base64.b64encode(client_final) + b"\r\n")
    server_final = read_line(stream, "maximum SCRAM client-final response")
    if not server_final.startswith(b"+ "):
        raise AssertionError(f"maximal SCRAM client-final отклонён: {server_final!r}")
    result = base64.b64decode(
        server_final.removeprefix(b"+ ").removesuffix(b"\r\n"), validate=True
    )
    if result != b"e=invalid-proof":
        raise AssertionError(f"неожиданный server-final на неверный proof: {result!r}")
    failed = exchange(stream, b"", b"-ERR")
    if failed != b"-ERR authentication failed\r\n":
        raise AssertionError(f"неожиданный исход невалидного SCRAM proof: {failed!r}")


def authenticate_plain(
    stream: socket.SocketIO,
    response: bytes,
    result_prefix: bytes,
) -> bytes:
    """Выбирает RFC 5034 initial-response или continuation по размеру команды."""
    initial_command = b"AUTH PLAIN " + response
    if len(initial_command) + len(b"\r\n") <= 255:
        return exchange(stream, initial_command, result_prefix)
    exchange(stream, b"AUTH PLAIN", b"+ ")
    return exchange(stream, response, result_prefix)


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
    match sasl_lines:
        case [sasl_line]:
            if b"DIGEST-MD5" in sasl_line:
                raise AssertionError("CAPA unexpectedly advertises DIGEST-MD5")
        case _:
            raise AssertionError(
                f"CAPA advertises unexpected SASL mechanisms: {sasl_lines!r}"
            )
    exchange(
        stream,
        b"AUTH DIGEST-MD5 ZHVtbXk=",
        b"-ERR unsupported SASL mechanism",
    )


def verify_sasl_capability_after_auth(stream: socket.SocketIO) -> None:
    """Проверяет, что успешный AUTH не удалил CAPA SASL из transaction state."""
    exchange(stream, b"CAPA", b"+OK")
    sasl_lines = []
    while True:
        line = read_line(stream, "post-auth SASL capability listing")
        if line == b".\r\n":
            break
        if line.startswith(b"SASL "):
            sasl_lines.append(line)
    match sasl_lines:
        case [sasl_line] if b"PLAIN" in sasl_line:
            return
        case _:
            raise AssertionError(
                f"успешный AUTH удалил или исказил SASL capability: {sasl_lines!r}"
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
