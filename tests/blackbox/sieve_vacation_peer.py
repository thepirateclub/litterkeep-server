"""Дренирует Sieve auto-reply через независимый SMTP peer и требует MAIL FROM:<>."""

import argparse
from enum import Enum
from pathlib import Path
import socket
import socketserver
import subprocess
import threading


class Command(Enum):
    EHLO = "EHLO "
    MAIL = "MAIL FROM:"
    RECIPIENT = "RCPT TO:"
    DATA = "DATA"
    QUIT = "QUIT"


class CaptureServer(socketserver.ThreadingTCPServer):
    allow_reuse_address = True
    daemon_threads = True

    def __init__(
        self,
        address: tuple[str, int],
        ehlo_reply: bytes = b"250 peer.test\r\n",
        greeting_reply: bytes = b"220 peer.test ready\r\n",
        mail_reply: bytes = b"250 sender accepted\r\n",
    ):
        super().__init__(address, CaptureHandler)
        self.envelope_sender = ""
        self.recipient = ""
        self.payload = b""
        self.ehlo_reply = ehlo_reply
        self.greeting_reply = greeting_reply
        self.mail_reply = mail_reply
        self.commands: list[Command] = []


class CaptureHandler(socketserver.StreamRequestHandler):
    def handle(self) -> None:
        server = self.server
        assert isinstance(server, CaptureServer)
        self.wfile.write(server.greeting_reply)
        while line := self.rfile.readline():
            command = classify(line.decode("ascii", "replace"))
            if command is not None:
                server.commands.append(command)
            match command:
                case Command.EHLO:
                    self.wfile.write(server.ehlo_reply)
                case Command.MAIL:
                    server.envelope_sender = line.decode("ascii", "replace").strip()
                    self.wfile.write(server.mail_reply)
                case Command.RECIPIENT:
                    server.recipient = line.decode("ascii", "replace").strip()
                    self.wfile.write(b"250 recipient accepted\r\n")
                case Command.DATA:
                    self.wfile.write(b"354 send data\r\n")
                    server.payload = read_data(self.rfile)
                    self.wfile.write(b"250 response accepted\r\n")
                case Command.QUIT:
                    self.wfile.write(b"221 bye\r\n")
                    return
                case None:
                    self.wfile.write(b"500 unsupported command\r\n")
            self.wfile.flush()


def classify(line: str) -> Command | None:
    return next((command for command in Command if line.upper().startswith(command.value)), None)


def read_data(reader) -> bytes:
    lines = []
    while line := reader.readline():
        if line == b".\r\n":
            return b"".join(lines)
        if line.startswith(b".."):
            line = line[1:]
        lines.append(line)
    raise AssertionError("SMTP peer closed before DATA terminator")


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--data-dir", required=True)
    options = parser.parse_args()
    repository = Path(__file__).resolve().parent.parent.parent
    data_root = (repository / "data").resolve()
    data_path = Path(options.data_dir).resolve()
    data_relative = data_path.relative_to(data_root).as_posix()
    gateway = docker_host_gateway()
    with CaptureServer(("0.0.0.0", 0)) as peer:
        _, port = peer.server_address
        worker = threading.Thread(target=peer.serve_forever, daemon=True)
        worker.start()
        try:
            result = subprocess.run(
                docker_queue_drain(repository, data_relative, gateway, port),
                check=False,
                capture_output=True,
                text=True,
                timeout=90,
            )
        finally:
            peer.shutdown()
            worker.join(timeout=5)
        if result.returncode != 0:
            raise AssertionError(f"queue-drain failed: {result.stderr}")
        report = ("delivered=1", "deferred=0", "permanent_failures=0")
        if not all(value in result.stdout for value in report):
            raise AssertionError(f"unexpected outbound completion report: {result.stdout}")
        if peer.envelope_sender != "MAIL FROM:<>" or peer.recipient != "RCPT TO:<alice@peer.test>":
            raise AssertionError(f"unexpected SMTP envelope: {peer.envelope_sender!r}, {peer.recipient!r}")
        required = (b"Auto-Submitted: auto-replied", b"To: alice@peer.test", b"Subject: Out of office")
        if not all(marker in peer.payload for marker in required):
            raise AssertionError("captured auto-reply lacks its RFC response fields")
    print("Sieve vacation durable queue -> independent SMTP peer: PASS")


def docker_host_gateway() -> str:
    """Находит host-gateway из одноразового Linux контейнера test image."""
    result = subprocess.run(
        [
            "docker", "run", "--rm", "--add-host", "host.docker.internal:host-gateway",
            "--entrypoint", "getent", "litterkeep-linux-test:latest",
            "ahostsv4", "host.docker.internal",
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


def docker_queue_drain(repository: Path, data_relative: str, gateway: str, port: int) -> list[str]:
    """Собирает bounded queue-drain Linux команды с одним bind mount stage data."""
    source = str(repository / "data")
    mount = f"type=bind,source={source},target=/data"
    route = f"{gateway}:{port}"
    return [
        "docker", "run", "--rm", "--add-host", "host.docker.internal:host-gateway",
        "--mount", mount, "--entrypoint", "/source/target/release/litterkeep-server",
        "litterkeep-linux-test:latest", "queue-drain", f"/data/{data_relative}", route, "8",
    ]


if __name__ == "__main__":
    main()
