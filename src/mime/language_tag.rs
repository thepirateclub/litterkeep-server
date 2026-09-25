//! Типизированный и ограниченный синтаксический профиль BCP 47 language tag.

#[cfg(test)]
#[path = "language_tag_tests.rs"]
mod tests;

/// Исходная запись синтаксически корректного BCP 47 tag.
///
/// Регистр и порядок subtags сохраняются, потому что MIME parameter передаёт
/// метаданные отображения; нормализация и IANA registry lookup здесь не нужны.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LanguageTag(String);

impl LanguageTag {
    /// Проверяет RFC 5646 ABNF и сохраняет исходное представление tag.
    pub fn parse(value: &str) -> Result<Self, &'static str> {
        validate(value)?;
        Ok(Self(value.to_owned()))
    }

    /// Возвращает исходное BCP 47 представление без каноникализации.
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// Разбирает normal, private-use и исторические grandfathered формы BCP 47.
fn validate(value: &str) -> Result<(), &'static str> {
    if value.is_empty() || !value.is_ascii() {
        return Err("empty or non-ASCII BCP 47 language tag");
    }
    if GrandfatheredTag::parse(value).is_some() {
        return Ok(());
    }
    let mut subtags = value.split('-').peekable();
    let Some(language) = subtags.next() else {
        return Err("missing BCP 47 language subtag");
    };
    if is_private(language) {
        return validate_private_use(&mut subtags);
    }
    if !is_language(language) {
        return Err("invalid BCP 47 primary language subtag");
    }
    consume_extlangs(&mut subtags, language);
    consume_optional(&mut subtags, is_script);
    consume_optional(&mut subtags, is_region);
    validate_variants(&mut subtags)?;
    validate_extensions(&mut subtags)?;
    match subtags.peek().copied() {
        Some(value) if is_private(value) => {
            subtags.next();
            validate_private_use(&mut subtags)?;
        }
        _ => {}
    }
    match subtags.next() {
        Some(_) => Err("unexpected BCP 47 language subtag"),
        None => Ok(()),
    }
}

/// Потребляет разрешённые 3-alpha extlang subtags только после короткого tag.
fn consume_extlangs<'a>(
    subtags: &mut std::iter::Peekable<std::str::Split<'a, char>>,
    language: &str,
) {
    if !(2..=3).contains(&language.len()) {
        return;
    }
    for _ in 0..3 {
        match subtags.peek().copied() {
            Some(value) if is_extlang(value) => {
                subtags.next();
            }
            _ => return,
        }
    }
}

/// Снимает необязательный script/region subtag, если он соответствует ABNF.
fn consume_optional<'a>(
    subtags: &mut std::iter::Peekable<std::str::Split<'a, char>>,
    predicate: fn(&str) -> bool,
) {
    match subtags.peek().copied() {
        Some(value) if predicate(value) => {
            subtags.next();
        }
        _ => {}
    }
}

/// Проверяет variant grammar и запрещает повтор одного variant без учёта регистра.
fn validate_variants<'a>(
    subtags: &mut std::iter::Peekable<std::str::Split<'a, char>>,
) -> Result<(), &'static str> {
    let mut variants = std::collections::HashSet::new();
    while let Some(value) = subtags.peek().copied().filter(|value| is_variant(value)) {
        let normalized = value.to_ascii_lowercase();
        if !variants.insert(normalized) {
            return Err("duplicate BCP 47 variant subtag");
        }
        subtags.next();
    }
    Ok(())
}

/// Проверяет extension sequences и не допускает дублирующий singleton.
fn validate_extensions<'a>(
    subtags: &mut std::iter::Peekable<std::str::Split<'a, char>>,
) -> Result<(), &'static str> {
    let mut singletons = std::collections::HashSet::new();
    while let Some(singleton) = subtags.peek().copied().filter(|value| is_singleton(value)) {
        if !singletons.insert(singleton.to_ascii_lowercase()) {
            return Err("duplicate BCP 47 extension singleton");
        }
        subtags.next();
        let mut extension_parts = 0_usize;
        while let Some(value) = subtags
            .peek()
            .copied()
            .filter(|value| is_extension_part(value))
        {
            let _ = value;
            subtags.next();
            extension_parts = extension_parts.saturating_add(1);
        }
        if extension_parts == 0 {
            return Err("empty BCP 47 extension sequence");
        }
    }
    Ok(())
}

