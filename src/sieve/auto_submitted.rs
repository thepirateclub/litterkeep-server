//! Разбор RFC 3834 Auto-Submitted для защиты Sieve vacation от mail loops.

/// Семантический класс keyword после проверки RFC 3834 syntax.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum Keyword {
    /// Источник явно указывает на ручное сообщение.
    No,
    /// Сообщение создано автоматическим процессом или автоответчиком.
    Automated,
    /// Корректный extension keyword, семантика которого неизвестна.
    Extension,
    /// Невалидное значение, которое policy обрабатывает fail-closed.
    Invalid,
}

/// Разбирает keyword, CFWS comments и необязательные RFC 2045 parameters.
pub(crate) fn parse(value: &str) -> Keyword {
    let Ok(without_comments) = crate::mime::structured_comments::remove(value) else {
        return Keyword::Invalid;
    };
    let normalized = crate::mime::wsp::trim(&without_comments);
    let boundary = normalized
        .find([';', ' ', '\t'])
        .unwrap_or(normalized.len());
    let Some(keyword) = normalized.get(..boundary) else {
        return Keyword::Invalid;
    };
    if !valid_token(keyword) {
        return Keyword::Invalid;
    }
    let Some(parameters) = normalized.get(boundary..) else {
        return Keyword::Invalid;
    };
    match valid_parameter_list(parameters) {
        false => Keyword::Invalid,
        true => classify(keyword),
    }
}

fn classify(keyword: &str) -> Keyword {
    match keyword.to_ascii_lowercase().as_str() {
        "no" => Keyword::No,
        "auto-generated" | "auto-replied" => Keyword::Automated,
        _ => Keyword::Extension,
    }
}

fn valid_parameter_list(mut value: &str) -> bool {
    value = crate::mime::wsp::trim(value);
    loop {
        if value.is_empty() {
            return true;
        }
        let Some(after_separator) = value.strip_prefix(';') else {
            return false;
        };
        let after_separator = crate::mime::wsp::trim(after_separator);
        let Some((parameter, remainder)) = take_parameter(after_separator) else {
            return false;
        };
        if !valid_parameter(parameter) {
            return false;
        }
        value = crate::mime::wsp::trim(remainder);
    }
}

fn take_parameter(value: &str) -> Option<(&str, &str)> {
    let mut quoted = false;
    let mut escaped = false;
    for (position, character) in value.char_indices() {
        match (quoted, escaped, character) {
            (true, true, _) => escaped = false,
            (true, false, '\\') => escaped = true,
            (true, false, '"') => quoted = false,
            (false, false, '"') => quoted = true,
            (false, false, ';') => {
                let parameter = value.get(..position)?;
                let next = position.saturating_add(character.len_utf8());
                let remainder = value.get(next..)?;
                return Some((parameter, remainder));
            }
            _ => {}
        }
    }
    match (quoted, escaped) {
        (true, _) | (_, true) => None,
        (false, false) => Some((value, "")),
    }
}

fn valid_parameter(value: &str) -> bool {
    let Some((attribute, parameter_value)) = value.split_once('=') else {
        return false;
    };
    valid_token(attribute) && valid_parameter_value(parameter_value)
}

fn valid_parameter_value(value: &str) -> bool {
    match value
        .strip_prefix('"')
        .and_then(|tail| tail.strip_suffix('"'))
    {
        Some(quoted) => valid_quoted_value(quoted),
        None => valid_token(value),
    }
}

fn valid_quoted_value(value: &str) -> bool {
    let mut bytes = value.bytes();
    while let Some(byte) = bytes.next() {
        match byte {
            b'\\' => match bytes.next() {
                Some(quoted) if valid_quoted_octet(quoted) => {}
                _ => return false,
            },
            b'"' => return false,
            byte if valid_quoted_octet(byte) => {}
            _ => return false,
        }
    }
    true
}

fn valid_quoted_octet(byte: u8) -> bool {
    matches!(byte, b'\t' | 32..=126)
}

/// Проверяет RFC 2045 token: printable ASCII без tspecials.
fn valid_token(value: &str) -> bool {
    !value.is_empty()
        && value.bytes().all(|byte| {
            (33..=126).contains(&byte)
                && !matches!(
                    byte,
                    b'(' | b')'
                        | b'<'
                        | b'>'
                        | b'@'
                        | b','
                        | b';'
                        | b':'
                        | b'\\'
                        | b'"'
                        | b'/'
                        | b'['
                        | b']'
                        | b'?'
                        | b'='
                )
        })
}
