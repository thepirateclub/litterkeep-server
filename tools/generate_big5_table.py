"""Создаёт Big5 decoder table с проверкой первичного WHATWG index файла."""

from __future__ import annotations

import argparse
import hashlib
import re
import struct
from pathlib import Path


INDEX_SHA256 = "08e24270c8e95d998c994c03f907e972480dc01f58743e078654cc466203c8ff"
POINTER_COUNT = 19_782
HEADER = b"LKBG" + struct.pack("<I", POINTER_COUNT)
POINTER_PATTERN = re.compile(r"^\s*(?P<pointer>\d+)\s+(?P<codepoint>0x[0-9A-F]+)\b", re.MULTILINE)


def read_verified_index(path: Path) -> str:
    """Проверяет digest официального index-big5.txt до разбора таблицы."""
    payload = path.read_bytes()
    if hashlib.sha256(payload).hexdigest() != INDEX_SHA256:
        raise ValueError(f"SHA-256 Big5 index не совпал: {path}")
    return payload.decode("utf-8")


def dense_pointer_table(source: str) -> bytes:
    """Строит O(1) таблицу code point, кодируя unmapped как нулевой sentinel."""
    entries = iter(POINTER_PATTERN.finditer(source))
    current = next(entries, None)
    table = bytearray()
    for pointer in range(POINTER_COUNT):
        match current:
            case entry if entry is not None and int(entry.group("pointer")) == pointer:
                codepoint = int(entry.group("codepoint"), 16)
                if codepoint == 0 or codepoint > 0x10FFFF:
                    raise ValueError("Big5 index содержит невалидный Unicode scalar")
                table.extend(struct.pack("<I", codepoint))
                current = next(entries, None)
            case entry if entry is not None and int(entry.group("pointer")) < pointer:
                raise ValueError("Big5 index pointers должны быть уникальны и отсортированы")
            case _:
                table.extend(b"\x00\x00\x00\x00")
    if current is not None:
        raise ValueError("Big5 pointer выходит за declared index bounds")
    return bytes(table)


def parse_arguments() -> argparse.Namespace:
    """Разбирает локальный source index и путь выходного Rust asset."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--index", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    return parser.parse_args()


def generate(arguments: argparse.Namespace) -> None:
    """Проверяет source и записывает фиксированную header/table структуру."""
    source = read_verified_index(arguments.index)
    arguments.output.parent.mkdir(parents=True, exist_ok=True)
    arguments.output.write_bytes(HEADER + dense_pointer_table(source))


if __name__ == "__main__":
    generate(parse_arguments())
