"""Генерирует закреплённые таблицы SASLprep Unicode 3.2 для Rust runtime."""

from __future__ import annotations

import hashlib
from itertools import islice
import re
import stringprep
import subprocess
import unicodedata
from pathlib import Path


ROOT = Path(__file__).resolve().parents[1]
OUTPUT = ROOT / "src" / "sasl" / "saslprep_data"
DATA = ROOT / "data" / "g18-saslprep-20260928"
SHARD_ENTRIES = 200

PINNED_INPUTS = {
    "rfc3454.txt": "EB722FA698FB7E8823B835D9FD263E4CDB8F1C7B0D234EDF7F0E3BD2CCBB2C79",
    "rfc4013.txt": "63DAA764693815AE8544728099D17A1C46E96B5818F2A399B3EC475EA153A991",
    "UnicodeData-3.2.0.txt": "5E444028B6E76D96F9DC509609C5E3222BF609056F35E5FCDE7E6FB8A58CD446",
    "NormalizationTest-3.2.0.txt": "C4513869BB7098D19838BE4A1FD5D760843C5804BFE03BD6BBB20623CEB6E57D",
}

# Биты соответствуют таблицам RFC 3454 и используются одним компактным индексом.
PROPERTY_TABLES = (
    ("in_table_a1", 1),
    ("in_table_b1", 2),
    ("in_table_c12", 4),
    ("in_table_c21", 8),
    ("in_table_c22", 16),
    ("in_table_c3", 32),
    ("in_table_c4", 64),
    ("in_table_c5", 128),
    ("in_table_c6", 256),
    ("in_table_c7", 512),
    ("in_table_c8", 1024),
    ("in_table_c9", 2048),
    ("in_table_d1", 4096),
    ("in_table_d2", 8192),
)
RFC_TABLES = (
    ("A.1", "in_table_a1"),
    ("B.1", "in_table_b1"),
    ("C.1.2", "in_table_c12"),
    ("C.2.1", "in_table_c21"),
    ("C.2.2", "in_table_c22"),
    ("C.3", "in_table_c3"),
    ("C.4", "in_table_c4"),
    ("C.5", "in_table_c5"),
    ("C.6", "in_table_c6"),
    ("C.7", "in_table_c7"),
    ("C.8", "in_table_c8"),
    ("C.9", "in_table_c9"),
    ("D.1", "in_table_d1"),
    ("D.2", "in_table_d2"),
)


def verify_pinned_sources() -> None:
    """Проверяет первоисточники и все scalar таблиц RFC 3454."""
    for name, expected_hash in PINNED_INPUTS.items():
        payload = (DATA / name).read_bytes()
        actual_hash = hashlib.sha256(payload).hexdigest().upper()
        if actual_hash != expected_hash:
            raise RuntimeError(f"pinned source hash mismatch: {name}")
    rfc_text = (DATA / "rfc3454.txt").read_text(encoding="utf-8-sig")
    for table_name, predicate_name in RFC_TABLES:
        expected_ranges = parse_rfc_ranges(rfc_text, table_name)
        compare_rfc_property(table_name, predicate_name, expected_ranges)


def parse_rfc_ranges(text: str, table_name: str) -> list[tuple[int, int]]:
    """Извлекает опубликованные включительные диапазоны одной RFC-таблицы."""
    capture = False
    result = []
    for line in text.splitlines():
        if f"Start Table {table_name}" in line:
            capture = True
            continue
        if f"End Table {table_name}" in line:
            break
        if not capture:
            continue
        match = re.match(r"\s*([0-9A-F]+)(?:-([0-9A-F]+))?(?:;|$)", line)
        if match:
            start = int(match.group(1), 16)
            end = int(match.group(2) or match.group(1), 16)
            result.append((start, end))
    if not result:
        raise RuntimeError(f"RFC 3454 table not found: {table_name}")
    return result


