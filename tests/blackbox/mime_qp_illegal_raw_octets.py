"""Проверяет fail-closed MIME QP projection при незаконных literal octets."""

import argparse
import base64
import csv
from enum import Enum
import re
import socket
import ssl
import uuid


class Account(Enum):
    """Стандартные fixture-аккаунты, нужные acceptance-сценарию."""

    BOB = "bob"


class ImapStatus(Enum):
    """Tagged completion, который принимает отдельная проверка."""

    OK = "OK"
    NO = "NO"


class ImapWire:
    """Читает полные IMAP wire-ответы, не теряя response-code и literal."""

    def __init__(self, host: str, port: int, context: ssl.SSLContext):
        self.connection = socket.create_connection((host, port), timeout=15)
        self.stream = self.connection.makefile("rb")
        self.counter = 0
        greeting = self.stream.readline()
        if not greeting.startswith(b"* OK"):
            raise AssertionError(f"Неожиданное приветствие IMAP: {greeting!r}")
        starttls = self.command(b"STARTTLS")
        if b" OK " not in starttls:
            raise AssertionError(f"STARTTLS завершился ошибкой: {starttls!r}")
        self.stream.close()
        self.connection = context.wrap_socket(self.connection, server_hostname=host)
        self.stream = self.connection.makefile("rb")

    def command(self, command: bytes) -> bytes:
        """Выполняет команду и сохраняет байты до её tagged completion."""
        self.counter += 1
        tag = f"Q{self.counter:03}".encode("ascii")
        self.connection.sendall(tag + b" " + command + b"\r\n")
        return self.read_tagged(tag)

    def append(self, message: bytes) -> bytes:
        """Передаёт synchronizing literal без преобразования байтов письма."""
        self.counter += 1
        tag = f"Q{self.counter:03}".encode("ascii")
        command = tag + b' APPEND "INBOX" {' + str(len(message)).encode() + b"}\r\n"
        self.connection.sendall(command)
        continuation = self.stream.readline()
        if not continuation.startswith(b"+"):
            raise AssertionError(f"APPEND не запросил literal: {continuation!r}")
        self.connection.sendall(message + b"\r\n")
        return self.read_tagged(tag)

    def read_tagged(self, tag: bytes) -> bytes:
        """Собирает строки и IMAP literals до завершения конкретного тега."""
        response = bytearray()
        while True:
            line = self.stream.readline()
            if not line:
                raise AssertionError("Сервер закрыл IMAP соединение")
            response.extend(line)
            marker = re.search(rb"\{([0-9]+)\}\r\n$", line)
            if marker is not None:
                length = int(marker.group(1))
                literal = self.stream.read(length)
                if len(literal) != length:
                    raise AssertionError("IMAP literal усечён")
                response.extend(literal)
                continue
            if line.startswith(tag + b" "):
                return bytes(response)

    def close(self) -> None:
        """Закрывает TLS reader и сетевой сокет."""
        self.stream.close()
        self.connection.close()


def read_bob_password(path: str) -> str:
    """Читает локальный секрет Bob без его вывода или передачи через argv."""
    with open(path, encoding="utf-8-sig", newline="") as source:
        for account_name, secret, *_ in csv.reader(source, delimiter="\t"):
            try:
                account = Account(account_name)
            except ValueError:
                continue
            if account is Account.BOB:
                return secret
    raise ValueError("В fixture отсутствует пароль Bob")


def require_status(response: bytes, expected: ImapStatus, operation: str) -> bytes:
    """Проверяет typed tagged status от IMAP и сохраняет весь wire-ответ."""
    tagged = re.search(rb"(?m)^Q[0-9]+ ([A-Z]+) ", response)
    if tagged is None:
        raise AssertionError(f"{operation}: нет tagged completion: {response!r}")
    try:
        observed = ImapStatus(tagged.group(1).decode("ascii"))
    except ValueError as error:
        raise AssertionError(f"{operation}: неизвестный статус: {response!r}") from error
    if observed is not expected:
        raise AssertionError(f"{operation}: ожидался {expected.name}: {response!r}")
    return response


