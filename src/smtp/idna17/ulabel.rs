//! Закрытая проверка IDNA2008 U-label и кодирование в A-label.

/// Причина, по которой U-label нельзя преобразовать в DNS A-label.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UlabelError {
    /// Пустая строка не является доменной меткой.
    Empty,
    /// Вход превышает локальную границу контекстного сканера.
    InputTooLong,
    /// Вход уже содержит A-label prefix и требует отдельного lookup-процесса.
    AlreadyAscii,
    /// U-label не находится в Unicode NFC.
    NotNormalized,
    /// Метка начинается/заканчивается дефисом или использует зарезервированные дефисы.
    HyphenRestriction,
    /// Кодовая точка имеет свойство, отличное от PVALID.
    NonPValidCodePoint,
    /// Для кодовой точки требуется контекстное правило, которое ещё не реализовано.
    ContextRuleRequired,
    /// Контекстное правило известно, но условие для этой кодовой точки нарушено.
    InvalidContext,
    /// Метка нарушает один из шести условий Bidi Rule RFC 5893.
    BidiRule,
    /// Получившаяся ASCII-метка превышает DNS-предел в 63 октета.
    EncodedLabelTooLong,
    /// Низкоуровневый кодек Punycode не смог выполнить преобразование.
    Punycode,
}

/// Проверяет U-label только из PVALID-символов и возвращает IDNA ACE-форму.
///
/// Общий DNS-предел домена остаётся ответственностью вызывающего слоя.
pub fn to_alabel(label: &str) -> Result<String, UlabelError> {
    validate_input(label)?;
    let punycode = super::super::punycode::encode(label).map_err(|_| UlabelError::Punycode)?;
    let alabel = format!("xn--{punycode}");
    match alabel.len() <= 63 {
        true => Ok(alabel),
        false => Err(UlabelError::EncodedLabelTooLong),
    }
}

/// Проверяет свойства U-label и структурные ограничения перед кодированием.
fn validate_input(label: &str) -> Result<(), UlabelError> {
    match (label.is_empty(), label.len() > 1024, label.is_ascii()) {
        (true, _, _) => return Err(UlabelError::Empty),
        (false, true, _) => return Err(UlabelError::InputTooLong),
        (false, false, true) => return Err(UlabelError::AlreadyAscii),
        (false, false, false) => {}
    }
    match crate::unicode::nfc::normalize(label) == label {
        true => {}
        false => return Err(UlabelError::NotNormalized),
    }
    validate_hyphens(label)?;
    let characters = label.chars().collect::<Vec<_>>();
    let has_japanese_script = contains_japanese_script(&characters);
    let has_arabic_indic_digits = contains_arabic_indic_digits(&characters);
    let has_extended_arabic_digits = contains_extended_arabic_digits(&characters);
    for (position, character) in characters.iter().copied().enumerate() {
        match super::derived_property(character) {
            super::DerivedProperty::PValid => {}
            super::DerivedProperty::ContextJ | super::DerivedProperty::ContextO => {
                validate_contextual(
                    &characters,
                    position,
                    character,
                    has_japanese_script,
                    has_arabic_indic_digits,
                    has_extended_arabic_digits,
                )?;
            }
            super::DerivedProperty::DisallowedOrUnassigned => {
                return Err(UlabelError::NonPValidCodePoint);
            }
        }
    }
    match super::bidi::validate_label(&characters) {
        true => Ok(()),
        false => Err(UlabelError::BidiRule),
    }
}

/// Проверяет contextual code point по установленному RFC правилу.
fn validate_contextual(
    characters: &[char],
    position: usize,
    character: char,
    has_japanese_script: bool,
    has_arabic_indic_digits: bool,
    has_extended_arabic_digits: bool,
) -> Result<(), UlabelError> {
    match character {
        '\u{00b7}' if between_l(characters, position) => Ok(()),
        '\u{0375}'
            if followed_by_script(characters, position, super::script_property::Script::Greek) =>
        {
            Ok(())
        }
        '\u{05f3}' | '\u{05f4}'
            if preceded_by_script(characters, position, super::script_property::Script::Hebrew) =>
        {
            Ok(())
        }
        '\u{30fb}' if has_japanese_script => Ok(()),
        '\u{0660}'..='\u{0669}' if !has_extended_arabic_digits => Ok(()),
        '\u{06f0}'..='\u{06f9}' if !has_arabic_indic_digits => Ok(()),
        '\u{200c}' if valid_zwnj_context(characters, position) => Ok(()),
        '\u{200d}' if previous_is_virama(characters, position) => Ok(()),
        '\u{200c}' | '\u{200d}' => Err(UlabelError::InvalidContext),
        '\u{00b7}'
        | '\u{0375}'
        | '\u{05f3}'
        | '\u{05f4}'
        | '\u{30fb}'
        | '\u{0660}'..='\u{0669}'
        | '\u{06f0}'..='\u{06f9}' => Err(UlabelError::InvalidContext),
        _ => Err(UlabelError::ContextRuleRequired),
    }
}

