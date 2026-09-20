"""Независимо проверяет ManageSieve literal, SMTP-доставку и IMAP-чтение.

Клиент не импортирует код litterkeep: он использует socket для RFC 5804 framing,
а затем стандартные smtplib и imaplib. Тело Sieve script намеренно передаётся
двумя sendall, чтобы подтвердить корректность границы TCP-пакетов.
"""

import argparse
import base64
import imaplib
import smtplib
import socket
import ssl
from email.message import EmailMessage


def require(condition: bool, message: str) -> None:
    if not condition:
        raise AssertionError(message)


def read_until(client: socket.socket, marker: bytes) -> bytes:
    received = bytearray()
    while marker not in received:
        chunk = client.recv(4096)
        require(chunk != b"", "сервер закрыл ManageSieve-соединение")
        received.extend(chunk)
    return bytes(received)


def command(client: socket.socket, wire: bytes) -> bytes:
    client.sendall(wire)
    return read_until(client, b"\r\n")


def command_until(client: socket.socket, wire: bytes, marker: bytes) -> bytes:
    client.sendall(wire)
    return read_until(client, marker)


def configure_sieve(options: argparse.Namespace, context: ssl.SSLContext) -> None:
    with socket.create_connection((options.host, options.managesieve_port), timeout=10) as plain:
        greeting = read_until(plain, b'OK "ready"\r\n')
        require(b'"STARTTLS"' in greeting, "ManageSieve не объявил STARTTLS")
        plaintext_credentials = base64.b64encode(b"\x00bob\x00bob-password")
        plaintext_auth = b'AUTHENTICATE "PLAIN" "' + plaintext_credentials + b'"\r\n'
        require(
            command(plain, plaintext_auth) == b'NO "encryption required"\r\n',
            "plaintext AUTHENTICATE был ошибочно принят",
        )
        starttls = command(plain, b"STARTTLS\r\n")
        require(starttls == b'OK "begin TLS"\r\n', "STARTTLS ManageSieve отклонён")
        with context.wrap_socket(plain, server_hostname="litterkeep.test") as client:
            credentials = base64.b64encode(b"\x00bob\x00bob-password")
            authentication = b'AUTHENTICATE "PLAIN" "' + credentials + b'"\r\n'
            require(
                command(client, authentication) == b'OK "authenticated"\r\n',
                "ManageSieve AUTHENTICATE не принят",
            )
            checked_source = b"keep;"
            check_marker = f"CHECKSCRIPT {{{len(checked_source)}}}\r\n".encode()
            require(
                command(client, check_marker) == b'OK "ready for literal data"\r\n',
                "синхронизирующий CHECKSCRIPT literal не получил continuation",
            )
            require(
                command(client, checked_source) == b'OK "completed"\r\n',
                "CHECKSCRIPT literal не проверен",
            )
            first_chunk = b'require ["fileinto"]; '
            second_chunk = b'if header :contains "subject" "[invoice]" { fileinto "Invoices"; stop; }'
            source = first_chunk + second_chunk
            havespace = f'HAVESPACE "invoices" {len(source)}\r\n'.encode()
            require(
                command(client, havespace) == b'OK "space available"\r\n',
                "HAVESPACE не подтвердил допустимый script",
            )
            require(
                command(client, b'HAVESPACE "oversized" 65537\r\n')
                == b'NO (QUOTA) "space unavailable"\r\n',
                "HAVESPACE не отверг превышенную квоту",
            )
            marker = f'PUTSCRIPT "invoices" {{{len(source)}+}}\r\n'.encode()
            client.sendall(marker)
            client.sendall(first_chunk)
            client.sendall(second_chunk)
            require(
                read_until(client, b"\r\n") == b'OK "completed"\r\n',
                "PUTSCRIPT literal не сохранён",
            )
            require(
                command(client, b'SETACTIVE "invoices"\r\n') == b'OK "completed"\r\n',
                "SETACTIVE не сохранён",
            )
            invalid_source = b"if true { unsupported; }"
            invalid_marker = f'PUTSCRIPT "broken" {{{len(invalid_source)}+}}\r\n'.encode()
            client.sendall(invalid_marker)
            client.sendall(invalid_source)
            require(
                read_until(client, b"\r\n") == b'NO "operation failed"\r\n',
                "некорректный PUTSCRIPT был ошибочно сохранён",
            )
            listed = command_until(client, b"LISTSCRIPTS\r\n", b'OK "list completed"\r\n')
            require(b'"invoices" ACTIVE\r\n' in listed, "LISTSCRIPTS не вернул active script")
            require(b'"broken"' not in listed, "невалидная версия попала в LISTSCRIPTS")
            source_reply = command_until(client, b'GETSCRIPT "invoices"\r\n', b'OK "get completed"\r\n')
            require(source in source_reply, "GETSCRIPT не вернул исходные байты source")
            scratch = b"keep;"
            scratch_marker = f'PUTSCRIPT "scratch" {{{len(scratch)}+}}\r\n'.encode()
            client.sendall(scratch_marker)
            client.sendall(scratch)
            require(
                read_until(client, b"\r\n") == b'OK "completed"\r\n',
                "PUTSCRIPT scratch не сохранён",
            )
            require(
                command(client, b'RENAMESCRIPT "scratch" "temporary"\r\n') == b'OK "completed"\r\n',
                "RENAMESCRIPT не сохранён",
            )
            require(
                command(client, b'DELETESCRIPT "temporary"\r\n') == b'OK "completed"\r\n',
                "DELETESCRIPT не сохранён",
            )
            require(
                command(client, b'DELETESCRIPT "invoices"\r\n') == b'NO "operation failed"\r\n',
                "DELETESCRIPT удалил active script",
            )
            require(command(client, b"NOOP\r\n") == b'OK "noop"\r\n', "NOOP не принят")
            require(command(client, b"LOGOUT\r\n") == b'OK "bye"\r\n', "LOGOUT не принят")


