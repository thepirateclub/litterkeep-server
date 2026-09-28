"""Создаёт закреплённую O(1) таблицу индекса EUC-KR WHATWG."""

from __future__ import annotations

import argparse
import hashlib
import re
import struct
from pathlib import Path


INDEX_SHA256 = "89af20dd867c84cefb710b1790229786cfef2bf11916361a210d81b90381e267"
POINTER_COUNT = 23_940
HEADER = b"LKEK" + struct.pack("<I", POINTER_COUNT)
ENTRY = re.compile(r"^\s*(?P<pointer>\d+)\s+0x(?P<scalar>[0-9A-F]+)\b", re.MULTILINE)


def build_table(source_path: Path) -> bytes:
    """Проверяет исходный hash, плотность порядка и каждую Unicode scalar."""
    source = source_path.read_bytes()
    actual_hash = hashlib.sha256(source).hexdigest()
    if actual_hash != INDEX_SHA256:
        raise ValueError(f"SHA-256 WHATWG index не совпал: {source_path}")
    matches = iter(ENTRY.finditer(source.decode("utf-8")))
    current = next(matches, None)
    table = bytearray(HEADER)
    for pointer in range(POINTER_COUNT):
        match current:
            case item if item is not None and int(item.group("pointer")) == pointer:
                scalar = int(item.group("scalar"), 16)
                match scalar:
                    case 0:
                        raise ValueError(f"Недопустимая Unicode scalar: U+{scalar:04X}")
                    case value if 0xD800 <= value <= 0xDFFF:
                        raise ValueError(f"Недопустимая Unicode scalar: U+{scalar:04X}")
                    case value if value > 0x10FFFF:
                        raise ValueError(f"Недопустимая Unicode scalar: U+{scalar:04X}")
                    case _:
                        pass
                table.extend(struct.pack("<I", scalar))
                current = next(matches, None)
            case item if item is not None and int(item.group("pointer")) < pointer:
                raise ValueError("WHATWG index не отсортирован или содержит duplicate")
            case _:
                table.extend(b"\x00\x00\x00\x00")
    if current is not None:
        raise ValueError("WHATWG index содержит pointer вне объявленного размера")
    return bytes(table)


def main() -> None:
    """Публикует deterministic binary lookup asset в целевой каталог."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--index", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    arguments = parser.parse_args()
    arguments.output.parent.mkdir(parents=True, exist_ok=True)
    arguments.output.write_bytes(build_table(arguments.index))


if __name__ == "__main__":
    main()
