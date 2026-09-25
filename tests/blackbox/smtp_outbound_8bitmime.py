"""Проверяет EHLO negotiation и BODY=8BITMIME на outbound SMTP."""

import argparse
from pathlib import Path
import socket
import subprocess
import threading

from sieve_vacation_peer import CaptureServer


def require(condition: bool, message: str) -> None:
    """Завершает acceptance при неверной 8BITMIME policy или payload."""
    if not condition:
        raise AssertionError(message)


def docker_gateway(image: str) -> str:
    """Определяет адрес Docker host gateway без дополнительного образа."""
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
    address = next(iter(first_line.split()), "")
    socket.inet_aton(address)
    return address


def deliver_case(
    image: str,
    data_root: Path,
    message_path: Path,
    payload: bytes,
    route: str,
    ehlo_reply: bytes,
    should_deliver: bool,
) -> None:
    """Проверяет параметр MAIL и точные DATA bytes у независимого peer."""
    message_path.write_bytes(payload)
    relative_path = message_path.resolve().relative_to(data_root).as_posix()
    mount = f"type=bind,source={data_root},target=/data"
    with CaptureServer(("0.0.0.0", 0), ehlo_reply) as peer:
        _, port = peer.server_address
        worker = threading.Thread(target=peer.serve_forever, daemon=True)
        worker.start()
        try:
            result = subprocess.run(
                [
                    "docker", "run", "--rm", "--add-host",
                    "host.docker.internal:host-gateway", "--mount", mount, image,
                    "deliver", f"{route}:{port}", "sender@source.test",
                    "recipient@peer.test", f"/data/{relative_path}",
                ],
                check=False,
                capture_output=True,
                text=True,
                timeout=60,
            )
        finally:
            peer.shutdown()
            worker.join(timeout=5)
    match should_deliver:
        case True:
            require(result.returncode == 0, f"delivery failed: {result.stderr}")
            require("BODY=8BITMIME" in peer.envelope_sender, "MAIL omitted BODY=8BITMIME")
            expected = payload
            if not payload.endswith(b"\r\n"):
                expected += b"\r\n"
            require(peer.payload == expected, "8-bit DATA octets changed")
        case False:
            require(result.returncode != 0, "unsupported 8-bit DATA was delivered")
            require(not peer.envelope_sender, "unsupported 8-bit DATA started MAIL")
            require(not peer.payload, "unsupported 8-bit DATA reached DATA")


def main() -> None:
    """Проверяет high-bit body только при рекламированном extension."""
    parser = argparse.ArgumentParser()
    parser.add_argument("--image", required=True)
    parser.add_argument("--stage", required=True)
    options = parser.parse_args()
    repository = Path(__file__).resolve().parent.parent.parent
    data_root = (repository / "data").resolve()
    stage = Path(options.stage).resolve()
    stage.relative_to(data_root)
    route = docker_gateway(options.image)
    message_path = stage / "outbound-8bitmime.eml"
    payload = (
        "Subject: UTF-8 MIME body\r\n"
        "MIME-Version: 1.0\r\n"
        "Content-Type: text/plain; charset=UTF-8\r\n"
        "Content-Transfer-Encoding: 8bit\r\n\r\n"
        "Привет\r\n"
    ).encode("utf-8")
    supported = b"250-peer.test\r\n250-PIPELINING\r\n250 8BITMIME\r\n"
    unsupported = b"250-peer.test\r\n250 SIZE 1048576\r\n"
    deliver_case(
        options.image, data_root, message_path, payload, route, supported, True
    )
    deliver_case(
        options.image, data_root, message_path, payload, route, unsupported, False
    )
    print("PASS smtp-outbound-8bitmime: BODY negotiation and pre-envelope refusal")


if __name__ == "__main__":
    main()
