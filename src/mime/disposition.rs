//! Типизированная модель MIME Content-Disposition для BODYSTRUCTURE.

use super::language_tag::LanguageTag;
use super::parameters;
use std::collections::BTreeMap;

/// Известное назначение entity либо совместимый extension disposition token.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DispositionKind {
    Inline,
    Attachment,
    FormData,
    Extension(String),
}

impl DispositionKind {
    /// Разбирает регистронезависимый RFC 2183 token в доменный вариант.
    fn parse(value: &str) -> Result<Self, &'static str> {
        let normalized = value.to_ascii_lowercase();
        parameters::parse(&format!("{normalized}/x"))?;
        Ok(match normalized.as_str() {
            "inline" => Self::Inline,
            "attachment" => Self::Attachment,
            "form-data" => Self::FormData,
            _ => Self::Attachment,
        })
    }

    /// Возвращает безопасное ASCII-представление disposition token.
    pub fn as_str(&self) -> &str {
        match self {
            Self::Inline => "inline",
            Self::Attachment => "attachment",
            Self::FormData => "form-data",
            Self::Extension(value) => value,
        }
    }
}

/// Разобранный disposition и уже объединённые RFC 2231 параметры.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Disposition {
    pub kind: DispositionKind,
    pub parameters: BTreeMap<String, String>,
    /// Языковая подсказка RFC 2231 остаётся при декодированном MIME-параметре.
    pub parameter_languages: BTreeMap<String, LanguageTag>,
}

/// Временный набор параметров перед построением публичного disposition.
struct DispositionParameters {
    values: BTreeMap<String, String>,
    languages: BTreeMap<String, LanguageTag>,
}

/// Один успешно распарсенный MIME parameter вместе с optional языковой меткой.
struct ParsedParameter {
    name: String,
    value: String,
    language: Option<LanguageTag>,
}

/// Имена параметров RFC 2183 с отдельными синтаксическими ограничениями.
enum DispositionParameterName {
    Size,
    CreationDate,
    ModificationDate,
    ReadDate,
    Other,
}

impl DispositionParameterName {
    /// Преобразует нормализованное имя общего MIME parser-а в типизированный вариант.
    fn parse(name: &str) -> Self {
        match name {
            "size" => Self::Size,
            "creation-date" => Self::CreationDate,
            "modification-date" => Self::ModificationDate,
            "read-date" => Self::ReadDate,
            _ => Self::Other,
        }
    }
}

/// Разбирает одно поле Content-Disposition, сохраняя расширения RFC 2231.
pub fn parse(value: &str) -> Result<Disposition, &'static str> {
    let normalized = super::structured_comments::remove(value)?;
    let mut sections = normalized.splitn(2, ';');
    let raw_kind = sections.next().ok_or("missing MIME disposition token")?;
    let kind = DispositionKind::parse(super::wsp::trim(raw_kind))?;
    let parsed = match sections.next() {
        Some(tail) => parse_parameters_ignoring_unrecognized(tail)?,
        None => DispositionParameters {
            values: BTreeMap::new(),
            languages: BTreeMap::new(),
        },
    };
    Ok(Disposition {
        kind,
        parameters: parsed.values,
        parameter_languages: parsed.languages,
    })
}

/// Игнорирует отдельные неизвестные disposition-параметры по RFC 2183 §2.8.
fn parse_parameters_ignoring_unrecognized(
    tail: &str,
) -> Result<DispositionParameters, &'static str> {
    merge_parameters(collect_parameters(tail)?)
}

/// Объединяет параметры после раздельной обработки их ошибок.
fn merge_parameters(parsed: Vec<ParsedParameter>) -> Result<DispositionParameters, &'static str> {
    let mut grouped: BTreeMap<String, Vec<ParsedParameter>> = BTreeMap::new();
    for parameter in parsed {
        grouped
            .entry(parameter.name.clone())
            .or_default()
            .push(parameter);
    }
    let mut values = BTreeMap::new();
    let mut languages = BTreeMap::new();
    for (name, entries) in grouped {
        if let [parameter] = entries.as_slice() {
            values.insert(name.clone(), parameter.value.clone());
            if let Some(language) = &parameter.language {
                languages.insert(name, language.clone());
            }
        }
    }
    Ok(DispositionParameters { values, languages })
}

/// Парсит одиночное валидное значение через общий RFC 2231 decoder.
fn parse_one_parameter(parameter: &str) -> Option<ParsedParameter> {
    let parsed = parameters::parse(&format!("application/x;{parameter}")).ok()?;
    let (name, value) = parsed.parameters.into_iter().next()?;
    if !valid_parameter_value(&name, &value, parameter) {
        return None;
    }
    let language = parsed.parameter_languages.get(&name).cloned();
    Some(ParsedParameter {
        name,
        value,
        language,
    })
}

