//! Удаление RFC 5322 comments при сохранении quoted strings и domain-literals.

/// Удаляет комментарии, одновременно проверяя содержимое domain-literal.
pub(super) fn strip_comments(value: &str) -> Result<String, &'static str> {
    let mut output = String::with_capacity(value.len());
    let mut quoted = false;
    let mut escaped = false;
    let mut domain_literal = false;
    let mut depth = 0_usize;
    for character in value.chars() {
        if escaped {
            if domain_literal {
                super::envelope_address_dtext::validate(character, true)?;
            }
            if depth == 0 {
                output.push(character);
            }
            escaped = false;
            continue;
        }
        if domain_literal {
            match character {
                '\\' => {
                    output.push(character);
                    escaped = true;
                }
                ']' => {
                    output.push(character);
                    domain_literal = false;
                }
                '[' => return Err("nested domain literal"),
                _ => {
                    super::envelope_address_dtext::validate(character, false)?;
                    output.push(character);
                }
            }
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
            ('[', false, 0) => {
                domain_literal = true;
                output.push(character);
            }
            (']', false, 0) => return Err("unmatched domain literal terminator"),
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
    match (quoted, escaped, depth, domain_literal) {
        (true, _, _, _) | (_, true, _, _) | (_, _, 1.., _) | (_, _, _, true) => {
            Err("unterminated address comment, quote, or domain literal")
        }
        (false, false, 0, false) => Ok(output),
    }
}
