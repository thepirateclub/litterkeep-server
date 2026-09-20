"""Создаёт повторяемый bounded churn независимых почтовых клиентов."""

import argparse
import socket
from concurrent.futures import ThreadPoolExecutor
from enum import Enum


class Protocol(Enum):
    """Endpoint и ожидаемый префикс приветствия одного протокола."""

    SMTP = "smtp"
    IMAP = "imap"


def arguments() -> argparse.Namespace:
    """Читает профиль нагрузки без скрытых значений среды."""
    parser = argparse.ArgumentParser()
    parser.add_argument("protocol", type=Protocol)
    parser.add_argument("host")
    parser.add_argument("port", type=int)
    parser.add_argument("connections", type=int)
    parser.add_argument("concurrency", type=int)
    return parser.parse_args()


def expected_banner(protocol: Protocol) -> bytes:
    """Возвращает wire-префикс через типизированный выбор протокола."""
    match protocol:
        case Protocol.SMTP:
            return b"220"
        case Protocol.IMAP:
            return b"* OK"


def connect_once(protocol: Protocol, host: str, port: int) -> None:
    """Проверяет принятое соединение и закрывает его без protocol logout."""
    with socket.create_connection((host, port), timeout=10) as client:
        client.settimeout(10)
        banner = client.recv(1024)
        if not banner.startswith(expected_banner(protocol)):
            raise AssertionError(f"unexpected {protocol.value} banner: {banner!r}")


def main() -> None:
    """Выполняет заданное число подключений конечным пулом клиентов."""
    options = arguments()
    if options.connections < 1:
        raise ValueError("connections must be positive")
    if options.concurrency < 1:
        raise ValueError("concurrency must be positive")
    with ThreadPoolExecutor(max_workers=options.concurrency) as executor:
        futures = [
            executor.submit(
                connect_once,
                options.protocol,
                options.host,
                options.port,
            )
            for _ in range(options.connections)
        ]
        for future in futures:
            future.result()
    print(
        f"PASS protocol={options.protocol.value} "
        f"connections={options.connections} concurrency={options.concurrency}"
    )


if __name__ == "__main__":
    main()
