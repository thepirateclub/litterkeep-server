"""Удерживает TLS-сессии и подтверждает внешнюю cooperative cancellation."""

import argparse
import socket
import ssl
import time


def arguments() -> argparse.Namespace:
    """Читает endpoint, trust anchor и размер серии."""
    parser = argparse.ArgumentParser()
    parser.add_argument("host")
    parser.add_argument("port", type=int)
    parser.add_argument("ca_file")
    parser.add_argument("sessions", type=int)
    parser.add_argument("hold_seconds", type=float)
    return parser.parse_args()


def connect(options: argparse.Namespace, context: ssl.SSLContext) -> ssl.SSLSocket:
    """Завершает handshake и проверяет SMTP greeting до отмены."""
    plain = socket.create_connection((options.host, options.port), timeout=10)
    client = context.wrap_socket(plain, server_hostname=options.host)
    client.settimeout(5)
    banner = client.recv(256)
    if not banner.startswith(b"220"):
        raise AssertionError(f"unexpected SMTP greeting: {banner!r}")
    return client


def is_cancelled(client: ssl.SSLSocket) -> bool:
    """Принимает EOF и TLS EOF без close_notify как закрытие socket owner-ом."""
    try:
        return client.recv(1) == b""
    except (ssl.SSLEOFError, ConnectionResetError):
        return True


def main() -> None:
    """Даёт runner-у окно для cancel и проверяет закрытие каждой сессии."""
    options = arguments()
    context = ssl.create_default_context(cafile=options.ca_file)
    clients = [connect(options, context) for _ in range(options.sessions)]
    print(f"READY tls_sessions={len(clients)}", flush=True)
    time.sleep(options.hold_seconds)
    cancelled = sum(is_cancelled(client) for client in clients)
    for client in clients:
        client.close()
    if cancelled != len(clients):
        raise AssertionError(f"cancelled only {cancelled} of {len(clients)} sessions")
    print(f"PASS cancelled_sessions={cancelled}", flush=True)


if __name__ == "__main__":
    main()
