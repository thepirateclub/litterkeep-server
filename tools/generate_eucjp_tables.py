"""Строит проверенные lookup assets WHATWG JIS 0208 и JIS 0212."""

from __future__ import annotations

import argparse
import hashlib
import re
import struct
from pathlib import Path


JIS0208_HASH = "341dcde7e8b984e9c7bbf5ed75c8da7c6087d47083a1a2b3ed558bfd5bef9468"
JIS0212_HASH = "6ac698a473822b282ff4f4d4f17873b970612ef9bd232e03941854c09b67c7ee"
JIS0208_POINTER_COUNT = 11_104
JIS0212_POINTER_COUNT = 8_836
HEADER = b"LKEJ" + struct.pack("<II", JIS0208_POINTER_COUNT, JIS0212_POINTER_COUNT)
PATTERN = re.compile(r"^\s*(?P<pointer>\d+)\s+0x(?P<codepoint>[0-9A-F]+)\b", re.M)


def dense_table(path: Path, expected_hash: str, pointer_count: int) -> bytes:
    """Проверяет источник и создаёт плотную little-endian pointer таблицу."""
    payload = path.read_bytes()
    if hashlib.sha256(payload).hexdigest() != expected_hash:
        raise ValueError(f"SHA-256 WHATWG индекса не совпал: {path}")
    text = payload.decode("utf-8")
    entries = iter(PATTERN.finditer(text))
    current = next(entries, None)
    result = bytearray()
    for pointer in range(pointer_count):
        match current:
            case entry if entry is not None and int(entry.group("pointer")) == pointer:
                code_point = int(entry.group("codepoint"), 16)
                match code_point:
                    case 0:
                        raise ValueError("индекс содержит недопустимый Unicode scalar")
                    case value if value > 0x10FFFF:
                        raise ValueError("индекс содержит недопустимый Unicode scalar")
                    case _:
                        pass
                result.extend(struct.pack("<I", code_point))
                current = next(entries, None)
            case entry if entry is not None and int(entry.group("pointer")) < pointer:
                raise ValueError("индекс не упорядочен или содержит дубликаты")
            case _:
                result.extend(b"\x00\x00\x00\x00")
    if current is not None:
        raise ValueError("указатель находится вне объявленного диапазона")
    return bytes(result)


def main() -> None:
    """Генерирует единый deterministic asset после проверки двух источников."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--jis0208", type=Path, required=True)
    parser.add_argument("--jis0212", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    args = parser.parse_args()
    table_0208 = dense_table(args.jis0208, JIS0208_HASH, JIS0208_POINTER_COUNT)
    table_0212 = dense_table(args.jis0212, JIS0212_HASH, JIS0212_POINTER_COUNT)
    args.output.parent.mkdir(parents=True, exist_ok=True)
    args.output.write_bytes(HEADER + table_0208 + table_0212)


if __name__ == "__main__":
    main()
