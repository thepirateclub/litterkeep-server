//! Синтаксическая проверка URI-reference по общей грамматике RFC 3986.
//!
//! Проверка намеренно работает только с ASCII URI: RFC 3986 задаёт octet
//! grammar, а Unicode IRI требует отдельной модели и правил преобразования.
//! Percent-encoded octets проверяются, но остаются в исходной строке.

use std::net::Ipv6Addr;

/// Проверяет абсолютный URI или относительную ссылку, не разрешая и не меняя её.
pub(crate) fn is_valid(value: &str) -> bool {
    match value.is_ascii() {
        false => false,
        true => validate_reference(value),
    }
}

fn validate_reference(value: &str) -> bool {
    // Fragment отделяется первым, чтобы вопросительный знак внутри него
    // оставался разрешённым символом fragment, а не начинал query.
    let (without_fragment, fragment_valid) = split_optional(value, '#', Component::Query);
    let (hierarchy, query_valid) = split_optional(without_fragment, '?', Component::Query);
    match fragment_valid && query_valid {
        false => false,
        true => validate_hierarchy(hierarchy),
    }
}

fn split_optional(value: &str, delimiter: char, component: Component) -> (&str, bool) {
    // Разделяется только первый delimiter: query допускает последующие `?`,
    // тогда как второй `#` будет отклонён грамматикой fragment.
    match value.split_once(delimiter) {
        Some((head, tail)) => (head, validate_component(tail, component)),
        None => (value, true),
    }
}

fn validate_hierarchy(value: &str) -> bool {
    // Двоеточие до первого slash вводит scheme только при корректном имени;
    // для относительной ссылки двоеточие в первом сегменте запрещено.
    match scheme_separator(value) {
        Some(separator) => {
            let (scheme, remainder) = value.split_at(separator);
            validate_scheme(scheme)
                && remainder
                    .strip_prefix(':')
                    .is_some_and(|part| validate_hier_part(part, true))
        }
        None => validate_hier_part(value, false),
    }
}

fn scheme_separator(value: &str) -> Option<usize> {
    // По RFC 3986 scheme распознаётся до path, то есть до первого `/`.
    let colon = value.find(':')?;
    let slash = value.find('/').unwrap_or(usize::MAX);
    match colon < slash {
        true => Some(colon),
        false => None,
    }
}

fn validate_scheme(value: &str) -> bool {
    // Первый символ scheme обязан быть ASCII-буквой, далее разрешены ALPHA,
    // DIGIT, плюс, точка и дефис.
    let mut characters = value.chars();
    match characters.next() {
        Some(first) if first.is_ascii_alphabetic() => characters.all(|character| {
            character.is_ascii_alphanumeric() || matches!(character, '+' | '.' | '-')
        }),
        _ => false,
    }
}

fn validate_hier_part(value: &str, absolute: bool) -> bool {
    // Authority появляется исключительно после двух начальных slash.
    match value.strip_prefix("//") {
        Some(authority_and_path) => {
            let (authority, path) = split_authority(authority_and_path);
            validate_authority(authority) && validate_path_abempty(path)
        }
        None => validate_path_without_authority(value, absolute),
    }
}

fn split_authority(value: &str) -> (&str, &str) {
    // Первый slash завершает authority; остаток проверяется как path-abempty.
    match value.split_once('/') {
        Some((authority, suffix)) => (authority, suffix.get(0..).unwrap_or_default()),
        None => (value, ""),
    }
}

fn validate_path_abempty(value: &str) -> bool {
    // Пустой path допустим, а каждый непустой символ должен принадлежать
    // pchar или быть разделителем сегментов `/`.
    validate_component(value, Component::Path)
}

fn validate_path_without_authority(value: &str, absolute: bool) -> bool {
    // Без authority путь бывает абсолютным (начинается с `/`) или rootless.
    match value.starts_with('/') {
        true => validate_component(value, Component::Path),
        false => match !absolute && first_segment_has_colon(value) {
            true => false,
            false => validate_component(value, Component::Path),
        },
    }
}

fn first_segment_has_colon(value: &str) -> bool {
    // relative-part начинается path-noscheme, чей первый сегмент не содержит `:`.
    value
        .split('/')
        .next()
        .is_some_and(|segment| segment.contains(':'))
}