/// Проверяет private-use subtags после зарезервированного singleton `x`.
fn validate_private_use<'a>(
    subtags: &mut std::iter::Peekable<std::str::Split<'a, char>>,
) -> Result<(), &'static str> {
    let mut count = 0_usize;
    for value in subtags.by_ref() {
        if !is_private_part(value) {
            return Err("invalid BCP 47 private-use subtag");
        }
        count = count.saturating_add(1);
    }
    match count {
        0 => Err("empty BCP 47 private-use sequence"),
        _ => Ok(()),
    }
}

/// Проверяет primary language grammar: 2-8 ASCII letters.
fn is_language(value: &str) -> bool {
    (2..=8).contains(&value.len()) && ascii_alpha(value)
}

/// Проверяет трёхбуквенный extlang subtag из RFC 5646.
fn is_extlang(value: &str) -> bool {
    value.len() == 3 && ascii_alpha(value)
}

/// Проверяет четырёхбуквенную script форму.
fn is_script(value: &str) -> bool {
    value.len() == 4 && ascii_alpha(value)
}

/// Проверяет регион как две буквы либо три десятичные цифры.
fn is_region(value: &str) -> bool {
    (value.len() == 2 && ascii_alpha(value)) || (value.len() == 3 && ascii_digit(value))
}

/// Проверяет вариант: 5-8 alphanum либо 4 знака с начальной цифрой.
fn is_variant(value: &str) -> bool {
    ((5..=8).contains(&value.len())
        || (value.len() == 4 && value.starts_with(|character: char| character.is_ascii_digit())))
        && ascii_alphanumeric(value)
}

/// Проверяет extension singleton, исключая зарезервированный private-use `x`.
fn is_singleton(value: &str) -> bool {
    value.len() == 1 && ascii_alphanumeric(value) && !is_private(value)
}

/// Проверяет payload одной extension sequence длиной 2-8 alphanum.
fn is_extension_part(value: &str) -> bool {
    (2..=8).contains(&value.len()) && ascii_alphanumeric(value)
}

/// Сравнивает зарезервированный private-use subtag без учёта ASCII-регистра.
fn is_private(value: &str) -> bool {
    value.eq_ignore_ascii_case("x")
}

/// Проверяет private-use subtag длиной от одного до восьми ASCII знаков.
fn is_private_part(value: &str) -> bool {
    (1..=8).contains(&value.len()) && ascii_alphanumeric(value)
}

/// Проверяет непустую последовательность только ASCII letters.
fn ascii_alpha(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_alphabetic())
}

/// Проверяет непустую последовательность только ASCII decimal digits.
fn ascii_digit(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit())
}

/// Проверяет непустую последовательность ASCII letters или digits.
fn ascii_alphanumeric(value: &str) -> bool {
    !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_alphanumeric())
}

/// Фиксированный набор grandfathered tags, заданный RFC 5646 §2.1.
#[derive(Clone, Copy)]
enum GrandfatheredTag {
    EnGbOed,
    IAmi,
    IBnn,
    IDefault,
    IEnochian,
    IHak,
    IKlingon,
    ILux,
    IMingo,
    INavajo,
    IPwn,
    ITao,
    ITay,
    ITsu,
    SgnBeFr,
    SgnBeNl,
    SgnChDe,
}

impl GrandfatheredTag {
    fn parse(value: &str) -> Option<Self> {
        match value.to_ascii_lowercase().as_str() {
            "en-gb-oed" => Some(Self::EnGbOed),
            "i-ami" => Some(Self::IAmi),
            "i-bnn" => Some(Self::IBnn),
            "i-default" => Some(Self::IDefault),
            "i-enochian" => Some(Self::IEnochian),
            "i-hak" => Some(Self::IHak),
            "i-klingon" => Some(Self::IKlingon),
            "i-lux" => Some(Self::ILux),
            "i-mingo" => Some(Self::IMingo),
            "i-navajo" => Some(Self::INavajo),
            "i-pwn" => Some(Self::IPwn),
            "i-tao" => Some(Self::ITao),
            "i-tay" => Some(Self::ITay),
            "i-tsu" => Some(Self::ITsu),
            "sgn-be-fr" => Some(Self::SgnBeFr),
            "sgn-be-nl" => Some(Self::SgnBeNl),
            "sgn-ch-de" => Some(Self::SgnChDe),
            _ => None,
        }
    }
}
