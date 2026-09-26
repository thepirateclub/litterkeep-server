"""Проверяет redirect :copy до local inbox и независимого SMTP peer."""

import argparse
import base64
from email import policy
from email.message import EmailMessage
from email.parser import BytesParser
from enum import Enum
import imaplib
from pathlib import Path
import smtplib
import socket
import ssl
import subprocess
import threading

from sieve_vacation_peer import CaptureServer


class Account(Enum):
    """Тестовые mailbox identities из штатного bootstrap fixture."""

    ALICE = "alice"
    BOB = "bob"


class ProtocolStatus(Enum):
    """Единственный ожидаемый успешный status внешних protocol clients."""

    OK = "OK"


def arguments() -> argparse.Namespace:
    """Читает адреса listeners, fixture и управляемые Docker targets."""
    parser = argparse.ArgumentParser()
    parser.add_argument("--data-dir", required=True)
    parser.add_argument("--container", required=True)
    parser.add_argument("--image", required=True)
    parser.add_argument("--smtp-port", type=int, required=True)
    parser.add_argument("--imap-port", type=int, required=True)
    parser.add_argument("--sieve-port", type=int, required=True)
    parser.add_argument("--ca", required=True)
    parser.add_argument("--credentials", required=True)
    return parser.parse_args()


def credentials(path: Path) -> dict[Account, str]:
    """Загружает случайные stage passwords, не печатая их в output."""
    secrets: dict[Account, str] = {}
    for line in path.read_text(encoding="utf-8").splitlines():
        account_name, separator, password = line.partition("\t")
        if not separator or not password:
            continue
        try:
            secrets[Account(account_name)] = password
        except ValueError:
            continue
    if not all(account in secrets for account in Account):
        raise AssertionError("Alice/Bob acceptance credentials are missing")
    return secrets


def read_line(stream) -> bytes:
    """Считывает одну CRLF-строку ManageSieve независимо от TCP fragments."""
    line = stream.readline()
    if not line:
        raise AssertionError("ManageSieve closed before protocol response")
    return line


def configure_redirect_copy(
    host: str,
    port: int,
    context: ssl.SSLContext,
    password: str,
) -> None:
    """Активирует требующий `copy` redirect через независимый ManageSieve."""
    with socket.create_connection((host, port), timeout=10) as plain:
        stream = plain.makefile("rb")
        greeting = bytearray()
        while True:
            line = read_line(stream)
            greeting.extend(line)
            if line.startswith(b"OK "):
                break
        if b"copy" not in bytes(greeting).lower():
            raise AssertionError("ManageSieve did not advertise RFC 3894 copy")
        plain.sendall(b"STARTTLS\r\n")
        if read_line(stream) != b'OK "begin TLS"\r\n':
            raise AssertionError("ManageSieve STARTTLS failed")
        stream.close()
        with context.wrap_socket(plain, server_hostname="litterkeep.test") as secure:
            tls_stream = secure.makefile("rb")
            while not read_line(tls_stream).startswith(b"OK "):
                continue
            sasl = base64.b64encode(b"\x00bob\x00" + password.encode("ascii"))
            secure.sendall(b'AUTHENTICATE "PLAIN" "' + sasl + b'"\r\n')
            while True:
                response = read_line(tls_stream)
                if response.startswith(b"OK "):
                    break
                if response.startswith(b"NO ") or response.startswith(b"BYE "):
                    raise AssertionError(f"ManageSieve AUTH failed: {response!r}")
            source = b'require "copy"; redirect :copy "remote@peer.test";'
            marker = f'PUTSCRIPT "redirect-copy" {{{len(source)}+}}\r\n'.encode()
            secure.sendall(marker + source)
            if read_line(tls_stream) != b'OK "completed"\r\n':
                raise AssertionError("ManageSieve PUTSCRIPT failed")
            secure.sendall(b'SETACTIVE "redirect-copy"\r\n')
            if read_line(tls_stream) != b'OK "completed"\r\n':
                raise AssertionError("ManageSieve SETACTIVE failed")


def message_with_attachment() -> tuple[bytes, bytes]:
    """Строит воспроизводимое multipart письмо и ожидаемые attachment bytes."""
    attachment = bytes((0, 1, 127, 128, 254, 255))
    message = EmailMessage(policy=policy.SMTP)
    message["From"] = "alice@litterkeep.test"
    message["To"] = "bob@litterkeep.test"
    message["Subject"] = "RFC 3894 redirect copy attachment"
    message.set_content("redirect copy body")
    message.add_attachment(
        attachment,
        maintype="application",
        subtype="octet-stream",
        filename="copy.bin",
    )
    return message.as_bytes(), attachment


def submit_message(host: str, port: int, context: ssl.SSLContext, password: str, message: bytes) -> None:
    """Посылает исходное multipart письмо Alice в mailbox Bob по STARTTLS."""
    with smtplib.SMTP(host, port, timeout=10) as client:
        client.ehlo()
        client.starttls(context=context)
        client.ehlo()
        client.login(Account.ALICE.value, password)
        result = client.sendmail("alice@litterkeep.test", ["bob@litterkeep.test"], message)
        if result:
            raise AssertionError(f"SMTP rejected recipients: {result!r}")


