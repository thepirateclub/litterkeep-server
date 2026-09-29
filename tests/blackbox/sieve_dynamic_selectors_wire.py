#!/usr/bin/env python3
"""Проверяет переменные Sieve selector-ы по ManageSieve, SMTP и IMAP."""

import argparse
import base64
import imaplib
import os
import smtplib
import socket
import ssl
from email.message import EmailMessage
from email.policy import SMTP


def require(condition: bool, message: str) -> None:
    """Останавливает wire acceptance на первом нарушенном контракте."""
    if not condition:
        raise AssertionError(message)


def read_until(connection: socket.socket, marker: bytes) -> bytes:
    """Читает ответ целиком, не предполагая совпадения TCP и строк."""
    received = bytearray()
    while marker not in received:
        chunk = connection.recv(4096)
        require(bool(chunk), "ManageSieve закрыл соединение до ответа")
        received.extend(chunk)
    return bytes(received)


def command(connection: socket.socket, wire: bytes) -> bytes:
    """Передаёт одну короткую RFC 5804 команду."""
    connection.sendall(wire)
    return read_until(connection, b"\r\n")


def install_script(options: argparse.Namespace, context: ssl.SSLContext) -> None:
    """Активирует правило с переменными comparator и envelope-part."""
    bob_password = os.environ["LITTERKEEP_BOB_PASSWORD"]
    source = (
        b'require ["envelope", "fileinto", "variables"]; '
        b'set "part" "to"; set "cmp" "i;ascii-casemap"; '
        b'if envelope :comparator "${cmp}" "${part}" '
        b'"bob@litterkeep.test" { fileinto "Invoices"; }'
    )
    with socket.create_connection((options.host, options.managesieve_port), timeout=10) as plain:
        greeting = read_until(plain, b'OK "ready"\r\n')
        require(b'"variables"' not in greeting, "частичный Variables ошибочно рекламирован")
        require(command(plain, b"STARTTLS\r\n") == b'OK "begin TLS"\r\n', "STARTTLS")
        with context.wrap_socket(plain, server_hostname="litterkeep.test") as client:
            read_until(client, b'OK "ready"\r\n')
            credential = base64.b64encode(b"\x00bob\x00" + bob_password.encode())
            auth = b'AUTHENTICATE "PLAIN" "' + credential + b'"\r\n'
            client.sendall(auth)
            require(read_until(client, b'OK "authenticated"\r\n').endswith(b'OK "authenticated"\r\n'), "ManageSieve AUTH")
            check = f"CHECKSCRIPT {{{len(source)}}}\r\n".encode("ascii")
            require(command(client, check) == b'OK "ready for literal data"\r\n', "CHECKSCRIPT")
            client.sendall(source + b"\r\n")
            require(read_until(client, b"\r\n") == b'OK "completed"\r\n', "CHECKSCRIPT result")
            marker = f'PUTSCRIPT "dynamic" {{{len(source)}+}}\r\n'.encode("ascii")
            client.sendall(marker + source + b"\r\n")
            require(read_until(client, b"\r\n") == b'OK "completed"\r\n', "PUTSCRIPT")
            require(command(client, b'SETACTIVE "dynamic"\r\n') == b'OK "completed"\r\n', "SETACTIVE")
            require(command(client, b"LOGOUT\r\n") == b'OK "bye"\r\n', "ManageSieve LOGOUT")


def submit_message(options: argparse.Namespace, context: ssl.SSLContext) -> None:
    """Доставляет контрольное письмо через STARTTLS SMTP submission."""
    message = EmailMessage()
    message["From"] = "alice@litterkeep.test"
    message["To"] = "bob@litterkeep.test"
    message["Subject"] = "dynamic Sieve selector acceptance"
    message.set_content("The envelope selector selected Invoices.")
    with smtplib.SMTP(options.host, options.smtp_port, timeout=10) as client:
        client.ehlo()
        client.starttls(context=context)
        client.ehlo()
        client.login("alice", os.environ["LITTERKEEP_ALICE_PASSWORD"])
        client.send_message(message)


def mailbox_payload(client: imaplib.IMAP4, mailbox: str) -> list[bytes]:
    """Возвращает полные RFC822 literals выбранного mailbox."""
    status, _ = client.select(mailbox, readonly=True)
    require(status == "OK", f"IMAP SELECT {mailbox}")
    status, results = client.uid("SEARCH", None, "ALL")
    require(status == "OK", f"IMAP UID SEARCH {mailbox}")
    identifiers = next(iter(results), b"").split()
    payloads = []
    for identifier in identifiers:
        status, records = client.uid("FETCH", identifier, "RFC822")
        require(status == "OK", f"IMAP UID FETCH {mailbox}")
        for record in records:
            if isinstance(record, tuple):
                _, payload = record
                if isinstance(payload, bytes):
                    payloads.append(payload)
    return payloads


def verify_delivery(options: argparse.Namespace, context: ssl.SSLContext) -> None:
    """Доказывает маршрутизацию selector-ом чтением независимым IMAP-клиентом."""
    with imaplib.IMAP4(options.host, options.imap_port, timeout=10) as client:
        status, _ = client.starttls(ssl_context=context)
        require(status == "OK", "IMAP STARTTLS")
        require(client.login("bob", os.environ["LITTERKEEP_BOB_PASSWORD"])[0] == "OK", "IMAP LOGIN")
        inbox = mailbox_payload(client, "INBOX")
        invoices = mailbox_payload(client, "Invoices")
        require(not inbox, "сообщение осталось в INBOX: envelope selector не совпал")
        require(len(invoices) == 1, "Invoices не содержит ровно одно сообщение")
        require(b"dynamic Sieve selector acceptance" in invoices[0], "прочитано другое письмо")


def main() -> None:
    """Запускает полный ManageSieve -> SMTP -> IMAP сценарий."""
    parser = argparse.ArgumentParser()
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--smtp-port", type=int, required=True)
    parser.add_argument("--imap-port", type=int, required=True)
    parser.add_argument("--managesieve-port", type=int, required=True)
    parser.add_argument("--ca", required=True)
    options = parser.parse_args()
    context = ssl.create_default_context(cafile=options.ca)
    install_script(options, context)
    submit_message(options, context)
    verify_delivery(options, context)
    print("ManageSieve dynamic selectors -> SMTP -> IMAP: PASS")


if __name__ == "__main__":
    main()