fn validate_authority(value: &str) -> bool {
    // Userinfo отделяется единственным `@`; оставшаяся часть разбирается как
    // host и optional port, не допуская двусмысленной второй границы.
    let (userinfo, host_port) = match value.split_once('@') {
        Some((userinfo, host)) if !host.contains('@') => (Some(userinfo), host),
        Some(_) => return false,
        None => (None, value),
    };
    match userinfo {
        Some(user) if !validate_component(user, Component::UserInfo) => return false,
        _ => {}
    }
    validate_host_port(host_port)
}

fn validate_host_port(value: &str) -> bool {
    // IP-literal обязан быть заключён в квадратные скобки; обычное имя хоста
    // разбирается отдельно и не может содержать необрамлённый IPv6.
    match value.strip_prefix('[') {
        Some(bracketed) => validate_ip_literal(bracketed),
        None => validate_reg_name_and_port(value),
    }
}

fn validate_ip_literal(value: &str) -> bool {
    // После закрывающей скобки разрешён только пустой суффикс либо `:port`.
    let (address, suffix) = match value.split_once(']') {
        Some(parts) => parts,
        None => return false,
    };
    let address_valid = address.parse::<Ipv6Addr>().is_ok() || validate_ipv_future(address);
    address_valid && validate_port_suffix(suffix)
}

fn validate_ipv_future(value: &str) -> bool {
    // IPvFuture имеет вид `v` + hex version + точка + непустой адресный хвост.
    let digits_and_tail = value.strip_prefix('v').or_else(|| value.strip_prefix('V'));
    match digits_and_tail.and_then(|rest| rest.split_once('.')) {
        Some((digits, tail)) => {
            !digits.is_empty()
                && digits
                    .chars()
                    .all(|character| character.is_ascii_hexdigit())
                && !tail.is_empty()
                && validate_ipv_future_tail(tail)
        }
        None => false,
    }
}

fn validate_ipv_future_tail(value: &str) -> bool {
    value
        .bytes()
        .all(|byte| byte.is_ascii() && (is_unreserved(byte) || is_sub_delim(byte) || byte == b':'))
}

fn validate_reg_name_and_port(value: &str) -> bool {
    // Необрамлённое имя может содержать не более одного разделителя port;
    // двоеточия в IPv6 без `[]` тем самым отклоняются.
    let (host, port) = match value.split_once(':') {
        Some((host, port)) if !port.contains(':') => (host, Some(port)),
        Some(_) => return false,
        None => (value, None),
    };
    validate_component(host, Component::RegName)
        && port.is_none_or(|candidate| validate_component(candidate, Component::Port))
}

fn validate_port_suffix(value: &str) -> bool {
    // RFC допускает пустой port после двоеточия, иначе принимает только цифры.
    match value.strip_prefix(':') {
        Some(port) => validate_component(port, Component::Port),
        None => value.is_empty(),
    }
}

fn validate_component(value: &str, component: Component) -> bool {
    // Проверка идёт одним проходом по байтам. `%` всегда требует ровно две
    // следующие hex-цифры; побайтовое индексирование входа не используется.
    let mut bytes = value.bytes().peekable();
    while let Some(byte) = bytes.next() {
        match byte {
            b'%' => match (bytes.next(), bytes.next()) {
                (Some(high), Some(low)) if is_hex(high) && is_hex(low) => {}
                _ => return false,
            },
            byte if component.accepts(byte) => {}
            _ => return false,
        }
    }
    true
}

fn is_hex(byte: u8) -> bool {
    byte.is_ascii_hexdigit()
}

#[derive(Clone, Copy)]
enum Component {
    Path,
    Query,
    UserInfo,
    RegName,
    Port,
}

impl Component {
    fn accepts(self, byte: u8) -> bool {
        match self {
            Self::Path => is_pchar(byte) || byte == b'/',
            Self::Query => is_pchar(byte) || matches!(byte, b'/' | b'?'),
            Self::UserInfo => is_unreserved(byte) || is_sub_delim(byte) || byte == b':',
            Self::RegName => is_unreserved(byte) || is_sub_delim(byte),
            Self::Port => byte.is_ascii_digit(),
        }
    }
}

fn is_pchar(byte: u8) -> bool {
    is_unreserved(byte) || is_sub_delim(byte) || matches!(byte, b':' | b'@')
}

fn is_unreserved(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~')
}

fn is_sub_delim(byte: u8) -> bool {
    matches!(
        byte,
        b'!' | b'$' | b'&' | b'\'' | b'(' | b')' | b'*' | b'+' | b',' | b';' | b'='
    )
}

#[cfg(test)]
#[path = "uri_reference_tests.rs"]
mod tests;
