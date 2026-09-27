"""Проверяет exact-byte отказ на BDAT без объявленного CHUNKING."""

from enum import Enum, IntEnum
import argparse
import re
import socket


class ReplyCode(IntEnum):
    """Числовые SMTP-коды проверяемого отказного сценария."""

    GREETING = 220
    OK = 250
    BAD_SEQUENCE = 502
    BYE = 221


class ReplySeparator(Enum):
    """Маркер конца SMTP single-line или multiline reply."""

    CONTINUATION = b"-"
    FINAL = b" "


class Capability(Enum):
    """SMTP EHLO capability atom, который пока нельзя рекламировать."""

    CHUNKING = "CHUNKING"


REPLY = re.compile(rb"\A([0-9]{3})([- ])")


def read_reply(stream):
    """Считывает весь SMTP frame, проверяя единый код multiline ответа."""
    lines = []
    code = None
    separator = None
    while separator is not ReplySeparator.FINAL:
        line = stream.readline()
        match = REPLY.match(line)
        if match is None:
            raise AssertionError(f"malformed SMTP reply: {line!r}")
        current, raw_separator = match.groups()
        separator = ReplySeparator(raw_separator)
        if code is None:
            code = current
        if current != code:
            raise AssertionError(f"mixed SMTP codes in reply: {lines!r}")
        lines.append(line)
    return ReplyCode(int(code)), tuple(lines)


def expect_reply(stream, expected):
    """Сверяет typed reply code и возвращает payload lines."""
    actual, lines = read_reply(stream)
    if actual is not expected:
        raise AssertionError(f"expected {expected}, received {actual}: {lines!r}")
    return lines


def verify_capability(lines):
    """Не принимает отсутствие или случайный фрагмент CHUNKING за capability."""
    atoms = tuple(
        atom.decode("ascii").upper()
        for line in lines
        for atom in line.removeprefix(b"250-").removeprefix(b"250 ").strip().split()
    )
    known_capabilities = {
        Capability.__members__.get(atom)
        for atom in atoms
    }
    if Capability.CHUNKING in known_capabilities:
        raise AssertionError("server advertised CHUNKING before implementing BDAT")


def main():
    """Проверяет raw chunk drain, command boundary и session recovery."""
    parser = argparse.ArgumentParser()
    parser.add_argument("--host", default="127.0.0.1")
    parser.add_argument("--port", type=int, default=1587)
    options = parser.parse_args()
    with socket.create_connection((options.host, options.port), timeout=10) as peer:
        stream = peer.makefile("rwb", buffering=0)
        expect_reply(stream, ReplyCode.GREETING)
        stream.write(b"EHLO bdat-probe.test\r\n")
        verify_capability(expect_reply(stream, ReplyCode.OK))

        payload = b"\x00\r\nX!!"
        size = str(len(payload)).encode("ascii")
        stream.write(b"BDAT " + size + b" LAST\r\n" + payload + b"NOOP\r\n")
        expect_reply(stream, ReplyCode.BAD_SEQUENCE)
        expect_reply(stream, ReplyCode.OK)
        stream.write(b"QUIT\r\n")
        expect_reply(stream, ReplyCode.BYE)
        stream.close()
    print("PASS smtp-bdat-unadvertised: exact drain and NOOP synchronization")


if __name__ == "__main__":
    main()
