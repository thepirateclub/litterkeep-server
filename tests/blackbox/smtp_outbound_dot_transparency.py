"""Независимо проверяет SMTP DATA dot-transparency исходящего транспорта."""

import argparse
from pathlib import Path
import socket
import subprocess
import threading

from sieve_vacation_peer import CaptureServer


def require(condition: bool, message: str) -> None:
    """Останавливает acceptance при расхождении SMTP wire semantics."""
    if not condition:
        raise AssertionError(message)


def docker_gateway(image: str) -> str:
    """Получает адрес host-gateway из временного Linux контейнера."""
    result = subprocess.run(
        [
            "docker", "run", "--rm", "--add-host",
            "host.docker.internal:host-gateway", "--entrypoint", "getent",
            image, "ahostsv4", "host.docker.internal",
        ],
        check=True,
        capture_output=True,
        text=True,
        timeout=30,
    )
    first_line = next(iter(result.stdout.splitlines()), "")
    gateway = next(iter(first_line.split()), "")
    socket.inet_aton(gateway)
    return gateway


def deliver(
    image: str,
    data_root: Path,
    relative_message: str,
    address: str,
) -> None:
    """Запускает одноразовый штатный SMTP delivery CLI в Linux image."""
    mount = f"type=bind,source={data_root},target=/data"
    result = subprocess.run(
        [
            "docker", "run", "--rm", "--add-host",
            "host.docker.internal:host-gateway", "--mount", mount, image,
            "deliver", address, "sender@source.test", "recipient@peer.test",
            f"/data/{relative_message}",
        ],
        check=False,
        capture_output=True,
        text=True,
        timeout=60,
    )
    if result.returncode != 0:
        raise AssertionError(f"outbound SMTP CLI failed: {result.stderr}")


def main() -> None:
    """Сравнивает снятые peer-ом DATA octets с исходным RFC 5322 файлом."""
    parser = argparse.ArgumentParser()
    parser.add_argument("--image", required=True)
    parser.add_argument("--stage", required=True)
    options = parser.parse_args()
    repository = Path(__file__).resolve().parent.parent.parent
    data_root = (repository / "data").resolve()
    stage = Path(options.stage).resolve()
    stage.relative_to(data_root)
    fixture = (
        b"From: sender@source.test\r\n"
        b"To: recipient@peer.test\r\n"
        b"Subject: dot transparency\r\n\r\n"
        b"first\r\n.\r\n..two\r\n.end"
    )
    message_path = stage / "outbound-dot-transparency.eml"
    message_path.write_bytes(fixture)
    relative_message = message_path.resolve().relative_to(data_root).as_posix()
    gateway = docker_gateway(options.image)
    with CaptureServer(("0.0.0.0", 0)) as peer:
        _, port = peer.server_address
        worker = threading.Thread(target=peer.serve_forever, daemon=True)
        worker.start()
        try:
            deliver(options.image, data_root, relative_message, f"{gateway}:{port}")
        finally:
            peer.shutdown()
            worker.join(timeout=5)
    expected = fixture + b"\r\n"
    require(peer.payload == expected, f"peer DATA differs: {peer.payload!r} != {expected!r}")
    print("PASS smtp-outbound-dot-transparency: exact DATA octets at independent peer")


if __name__ == "__main__":
    main()