def compare_rfc_property(table: str, predicate_name: str, ranges: list[tuple[int, int]]) -> None:
    """Сверяет predicate Unicode 3.2 с RFC ranges на всех scalar values."""
    predicate = getattr(stringprep, predicate_name)
    range_iterator = iter(ranges)
    active_range = next(range_iterator, None)
    for point in range(0x110000):
        while active_range is not None:
            range_start, range_end = active_range
            if range_end >= point:
                break
            active_range = next(range_iterator, None)
        if active_range is None:
            expected = False
        else:
            range_start, range_end = active_range
            expected = range_start <= point <= range_end
        if bool(predicate(chr(point))) != expected:
            raise RuntimeError(f"RFC property mismatch: {table} U+{point:04X}")


def property_flags(point: int) -> int:
    """Собирает RFC-табличные свойства scalar без зависимости runtime."""
    character = chr(point)
    flags = 0
    for table_name, bit in PROPERTY_TABLES:
        if getattr(stringprep, table_name)(character):
            flags |= bit
    return flags


def property_ranges() -> list[tuple[int, int, int]]:
    """Сжимает одинаковые свойства в отсортированные включительные диапазоны."""
    result: list[tuple[int, int, int]] = []
    start = 0
    previous = 0
    current_flags = property_flags(0)
    for point in range(1, 0x110000):
        flags = property_flags(point)
        if flags == current_flags and flags != 0:
            previous = point
            continue
        if current_flags:
            result.append((start, previous, current_flags))
        start = point
        previous = point
        current_flags = flags
    if current_flags:
        result.append((start, previous, current_flags))
    return result


def nfkd_entries(ucd: unicodedata.UCD) -> list[tuple[int, tuple[int, ...]]]:
    """Сохраняет полную совместимую декомпозицию Unicode 3.2."""
    result = []
    for point in range(0x110000):
        if 0xD800 <= point <= 0xDFFF:
            continue
        character = chr(point)
        normalized = ucd.normalize("NFKD", character)
        if normalized != character:
            result.append((point, tuple(map(ord, normalized))))
    return result


def combining_ranges(ucd: unicodedata.UCD) -> list[tuple[int, int, int]]:
    """Сжимает canonical combining class по исходным Unicode 3.2 данным."""
    result: list[tuple[int, int, int]] = []
    start = 0
    previous = 0
    current = ucd.combining(chr(0))
    for point in range(1, 0x110000):
        value = 0 if 0xD800 <= point <= 0xDFFF else ucd.combining(chr(point))
        if value == current and value != 0:
            previous = point
            continue
        if current:
            result.append((start, previous, current))
        start = point
        previous = point
        current = value
    if current:
        result.append((start, previous, current))
    return result


def compositions(ucd: unicodedata.UCD) -> list[tuple[int, int, int]]:
    """Выбирает пары NFC-композиции, исключая compatibility mappings."""
    result = []
    for point in range(0x110000):
        if 0xD800 <= point <= 0xDFFF:
            continue
        character = chr(point)
        mapping = ucd.decomposition(character)
        if not mapping or mapping.startswith("<"):
            continue
        parts = tuple(int(part, 16) for part in mapping.split())
        if len(parts) != 2 or ucd.combining(chr(parts[0])) != 0:
            continue
        pair = "".join(map(chr, parts))
        if ucd.normalize("NFC", pair) == character:
            result.append((parts[0], parts[1], point))
    return sorted(result)


def rust_table(name: str, rows: list[tuple], kind: str) -> list[str]:
    """Форматирует одну ограниченную строками Rust shard таблицу."""
    lines = ["//! Табличные данные Unicode 3.2 для профиля SASLprep."]
    if kind == "ranges":
        lines.append("pub(super) const ENTRIES: &[(u32, u32, u16)] = &[")
        lines.extend(f"    (0x{a:06X}, 0x{b:06X}, {c})," for a, b, c in rows)
    elif kind == "classes":
        lines.append("pub(super) const ENTRIES: &[(u32, u32, u8)] = &[")
        lines.extend(f"    (0x{a:06X}, 0x{b:06X}, {c})," for a, b, c in rows)
    elif kind == "pairs":
        lines.append("pub(super) const ENTRIES: &[(u32, u32, u32)] = &[")
        lines.extend(f"    (0x{a:06X}, 0x{b:06X}, 0x{c:06X})," for a, b, c in rows)
    else:
        lines.append("pub(super) const ENTRIES: &[(u32, &[u32])] = &[")
        for point, mapping in rows:
            values = ", ".join(f"0x{value:06X}" for value in mapping)
            lines.append(f"    (0x{point:06X}, &[{values}]),")
    lines.append("];")
    return lines


