"""Минимальный независимый SMTP peer, завершающий RCPT постоянным отказом."""

import argparse
import socket


def command(client: socket.socket) -> bytes:
    """Читает одну CRLF command без использования реализации litterkeep."""
    result = bytearray()
    while not result.endswith(b"\r\n"):
        chunk = client.recv(4096)
        if chunk == b"":
            raise AssertionError("client closed SMTP connection")
        result.extend(chunk)
    return bytes(result)


def require_prefix(actual: bytes, expected: bytes) -> None:
    """Проверяет стандартный SMTP command в ожидаемой последовательности."""
    if not actual.startswith(expected):
        raise AssertionError(f"expected {expected!r}, got {actual!r}")


def main() -> None:
    """Принимает один SMTP-сеанс и возвращает 550 после RCPT."""
    parser = argparse.ArgumentParser()
    parser.add_argument("--port", type=int, required=True)
    options = parser.parse_args()
    with socket.socket(socket.AF_INET, socket.SOCK_STREAM) as listener:
        listener.setsockopt(socket.SOL_SOCKET, socket.SO_REUSEADDR, 1)
        listener.bind(("127.0.0.1", options.port))
        listener.listen(1)
        client, _ = listener.accept()
        with client:
            client.sendall(b"220 reject peer ready\r\n")
            require_prefix(command(client), b"EHLO")
            client.sendall(b"250 peer\r\n")
            require_prefix(command(client), b"MAIL FROM:")
            client.sendall(b"250 sender\r\n")
            require_prefix(command(client), b"RCPT TO:")
            client.sendall(b"550 5.1.1 unknown recipient\r\n")
            require_prefix(command(client), b"RSET")
            client.sendall(b"250 reset\r\n")
    print("SMTP 550 peer: PASS")


if __name__ == "__main__":
    main()