/// Проверяет локальные ограничения параметра сверх общей MIME-грамматики.
fn valid_parameter_value(name: &str, value: &str, wire_parameter: &str) -> bool {
    match DispositionParameterName::parse(name) {
        DispositionParameterName::Size => valid_size_parameter(value, wire_parameter),
        DispositionParameterName::CreationDate
        | DispositionParameterName::ModificationDate
        | DispositionParameterName::ReadDate => valid_date_parameter(value, wire_parameter),
        DispositionParameterName::Other => true,
    }
}

/// Принимает quoted RFC date-time с числовой зоной и корректной календарной датой.
fn valid_date_parameter(value: &str, wire_parameter: &str) -> bool {
    let raw_value = wire_parameter
        .split_once('=')
        .map(|(_, raw_value)| super::wsp::trim(raw_value));
    let quoted_value = raw_value
        .and_then(|raw| raw.strip_prefix('"'))
        .and_then(|raw| raw.strip_suffix('"'));
    match quoted_value {
        Some(_) => {
            let without_comments = match super::structured_comments::remove(value) {
                Ok(cleaned) => cleaned,
                Err(_) => return false,
            };
            crate::imap::sent_date::parse(without_comments.as_bytes()).is_some()
        }
        None => false,
    }
}

/// Требует unquoted `size=1*DIGIT`, не принимая RFC 2231 extended spelling.
fn valid_size_parameter(value: &str, wire_parameter: &str) -> bool {
    let mut components = wire_parameter.split('=');
    let name = components.next().map(super::wsp::trim);
    let raw_value = components.next().map(super::wsp::trim);
    match (name, raw_value, components.next()) {
        (Some(name), Some(raw_value), None)
            if name.eq_ignore_ascii_case("size") && raw_value == value =>
        {
            !raw_value.is_empty()
                && raw_value
                    .chars()
                    .all(|character| character.is_ascii_digit())
        }
        _ => false,
    }
}

/// Возвращает имя и значение, если entry является ровно одним parameter.
fn collect_parameters(tail: &str) -> Result<Vec<ParsedParameter>, &'static str> {
    let parsed = split_parameter_sections(tail)?
        .into_iter()
        .map(super::wsp::trim)
        .filter(|parameter| !parameter.is_empty())
        .filter_map(parse_one_parameter)
        .collect();
    Ok(parsed)
}

/// Разделяет параметры, не трактуя точку с запятой в quoted-string как delimiter.
fn split_parameter_sections(input: &str) -> Result<Vec<&str>, &'static str> {
    let mut sections = Vec::new();
    let mut quoted = false;
    let mut escaped = false;
    let mut start = 0_usize;
    for (position, character) in input.char_indices() {
        match (quoted, escaped, character) {
            (true, true, _) => escaped = false,
            (true, false, '\\') => escaped = true,
            (_, false, '"') => quoted = !quoted,
            (false, false, ';') => {
                sections.push(
                    input
                        .get(start..position)
                        .ok_or("invalid disposition boundary")?,
                );
                start = position.saturating_add(1);
            }
            _ => {}
        }
    }
    if quoted || escaped {
        return Err("unterminated disposition quoted-string");
    }
    sections.push(input.get(start..).ok_or("invalid disposition tail")?);
    Ok(sections)
}

/// Сериализует disposition extension как IMAP list и string values.
pub fn encode_bodystructure(value: &Disposition) -> Result<String, &'static str> {
    let mut output = format!("(\"{}\"", quote(value.kind.as_str())?);
    if value.parameters.is_empty() {
        output.push_str(" NIL)");
        return Ok(output);
    }
    output.push_str(" (");
    let mut separator = "";
    for (name, parameter) in &value.parameters {
        output.push_str(separator);
        output.push_str(&format!(
            "\"{}\" \"{}\"",
            quote(&name.to_ascii_uppercase())?,
            quote(parameter)?
        ));
        separator = " ";
    }
    output.push_str("))");
    Ok(output)
}

/// Кавычит IMAP nstring содержимое и запрещает CR/LF injection.
fn quote(value: &str) -> Result<String, &'static str> {
    let mut output = String::with_capacity(value.len());
    for character in value.chars() {
        match character {
            '\r' | '\n' => return Err("newline in MIME disposition value"),
            '\\' | '"' => {
                output.push('\\');
                output.push(character);
            }
            _ => output.push(character),
        }
    }
    Ok(output)
}

#[cfg(test)]
#[path = "disposition_tests.rs"]
mod tests;
