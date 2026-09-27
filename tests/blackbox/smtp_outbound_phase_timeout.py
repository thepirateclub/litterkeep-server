"""Проверяет timeout независимого SMTP транспорта на выбранных фазах."""

import argparse
from enum import Enum
from pathlib import Path
import socket
import socketserver
import subprocess
import threading
import time


class Phase(Enum):
    """Фазы исходящего SMTP, где peer намеренно не отвечает."""

    GREETING = "greeting"
    EHLO = "ehlo"
    MAIL = "mail"
    RCPT = "rcpt"
    DATA_START = "data-start"
    DATA_FINAL = "data-final"


class Command(Enum):
    """SMTP-команды, получение которых отдельно фиксирует peer."""

    EHLO = "EHLO"
    MAIL = "MAIL"
    RCPT = "RCPT"
    DATA = "DATA"


class Peer(socketserver.ThreadingTCPServer):
    """Одноразовый TCP peer, который останавливается на выбранной фазе."""

    allow_reuse_address = True
    daemon_threads = True

    def __init__(self, address: tuple[str, int], phase: Phase):
        super().__init__(address, Handler)
        self.phase = phase
        self.observed_commands: set[Command] = set()
        self.observed_terminator = False


class Handler(socketserver.StreamRequestHandler):
    """Ведёт минимальный SMTP диалог и фиксирует ожидаемую точку зависания."""

    def handle(self) -> None:
        peer = self.server
        if not isinstance(peer, Peer):
            raise AssertionError("unexpected server type")
        if peer.phase is Phase.GREETING:
            self.wait_for_client_timeout()
            return
        self.wfile.write(b"220 peer.test ready\r\n")
        self.wfile.flush()
        while line := self.rfile.readline():
            upper = line.upper()
            command = next(iter(upper.split(maxsplit=1)), b"")
            parsed = parse_command(command)
            if parsed is not None:
                peer.observed_commands.add(parsed)
            match (parsed, peer.phase):
                case (Command.EHLO, Phase.EHLO):
                    self.wait_for_client_timeout()
                    return
                case (Command.EHLO, _):
                    self.wfile.write(b"250 peer.test\r\n")
                case (Command.MAIL, Phase.MAIL):
                    self.wait_for_client_timeout()
                    return
                case (Command.MAIL, _):
                    self.wfile.write(b"250 sender accepted\r\n")
                case (Command.RCPT, Phase.RCPT):
                    self.wait_for_client_timeout()
                    return
                case (Command.RCPT, _):
                    self.wfile.write(b"250 recipient accepted\r\n")
                case (Command.DATA, _):
                    if peer.phase is Phase.DATA_START:
                        self.wait_for_client_timeout()
                        return
                    self.wfile.write(b"354 send data\r\n")
                    self.wfile.flush()
                    self.read_message(peer)
                    if peer.phase is Phase.DATA_FINAL:
                        self.wait_for_client_timeout()
                        return
                    self.wfile.write(b"250 queued\r\n")
                case (None, _):
                    self.wfile.write(b"500 unsupported\r\n")
                case _:
                    self.wfile.write(b"500 unsupported\r\n")
            self.wfile.flush()

    def read_message(self, peer: Peer) -> None:
        """Считывает DATA до terminator и отмечает его у независимого peer."""
        while line := self.rfile.readline():
            if line == b".\r\n":
                peer.observed_terminator = True
                return

    def wait_for_client_timeout(self) -> None:
        """Не закрывает socket до timeout клиента и фиксирует реальное ожидание."""
        self.connection.recv(1)


def arguments() -> argparse.Namespace:
    """Разбирает путь к существующему Linux image и fixture stage."""
    parser = argparse.ArgumentParser()
    parser.add_argument("--image", required=True)
    parser.add_argument("--stage", required=True)
    return parser.parse_args()


