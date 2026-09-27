//! Закрытая проверка IDNA2008 U-label и кодирование в A-label.

/// Причина, по которой U-label нельзя преобразовать в DNS A-label.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum UlabelError {
    /// Пустая строка не является доменной меткой.
    Empty,
    /// Вход уже содержит A-label prefix и требует отдельного lookup-процесса.
    AlreadyAscii,
    /// U-label не находится в Unicode NFC.
    NotNormalized,
    /// Метка начинается/заканчивается дефисом или использует зарезервированные дефисы.
    HyphenRestriction,
    /// Кодовая точка имеет свойство, отличное от PVALID.
    NonPValidCodePoint,
    /// Кодовая точка требует контекстной проверки, отсутствующей на этом этапе.
    ContextRuleRequired,
    /// Получившаяся ASCII-метка превышает DNS-предел в 63 октета.
    EncodedLabelTooLong,
    /// Низкоуровневый кодек Punycode не смог выполнить преобразование.
    Punycode,
}

/// Проверяет U-label только из PVALID-символов и возвращает IDNA ACE-форму.
///
/// Контекстные правила IDNA и общий DNS-предел домена остаются ответственностью
/// вызывающего слоя. CONTEXTJ/CONTEXTO отклоняются до реализации их правил.
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
    match (label.is_empty(), label.is_ascii()) {
        (true, _) => return Err(UlabelError::Empty),
        (false, true) => return Err(UlabelError::AlreadyAscii),
        (false, false) => {}
    }
    match crate::unicode::nfc::normalize(label) == label {
        true => {}
        false => return Err(UlabelError::NotNormalized),
    }
    validate_hyphens(label)?;
    for character in label.chars() {
        match super::derived_property(character) {
            super::DerivedProperty::PValid => {}
            super::DerivedProperty::ContextJ | super::DerivedProperty::ContextO => {
                return Err(UlabelError::ContextRuleRequired);
            }
            super::DerivedProperty::DisallowedOrUnassigned => {
                return Err(UlabelError::NonPValidCodePoint);
            }
        }
    }
    Ok(())
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
