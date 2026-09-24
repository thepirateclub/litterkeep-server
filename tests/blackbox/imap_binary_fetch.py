"""Сравнивает RFC 9051 BINARY с исходным MIME вложением через STARTTLS."""

import argparse
import base64
import email
import os
import re
import smtplib
import socket
import ssl
import uuid
from itertools import islice
from email.message import EmailMessage
from email.policy import SMTP


def options():
    parser = argparse.ArgumentParser()
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--smtp-port", type=int, default=2525)
    parser.add_argument("--imap-port", type=int, default=2143)
    parser.add_argument("--ca", default="data/pki/ca.pem")
    return parser.parse_args()


def message_fixture(subject):
    attachment = bytes(range(256)) + b"\x00binary-fetch\xff"
    message = EmailMessage()
    message["From"] = "Alice <alice@test>"
    message["To"] = "Bob <bob@test>"
    message["Subject"] = subject
    message.set_content("BINARY acceptance body")
    message.add_attachment(
        attachment,
        maintype="application",
        subtype="octet-stream",
        filename="binary.bin",
    )
    message.set_boundary("g14-binary-" + uuid.uuid4().hex)
    return message.as_bytes(policy=SMTP), attachment


class ImapWire:
    """Минимальный независимый reader для literal8, которого нет в imaplib."""

    def __init__(self, host, port, context):
        self.socket = socket.create_connection((host, port), timeout=10)
        self.stream = self.socket.makefile("rb")
        self.counter = 0
        greeting = self.stream.readline()
        if not greeting.startswith(b"* OK"):
            raise AssertionError(f"unexpected greeting: {greeting!r}")
        response = self.command(b"STARTTLS")
        if b" OK " not in response:
            raise AssertionError(f"STARTTLS failed: {response!r}")
        self.stream.close()
        self.socket = context.wrap_socket(self.socket, server_hostname=host)
        self.stream = self.socket.makefile("rb")

    def command(self, command):
        self.counter += 1
        tag = f"B{self.counter:03}".encode("ascii")
        self.socket.sendall(tag + b" " + command + b"\r\n")
        response = bytearray()
        while True:
            line = self.stream.readline()
            if not line:
                raise AssertionError("server closed during IMAP response")
            response.extend(line)
            marker = re.search(rb"(?:~)?\{(?P<size>[0-9]+)\}\r\n$", line)
            if marker is not None:
                literal_size = int(marker.group("size"))
                literal = self.stream.read(literal_size)
                if len(literal) != literal_size:
                    raise AssertionError("truncated IMAP literal")
                response.extend(literal)
                response.extend(self.stream.readline())
                continue
            if line.startswith(tag + b" "):
                return bytes(response)

    def close(self):
        try:
            self.command(b"LOGOUT")
        finally:
            self.stream.close()
            self.socket.close()


def require_ok(response, operation):
    if re.search(rb"(?m)^B[0-9]+ OK ", response) is None:
        raise AssertionError(f"{operation} failed: {response!r}")