def deliver(options: argparse.Namespace, context: ssl.SSLContext) -> None:
    message = EmailMessage()
    message["From"] = "alice@litterkeep.test"
    message["To"] = "bob@litterkeep.test"
    message["Subject"] = "[invoice] paid"
    message.set_content("invoice body")
    with smtplib.SMTP(options.host, options.smtp_port, timeout=10) as client:
        client.ehlo()
        client.starttls(context=context)
        client.ehlo()
        client.login("alice", "alice-password")
        client.send_message(message)


def verify_mailbox(options: argparse.Namespace, context: ssl.SSLContext) -> None:
    with imaplib.IMAP4(options.host, options.imap_port, timeout=10) as client:
        tls_status, _ = client.starttls(ssl_context=context)
        require(tls_status == "OK", "IMAP TLS")
        require(client.login("bob", "bob-password")[0] == "OK", "IMAP LOGIN")
        status, data = client.select("Invoices")
        require(status == "OK", "Sieve folder не выбран по IMAP")
        require(data != [b"0"], "Sieve не доставил письмо в Invoices")
        status, data = client.uid("SEARCH", None, "ALL")
        require(status == "OK", "IMAP UID SEARCH")
        search_result = next(iter(data), b"").strip()
        identifier, _, _ = search_result.partition(b" ")
        require(identifier != b"", "не найден UID доставленного письма")
        status, fetched = client.uid("FETCH", identifier, "RFC822")
        require(status == "OK", "IMAP UID FETCH")
        payload = b"".join(fetch_bytes(item) for item in fetched)
        require(b"[invoice] paid" in payload, "прочитано не то письмо")


def fetch_bytes(item: object) -> bytes:
    if not isinstance(item, tuple):
        return b""
    _, payload = item
    return payload if isinstance(payload, bytes) else b""


def parse_arguments() -> argparse.Namespace:
    parser = argparse.ArgumentParser()
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--smtp-port", type=int, required=True)
    parser.add_argument("--imap-port", type=int, required=True)
    parser.add_argument("--managesieve-port", type=int, required=True)
    parser.add_argument("--ca", required=True)
    return parser.parse_args()


def main() -> None:
    options = parse_arguments()
    context = ssl.create_default_context(cafile=options.ca)
    configure_sieve(options, context)
    deliver(options, context)
    verify_mailbox(options, context)
    print("ManageSieve literal -> SMTP -> IMAP: PASS")


if __name__ == "__main__":
    main()
