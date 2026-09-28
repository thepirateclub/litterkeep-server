//! Строгий ISO-2022-JP decoder по WHATWG §12.2 и RFC 1468.

/// Набор режимов, назначаемых ISO-2022-JP escape sequences.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Mode {
    Ascii,
    Roman,
    Katakana,
    Jis0208,
}

/// Явные промежуточные состояния escape и JIS-пары исключают догадки о границах.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum State {
    Mode(Mode, bool),
    JisTrailing(u8),
    EscapeStart { emitted: bool },
    Escape { emitted: bool, lead: u8 },
}

/// Декодирует 7-bit ISO-2022-JP, прерываясь на первом state-machine error.
pub(super) fn decode(bytes: &[u8]) -> Result<String, &'static str> {
    let mut output = String::with_capacity(bytes.len());
    let mut state = State::Mode(Mode::Ascii, false);
    for byte in bytes.iter().copied() {
        state = match state {
            State::Mode(Mode::Ascii, emitted) => decode_ascii(byte, emitted, &mut output)?,
            State::Mode(Mode::Roman, emitted) => decode_roman(byte, emitted, &mut output)?,
            State::Mode(Mode::Katakana, emitted) => decode_katakana(byte, emitted, &mut output)?,
            State::Mode(Mode::Jis0208, _) => decode_jis(byte)?,
            State::JisTrailing(lead) => decode_jis_trail(lead, byte, &mut output)?,
            State::EscapeStart { emitted } => decode_escape_start(emitted, byte)?,
            State::Escape { emitted, lead } => decode_escape(emitted, lead, byte)?,
        };
    }
    finish(state)?;
    Ok(output)
}

/// Обрабатывает ASCII state и запрещённые управляющие переключатели.
fn decode_ascii(byte: u8, emitted: bool, output: &mut String) -> Result<State, &'static str> {
    match byte {
        0x1b => Ok(State::EscapeStart { emitted }),
        0x00..=0x0d | 0x10..=0x7f => push(char::from(byte), output, Mode::Ascii),
        _ => Err("forbidden control octet in ISO-2022-JP ASCII mode"),
    }
}

/// Обрабатывает Roman state с yen/overline соответствиями.
fn decode_roman(byte: u8, emitted: bool, output: &mut String) -> Result<State, &'static str> {
    match byte {
        0x1b => Ok(State::EscapeStart { emitted }),
        0x5c => push('¥', output, Mode::Roman),
        0x7e => push('‾', output, Mode::Roman),
        0x00..=0x0d | 0x10..=0x5b | 0x5d..=0x7d | 0x7f => {
            push(char::from(byte), output, Mode::Roman)
        }
        _ => Err("forbidden control octet in ISO-2022-JP Roman mode"),
    }
}

/// Обрабатывает однобайтовый Katakana state.
fn decode_katakana(byte: u8, emitted: bool, output: &mut String) -> Result<State, &'static str> {
    match byte {
        0x1b => Ok(State::EscapeStart { emitted }),
        0x21..=0x5f => {
            let scalar = char::from_u32(0xff61 + u32::from(byte - 0x21))
                .ok_or("invalid ISO-2022-JP katakana scalar")?;
            push(scalar, output, Mode::Katakana)
        }
        _ => Err("invalid ISO-2022-JP katakana octet"),
    }
}

/// Принимает только JIS X 0208 leading byte либо начало escape.
fn decode_jis(byte: u8) -> Result<State, &'static str> {
    match byte {
        0x1b => Ok(State::EscapeStart { emitted: false }),
        0x21..=0x7e => Ok(State::JisTrailing(byte)),
        _ => Err("invalid ISO-2022-JP JIS leading octet"),
    }
}

/// Преобразует JIS X 0208 pair через таблицу, общую с EUC-JP.
fn decode_jis_trail(lead: u8, trail: u8, output: &mut String) -> Result<State, &'static str> {
    match trail {
        0x21..=0x7e => {
            let pointer = usize::from(lead - 0x21) * 94 + usize::from(trail - 0x21);
            let character = super::charset_euc_jp_table::jis0208(pointer)
                .ok_or("unmapped ISO-2022-JP JIS 0208 pair")?;
            push(character, output, Mode::Jis0208)
        }
        _ => Err("invalid ISO-2022-JP JIS trailing octet"),
    }
}

/// Начинает только зарегистрированные `ESC $` или `ESC (` designation forms.
fn decode_escape_start(emitted: bool, byte: u8) -> Result<State, &'static str> {
    match byte {
        0x24 | 0x28 => Ok(State::Escape {
            emitted,
            lead: byte,
        }),
        _ => Err("invalid ISO-2022-JP escape prefix"),
    }
}

/// Применяет четыре WHATWG designations и не допускает пустых повторных shifts.
fn decode_escape(emitted: bool, lead: u8, byte: u8) -> Result<State, &'static str> {
    let mode = match (lead, byte) {
        (0x28, 0x42) => Mode::Ascii,
        (0x28, 0x4a) => Mode::Roman,
        (0x28, 0x49) => Mode::Katakana,
        (0x24, 0x40 | 0x42) => Mode::Jis0208,
        _ => return Err("unsupported ISO-2022-JP escape designation"),
    };
    match emitted {
        true => Err("redundant ISO-2022-JP shift after emitted data"),
        false => Ok(State::Mode(mode, true)),
    }
}

/// Дописывает scalar и фиксирует, что он стал последним выводом активного режима.
fn push(character: char, output: &mut String, mode: Mode) -> Result<State, &'static str> {
    output.push(character);
    Ok(State::Mode(mode, false))
}

/// Отклоняет незакрытые формы и завершение вне ASCII, обязательное для почты.
fn finish(state: State) -> Result<(), &'static str> {
    match state {
        State::Mode(Mode::Ascii, _) => Ok(()),
        State::Mode(_, _) => Err("ISO-2022-JP message must end in ASCII mode"),
        State::JisTrailing(_) => Err("incomplete ISO-2022-JP JIS pair"),
        State::EscapeStart { .. } | State::Escape { .. } => Err("incomplete ISO-2022-JP escape"),
    }
}

#[cfg(test)]
#[path = "charset_iso2022_jp_tests.rs"]
mod tests;