def main():
    selected = options()
    context = ssl.create_default_context(cafile=selected.ca)
    subject = "binary fetch " + uuid.uuid4().hex
    raw_message, expected = message_fixture(subject)
    smtp_context = ssl.create_default_context()
    smtp_context.check_hostname = False
    smtp_context.verify_mode = ssl.CERT_NONE
    with smtplib.SMTP(selected.host, selected.smtp_port, timeout=10) as smtp:
        smtp.starttls(context=smtp_context)
        smtp.login("alice", os.environ["LITTERKEEP_ALICE_PASSWORD"])
        smtp.sendmail("alice@test", ["bob@test"], raw_message)

    client = ImapWire(selected.host, selected.imap_port, context)
    try:
        password = os.environ["LITTERKEEP_BOB_PASSWORD"]
        sasl = base64.b64encode(b"\x00bob\x00" + password.encode("utf-8"))
        require_ok(client.command(b"AUTHENTICATE PLAIN " + sasl), "authentication")
        require_ok(client.command(b"SELECT INBOX"), "mailbox selection")
        search = client.command(b'SEARCH HEADER Subject "' + subject.encode() + b'"')
        require_ok(search, "message search")
        search_line = next(
            line for line in search.splitlines() if line.startswith(b"* SEARCH ")
        )
        sequence = next(iter(search_line.split()[2:]), None)
        if sequence is None:
            raise AssertionError(f"fixture message not found: {search!r}")

        peek = client.command(
            b"FETCH " + sequence + b" (BINARY.SIZE[2] BINARY.PEEK[2])"
        )
        require_ok(peek, "BINARY.PEEK")
        expected_marker = b"~{" + str(len(expected)).encode("ascii") + b"}\r\n"
        if expected_marker not in peek:
            raise AssertionError(f"NUL payload was not encoded as literal8: {peek!r}")
        if expected not in peek:
            raise AssertionError("BINARY decoded octets differ from SMTP attachment")
        size_match = re.search(rb"BINARY\.SIZE\[2\] (?P<size>[0-9]+)", peek)
        if size_match is None or int(size_match.group("size")) != len(expected):
            raise AssertionError(f"BINARY.SIZE differs from decoded length: {peek!r}")

        flags = client.command(b"FETCH " + sequence + b" (FLAGS)")
        require_ok(flags, "FLAGS after BINARY.PEEK")
        if b"\\Seen" in flags:
            raise AssertionError("BINARY.PEEK unexpectedly set \\Seen")

        partial = client.command(
            b"FETCH " + sequence + b" (BINARY.PEEK[2]<3.8>)"
        )
        require_ok(partial, "decoded BINARY partial")
        decoded_partial = bytes(islice(iter(expected), 3, 11))
        if decoded_partial not in partial:
            raise AssertionError("BINARY partial was not applied to decoded octets")

        ordinary = client.command(b"FETCH " + sequence + b" (BINARY[2])")
        require_ok(ordinary, "BINARY")
        flags = client.command(b"FETCH " + sequence + b" (FLAGS)")
        require_ok(flags, "FLAGS after BINARY")
        if b"\\Seen" not in flags:
            raise AssertionError("BINARY did not set durable \\Seen")

        unsupported_subject = "unknown cte " + uuid.uuid4().hex
        unsupported = (
            b"From: Alice <alice@test>\r\n"
            b"To: Bob <bob@test>\r\n"
            + b"Subject: "
            + unsupported_subject.encode("ascii")
            + b"\r\nContent-Type: application/octet-stream\r\n"
            b"Content-Transfer-Encoding: x-unknown\r\n\r\nencoded bytes\r\n"
        )
        with smtplib.SMTP(selected.host, selected.smtp_port, timeout=10) as smtp:
            smtp.starttls(context=smtp_context)
            smtp.login("alice", os.environ["LITTERKEEP_ALICE_PASSWORD"])
            smtp.sendmail("alice@test", ["bob@test"], unsupported)
        search = client.command(
            b'SEARCH HEADER Subject "' + unsupported_subject.encode("ascii") + b'"'
        )
        require_ok(search, "unknown CTE message search")
        search_line = next(
            line for line in search.splitlines() if line.startswith(b"* SEARCH ")
        )
        unsupported_sequence = next(iter(search_line.split()[2:]), None)
        if unsupported_sequence is None:
            raise AssertionError(f"unknown CTE fixture not found: {search!r}")
        failure = client.command(b"FETCH " + unsupported_sequence + b" (BINARY[1])")
        if b"NO [UNKNOWN-CTE]" not in failure:
            raise AssertionError(f"unknown CTE did not produce response code: {failure!r}")
    finally:
        client.close()
    parsed = email.message_from_bytes(raw_message)
    decoded = next(
        part.get_payload(decode=True)
        for part in parsed.walk()
        if part.get_filename() == "binary.bin"
    )
    if decoded != expected:
        raise AssertionError("fixture MIME parser disagrees about attachment bytes")
    print("PASS imap-binary-fetch: size, literal8, partial, Seen, UNKNOWN-CTE")


if __name__ == "__main__":
    main()
