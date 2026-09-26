//! Типизированная модель MIME Content-Disposition для BODYSTRUCTURE.

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
            _ => Self::Extension(normalized),
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
}

/// Разбирает одно поле Content-Disposition, сохраняя расширения RFC 2231.
pub fn parse(value: &str) -> Result<Disposition, &'static str> {
    let normalized = super::structured_comments::remove(value)?;
    let mut sections = normalized.splitn(2, ';');
    let raw_kind = sections.next().ok_or("missing MIME disposition token")?;
    let kind = DispositionKind::parse(super::wsp::trim(raw_kind))?;
    let parameters = match sections.next() {
        Some(tail) => {
            let parsed = parameters::parse(&format!("application/x;{tail}"))?;
            parsed.parameters
        }
        None => BTreeMap::new(),
    };
    Ok(Disposition { kind, parameters })
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