def require_ok(response: bytes, operation: str) -> bytes:
    """Проверяет успешный tagged completion."""
    return require_status(response, ImapStatus.OK, operation)


def raw_message(subject: str) -> bytes:
    """Создаёт QP entity с запрещёнными сырыми control/high-bit octets."""
    return (
        f"Subject: {subject}\r\n".encode("ascii")
        + b"Content-Type: application/octet-stream\r\n"
        + b"Content-Transfer-Encoding: quoted-printable\r\n\r\n"
        + b"safe-prefix\x00\x0b\xff-safe-suffix\r\n"
    )


def require_unknown_cte(client: ImapWire, sequence: bytes) -> None:
    """Проверяет, что повреждённый QP не выдаётся как decoded BINARY data."""
    try:
        response = client.command(b"FETCH " + sequence + b" (BINARY.PEEK[1])")
    except OSError as error:
        raise AssertionError(f"FETCH оборвал IMAP соединение: {error!r}") from error
    require_status(response, ImapStatus.NO, "BINARY с недопустимыми QP octets")
    if re.search(rb"(?m)^Q[0-9]+ NO \[UNKNOWN-CTE\]", response) is None:
        raise AssertionError(f"Ожидался NO [UNKNOWN-CTE]: {response!r}")


def verify_original_and_flags(client: ImapWire, sequence: bytes, original: bytes) -> None:
    """Подтверждает точный RFC822 доступ и отсутствие побочного Seen."""
    records = require_ok(
        client.command(b"FETCH " + sequence + b" (BODY.PEEK[] FLAGS)"),
        "BODY.PEEK и FLAGS",
    )
    if original not in records:
        raise AssertionError("Исходный RFC822 literal изменился после BINARY отказа")
    if b"\\Seen" in records:
        raise AssertionError("Неуспешный BINARY FETCH установил Seen")


def run(settings: argparse.Namespace, password: str) -> None:
    """APPEND-ит raw fixture и проверяет projection через TLS IMAP peer."""
    subject = f"QP illegal octets {uuid.uuid4().hex}"
    original = raw_message(subject)
    context = ssl.create_default_context(cafile=settings.ca)
    client = ImapWire(settings.host, settings.port, context)
    try:
        sasl = base64.b64encode(b"\x00" + Account.BOB.value.encode() + b"\x00" + password.encode())
        require_ok(client.command(b"AUTHENTICATE PLAIN " + sasl), "LOGIN")
        require_ok(client.append(original), "APPEND literal")
        require_ok(client.command(b"SELECT INBOX"), "SELECT INBOX")
        search = require_ok(client.command(b'SEARCH HEADER SUBJECT "' + subject.encode() + b'"'), "SEARCH")
        search_line = next(line for line in search.splitlines() if line.startswith(b"* SEARCH "))
        numbers = search_line.removeprefix(b"* SEARCH ").split()
        match numbers:
            case [number]:
                sequence = number
            case _:
                raise AssertionError(f"Ожидалось одно письмо, получено {numbers!r}")
        require_unknown_cte(client, sequence)
        require_ok(client.command(b"NOOP"), "NOOP после BINARY отказа")
        verify_original_and_flags(client, sequence, original)
    finally:
        client.close()


def arguments() -> argparse.Namespace:
    """Разбирает endpoint и пути локального test fixture."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--host", default="localhost")
    parser.add_argument("--port", type=int, required=True)
    parser.add_argument("--ca", required=True)
    parser.add_argument("--credentials", required=True)
    return parser.parse_args()


def main() -> None:
    """Запускает независимую RFC 2045 QP projection проверку."""
    settings = arguments()
    password = read_bob_password(settings.credentials)
    run(settings, password)
    print("PASS MIME quoted-printable: illegal raw octets fail closed")


if __name__ == "__main__":
    main()
