//! Ограниченная грамматика обычного SMTP dot-atom mailbox.

/// Форма mailbox после ограниченного синтаксического разбора.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum MailboxForm {
    NullReversePath,
    DotAtom,
    BarePostmaster,
    Invalid,
}

/// Классифицирует поддержанные SMTP mailbox формы.
///
/// Quoted local-part и address-literal намеренно не принимаются этим профилем.
pub(super) fn classify_mailbox(mailbox: &str) -> MailboxForm {
    match mailbox {
        "" => MailboxForm::NullReversePath,
        _ if mailbox.eq_ignore_ascii_case("postmaster") => MailboxForm::BarePostmaster,
        _ if is_dot_atom_mailbox(mailbox) => MailboxForm::DotAtom,
        _ => MailboxForm::Invalid,
    }
}

/// Проверяет ASCII mailbox и длины его local-part/domain по SMTP/DNS профилю.
fn is_dot_atom_mailbox(mailbox: &str) -> bool {
    let Some((local_part, domain)) = mailbox.split_once('@') else {
        return false;
    };
    if domain.contains('@') || !valid_local_part(local_part) {
        return false;
    }
    valid_domain(domain)
}

/// Проверяет atext и точки без пустых dot-atom компонентов.
fn valid_local_part(local_part: &str) -> bool {
    if local_part.is_empty() || local_part.len() > super::MAX_LOCAL_PART_SIZE {
        return false;
    }
    local_part.split('.').all(valid_atom)
}

/// Проверяет один атом на допустимые RFC atext ASCII октеты.
fn valid_atom(atom: &str) -> bool {
    !atom.is_empty() && atom.bytes().all(valid_atext)
}

/// Классифицирует разрешённые ASCII символы без строковой ветвизации.
fn valid_atext(octet: u8) -> bool {
    octet.is_ascii_alphanumeric()
        || matches!(octet, b'!' | b'#'..=b'\'' | b'*'..=b'+' | b'-' | b'/' | b'=' | b'?' | b'^'..=b'`' | b'{'..=b'~')
}

/// Проверяет DNS labels: длина 1..63, буквы/цифры по краям, дефис внутри.
fn valid_domain(domain: &str) -> bool {
    if domain.is_empty() || domain.len() > 255 || !domain.is_ascii() {
        return false;
    }
    domain.split('.').all(valid_domain_label)
}

/// Проверяет одну DNS-метку без Unicode или завершающей пустой метки.
fn valid_domain_label(label: &str) -> bool {
    let bytes = label.as_bytes();
    match bytes {
        [only] => only.is_ascii_alphanumeric(),
        [first, middle @ .., last] => {
            bytes.len() <= 63
                && first.is_ascii_alphanumeric()
                && last.is_ascii_alphanumeric()
                && middle
                    .iter()
                    .all(|octet| octet.is_ascii_alphanumeric() || *octet == b'-')
        }
        [] => false,
    }
}

#[cfg(test)]
#[path = "address_tests.rs"]
mod tests;
