"""Проверяет RFC 2971 ID command, privacy response и negotiation."""

import argparse
from enum import Enum
import socket
import ssl


class Completion(Enum):
    """Разрешённые результаты проверяемых ID-команд."""

    OK = "OK"
    BAD = "BAD"


def options():
    """Читает параметры локального acceptance endpoint."""
    parser = argparse.ArgumentParser()
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=2143)
    parser.add_argument("--ca", default="data/pki/ca.pem")
    return parser.parse_args()


class ImapPeer:
    """Минимальный клиент tagged IMAP-команд с STARTTLS."""

    def __init__(self, host: str, port: int, context: ssl.SSLContext):
        self.host = host
        self.context = context
        self.socket = socket.create_connection((host, port), timeout=10)
        self.stream = self.socket.makefile("rb")
        self.counter = 0
        greeting = self.stream.readline()
        if not greeting.startswith(b"* OK"):
            raise AssertionError(f"unexpected greeting: {greeting!r}")

    def command(self, command: bytes) -> tuple[Completion, bytes]:
        """Читает ответ до собственного tag и декодирует enum завершения."""
        self.counter += 1
        tag = f"I{self.counter:03}".encode("ascii")
        self.socket.sendall(tag + b" " + command + b"\r\n")
        response = bytearray()
        while True:
            line = self.stream.readline()
            if not line:
                raise AssertionError("server closed during IMAP response")
            response.extend(line)
            if line.startswith(tag + b" "):
                match line.split(maxsplit=2):
                    case [_tag, atom, *_rest]:
                        try:
                            completion = Completion(atom.decode("ascii"))
                        except (UnicodeDecodeError, ValueError) as error:
                            raise AssertionError(
                                f"invalid tagged completion: {line!r}"
                            ) from error
                    case _:
                        raise AssertionError(f"invalid tagged completion: {line!r}")
                return completion, bytes(response)

    def id_literal(self, prefix: bytes, literal: bytes, suffix: bytes) -> bytes:
        """Отправляет один synchronizing literal и завершает ID-команду."""
        self.counter += 1
        tag = f"I{self.counter:03}".encode("ascii")
        marker = b"{" + str(len(literal)).encode("ascii") + b"}\r\n"
        self.socket.sendall(tag + b" ID (" + prefix + marker)
        continuation = self.stream.readline()
        if not continuation.startswith(b"+"):
            raise AssertionError(f"ID literal continuation missing: {continuation!r}")
        self.socket.sendall(literal + suffix + b")\r\n")
        response = bytearray()
        while True:
            line = self.stream.readline()
            if not line:
                raise AssertionError("server closed during literal ID response")
            response.extend(line)
            if line.startswith(tag + b" "):
                match line.split(maxsplit=2):
                    case [_tag, atom, *_rest]:
                        try:
                            completion = Completion(atom.decode("ascii"))
                        except (UnicodeDecodeError, ValueError) as error:
                            raise AssertionError(
                                f"invalid tagged completion: {line!r}"
                            ) from error
                    case _:
                        raise AssertionError(f"invalid tagged completion: {line!r}")
                require(completion, Completion.OK, "ID synchronizing literal")
                if b"* ID NIL\r\n" not in response:
                    raise AssertionError(f"privacy ID response missing: {response!r}")
                return bytes(response)

    def starttls(self) -> None:
        """Продолжает ту же IMAP-сессию поверх TLS после tagged OK."""
        completion, _response = self.command(b"STARTTLS")
        require(completion, Completion.OK, "STARTTLS")
        self.stream.close()
        self.socket = self.context.wrap_socket(self.socket, server_hostname=self.host)
        self.stream = self.socket.makefile("rb")

    def close(self) -> None:
        """Закрывает локальные transport handles."""
        self.stream.close()
        self.socket.close()


def require(observed: Completion, expected: Completion, operation: str) -> None:
    """Сравнивает typed completion и оставляет контекст неуспеха."""
    if observed is not expected:
        raise AssertionError(f"{operation}: expected {expected.name}, got {observed.name}")


def verify_id(peer: ImapPeer, command: bytes, expected: Completion) -> bytes:
    """Сверяет статус и NIL privacy response успешной ID-команды."""
    observed, response = peer.command(command)
    require(observed, expected, command.decode("ascii"))
    match expected:
        case Completion.OK if b"* ID NIL\r\n" not in response:
            raise AssertionError(f"server ID response must preserve privacy: {response!r}")
        case Completion.BAD if b"* ID " in response:
            raise AssertionError(f"rejected ID command emitted identification: {response!r}")
        case _:
            pass
    return response


def require_advertised(peer: ImapPeer, phase: str) -> None:
    """Проверяет отдельный RFC capability atom ID."""
    status, response = peer.command(b"CAPABILITY")
    require(status, Completion.OK, f"{phase} CAPABILITY")
    records = [line for line in response.splitlines() if line.startswith(b"* CAPABILITY ")]
    tokens = b" ".join(records).upper().split()
    if b"ID" not in tokens:
        raise AssertionError(f"{phase} CAPABILITY omitted ID: {tokens!r}")


def main() -> None:
    """Проверяет ID negotiation, strict syntax, privacy и session recovery."""
    selected = options()
    context = ssl.create_default_context(cafile=selected.ca)
    peer = ImapPeer(selected.host, selected.port, context)
    try:
        require_advertised(peer, "pre-STARTTLS")
        verify_id(peer, b"ID NIL", Completion.OK)
        verify_id(peer, b'ID ("name" "independent-client" "vendor" NIL)', Completion.OK)
        peer.id_literal(b"", b"name", b' "client"')
        peer.id_literal(b'"name" ', b"client", b"")
        verify_id(peer, b'ID ("name" NIL "NAME" NIL)', Completion.BAD)
        noop_status, _noop_response = peer.command(b"NOOP")
        require(noop_status, Completion.OK, "NOOP after invalid ID")
        peer.starttls()
        require_advertised(peer, "post-STARTTLS")
        verify_id(peer, b"ID ()", Completion.OK)
    finally:
        peer.close()
    print("PASS IMAP ID RFC 2971 negotiation, validation, privacy and recovery")


if __name__ == "__main__":
    main()
