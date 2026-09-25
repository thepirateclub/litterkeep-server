"""Проверяет EHLO negotiation и BODY=8BITMIME на outbound SMTP."""

import argparse
from pathlib import Path
import socket
import subprocess
import threading

from sieve_vacation_peer import CaptureServer, Command


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
            require(
                f"SIZE={len(payload)}" in peer.envelope_sender,
                "MAIL omitted the exact advertised SIZE",
            )
            expected = payload
            if not payload.endswith(b"\r\n"):
                expected += b"\r\n"
            require(peer.payload == expected, "8-bit DATA octets changed")
        case False:
            require(result.returncode != 0, "unsupported 8-bit DATA was delivered")
            require(not peer.envelope_sender, "unsupported 8-bit DATA started MAIL")
            require(not peer.payload, "unsupported 8-bit DATA reached DATA")


def verify_reply_line_limits(
    image: str, data_root: Path, stage: Path, route: str
) -> None:
    """Проверяет лимит 512 октетов на greeting и EHLO reply lines."""
    message_path = stage / "outbound-reply-limit.eml"
    payload = b"Subject: bounded reply\r\n\r\nbody\r\n"
    message_path.write_bytes(payload)
    relative_path = message_path.resolve().relative_to(data_root).as_posix()
    mount = f"type=bind,source={data_root},target=/data"
    capability = b"250-8BITMIME "
    boundary_line = capability.ljust(510, b"x") + b"\r\n"
    oversized_line = b"250 " + b"x" * 507 + b"\r\n"
    oversized_greeting = b"220 " + b"x" * 507 + b"\r\n"
    cases = (
        (b"220 peer.test ready\r\n", boundary_line + b"250 peer.test\r\n", True),
        (b"220 peer.test ready\r\n", oversized_line, False),
        (oversized_greeting, b"250 peer.test\r\n", False),
    )
    for greeting, ehlo_reply, should_connect in cases:
        with CaptureServer(
            ("0.0.0.0", 0), ehlo_reply, greeting_reply=greeting
        ) as peer:
            _, port = peer.server_address
            worker = threading.Thread(target=peer.serve_forever, daemon=True)
            worker.start()
            try:
                result = subprocess.run(
                    [
                        "docker", "run", "--rm", "--add-host",
                        "host.docker.internal:host-gateway", "--mount", mount,
                        image, "deliver", f"{route}:{port}",
                        "sender@source.test", "recipient@peer.test",
                        f"/data/{relative_path}",
                    ],
                    check=False,
                    capture_output=True,
                    text=True,
                    timeout=60,
                )
            finally:
                peer.shutdown()
                worker.join(timeout=5)
        match should_connect:
            case True:
                require(result.returncode == 0, "512-octet SMTP reply was rejected")
                require(bool(peer.envelope_sender), "valid reply did not reach MAIL")
            case False:
                require(result.returncode != 0, "oversized SMTP reply was accepted")
                require(not peer.envelope_sender, "oversized reply reached MAIL")


