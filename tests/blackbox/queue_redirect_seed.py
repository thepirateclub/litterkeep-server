"""Создаёт реальный Sieve redirect job независимыми ManageSieve и SMTP clients."""
import argparse
import base64
import smtplib
import socket
import ssl
from email.message import EmailMessage
from email.policy import SMTP


def require(value: bool, message: str) -> None:
    if not value:
        raise AssertionError(message)


def read_line(client: socket.socket) -> bytes:
    result = bytearray()
    while not result.endswith(b"\r\n"):
        chunk = client.recv(4096)
        require(chunk != b"", "ManageSieve closed connection")
        result.extend(chunk)
    return bytes(result)


def command(client: socket.socket, wire: bytes) -> bytes:
    client.sendall(wire)
    return read_line(client)


def configure(options: argparse.Namespace, context: ssl.SSLContext) -> None:
    with socket.create_connection((options.host, options.managesieve_port), timeout=10) as plain:
        require(b"STARTTLS" in read_line(plain), "STARTTLS missing")
        require(command(plain, b"STARTTLS\r\n") == b'OK "begin TLS"\r\n', "STARTTLS")
        with context.wrap_socket(plain, server_hostname="litterkeep.test") as client:
            credentials = base64.b64encode(b"\x00bob\x00bob-password")
            wire = b'AUTHENTICATE "PLAIN" "' + credentials + b'"\r\n'
            require(command(client, wire) == b'OK "authenticated"\r\n', "AUTH")
            source = b'redirect "remote@peer.test"; discard;'
            marker = f'PUTSCRIPT "forward" {{{len(source)}+}}\r\n'.encode()
            client.sendall(marker + source)
            require(read_line(client) == b'OK "completed"\r\n', "PUTSCRIPT")
            require(command(client, b'SETACTIVE "forward"\r\n') == b'OK "completed"\r\n', "SETACTIVE")


def deliver(options: argparse.Namespace, context: ssl.SSLContext) -> None:
    message = EmailMessage()
    message["From"] = "alice@litterkeep.test"
    message["To"] = "bob@litterkeep.test"
    message["Subject"] = "queue redirect peer"
    message.set_content("queue body")
    with smtplib.SMTP(options.host, options.smtp_port, timeout=10) as client:
        client.ehlo(); client.starttls(context=context); client.ehlo()
        client.login("alice", "alice-password")
        client.sendmail("alice@litterkeep.test", ["bob@litterkeep.test"], message.as_bytes(policy=SMTP))


def main() -> None:
    parser = argparse.ArgumentParser()
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--smtp-port", type=int, required=True)
    parser.add_argument("--managesieve-port", type=int, required=True)
    parser.add_argument("--ca", required=True)
    options = parser.parse_args()
    context = ssl.create_default_context(cafile=options.ca)
    configure(options, context); deliver(options, context)
    print("queue redirect seed: PASS")


if __name__ == "__main__":
    main()
