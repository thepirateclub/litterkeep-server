//! Безопасное выделение RFC 5322-разделителей вне строковых конструкций.

/// Разделяет список по запятым, не затрагивая quoted-строки и комментарии.
pub(super) fn split_commas(value: &str) -> Result<Vec<&str>, &'static str> {
    let mut result = Vec::new();
    let mut start = 0_usize;
    scan(value, |position, character, state| {
        if let (',', true) = (character, state.top_level()) {
            result.push(
                value
                    .get(start..position)
                    .ok_or("invalid address boundary")?,
            );
            start = position.saturating_add(1);
        }
        Ok(())
    })?;
    result.push(value.get(start..).ok_or("invalid address tail")?);
    Ok(result)
}

/// Находит один group separator, если он расположен вне addr-spec и CFWS.
pub(super) fn split_optional(
    value: &str,
    delimiter: char,
) -> Result<(&str, Option<&str>), &'static str> {
    let mut found = None;
    scan(value, |position, character, state| {
        match (character == delimiter, state.top_level(), found) {
            (true, true, None) => found = Some(position),
            (true, true, Some(_)) => return Err("repeated address group delimiter"),
            _ => {}
        }
        Ok(())
    })?;
    match found {
        Some(position) => split_at(value, position, delimiter),
        None => Ok((value, None)),
    }
}

/// Находит разделитель addr-spec вне quoted local-part и domain-literal.
pub(super) fn split_addr_spec(value: &str) -> Result<(&str, &str), &'static str> {
    let mut separator = None;
    scan(value, |position, character, state| {
        match (character == '@', state.top_level(), separator) {
            (true, true, None) => separator = Some(position),
            (true, true, Some(_)) => return Err("repeated addr-spec separator"),
            _ => {}
        }
        Ok(())
    })?;
    match separator {
        Some(position) => {
            let local = value.get(..position).ok_or("invalid local-part boundary")?;
            let domain = value
                .get(position.saturating_add(1)..)
                .ok_or("invalid domain boundary")?;
            Ok((local, domain))
        }
        None => Err("address has no domain separator"),
    }
}

/// Убирает CFWS вокруг точек в obsolete dot-separated local-part/domain.
pub(super) fn normalize_dot_cfws(value: &str) -> String {
    let mut characters = value.chars().peekable();
    let mut normalized = String::with_capacity(value.len());
    let mut quoted = false;
    let mut escaped = false;
    let mut domain_literal = false;
    let mut previous_dot = false;
    while let Some(character) = characters.next() {
        match (quoted, escaped, domain_literal, character) {
            (true, true, _, value) => {
                normalized.push(value);
                escaped = false;
            }
            (false, true, true, value) => {
                normalized.push(value);
                escaped = false;
            }
            (true, false, _, '\\') => {
                normalized.push(character);
                escaped = true;
            }
            (true, false, _, '"') => {
                normalized.push(character);
                quoted = false;
            }
            (true, false, _, _) => normalized.push(character),
            (false, false, true, '\\') => {
                escaped = true;
            }
            (false, _, true, ']') => {
                normalized.push(character);
                domain_literal = false;
            }
            (false, _, true, _) => normalized.push(character),
            (false, _, false, '"') => {
                normalized.push(character);
                quoted = true;
                previous_dot = false;
            }
            (false, _, false, '[') => {
                normalized.push(character);
                domain_literal = true;
                previous_dot = false;
            }
            (false, _, false, ' ' | '\t') => {
                while let Some(next) = characters.peek().copied() {
                    match next {
                        ' ' | '\t' => {
                            characters.next();
                        }
                        _ => break,
                    }
                }
                let before_dot = matches!(characters.peek(), Some('.'));
                match (previous_dot, before_dot) {
                    (true, _) | (_, true) => {}
                    (false, false) => normalized.push(' '),
                }
            }
            (false, _, false, '.') => {
                normalized.push(character);
                previous_dot = true;
            }
            (false, _, false, _) => {
                normalized.push(character);
                previous_dot = false;
            }
        }
    }
    normalized
}

fn split_at(
    value: &str,
    position: usize,
    delimiter: char,
) -> Result<(&str, Option<&str>), &'static str> {
    let left = value.get(..position).ok_or("invalid address prefix")?;
    let right = value
        .get(position.saturating_add(1)..)
        .ok_or("invalid address suffix")?;
    match delimiter {
        ';' => match super::envelope_address_comments::strip_comments(right)?
            .trim()
            .is_empty()
        {
            true => Ok((left, Some(right))),
            false => Err("content follows address group terminator"),
        },
        _ => Ok((left, Some(right))),
    }
}

#[derive(Clone, Copy, Default)]
struct ScanState {
    quoted: bool,
    escaped: bool,
    angle: bool,
    domain_literal: bool,
    comments: usize,
}

impl ScanState {
    fn top_level(self) -> bool {
        matches!(
            (self.quoted, self.angle, self.domain_literal, self.comments),
            (false, false, false, 0)
        )
    }
}

fn scan(
    value: &str,
    mut visit: impl FnMut(usize, char, ScanState) -> Result<(), &'static str>,
) -> Result<(), &'static str> {
    let mut state = ScanState::default();
    for (position, character) in value.char_indices() {
        visit(position, character, state)?;
        state = advance(state, character)?;
    }
    match (
        state.quoted,
        state.angle,
        state.domain_literal,
        state.comments,
    ) {
        (true, _, _, _) | (_, true, _, _) | (_, _, true, _) | (_, _, _, 1..) => {
            Err("unterminated address syntax")
        }
        _ => Ok(()),
    }
}

fn advance(mut state: ScanState, character: char) -> Result<ScanState, &'static str> {
    if state.escaped {
        state.escaped = false;
        return Ok(state);
    }
    if state.domain_literal {
        // В domain-literal круглые скобки являются dtext, а obsolete quoted-pair защищает следующий знак.
        match character {
            '\\' => state.escaped = true,
            ']' => state.domain_literal = false,
            '[' => return Err("nested domain literal"),
            _ => {}
        }
        return Ok(state);
    }
    match (character, state.quoted, state.comments, state.angle) {
        ('\\', true, _, _) | ('\\', _, 1.., _) => state.escaped = true,
        ('"', false, 0, _) => state.quoted = true,
        ('"', true, 0, _) => state.quoted = false,
        ('(', false, 0, _) => state.comments = 1,
        ('(', _, 1.., _) => state.comments = state.comments.saturating_add(1),
        (')', _, 1, _) => state.comments = 0,
        (')', _, 2.., _) => state.comments = state.comments.saturating_sub(1),
        (')', false, 0, _) => return Err("unmatched address comment"),
        ('<', false, 0, false) if !state.domain_literal => state.angle = true,
        ('<', false, 0, true) if !state.domain_literal => {
            return Err("nested angle address");
        }
        ('>', false, 0, true) if !state.domain_literal => state.angle = false,
        ('>', false, 0, false) if !state.domain_literal => {
            return Err("unmatched angle address");
        }
        ('[', false, 0, _) if !state.domain_literal => state.domain_literal = true,
        (']', false, 0, _) if state.domain_literal => state.domain_literal = false,
        (']', false, 0, _) => return Err("unmatched domain literal terminator"),
        _ => {}
    }
    Ok(state)
}
