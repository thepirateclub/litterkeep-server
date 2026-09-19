"""Проверяет предел TLS-сессий и освобождение всех просроченных slots."""

import argparse
import socket
import time


def arguments() -> argparse.Namespace:
    """Читает endpoint и документированную ёмкость admission."""
    parser = argparse.ArgumentParser()
    parser.add_argument("host")
    parser.add_argument("port", type=int)
    parser.add_argument("capacity", type=int, nargs="?", default=256)
    return parser.parse_args()


def open_connection(host: str, port: int) -> socket.socket:
    """Открывает TCP, намеренно не отправляя TLS ClientHello."""
    connection = socket.create_connection((host, port), timeout=10)
    connection.settimeout(1)
    return connection


def is_closed(connection: socket.socket) -> bool:
    """Отличает серверный EOF от живого соединения без входных данных."""
    try:
        return connection.recv(1) == b""
    except TimeoutError:
        return False
    except ConnectionResetError:
        return True


def main() -> None:
    """Заполняет лимит, проверяет отказ и последующее освобождение."""
    options = arguments()
    connections: list[socket.socket] = []
    try:
        for _ in range(options.capacity):
            connections.append(open_connection(options.host, options.port))
        overflow = open_connection(options.host, options.port)
        try:
            if not is_closed(overflow):
                raise AssertionError("connection above admission limit stayed open")
        finally:
            overflow.close()

        time.sleep(5.5)
        closed = sum(is_closed(connection) for connection in connections)
        if closed != options.capacity:
            raise AssertionError(
                f"only {closed} of {options.capacity} stalled sessions were released"
            )
    finally:
        for connection in connections:
            connection.close()
    print(f"TLS admission limit passed: capacity={options.capacity}")


if __name__ == "__main__":
    main()