def verify_size_declaration(
    image: str, data_root: Path, stage: Path, route: str
) -> None:
    """Проверяет SIZE declaration, maximum peer-а и точный размер DATA."""
    message_path = stage / "outbound-size-declaration.eml"
    payload = b"Subject: SIZE\r\n\r\nbody"
    expected_octets = len(payload) + 2
    message_path.write_bytes(payload)
    relative_path = message_path.resolve().relative_to(data_root).as_posix()
    mount = f"type=bind,source={data_root},target=/data"
    cases = (
        (b"250-peer.test\r\n250 PIPELINING\r\n", None, True),
        (b"250-peer.test\r\n250 SIZE 1024\r\n", expected_octets, True),
        (b"250-peer.test\r\n250 SIZE 18446744073709551616\r\n", expected_octets, True),
        (b"250-peer.test\r\n250 SIZE 10\r\n", None, False),
    )
    for ehlo_reply, declared_size, should_deliver in cases:
        with CaptureServer(("0.0.0.0", 0), ehlo_reply) as peer:
            _, port = peer.server_address
            worker = threading.Thread(target=peer.serve_forever, daemon=True)
            worker.start()
            try:
                result = subprocess.run(
                    [
                        "docker", "run", "--rm", "--add-host",
                        "host.docker.internal:host-gateway", "--mount", mount,
                        image, "deliver", f"{route}:{port}",
                        "sender@source.test", "recipient@peer.test",
                        f"/data/{relative_path}",
                    ],
                    check=False,
                    capture_output=True,
                    text=True,
                    timeout=60,
                )
            finally:
                peer.shutdown()
                worker.join(timeout=5)
        match (should_deliver, declared_size):
            case (True, None):
                require(result.returncode == 0, "SMTP without SIZE was rejected")
                require(" SIZE=" not in peer.envelope_sender, "unadvertised SIZE was sent")
            case (True, int()):
                require(result.returncode == 0, "SIZE accepted within peer maximum failed")
                require(
                    f"SIZE={declared_size}" in peer.envelope_sender,
                    "declared SIZE does not match transmitted message octets",
                )
            case (False, None):
                require(result.returncode != 0, "message over peer SIZE maximum was sent")
                require(not peer.envelope_sender, "over-limit message reached MAIL")
            case _:
                raise AssertionError("invalid SIZE acceptance case")


def verify_phase_specific_reply_codes(
    image: str, data_root: Path, stage: Path, route: str
) -> None:
    """Проверяет, что MAIL не принимает DATA continuation или QUIT code."""
    message_path = stage / "outbound-reply-phase.eml"
    message_path.write_bytes(b"Subject: reply phase\r\n\r\nbody\r\n")
    relative_path = message_path.resolve().relative_to(data_root).as_posix()
    mount = f"type=bind,source={data_root},target=/data"
    for mail_reply in (b"354 send data\r\n", b"221 bye\r\n"):
        with CaptureServer(
            ("0.0.0.0", 0), mail_reply=mail_reply
        ) as peer:
            _, port = peer.server_address
            worker = threading.Thread(target=peer.serve_forever, daemon=True)
            worker.start()
            try:
                result = subprocess.run(
                    [
                        "docker", "run", "--rm", "--add-host",
                        "host.docker.internal:host-gateway", "--mount", mount,
                        image, "deliver", f"{route}:{port}",
                        "sender@source.test", "recipient@peer.test",
                        f"/data/{relative_path}",
                    ],
                    check=False,
                    capture_output=True,
                    text=True,
                    timeout=60,
                )
            finally:
                peer.shutdown()
                worker.join(timeout=5)
        require(result.returncode != 0, "MAIL accepted an unrelated positive reply")
        require(
            Command.RECIPIENT not in peer.commands,
            "client continued to RCPT after an unexpected MAIL reply",
        )


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
    supported = (
        b"250-peer.test\r\n250-SIZE 1048576\r\n"
        b"250-PIPELINING\r\n250 8BITMIME\r\n"
    )
    unsupported = b"250-peer.test\r\n250 SIZE 1048576\r\n"
    deliver_case(
        options.image, data_root, message_path, payload, route, supported, True
    )
    deliver_case(
        options.image, data_root, message_path, payload, route, unsupported, False
    )
    verify_reply_line_limits(options.image, data_root, stage, route)
    verify_size_declaration(options.image, data_root, stage, route)
    verify_phase_specific_reply_codes(options.image, data_root, stage, route)
    print("PASS smtp-outbound-8bitmime: BODY negotiation and pre-envelope refusal")
    print("PASS smtp-outbound-reply-limit: 512-octet boundary and oversized rejection")
    print("PASS smtp-outbound-size: exact SIZE declaration and advertised maximum")
    print("PASS smtp-outbound-replies: unexpected positive replies stop the transaction")


if __name__ == "__main__":
    main()
