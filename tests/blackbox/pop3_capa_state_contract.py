"""Проверяет RFC 2449 CAPA в четырёх POP3 протокольных состояниях."""

from __future__ import annotations

import argparse
import base64
import os
import socket
import ssl


def read_line(stream: socket.SocketIO, context: str) -> bytes:
    """Читает полную POP3 CRLF-строку, сохраняя диагностику wire-обрыва."""
    line = stream.readline()
    if not line.endswith(b"\r\n"):
        raise AssertionError(f"{context}: сервер оборвал строку {line!r}")
    return line


def read_capabilities(stream: socket.SocketIO, phase: str) -> set[bytes]:
    """Считывает CAPA целиком и возвращает регистронезависимые wire-теги."""
    stream.write(b"CAPA\r\n")
    stream.flush()
    greeting = read_line(stream, f"{phase} CAPA status")
    if not greeting.startswith(b"+OK"):
        raise AssertionError(f"{phase}: CAPA завершился отказом {greeting!r}")
    lines: list[bytes] = []
    while True:
        line = read_line(stream, f"{phase} CAPA body")
        if line == b".\r\n":
            break
        lines.append(line.upper())
    return set(lines)


def require_capabilities(
    actual: set[bytes], phase: str, *, starttls: bool, sasl: bool
) -> None:
    """Сверяет capability-переключение относительно границы TLS."""
    expected: set[bytes] = {
        b"USER\r\n",
        b"UIDL\r\n",
        b"TOP\r\n",
        b"RESP-CODES\r\n",
        b"UTF8\r\n",
        b"LANG\r\n",
    }
    match starttls:
        case True:
            expected.add(b"STLS\r\n")
        case False:
            if b"STLS\r\n" in actual:
                raise AssertionError(f"{phase}: STLS остался после TLS")
    match sasl:
        case True:
            expected.add(b"SASL PLAIN\r\n")
        case False:
            if b"SASL PLAIN\r\n" in actual:
                raise AssertionError(f"{phase}: SASL рекламируется до TLS")
    if actual != expected:
        raise AssertionError(f"{phase}: CAPA {actual!r}, ожидалось {expected!r}")


def exchange(stream: socket.SocketIO, command: bytes, expected: bytes) -> bytes:
    """Проверяет single-line ответ перед чтением следующей команды."""
    stream.write(command + b"\r\n")
    stream.flush()
    reply = read_line(stream, command.decode("ascii"))
    if not reply.startswith(expected):
        raise AssertionError(f"{command!r}: получен неожиданный ответ {reply!r}")
    return reply


def verify(host: str, port: int, password: str) -> None:
    """Сверяет CAPA до/после STLS, после USER и в TRANSACTION."""
    connection = socket.create_connection((host, port), timeout=10)
    stream = connection.makefile("rwb", buffering=0)
    if not read_line(stream, "greeting").startswith(b"+OK"):
        raise AssertionError("неожиданный POP3 greeting")
    require_capabilities(
        read_capabilities(stream, "AUTHORIZATION без TLS"),
        "AUTHORIZATION без TLS",
        starttls=True,
        sasl=False,
    )
    exchange(stream, b"STLS", b"+OK")
    stream.close()
    context = ssl.SSLContext(ssl.PROTOCOL_TLS_CLIENT)
    context.check_hostname = False
    context.verify_mode = ssl.CERT_NONE
    with context.wrap_socket(connection, server_hostname=host) as secure:
        secure_stream = secure.makefile("rwb", buffering=0)
        require_capabilities(
            read_capabilities(secure_stream, "AUTHORIZATION после TLS"),
            "AUTHORIZATION после TLS",
            starttls=False,
            sasl=True,
        )
        exchange(secure_stream, b"USER bob", b"+OK")
        require_capabilities(
            read_capabilities(secure_stream, "AUTHORIZATION после USER"),
            "AUTHORIZATION после USER",
            starttls=False,
            sasl=True,
        )
        response = base64.b64encode(b"\x00bob\x00" + password.encode("ascii"))
        exchange(secure_stream, b"AUTH PLAIN " + response, b"+OK")
        require_capabilities(
            read_capabilities(secure_stream, "TRANSACTION"),
            "TRANSACTION",
            starttls=False,
            sasl=True,
        )
        exchange(secure_stream, b"QUIT", b"+OK")
        secure_stream.close()


def main() -> None:
    """Запускает независимую POP3 STARTTLS проверку capability-state matrix."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, required=True)
    parser.add_argument("--password", default=os.environ.get("LITTERKEEP_BOB_PASSWORD"))
    options = parser.parse_args()
    if options.password is None:
        raise SystemExit("укажите --password или LITTERKEEP_BOB_PASSWORD")
    verify(options.host, options.port, options.password)
    print("PASS POP3 CAPA: auth clear/TLS/USER and transaction states")


if __name__ == "__main__":
    main()
