//! Разбор SMTP reverse-path/forward-path и typed ESMTP MAIL параметров.
use super::dsn_parameters::{ReturnContent, decode_xtext, parse_notify};
use super::{
    BodyMode, MAX_PATH_SIZE, MailDsn, MailFrom, ParseError, RcptTo, RecipientDsn, address,
};

/// Разбирает reverse-path и ровно один необязательный SIZE параметр.
pub(super) fn parse_mail_from(argument: &str) -> Result<(MailFrom, Option<MailDsn>), ParseError> {
    let (reverse_path, suffix) = parse_path(argument, "FROM:", PathKind::Reverse)?;
    let parsed_parameters = match suffix.is_empty() {
        true => Vec::new(),
        false => suffix
            .split(' ')
            .map(parse_mail_parameter)
            .collect::<Result<Vec<_>, _>>()?,
    };
    let mut declared_size = None;
    let mut body_mode = BodyMode::SevenBit;
    let mut smtp_utf8 = false;
    let mut dsn = MailDsn::default();
    let mut size_seen = false;
    let mut body_seen = false;
    let mut smtp_utf8_seen = false;
    let mut return_seen = false;
    let mut envelope_id_seen = false;
    for parameter in parsed_parameters {
        match parameter {
            MailParameter::Size(size) if !size_seen => {
                declared_size = Some(size);
                size_seen = true;
            }
            MailParameter::Body(mode) if !body_seen => {
                body_mode = mode;
                body_seen = true;
            }
            MailParameter::SmtpUtf8 if !smtp_utf8_seen => {
                smtp_utf8 = true;
                smtp_utf8_seen = true;
            }
            MailParameter::ReturnContent(value) if !return_seen => {
                dsn.return_content = Some(value);
                return_seen = true;
            }
            MailParameter::EnvelopeId(value) if !envelope_id_seen => {
                dsn.envelope_id = Some(value);
                envelope_id_seen = true;
            }
            MailParameter::Size(_)
            | MailParameter::Body(_)
            | MailParameter::SmtpUtf8
            | MailParameter::ReturnContent(_)
            | MailParameter::EnvelopeId(_) => {
                return Err(ParseError::Syntax);
            }
        }
    }
    if address::requires_smtp_utf8(&reverse_path) && !smtp_utf8 {
        return Err(ParseError::SmtpUtf8Required);
    }
    let dsn = match dsn == MailDsn::default() {
        true => None,
        false => Some(dsn),
    };
    Ok((
        MailFrom {
            reverse_path,
            declared_size,
            body_mode,
            smtp_utf8,
        },
        dsn,
    ))
}

/// Разбирает адрес и синтаксис DSN без принятия решения о capability.
pub(super) fn parse_recipient(argument: &str) -> Result<RcptTo, ParseError> {
    let (recipient, suffix) = parse_path(argument, "TO:", PathKind::Forward)?;
    let mut dsn = RecipientDsn::default();
    let mut notify_seen = false;
    let mut original_seen = false;
    for parameter in suffix.split(' ').filter(|value| !value.is_empty()) {
        let (name, value) = match parameter.split_once('=') {
            Some((name, value)) => (name, Some(value)),
            None => (parameter, None),
        };
        match (name.to_ascii_uppercase().as_str(), value) {
            ("NOTIFY", Some(value)) if !notify_seen => {
                dsn.notify = Some(parse_notify(value)?);
                notify_seen = true;
            }
            ("ORCPT", Some(value)) if !original_seen => {
                if parameter.len() > 500 {
                    return Err(ParseError::Syntax);
                }
                dsn.original_recipient = Some(parse_original_recipient(value)?);
                original_seen = true;
            }
            ("NOTIFY" | "ORCPT", _) => return Err(ParseError::Syntax),
            _ => return Err(ParseError::UnsupportedParameter),
        }
    }
    Ok(RcptTo {
        address: recipient,
        dsn,
    })
}

/// Разбирает ORCPT=rfc822;xtext и сохраняет исходный регистр octet-значения.
fn parse_original_recipient(value: &str) -> Result<super::OriginalRecipient, ParseError> {
    let (kind, address) = value.split_once(';').ok_or(ParseError::Syntax)?;
    let address_type = match kind.to_ascii_lowercase().as_str() {
        "rfc822" => super::OriginalAddressType::Rfc822,
        _ => return Err(ParseError::Syntax),
    };
    Ok(super::OriginalRecipient {
        address_type,
        address: decode_xtext(address, value.len())?,
    })
}

