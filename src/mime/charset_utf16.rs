//! Декодирование UTF-16 и проверка порядка байтов MIME charset.

/// Порядок байтов, определяемый MIME label или начальной сигнатурой.
#[derive(Clone, Copy)]
pub(super) enum ByteOrder {
    DetectBom,
    BigEndian,
    LittleEndian,
}

/// Декодирует UTF-16 с проверкой surrogate pairs, длины и запрещённого U+FFFE.
pub(super) fn decode(bytes: &[u8], requested_order: ByteOrder) -> Result<String, &'static str> {
    let (order, payload) = match (requested_order, bytes.get(..2)) {
        (ByteOrder::DetectBom, None) => {
            return Err("UTF-16 requires two octets to determine byte order");
        }
        (ByteOrder::DetectBom, Some([0xfe, 0xff])) => (
            ByteOrder::BigEndian,
            bytes.get(2..).ok_or("invalid UTF-16 BOM")?,
        ),
        (ByteOrder::DetectBom, Some([0xff, 0xfe])) => (
            ByteOrder::LittleEndian,
            bytes.get(2..).ok_or("invalid UTF-16 BOM")?,
        ),
        (ByteOrder::DetectBom, _) => (ByteOrder::BigEndian, bytes),
        (order, _) => (order, bytes),
    };
    if !payload.chunks_exact(2).remainder().is_empty() {
        return Err("odd UTF-16 octet count");
    }
    let units = payload
        .chunks_exact(2)
        .map(|pair| utf16_unit(pair, order))
        .collect::<Result<Vec<_>, _>>()?;
    let mut output = String::with_capacity(payload.len());
    for decoded in char::decode_utf16(units) {
        output.push(decoded.map_err(|_| "invalid UTF-16 surrogate pair")?);
    }
    Ok(output)
}

/// Собирает один unit из пары октетов без прямой индексации массива.
fn utf16_unit(pair: &[u8], order: ByteOrder) -> Result<u16, &'static str> {
    let octets: [u8; 2] = pair.try_into().map_err(|_| "invalid UTF-16 pair")?;
    let value = match order {
        ByteOrder::DetectBom | ByteOrder::BigEndian => u16::from_be_bytes(octets),
        ByteOrder::LittleEndian => u16::from_le_bytes(octets),
    };
    match value {
        0xfffe => Err("UTF-16 contains the invalid U+FFFE value"),
        _ => Ok(value),
    }
}