def docker_gateway(image: str) -> str:
    """Получает host-gateway из уже установленного Rust verification image."""
    result = subprocess.run(
        [
            "docker", "run", "--rm", "--add-host",
            "host.docker.internal:host-gateway", "--entrypoint", "getent",
            "rust:1.97-bookworm", "ahostsv4", "host.docker.internal",
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
    stage: Path,
    route: str,
    phase: Phase,
) -> None:
    """Ограничивает timeout только одному CLI процессу и проверяет фазу."""
    message = stage / f"outbound-timeout-{phase.value}.eml"
    message.write_bytes(b"Subject: timeout probe\r\n\r\nbody\r\n")
    relative = message.resolve().relative_to(data_root).as_posix()
    mount = f"type=bind,source={data_root},target=/data"
    timeout_setting = timeout_setting_for(phase)
    started = time.monotonic()
    with Peer(("0.0.0.0", 0), phase) as peer:
        _, port = peer.server_address
        worker = threading.Thread(target=peer.serve_forever, daemon=True)
        worker.start()
        try:
            result = subprocess.run(
                [
                    "docker", "run", "--rm", "--add-host",
                    "host.docker.internal:host-gateway", "--mount", mount,
                    "--env", f"{timeout_setting}=1", image, "deliver",
                    f"{route}:{port}", "sender@source.test",
                    "recipient@peer.test", f"/data/{relative}",
                ],
                check=False,
                capture_output=True,
                text=True,
                timeout=15,
            )
        finally:
            peer.shutdown()
            worker.join(timeout=5)
    elapsed = time.monotonic() - started
    if result.returncode == 0:
        raise AssertionError(f"delivery unexpectedly succeeded at {phase.value}")
    if elapsed > 10:
        raise AssertionError(f"{phase.value} timeout was not bounded: {elapsed:.2f}s")
    match phase:
        case Phase.GREETING:
            if Command.DATA in peer.observed_commands:
                raise AssertionError("greeting timeout reached DATA")
        case Phase.EHLO:
            require_command(peer, Command.EHLO)
        case Phase.MAIL:
            require_command(peer, Command.MAIL)
        case Phase.RCPT:
            require_command(peer, Command.RCPT)
        case Phase.DATA_START:
            if Command.DATA not in peer.observed_commands or peer.observed_terminator:
                raise AssertionError("DATA initiation timeout phase was not isolated")
        case Phase.DATA_FINAL:
            if not peer.observed_terminator:
                raise AssertionError("final timeout happened before DATA terminator")


def require_command(peer: Peer, command: Command) -> None:
    """Подтверждает, что timeout наступил только после нужной SMTP-команды."""
    if command not in peer.observed_commands:
        raise AssertionError(f"peer did not observe {command.value} before timeout")


def timeout_setting_for(phase: Phase) -> str:
    """Выбирает именованную process setting без сырых строковых условий."""
    match phase:
        case Phase.GREETING:
            return "LITTERKEEP_SMTP_GREETING_TIMEOUT_SECONDS"
        case Phase.DATA_START:
            return "LITTERKEEP_SMTP_DATA_START_TIMEOUT_SECONDS"
        case Phase.DATA_FINAL:
            return "LITTERKEEP_SMTP_DATA_FINAL_TIMEOUT_SECONDS"
        case Phase.EHLO | Phase.MAIL | Phase.RCPT:
            return "LITTERKEEP_SMTP_COMMAND_TIMEOUT_SECONDS"


def parse_command(value: bytes) -> Command | None:
    """Преобразует SMTP verb в enum до выбора тестовой ветви."""
    match value:
        case b"EHLO":
            return Command.EHLO
        case b"MAIL":
            return Command.MAIL
        case b"RCPT":
            return Command.RCPT
        case b"DATA":
            return Command.DATA
        case _:
            return None


def main() -> None:
    """Проверяет три command и три выделенных greeting/DATA deadline."""
    options = arguments()
    repository = Path(__file__).resolve().parent.parent.parent
    data_root = (repository / "data").resolve()
    stage = Path(options.stage).resolve()
    stage.relative_to(data_root)
    route = docker_gateway(options.image)
    for phase in Phase:
        run_case(options.image, data_root, stage, route, phase)
        print(f"PASS outbound SMTP {phase.value} deadline")


if __name__ == "__main__":
    main()
