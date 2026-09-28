//! Строгое Big5 MIME декодирование по WHATWG Encoding Standard §11.1.1.

/// Четыре Big5 pointer-и, которым стандарт сопоставляет пару scalars.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Big5Expansion {
    CapitalECircumflexMacron,
    CapitalECircumflexCaron,
    SmallECircumflexMacron,
    SmallECircumflexCaron,
}

/// Декодирует ASCII и Big5 пары, прекращая работу на первом недопустимом байте.
pub(super) fn decode(bytes: &[u8]) -> Result<String, &'static str> {
    let mut input = bytes.iter().copied();
    let mut output = String::with_capacity(bytes.len());
    while let Some(byte) = input.next() {
        match byte {
            0x00..=0x7f => output.push(char::from(byte)),
            0x81..=0xfe => decode_pair(byte, &mut input, &mut output)?,
            _ => return Err("invalid Big5 leading octet"),
        }
    }
    Ok(output)
}

/// Читает trail byte, распознаёт expansion exceptions и lookup обычных pointers.
fn decode_pair<I>(lead: u8, input: &mut I, output: &mut String) -> Result<(), &'static str>
where
    I: Iterator<Item = u8>,
{
    let trail = input.next().ok_or("incomplete Big5 sequence")?;
    let pointer = pointer(lead, trail).ok_or("invalid Big5 trailing octet")?;
    match expansion(pointer) {
        Some(exception) => push_expansion(exception, output),
        None => push_index_scalar(pointer, output)?,
    }
    Ok(())
}

/// Применяет Big5 byte-to-pointer формулу для двух допустимых trail ranges.
fn pointer(lead: u8, trail: u8) -> Option<usize> {
    let offset = match trail {
        0x40..=0x7e => 0x40,
        0xa1..=0xfe => 0x62,
        _ => return None,
    };
    Some(usize::from(lead - 0x81) * 157 + usize::from(trail - offset))
}

/// Возвращает типизированное исключение с двухсимвольным mapping.
fn expansion(pointer: usize) -> Option<Big5Expansion> {
    match pointer {
        1_133 => Some(Big5Expansion::CapitalECircumflexMacron),
        1_135 => Some(Big5Expansion::CapitalECircumflexCaron),
        1_164 => Some(Big5Expansion::SmallECircumflexMacron),
        1_166 => Some(Big5Expansion::SmallECircumflexCaron),
        _ => None,
    }
}

/// Добавляет оба нормативных Unicode scalars для специального Big5 pointer-а.
fn push_expansion(expansion: Big5Expansion, output: &mut String) {
    let (base, combining) = match expansion {
        Big5Expansion::CapitalECircumflexMacron => ('Ê', '\u{0304}'),
        Big5Expansion::CapitalECircumflexCaron => ('Ê', '\u{030c}'),
        Big5Expansion::SmallECircumflexMacron => ('ê', '\u{0304}'),
        Big5Expansion::SmallECircumflexCaron => ('ê', '\u{030c}'),
    };
    output.push(base);
    output.push(combining);
}

/// Добавляет найденный index scalar или ошибку для незаданного pointer-а.
fn push_index_scalar(pointer: usize, output: &mut String) -> Result<(), &'static str> {
    let character =
        super::charset_big5_table::code_point(pointer).ok_or("unmapped Big5 two-byte sequence")?;
    output.push(character);
    Ok(())
}

#[cfg(test)]
#[path = "charset_big5_tests.rs"]
mod tests;
