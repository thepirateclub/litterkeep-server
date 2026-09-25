"""Проверяет исходящий Sieve vacation quoted-printable encoder через SMTP peer."""

import argparse
import base64
from email import policy
from email.parser import BytesParser
from enum import Enum
from pathlib import Path
import quopri
import smtplib
import socket
import socketserver
import ssl
import subprocess
import threading

from sieve_vacation_peer import CaptureServer


class Account(Enum):
    """Bootstrap credentials владельцев входящего и исходящего писем."""

    ALICE = "alice"
    BOB = "bob"


class SieveReply(Enum):
    """Ожидаемые status строки текущего ManageSieve обмена."""

    GREETING = "vacation"
    STARTTLS = 'OK "begin TLS"'
    AUTHENTICATED = 'OK "authenticated"'
    COMPLETED = 'OK "completed"'


def require(condition: bool, message: str) -> None:
    """Завершает acceptance при нарушении проверяемого протокольного условия."""
    if not condition:
        raise AssertionError(message)


def read_line(stream) -> bytes:
    """Читает одну CRLF-строку без предположений о TCP fragment boundaries."""
    line = stream.readline()
    if not line:
        raise AssertionError("server closed before completing the protocol reply")
    return line


def read_capabilities(stream) -> bytes:
    """Собирает все строки greeting до завершающего положительного статуса."""
    lines = bytearray()
    while True:
        line = read_line(stream)
        lines.extend(line)
        if line.startswith(b"OK "):
            return bytes(lines)


def passwords(path: Path) -> dict[Account, str]:
    """Читает случайные stage credentials, не выводя секреты в журнал."""
    parsed = {}
    with path.open(encoding="utf-8") as credential_file:
        for line in credential_file:
            account, separator, secret = line.rstrip("\r\n").partition("\t")
            if separator:
                try:
                    parsed[Account(account)] = secret
                except ValueError:
                    continue
    require(Account.ALICE in parsed, "Alice bootstrap credential is missing")
    require(Account.BOB in parsed, "Bob bootstrap credential is missing")
    return parsed


def configure_vacation(
    host: str,
    port: int,
    context: ssl.SSLContext,
    bob_password: str,
    reason: bytes,
) -> None:
    """Устанавливает UTF-8 vacation reason с literal SPACE и TAB."""
    with socket.create_connection((host, port), timeout=10) as plain:
        stream = plain.makefile("rb")
        greeting = read_capabilities(stream).decode("ascii", "replace").lower()
        require(SieveReply.GREETING.value in greeting, "ManageSieve greeting omitted vacation")
        plain.sendall(b"STARTTLS\r\n")
        require(
            read_line(stream).strip().decode("ascii") == SieveReply.STARTTLS.value,
            "ManageSieve STARTTLS failed",
        )
        stream.close()
        with context.wrap_socket(plain, server_hostname="litterkeep.test") as secure:
            tls_stream = secure.makefile("rb")
            sasl = base64.b64encode(b"\x00bob\x00" + bob_password.encode("ascii"))
            secure.sendall(b'AUTHENTICATE "PLAIN" "' + sasl + b'"\r\n')
            require(
                read_line(tls_stream).strip().decode("ascii") == SieveReply.AUTHENTICATED.value,
                "ManageSieve authentication failed",
            )
            source = b'require "vacation"; vacation :days 1 "' + reason + b'";'
            command = f'PUTSCRIPT "qp-encoder" {{{len(source)}+}}\r\n'.encode("ascii")
            secure.sendall(command + source)
            require(
                read_line(tls_stream).strip().decode("ascii") == SieveReply.COMPLETED.value,
                "ManageSieve PUTSCRIPT failed",
            )
            secure.sendall(b'SETACTIVE "qp-encoder"\r\n')
            require(
                read_line(tls_stream).strip().decode("ascii") == SieveReply.COMPLETED.value,
                "ManageSieve SETACTIVE failed",
            )
            tls_stream.close()


def submit_message(
    host: str,
    port: int,
    context: ssl.SSLContext,
    alice_password: str,
) -> None:
    """Посылает личное письмо, запускающее vacation-ответ в durable queue."""
    raw = (
        b"From: alice@peer.test\r\n"
        b"To: bob@litterkeep.test\r\n"
        b"Subject: QP encoder acceptance\r\n"
        b"Message-ID: <qp-encoder@peer.test>\r\n"
        b"\r\ntrigger vacation\r\n"
    )
    with smtplib.SMTP(host, port, timeout=10) as client:
        client.ehlo()
        client.starttls(context=context)
        client.ehlo()
        client.login("alice", alice_password)
        client.sendmail("alice@peer.test", "bob@litterkeep.test", raw)


def stop_server(container: str) -> None:
    """Освобождает единственного Store writer перед отдельным queue-drain."""
    result = subprocess.run(
        ["docker", "stop", container],
        check=False,
        capture_output=True,
        text=True,
        timeout=30,
    )
    if result.returncode != 0:
        raise AssertionError(f"cannot stop Store owner before queue-drain: {result.stderr}")


