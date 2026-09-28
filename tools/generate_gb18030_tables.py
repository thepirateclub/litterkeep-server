"""Генерирует закреплённый MIME lookup asset из индексов WHATWG Encoding."""

from __future__ import annotations

import argparse
import hashlib
import re
import struct
from pathlib import Path


TWO_BYTE_SHA256 = "746b3c55f1a8ec4b90b451f384437a17fd37cd51cd668456a28071a758d10784"
RANGES_SHA256 = "874c6b6f6f74cf7d427ad228d5b41ddd9354fffd92a2259bf429f86e6baa7a1e"
TABLE_HEADER = b"LKGK" + struct.pack("<II", 23940, 207)
POINTER_PATTERN = re.compile(r"^\s*(?P<pointer>\d+)\s+(?P<codepoint>0x[0-9A-F]+)\b", re.MULTILINE)


def verified_text(path: Path, expected_hash: str) -> str:
    """Читает только зафиксированную версию исходного WHATWG index файла."""
    payload = path.read_bytes()
    actual_hash = hashlib.sha256(payload).hexdigest()
    if actual_hash != expected_hash:
        raise ValueError(f"SHA-256 источника не совпал: {path}")
    return payload.decode("utf-8")


def pointer_records(source: str) -> list[tuple[int, int]]:
    """Извлекает числовые пары pointer/code point из нормативного индекса."""
    return [
        (int(match.group("pointer")), int(match.group("codepoint"), 16))
        for match in POINTER_PATTERN.finditer(source)
    ]


def dense_two_byte_table(records: list[tuple[int, int]]) -> bytes:
    """Создаёт O(1) pointer table с нулём как sentinel незаданной позиции."""
    table = bytearray()
    record_iterator = iter(records)
    current = next(record_iterator, None)
    for pointer in range(23940):
        match current:
            case (record_pointer, codepoint) if record_pointer == pointer:
                if codepoint == 0:
                    raise ValueError("U+0000 конфликтует с sentinel таблицы")
                table.extend(struct.pack("<I", codepoint))
                current = next(record_iterator, None)
            case (record_pointer, _) if record_pointer < pointer:
                raise ValueError("указатели индекса не отсортированы")
            case _:
                table.extend(b"\x00\x00\x00\x00")
    if current is not None:
        raise ValueError("указатель выходит за пределы GB18030 двухбайтового индекса")
    return bytes(table)


def range_table(records: list[tuple[int, int]]) -> bytes:
    """Сериализует отсортированные начало диапазона и начальный code point."""
    if len(records) != 207:
        raise ValueError(f"Ожидалось 207 диапазонов, получено {len(records)}")
    previous_pointer = -1
    for pointer, codepoint in records:
        match (pointer > previous_pointer, codepoint <= 0x10FFFF):
            case (True, True):
                pass
            case _:
                raise ValueError("GB18030 ranges должны быть отсортированы и содержать scalars")
        previous_pointer = pointer
    return b"".join(struct.pack("<II", pointer, codepoint) for pointer, codepoint in records)


def generate(arguments: argparse.Namespace) -> None:
    """Проверяет первичные файлы и публикует детерминированный бинарный asset."""
    pair_source = verified_text(arguments.two_byte_index, TWO_BYTE_SHA256)
    range_source = verified_text(arguments.ranges_index, RANGES_SHA256)
    pairs = pointer_records(pair_source)
    ranges = pointer_records(range_source)
    if len(pairs) != 23940:
        raise ValueError(f"Ожидалось 23940 записей индекса, получено {len(pairs)}")
    if len(ranges) != 207:
        raise ValueError(f"Ожидалось 207 диапазонов, получено {len(ranges)}")
    arguments.output.parent.mkdir(parents=True, exist_ok=True)
    arguments.output.write_bytes(TABLE_HEADER + dense_two_byte_table(pairs) + range_table(ranges))


def parse_arguments() -> argparse.Namespace:
    """Разбирает пути к закреплённым индексам и выходному asset."""
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--two-byte-index", type=Path, required=True)
    parser.add_argument("--ranges-index", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    return parser.parse_args()


if __name__ == "__main__":
    generate(parse_arguments())
