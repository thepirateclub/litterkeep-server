"""Проверяет освобождение TCP-сессии, не начавшей TLS handshake."""

import argparse
import socket
import time


def arguments() -> argparse.Namespace:
    """Читает endpoint implicit-TLS listener-а."""
    parser = argparse.ArgumentParser()
    parser.add_argument("host")
    parser.add_argument("port", type=int)
    return parser.parse_args()


def main() -> None:
    """Оставляет соединение без ClientHello и ожидает серверное закрытие."""
    options = arguments()
    started = time.monotonic()
    with socket.create_connection((options.host, options.port), timeout=10) as client:
        client.settimeout(7)
        received = client.recv(1)
    elapsed = time.monotonic() - started
    if received != b"":
        raise AssertionError(f"unexpected bytes before handshake: {received!r}")
    match elapsed:
        case value if value < 4.5:
            raise AssertionError(f"handshake closed too early: {value:.3f}s")
        case value if value > 6.5:
            raise AssertionError(f"handshake deadline was late: {value:.3f}s")
        case _:
            pass
    print(f"TLS handshake timeout passed: elapsed={elapsed:.3f}s")


if __name__ == "__main__":
    main()