def write_table(name: str, rows: list[tuple], kind: str) -> list[str]:
    """Пишет shards и возвращает их Rust module declarations."""
    modules = []
    for offset in range(0, len(rows), SHARD_ENTRIES):
        module = f"{name}_{len(modules):03d}"
        shard = list(islice(rows, offset, offset + SHARD_ENTRIES))
        lines = rust_table(module, shard, kind)
        target = OUTPUT / f"{module}.rs"
        target.write_text("\n".join(lines) + "\n", encoding="utf-8")
        subprocess.run(["rustfmt", "--edition", "2024", str(target)], check=True)
        if len(target.read_text(encoding="utf-8").splitlines()) > 300:
            raise RuntimeError(f"{target} exceeds the 300-line limit")
        modules.append(module)
    return modules


def main() -> None:
    """Проверяет Unicode 3.2 источник и строит все runtime shards."""
    verify_pinned_sources()
    ucd = unicodedata.ucd_3_2_0
    if ucd.unidata_version != "3.2.0":
        raise RuntimeError("Python Unicode 3.2 normalization tables are required")
    OUTPUT.mkdir(parents=True, exist_ok=True)
    for pattern in ("properties_*.rs", "nfkd_*.rs", "combining_*.rs", "composition_*.rs", "*_index.rs"):
        for stale_file in OUTPUT.glob(pattern):
            stale_file.unlink()
    tables = (
        ("properties", property_ranges(), "ranges"),
        ("nfkd", nfkd_entries(ucd), "mappings"),
        ("combining", combining_ranges(ucd), "classes"),
        ("composition", compositions(ucd), "pairs"),
    )
    index = ["//! Индекс Unicode 3.2 таблиц SASLprep."]
    rust_types = {
        "ranges": "(u32, u32, u16)",
        "classes": "(u32, u32, u8)",
        "pairs": "(u32, u32, u32)",
        "mappings": "(u32, &[u32])",
    }
    for name, rows, kind in tables:
        modules = write_table(name, rows, kind)
        group_lines = [f"//! Подмодули таблицы {name} профиля SASLprep."]
        group_lines.extend(f'#[path = "{module}.rs"]\nmod {module};' for module in modules)
        references = ", ".join(f"{module}::ENTRIES" for module in modules)
        group_lines.append(
            f"pub(super) const ENTRIES: &[&[{rust_types[kind]}]] = &[{references}];"
        )
        group_path = OUTPUT / f"{name}_index.rs"
        group_path.write_text("\n".join(group_lines) + "\n", encoding="utf-8")
        subprocess.run(["rustfmt", "--edition", "2024", str(group_path)], check=True)
        if len(group_path.read_text(encoding="utf-8").splitlines()) > 300:
            raise RuntimeError(f"{group_path} exceeds the 300-line limit")
        index.extend(
            (
                f'#[path = "{name}_index.rs"]',
                f"mod {name};",
                f"pub(super) const {name.upper()}: &[&[{rust_types[kind]}]] = {name}::ENTRIES;",
            )
        )
    index_path = OUTPUT / "data.rs"
    index_path.write_text("\n".join(index) + "\n", encoding="utf-8")
    subprocess.run(["rustfmt", "--edition", "2024", str(index_path)], check=True)
    if len(index_path.read_text(encoding="utf-8").splitlines()) > 300:
        raise RuntimeError("generated data index exceeds the 300-line limit")
    print(f"generated Unicode 3.2 tables in {OUTPUT}")


if __name__ == "__main__":
    main()
