//! Статическая policy validation до атомарной публикации script версии.

use super::ast::{Action, AddressPart, Capability, Command, Condition, Script};
#[path = "vacation_validation.rs"]
mod vacation_validation;

pub(super) fn script(script: &Script) -> Result<(), &'static str> {
    if script
        .requires
        .iter()
        .any(|value| matches!(value, Capability::Unsupported(_)))
    {
        return Err("unsupported sieve require");
    }
    let mut count = 0;
    let fileinto = script
        .requires
        .iter()
        .any(|value| matches!(value, Capability::FileInto));
    let copy = script
        .requires
        .iter()
        .any(|value| matches!(value, Capability::Copy));
    let subaddress = script
        .requires
        .iter()
        .any(|value| matches!(value, Capability::Subaddress));
    let envelope = script
        .requires
        .iter()
        .any(|value| matches!(value, Capability::Envelope));
    let vacation = script
        .requires
        .iter()
        .any(|value| matches!(value, Capability::Vacation | Capability::VacationSeconds));
    let vacation_seconds = script
        .requires
        .iter()
        .any(|value| matches!(value, Capability::VacationSeconds));
    commands(
        &script.commands,
        0,
        &mut count,
        fileinto,
        copy,
        subaddress,
        envelope,
    )?;
    vacation_validation::actions(&script.commands, vacation, vacation_seconds)
}

fn commands(
    items: &[Command],
    depth: usize,
    count: &mut usize,
    fileinto: bool,
    copy: bool,
    subaddress: bool,
    envelope: bool,
) -> Result<(), &'static str> {
    if depth > 16 {
        return Err("sieve nesting limit exceeded");
    }
    for command in items {
        *count = count.saturating_add(1);
        if *count > 128 {
            return Err("sieve command limit exceeded");
        }
        match command {
            Command::Action(action) => action_valid(action, fileinto, copy)?,
            Command::If {
                condition,
                then_commands,
                else_commands,
            } => {
                condition_valid(condition, depth, subaddress, envelope)?;
                commands(
                    then_commands,
                    depth.saturating_add(1),
                    count,
                    fileinto,
                    copy,
                    subaddress,
                    envelope,
                )?;
                commands(
                    else_commands,
                    depth.saturating_add(1),
                    count,
                    fileinto,
                    copy,
                    subaddress,
                    envelope,
                )?;
            }
        }
    }
    Ok(())
}

/// Проверяет ограничения mailbox и обязательное объявление optional fileinto.
fn action_valid(action: &Action, fileinto: bool, copy: bool) -> Result<(), &'static str> {
    match action {
        Action::FileInto(_) if !fileinto => Err("fileinto extension must be required"),
        Action::FileInto(name) if safe_name(name) => Ok(()),
        Action::FileIntoCopy(_) if !fileinto => Err("fileinto extension must be required"),
        Action::FileIntoCopy(_) | Action::RedirectCopy(_) if !copy => {
            Err("copy extension must be required")
        }
        Action::FileIntoCopy(name) if safe_name(name) => Ok(()),
        Action::Redirect(address) | Action::RedirectCopy(address)
            if valid_redirect_address(address) =>
        {
            Ok(())
        }
        Action::FileInto(_)
        | Action::FileIntoCopy(_)
        | Action::Redirect(_)
        | Action::RedirectCopy(_) => Err("unsafe sieve action target"),
        Action::Keep | Action::Discard | Action::Stop | Action::Vacation(_) => Ok(()),
    }
}

/// Проверяет capability, единственность script-wide и адресные параметры.
/// Применяет RFC 5322 mailbox parser и запрещает sieve-address route/group формы.
fn valid_redirect_address(value: &str) -> bool {
    if has_route_or_group_syntax(value) {
        return false;
    }
    match crate::mime::envelope_address::parse_value(value) {
        Ok(mailboxes) => mailboxes.len() == 1,
        Err(_) => false,
    }
}

#[derive(Clone, Copy)]
enum RedirectSyntaxState {
    Text,
    Quoted,
    Comment(usize),
    DomainLiteral,
}

/// Отличает запрещённые route/group-разделители от текста в quote/comment/domain.
fn has_route_or_group_syntax(value: &str) -> bool {
    let mut state = RedirectSyntaxState::Text;
    let mut escaped = false;
    for character in value.chars() {
        if escaped {
            escaped = false;
            continue;
        }
        match state {
            RedirectSyntaxState::Text => match character {
                '"' => state = RedirectSyntaxState::Quoted,
                '(' => state = RedirectSyntaxState::Comment(1),
                '[' => state = RedirectSyntaxState::DomainLiteral,
                ':' | ';' => return true,
                _ => {}
            },
            RedirectSyntaxState::Quoted => match character {
                '\\' => escaped = true,
                '"' => state = RedirectSyntaxState::Text,
                _ => {}
            },
            RedirectSyntaxState::Comment(depth) => match character {
                '\\' => escaped = true,
                '(' => state = RedirectSyntaxState::Comment(depth.saturating_add(1)),
                ')' if depth <= 1 => state = RedirectSyntaxState::Text,
                ')' => state = RedirectSyntaxState::Comment(depth.saturating_sub(1)),
                _ => {}
            },
            RedirectSyntaxState::DomainLiteral => match character {
                '\\' => escaped = true,
                ']' => state = RedirectSyntaxState::Text,
                _ => {}
            },
        }
    }
    false
}

fn condition_valid(
    condition: &Condition,
    depth: usize,
    subaddress: bool,
    envelope: bool,
) -> Result<(), &'static str> {
    if depth > 16 {
        return Err("sieve nesting limit exceeded");
    }
    match condition {
        Condition::Header { names, matcher } => string_match_valid(names, matcher),
        Condition::Address {
            part: AddressPart::User | AddressPart::Detail,
            ..
        } if !subaddress => Err("subaddress extension must be required"),
        Condition::Address { names, matcher, .. } => string_match_valid(names, matcher),
        Condition::Envelope { .. } if !envelope => Err("envelope extension must be required"),
        Condition::Envelope {
            part: AddressPart::User | AddressPart::Detail,
            ..
        } if !subaddress => Err("subaddress extension must be required"),
        Condition::Envelope { matcher, .. } => matcher_keys_valid(matcher),
        Condition::Exists { names } if names.is_empty() => Err("empty sieve exists list"),
        Condition::Not(value) => {
            condition_valid(value, depth.saturating_add(1), subaddress, envelope)
        }
        Condition::AllOf(values) | Condition::AnyOf(values) if values.is_empty() => {
            Err("empty sieve condition list")
        }
        Condition::AllOf(values) | Condition::AnyOf(values) => {
            values.iter().try_for_each(|value| {
                condition_valid(value, depth.saturating_add(1), subaddress, envelope)
            })
        }
        Condition::True | Condition::False | Condition::Exists { .. } | Condition::Size { .. } => {
            Ok(())
        }
    }
}

fn string_match_valid(
    names: &[String],
    matcher: &super::ast::StringMatch,
) -> Result<(), &'static str> {
    if names.is_empty() {
        return Err("empty sieve string matcher");
    }
    matcher_keys_valid(matcher)
}

/// Строковый comparator обязан иметь хотя бы один key.
fn matcher_keys_valid(matcher: &super::ast::StringMatch) -> Result<(), &'static str> {
    if matcher.keys.is_empty() {
        return Err("empty sieve string matcher");
    }
    Ok(())
}

fn safe_name(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    name.split('/').all(safe_folder_part)
}

/// Не допускает пустой segment и переходы за пределы mail account.
fn safe_folder_part(part: &str) -> bool {
    !matches!(part, "" | "." | "..")
}