def local_message(
    host: str,
    port: int,
    context: ssl.SSLContext,
    password: str,
    subject: str,
) -> bytes:
    """Извлекает Bob inbox-копию через независимый IMAP STARTTLS клиент."""
    client = imaplib.IMAP4(host, port, timeout=10)
    client.starttls(ssl_context=context)
    status, _ = client.login(Account.BOB.value, password)
    if ProtocolStatus(status) is not ProtocolStatus.OK:
        raise AssertionError(f"IMAP LOGIN failed: {status!r}")
    status, count = client.select("INBOX", readonly=True)
    if ProtocolStatus(status) is not ProtocolStatus.OK or count != [b"1"]:
        raise AssertionError(f"redirect :copy did not keep one local copy: {count!r}")
    status, records = client.search(None, "SUBJECT", f'"{subject}"')
    if ProtocolStatus(status) is not ProtocolStatus.OK:
        raise AssertionError(f"IMAP SEARCH failed: {status!r}")
    identifiers = next(iter(records), b"").strip()
    if not identifiers:
        raise AssertionError("redirect :copy local message was not found")
    status, fetched = client.fetch(identifiers.decode("ascii"), "(RFC822)")
    if ProtocolStatus(status) is not ProtocolStatus.OK:
        raise AssertionError(f"IMAP FETCH failed: {status!r}")
    message = next(
        (
            item
            for row in fetched
            if isinstance(row, tuple)
            for item in row
            if isinstance(item, bytes)
            and (item.startswith(b"From:") or b"\r\nFrom:" in item)
        ),
        None,
    )
    client.logout()
    if message is None:
        raise AssertionError("IMAP FETCH did not return the RFC822 literal")
    return message


def stop_source(container: str) -> None:
    """Останавливает единственного Store owner перед отдельным queue drain."""
    result = subprocess.run(
        ["docker", "stop", container], capture_output=True, text=True, timeout=30
    )
    if result.returncode != 0:
        raise AssertionError(f"cannot stop source Store owner: {result.stderr}")


def drain_queue(repository: Path, options: argparse.Namespace, peer_port: int) -> str:
    """Доставляет durable redirect job независимому SMTP capture peer."""
    data_root = (repository / "data").resolve()
    stage = Path(options.data_dir).resolve()
    relative_stage = stage.relative_to(data_root).as_posix()
    mount = f"type=bind,source={data_root},target=/data"
    gateway_result = subprocess.run(
        [
            "docker", "run", "--rm", "--add-host",
            "host.docker.internal:host-gateway", "--entrypoint", "getent",
            options.image, "ahostsv4", "host.docker.internal",
        ],
        capture_output=True,
        text=True,
        timeout=30,
    )
    if gateway_result.returncode != 0:
        raise AssertionError(f"cannot resolve Docker host gateway: {gateway_result.stderr}")
    first_line = next(iter(gateway_result.stdout.splitlines()), "")
    gateway = next(iter(first_line.split()), "")
    socket.inet_aton(gateway)
    route = f"{gateway}:{peer_port}"
    result = subprocess.run(
        [
            "docker", "run", "--rm", "--add-host",
            "host.docker.internal:host-gateway", "--mount", mount,
            options.image, "queue-drain", f"/data/{relative_stage}", route, "8",
        ],
        capture_output=True,
        text=True,
        timeout=90,
    )
    if result.returncode != 0:
        raise AssertionError(f"queue-drain failed: {result.stderr}")
    return result.stdout


def verify_attachment(message: bytes, expected: bytes) -> None:
    """Сравнивает MIME-вложение после локальной и исходящей доставки."""
    parsed = BytesParser(policy=policy.default).parsebytes(message)
    attachments = [part.get_payload(decode=True) for part in parsed.iter_attachments()]
    if attachments != [expected]:
        raise AssertionError(f"MIME attachment bytes differ: {attachments!r}")


def main() -> None:
    """Проверяет :copy local keep и реальный durable SMTP redirect."""
    options = arguments()
    secrets = credentials(Path(options.credentials))
    context = ssl.create_default_context(cafile=options.ca)
    configure_redirect_copy(
        "127.0.0.1", options.sieve_port, context, secrets[Account.BOB]
    )
    message, attachment = message_with_attachment()
    submit_message("127.0.0.1", options.smtp_port, context, secrets[Account.ALICE], message)
    stored = local_message(
        "127.0.0.1", options.imap_port, context, secrets[Account.BOB],
        "RFC 3894 redirect copy attachment",
    )
    if not stored.endswith(message):
        raise AssertionError("local inbox changed submitted RFC822 bytes after trace headers")
    verify_attachment(stored, attachment)

    script_path = Path(__file__).resolve()
    repository = script_path.parent.parent.parent
    stop_source(options.container)
    with CaptureServer(("0.0.0.0", 0)) as peer:
        _, peer_port = peer.server_address
        worker = threading.Thread(target=peer.serve_forever, daemon=True)
        worker.start()
        try:
            report = drain_queue(repository, options, peer_port)
        finally:
            peer.shutdown()
            worker.join(timeout=5)
        if "delivered=1" not in report:
            raise AssertionError(f"expected one successful redirect job: {report!r}")
        if peer.envelope_sender != "MAIL FROM:<alice@litterkeep.test>":
            raise AssertionError(f"redirect sender differs: {peer.envelope_sender!r}")
        if peer.recipient != "RCPT TO:<remote@peer.test>":
            raise AssertionError(f"redirect recipient differs: {peer.recipient!r}")
        if peer.payload != stored:
            raise AssertionError("external redirected RFC822 bytes differ from local copy")
        verify_attachment(peer.payload, attachment)
    print("RFC 3894 redirect :copy passed: local IMAP + queued SMTP peer + exact attachment")


if __name__ == "__main__":
    main()
