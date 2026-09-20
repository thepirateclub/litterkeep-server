"""Удерживает заданное число реальных IMAP IDLE-сессий для измерения."""

import argparse
import socket
import ssl
import time


def arguments() -> argparse.Namespace:
    """Читает endpoint, trust anchor и точный профиль удержания."""
    parser = argparse.ArgumentParser()
    parser.add_argument("host")
    parser.add_argument("port", type=int)
    parser.add_argument("ca_file")
    parser.add_argument("sessions", type=int)
    parser.add_argument("hold_seconds", type=float)
    return parser.parse_args()


def tagged(client: socket.socket, tag: bytes) -> bytes:
    """Читает ограниченный ответ до tagged completion включительно."""
    response = bytearray()
    marker = tag + b" "
    while marker not in response:
        block = client.recv(4096)
        if not block:
            raise AssertionError("IMAP connection closed before completion")
        response.extend(block)
        if len(response) > 65_536:
            raise AssertionError("IMAP response exceeded test limit")
    return bytes(response)


def command(client: socket.socket, tag: bytes, wire: bytes) -> bytes:
    """Отправляет команду и требует успешный tagged ответ."""
    client.sendall(tag + b" " + wire + b"\r\n")
    response = tagged(client, tag)
    if tag + b" OK" not in response:
        raise AssertionError(f"IMAP command failed: {response!r}")
    return response


def connect(options: argparse.Namespace, context: ssl.SSLContext) -> ssl.SSLSocket:
    """Создаёт STARTTLS, authenticated, selected и idling session."""
    plain = socket.create_connection((options.host, options.port), timeout=10)
    plain.settimeout(10)
    if not plain.recv(4096).startswith(b"* OK"):
        raise AssertionError("missing IMAP greeting")
    command(plain, b"a1", b"STARTTLS")
    client = context.wrap_socket(plain, server_hostname=options.host)
    command(client, b"a2", b'LOGIN "bob" "bob-password"')
    command(client, b"a3", b'SELECT "INBOX"')
    client.sendall(b"a4 IDLE\r\n")
    if not client.recv(4096).startswith(b"+"):
        raise AssertionError("IDLE continuation missing")
    return client


def close(client: ssl.SSLSocket) -> None:
    """Завершает IDLE и LOGOUT, затем освобождает TCP/TLS resource."""
    client.sendall(b"DONE\r\n")
    response = tagged(client, b"a4")
    if b"a4 OK" not in response:
        raise AssertionError(f"IDLE completion failed: {response!r}")
    command(client, b"a5", b"LOGOUT")
    client.close()


def main() -> None:
    """Удерживает все leases одновременно, затем корректно освобождает их."""
    options = arguments()
    context = ssl.create_default_context(cafile=options.ca_file)
    clients = [connect(options, context) for _ in range(options.sessions)]
    print(f"READY idle_sessions={len(clients)}", flush=True)
    time.sleep(options.hold_seconds)
    for client in clients:
        close(client)
    print(f"PASS idle_sessions={len(clients)}", flush=True)


if __name__ == "__main__":
    main()
