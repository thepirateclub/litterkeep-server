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

/// Удаляет семантически невидимые RFC 5322 comments, сохраняя quoted strings.
pub(super) fn strip_comments(value: &str) -> Result<String, &'static str> {
    let mut output = String::with_capacity(value.len());
    let mut quoted = false;
    let mut escaped = false;
    let mut depth = 0_usize;
    for character in value.chars() {
        if escaped {
            if depth == 0 {
                output.push(character);
            }
            escaped = false;
            continue;
        }
        match (character, quoted, depth) {
            ('\\', true, _) | ('\\', false, 1..) => {
                if depth == 0 {
                    output.push(character);
                }
                escaped = true;
            }
            ('"', false, 0) => {
                quoted = true;
                output.push(character);
            }
            ('"', true, 0) => {
                quoted = false;
                output.push(character);
            }
            ('(', false, 0) => {
                depth = 1;
                output.push(' ');
            }
            ('(', _, 1..) => depth = depth.saturating_add(1),
            (')', _, 1) => depth = 0,
            (')', _, 2..) => depth = depth.saturating_sub(1),
            (_, _, 1..) => {}
            (_, _, 0) => output.push(character),
        }
    }
    match (quoted, escaped, depth) {
        (true, _, _) | (_, true, _) | (_, _, 1..) => Err("unterminated address comment or quote"),
        (false, false, 0) => Ok(output),
    }
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
    match (delimiter, right.trim().is_empty()) {
        (';', false) => Err("content follows address group terminator"),
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
    match (character, state.quoted, state.comments, state.angle) {
        ('\\', true, _, _) | ('\\', _, 1.., _) => state.escaped = true,
        ('"', false, 0, _) => state.quoted = true,
        ('"', true, 0, _) => state.quoted = false,
        ('(', false, 0, _) => state.comments = 1,
        ('(', _, 1.., _) => state.comments = state.comments.saturating_add(1),
        (')', _, 1, _) => state.comments = 0,
        (')', _, 2.., _) => state.comments = state.comments.saturating_sub(1),
        (')', false, 0, _) => return Err("unmatched address comment"),
        ('<', false, 0, false) => state.angle = true,
        ('<', false, 0, true) => return Err("nested angle address"),
        ('>', false, 0, true) => state.angle = false,
        ('>', false, 0, false) => return Err("unmatched angle address"),
        ('[', false, 0, _) if !state.angle => state.domain_literal = true,
        (']', false, 0, _) if state.domain_literal => state.domain_literal = false,
        _ => {}
    }
    Ok(state)
}
