"""Проверяет outbound SMTP ограничения длины строки и CRLF framing."""

import argparse
from pathlib import Path
import socket
import subprocess
import threading

from sieve_vacation_peer import CaptureServer


def require(condition: bool, message: str) -> None:
    """Останавливает acceptance при нарушении ожидаемого SMTP результата."""
    if not condition:
        raise AssertionError(message)


def gateway(image: str) -> str:
    """Определяет host-gateway из одноразового контейнера с тем же образом."""
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


def run_case(
    image: str,
    data_root: Path,
    message_path: Path,
    payload: bytes,
    gateway_address: str,
    should_deliver: bool,
) -> None:
    """Сравнивает accepted DATA bytes либо отказ до начала envelope."""
    message_path.write_bytes(payload)
    relative_path = message_path.resolve().relative_to(data_root).as_posix()
    mount = f"type=bind,source={data_root},target=/data"
    with CaptureServer(("0.0.0.0", 0)) as peer:
        _, port = peer.server_address
        worker = threading.Thread(target=peer.serve_forever, daemon=True)
        worker.start()
        try:
            result = subprocess.run(
                [
                    "docker", "run", "--rm", "--add-host",
                    "host.docker.internal:host-gateway", "--mount", mount, image,
                    "deliver", f"{gateway_address}:{port}", "sender@source.test",
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
                require(result.returncode == 0, f"valid DATA rejected: {result.stderr}")
                expected = payload
                if not payload.endswith(b"\r\n"):
                    expected += b"\r\n"
                require(peer.payload == expected, "peer received changed valid DATA")
            case False:
                require(result.returncode != 0, "overlong DATA unexpectedly delivered")
                require(not peer.envelope_sender, "invalid DATA started an SMTP envelope")
                require(not peer.payload, "invalid DATA reached SMTP DATA")


def main() -> None:
    """Проверяет RFC 5321 лимит ровно на границе 1000 octets."""
    parser = argparse.ArgumentParser()
    parser.add_argument("--image", required=True)
    parser.add_argument("--stage", required=True)
    options = parser.parse_args()
    repository = Path(__file__).resolve().parent.parent.parent
    data_root = (repository / "data").resolve()
    stage = Path(options.stage).resolve()
    stage.relative_to(data_root)
    message_path = stage / "outbound-line-profile.eml"
    prefix = b"From: sender@source.test\r\nTo: recipient@peer.test\r\n\r\n"
    valid_payload = prefix + b"x" * 998 + b"\r\n"
    invalid_payload = prefix + b"x" * 999 + b"\r\n"
    route = gateway(options.image)
    run_case(options.image, data_root, message_path, valid_payload, route, True)
    run_case(options.image, data_root, message_path, invalid_payload, route, False)
    print("PASS smtp-outbound-line-profile: 1000 accepted, 1001 rejected pre-envelope")


if __name__ == "__main__":
    main()