/// Применяет правила U+200C: virama либо допустимая пара joining types.
fn valid_zwnj_context(characters: &[char], position: usize) -> bool {
    previous_is_virama(characters, position)
        || matches!(
            (
                joining_before(characters, position),
                joining_after(characters, position)
            ),
            (
                Some(
                    super::joining_property::JoiningType::Left
                        | super::joining_property::JoiningType::Dual
                ),
                Some(
                    super::joining_property::JoiningType::Right
                        | super::joining_property::JoiningType::Dual
                )
            )
        )
}

/// Проверяет непосредственно предшествующую code point на Unicode Virama.
fn previous_is_virama(characters: &[char], position: usize) -> bool {
    position
        .checked_sub(1)
        .and_then(|index| characters.get(index))
        .is_some_and(|character| super::virama::is_virama(*character))
}

/// Находит ближайший joining type слева, пропуская Transparent символы.
fn joining_before(
    characters: &[char],
    position: usize,
) -> Option<super::joining_property::JoiningType> {
    characters
        .iter()
        .take(position)
        .rev()
        .copied()
        .map(super::joining_property::joining_type)
        .find(|joining| *joining != super::joining_property::JoiningType::Transparent)
}

/// Находит ближайший joining type справа, пропуская Transparent символы.
fn joining_after(
    characters: &[char],
    position: usize,
) -> Option<super::joining_property::JoiningType> {
    characters
        .iter()
        .skip(position.saturating_add(1))
        .copied()
        .map(super::joining_property::joining_type)
        .find(|joining| *joining != super::joining_property::JoiningType::Transparent)
}

/// Проверяет, что U+00B7 расположен ровно между двумя латинскими `l`.
fn between_l(characters: &[char], position: usize) -> bool {
    matches!(
        (
            position
                .checked_sub(1)
                .and_then(|index| characters.get(index)),
            characters.get(position.saturating_add(1))
        ),
        (Some('l'), Some('l'))
    )
}

/// Проверяет Script следующего символа без обращения по индексу массива.
fn followed_by_script(
    characters: &[char],
    position: usize,
    expected: super::script_property::Script,
) -> bool {
    characters
        .get(position.saturating_add(1))
        .and_then(|character| super::script_property::script_of(*character))
        == Some(expected)
}

/// Проверяет Script предыдущего символа без обращения по индексу массива.
fn preceded_by_script(
    characters: &[char],
    position: usize,
    expected: super::script_property::Script,
) -> bool {
    position
        .checked_sub(1)
        .and_then(|index| characters.get(index))
        .and_then(|character| super::script_property::script_of(*character))
        == Some(expected)
}

/// Проверяет наличие Hiragana, Katakana или Han в метке для U+30FB.
fn contains_japanese_script(characters: &[char]) -> bool {
    characters.iter().copied().any(|character| {
        matches!(
            super::script_property::script_of(character),
            Some(
                super::script_property::Script::Hiragana
                    | super::script_property::Script::Katakana
                    | super::script_property::Script::Han
            )
        )
    })
}

/// Ищет в метке цифры арабского набора U+0660..U+0669.
fn contains_arabic_indic_digits(characters: &[char]) -> bool {
    characters
        .iter()
        .any(|character| matches!(character, '\u{0660}'..='\u{0669}'))
}

/// Ищет в метке цифры восточно-арабского набора U+06F0..U+06F9.
fn contains_extended_arabic_digits(characters: &[char]) -> bool {
    characters
        .iter()
        .any(|character| matches!(character, '\u{06f0}'..='\u{06f9}'))
}

/// Проверяет начальный/конечный дефис и зарезервированные позиции 3 и 4.
fn validate_hyphens(label: &str) -> Result<(), UlabelError> {
    let mut characters = label.chars();
    let first = characters.next().ok_or(UlabelError::Empty)?;
    let last = label.chars().next_back().ok_or(UlabelError::Empty)?;
    let third = characters.nth(1);
    let fourth = characters.next();
    let reserved = matches!((third, fourth), (Some('-'), Some('-')));
    match (first == '-', last == '-', reserved) {
        (false, false, false) => Ok(()),
        _ => Err(UlabelError::HyphenRestriction),
    }
}

#[cfg(test)]
#[path = "ulabel_tests.rs"]
mod tests;