fn parse_path<'a>(
    argument: &'a str,
    prefix: &str,
    kind: PathKind,
) -> Result<(String, &'a str), ParseError> {
    let (actual, rest) = argument.split_once(':').ok_or(ParseError::Syntax)?;
    let expected = prefix.strip_suffix(':').unwrap_or(prefix);
    if !actual.eq_ignore_ascii_case(expected) {
        return Err(ParseError::Syntax);
    }
    // RFC 5321 требует path сразу после двоеточия, без нормализации пробелов.
    let Some(path) = rest.strip_prefix('<') else {
        return Err(ParseError::Syntax);
    };
    let Some(terminator) = closing_path_angle(path) else {
        return Err(ParseError::Syntax);
    };
    let (path_content, remainder) = path.split_at(terminator);
    let Some(suffix) = remainder.strip_prefix('>') else {
        return Err(ParseError::Syntax);
    };
    if path_content.len().saturating_add(2) > MAX_PATH_SIZE {
        return Err(ParseError::Syntax);
    }
    let mailbox = address::without_source_route(path_content).ok_or(ParseError::Syntax)?;
    match (kind, address::classify_mailbox(mailbox)) {
        (PathKind::Reverse, address::MailboxForm::NullReversePath)
        | (
            _,
            address::MailboxForm::DotAtom
            | address::MailboxForm::QuotedLocalPart
            | address::MailboxForm::AddressLiteral,
        )
        | (PathKind::Forward, address::MailboxForm::BarePostmaster) => {}
        _ => return Err(ParseError::Syntax),
    }
    match suffix.is_empty() {
        true => Ok((mailbox.to_owned(), "")),
        false => parameter_suffix(suffix).map(|parameters| (mailbox.to_owned(), parameters)),
    }
}

fn parameter_suffix(suffix: &str) -> Result<&str, ParseError> {
    let Some(parameters) = suffix.strip_prefix(' ') else {
        return Err(ParseError::Syntax);
    };
    if parameters.is_empty()
        || parameters.contains('\t')
        || parameters.split(' ').any(str::is_empty)
    {
        return Err(ParseError::Syntax);
    }
    Ok(parameters)
}

fn closing_path_angle(path: &str) -> Option<usize> {
    let mut quoted = false;
    let mut escaped = false;
    for (offset, character) in path.char_indices() {
        match (quoted, escaped, character) {
            (true, true, _) => escaped = false,
            (true, false, '\\') => escaped = true,
            (true, false, '"') => quoted = false,
            (false, _, '"') => quoted = true,
            (false, _, '>') => return Some(offset),
            _ => {}
        }
    }
    None
}

#[derive(Clone, Copy)]
enum PathKind {
    Reverse,
    Forward,
}

fn parse_mail_parameter(parameter: &str) -> Result<MailParameter, ParseError> {
    let (name, value) = match parameter.split_once('=') {
        Some((name, value)) => (name, Some(value)),
        None => (parameter, None),
    };
    match (name.to_ascii_uppercase().as_str(), value) {
        ("SIZE", Some(value)) => parse_size(value).map(MailParameter::Size),
        ("BODY", Some(value)) => parse_body_mode(value).map(MailParameter::Body),
        ("SMTPUTF8", None) => Ok(MailParameter::SmtpUtf8),
        ("RET", Some(value)) => parse_return_content(value).map(MailParameter::ReturnContent),
        ("ENVID", Some(value)) => decode_xtext(value, 100).map(MailParameter::EnvelopeId),
        ("SIZE" | "BODY" | "SMTPUTF8", _) => Err(ParseError::Syntax),
        ("RET" | "ENVID", _) => Err(ParseError::Syntax),
        _ => Err(ParseError::UnsupportedParameter),
    }
}

fn parse_body_mode(value: &str) -> Result<BodyMode, ParseError> {
    match value.to_ascii_uppercase().as_str() {
        "7BIT" => Ok(BodyMode::SevenBit),
        "8BITMIME" => Ok(BodyMode::EightBitMime),
        "BINARYMIME" => Ok(BodyMode::BinaryMime),
        _ => Err(ParseError::Syntax),
    }
}

#[derive(Debug, Eq, PartialEq)]
enum MailParameter {
    Size(u128),
    Body(BodyMode),
    SmtpUtf8,
    ReturnContent(ReturnContent),
    EnvelopeId(Vec<u8>),
}

/// Разбирает область возврата RFC 3461 без зависимости от регистра.
fn parse_return_content(value: &str) -> Result<ReturnContent, ParseError> {
    match value.to_ascii_uppercase().as_str() {
        "HDRS" => Ok(ReturnContent::Headers),
        "FULL" => Ok(ReturnContent::Full),
        _ => Err(ParseError::Syntax),
    }
}

fn parse_size(value: &str) -> Result<u128, ParseError> {
    let valid = (1..=20).contains(&value.len()) && value.bytes().all(|byte| byte.is_ascii_digit());
    match valid {
        true => value.parse().map_err(|_| ParseError::Syntax),
        false => Err(ParseError::Syntax),
    }
}