def drain_one_message(repository: Path, stage: Path, image: str, peer_port: int) -> str:
    """Запускает Linux queue-drain с одним stage bind mount и явным peer route."""
    data_root = (repository / "data").resolve()
    relative_stage = stage.resolve().relative_to(data_root).as_posix()
    gateway_result = subprocess.run(
        [
            "docker", "run", "--rm", "--add-host", "host.docker.internal:host-gateway",
            "--entrypoint", "getent", image, "ahostsv4", "host.docker.internal",
        ],
        check=False,
        capture_output=True,
        text=True,
        timeout=30,
    )
    if gateway_result.returncode != 0:
        raise AssertionError(f"cannot resolve Docker host gateway: {gateway_result.stderr}")
    first_line = next(iter(gateway_result.stdout.splitlines()), "")
    gateway = next(iter(first_line.split()), "")
    socket.inet_aton(gateway)
    mount = f"type=bind,source={data_root},target=/data"
    route = f"{gateway}:{peer_port}"
    command = [
        "docker", "run", "--rm", "--add-host", "host.docker.internal:host-gateway",
        "--mount", mount, "--entrypoint", "/usr/local/bin/litterkeep-server",
        image, "queue-drain", f"/data/{relative_stage}", route, "8",
    ]
    result = subprocess.run(
        command,
        check=False,
        capture_output=True,
        text=True,
        timeout=90,
    )
    if result.returncode != 0:
        raise AssertionError(f"queue-drain failed: {result.stderr}")
    return result.stdout


def verify_payload(payload: bytes, reason: bytes) -> None:
    """Проверяет quoted whitespace, soft line limits и точный UTF-8 round-trip."""
    parsed = BytesParser(policy=policy.default).parsebytes(payload)
    body = parsed.get_payload(decode=True)
    expected_body = reason + b"\r\n"
    if body != expected_body:
        raise AssertionError(f"decoded vacation body differs: {body!r} != {expected_body!r}")
    headers, separator, encoded_body = payload.partition(b"\r\n\r\n")
    if separator == b"":
        raise AssertionError("captured response has no header/body separator")
    if b"content-transfer-encoding: quoted-printable" not in headers.lower():
        raise AssertionError("generated response did not select quoted-printable CTE")
    encoded_lines = quopri.encodestring(body, quotetabs=True).splitlines()
    if b"=20" not in encoded_body or b"=09" not in encoded_body:
        raise AssertionError("generated body did not encode literal SPACE and TAB")
    if any(len(line) > 76 for line in encoded_lines):
        raise AssertionError("reference quoted-printable encoder exceeded 76 octets")
    for line in encoded_body.split(b"\r\n"):
        if len(line) > 76:
            raise AssertionError(f"server emitted an overlong quoted-printable line: {len(line)}")


def main() -> None:
    """Проходит ManageSieve -> SMTP submission -> queue -> независимый SMTP peer."""
    parser = argparse.ArgumentParser()
    parser.add_argument("--stage", required=True)
    parser.add_argument("--image", required=True)
    parser.add_argument("--server-container", required=True)
    parser.add_argument("--smtp-port", type=int, default=2158)
    parser.add_argument("--managesieve-port", type=int, default=4190)
    parser.add_argument("--ca", default="data/pki/ca.pem")
    settings = parser.parse_args()
    repository = Path(__file__).resolve().parent.parent.parent
    stage = Path(settings.stage).resolve()
    secrets = passwords(stage / "test-accounts.tsv")
    reason = ("Привет \tмир " * 10 + "конец").encode("utf-8")
    context = ssl.create_default_context(cafile=settings.ca)
    configure_vacation(
        "127.0.0.1",
        settings.managesieve_port,
        context,
        secrets[Account.BOB],
        reason,
    )
    submit_message("127.0.0.1", settings.smtp_port, context, secrets[Account.ALICE])
    stop_server(settings.server_container)
    with CaptureServer(("0.0.0.0", 0)) as peer:
        _, peer_port = peer.server_address
        worker = threading.Thread(target=peer.serve_forever, daemon=True)
        worker.start()
        try:
            report = drain_one_message(repository, stage, settings.image, peer_port)
        finally:
            peer.shutdown()
            worker.join(timeout=5)
        if "delivered=1" not in report:
            raise AssertionError(f"queue-drain did not deliver exactly one vacation: {report}")
        if peer.envelope_sender != "MAIL FROM:<>" or peer.recipient != "RCPT TO:<alice@peer.test>":
            raise AssertionError(f"unexpected vacation envelope: {peer.envelope_sender!r}, {peer.recipient!r}")
        verify_payload(peer.payload, reason)
    print("PASS sieve-vacation-qp-encoder: ManageSieve through independent SMTP peer")


if __name__ == "__main__":
    main()
