//! Декодирование IANA UTF-32 labels для MIME encoded-word и parameters.

#[cfg(test)]
#[path = "charset_utf32_tests.rs"]
mod tests;

/// Порядок байтов, выбранный MIME label или обязательным generic BOM.
#[derive(Clone, Copy)]
pub(super) enum ByteOrder {
    DetectBom,
    BigEndian,
    LittleEndian,
}

/// Преобразует UTF-32 units в Unicode scalars и отклоняет malformed input.
pub(super) fn decode(bytes: &[u8], requested_order: ByteOrder) -> Result<String, &'static str> {
    let (order, payload) = match (requested_order, bytes.get(..4)) {
        (ByteOrder::DetectBom, Some([0x00, 0x00, 0xfe, 0xff])) => (
            ByteOrder::BigEndian,
            bytes.get(4..).ok_or("invalid UTF-32 BOM")?,
        ),
        (ByteOrder::DetectBom, _) => (ByteOrder::BigEndian, bytes),
        (order, _) => (order, bytes),
    };
    if !payload.chunks_exact(4).remainder().is_empty() {
        return Err("UTF-32 octet count is not divisible by four");
    }
    payload
        .chunks_exact(4)
        .map(|unit| decode_unit(unit, order))
        .collect()
}

/// Проверяет границы Unicode scalar после сборки endian-specific unit.
fn decode_unit(unit: &[u8], order: ByteOrder) -> Result<char, &'static str> {
    let octets: [u8; 4] = unit.try_into().map_err(|_| "invalid UTF-32 unit")?;
    let scalar = match order {
        ByteOrder::DetectBom | ByteOrder::BigEndian => u32::from_be_bytes(octets),
        ByteOrder::LittleEndian => u32::from_le_bytes(octets),
    };
    char::from_u32(scalar).ok_or("invalid UTF-32 Unicode scalar")
}
