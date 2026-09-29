//! Статическая policy validation до атомарной публикации script версии.

use super::ast::{Action, AddressPart, Capability, Command, Condition, Script};
#[path = "validate_namespace.rs"]
mod namespace;
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
    let variables = script
        .requires
        .iter()
        .any(|value| matches!(value, Capability::Variables));
    let policy = ValidationPolicy {
        fileinto,
        copy,
        subaddress,
        envelope,
        variables,
    };
    commands(&script.commands, 0, &mut count, policy)?;
    vacation_validation::actions(&script.commands, vacation, vacation_seconds, variables)
}

pub(super) fn validate_expanded_vacation(
    action: &super::ast::VacationAction,
) -> Result<(), &'static str> {
    vacation_validation::validate_expanded(action)
}

#[derive(Clone, Copy)]
struct ValidationPolicy {
    fileinto: bool,
    copy: bool,
    subaddress: bool,
    envelope: bool,
    variables: bool,
}

fn commands(
    items: &[Command],
    depth: usize,
    count: &mut usize,
    policy: ValidationPolicy,
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
            Command::Action(action) => action_valid(action, policy)?,
            Command::If {
                condition,
                then_commands,
                else_commands,
            } => {
                condition_valid(condition, depth, policy)?;
                commands(then_commands, depth.saturating_add(1), count, policy)?;
                commands(else_commands, depth.saturating_add(1), count, policy)?;
            }
        }
    }
    Ok(())
}

/// Проверяет ограничения mailbox и обязательное объявление optional fileinto.
fn action_valid(action: &Action, policy: ValidationPolicy) -> Result<(), &'static str> {
    if policy.variables {
        namespace::action(action)?;
    }
    match action {
        Action::FileInto(_) if !policy.fileinto => Err("fileinto extension must be required"),
        Action::FileInto(name) if safe_name(name) => Ok(()),
        Action::FileIntoCopy(_) if !policy.fileinto => Err("fileinto extension must be required"),
        Action::FileIntoCopy(_) | Action::RedirectCopy(_) if !policy.copy => {
            Err("copy extension must be required")
        }
        Action::FileIntoCopy(name) if safe_name(name) => Ok(()),
        Action::Redirect(address) | Action::RedirectCopy(address)
            if policy.variables && address.contains("${") =>
        {
            Ok(())
        }
        Action::Redirect(address) | Action::RedirectCopy(address)
            if valid_redirect_address(address) =>
        {
            Ok(())
        }
        Action::Set(_) if !policy.variables => Err("variables extension must be required"),
        Action::Set(action) => {
            super::variables::VariableStore::validate_name(&action.name)
                .map_err(|_| "invalid variables name")?;
            super::variables::VariableStore::validate_modifiers(&action.modifiers)
                .map_err(|_| "conflicting variables modifiers")
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
pub(super) fn valid_redirect_address(value: &str) -> bool {
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
    policy: ValidationPolicy,
) -> Result<(), &'static str> {
    if policy.variables {
        namespace::condition(condition)?;
    }
    if depth > 16 {
        return Err("sieve nesting limit exceeded");
    }
    match condition {
        Condition::Header { names, matcher } => string_match_valid(names, matcher),
        Condition::String { .. } if !policy.variables => {
            Err("variables extension must be required")
        }
        Condition::String { sources, matcher } => {
            if sources.is_empty() {
                return Err("empty sieve string source list");
            }
            string_match_valid(sources, matcher)
        }
        Condition::Address {
            part: AddressPart::User | AddressPart::Detail,
            ..
        } if !policy.subaddress => Err("subaddress extension must be required"),
        Condition::Address { names, matcher, .. } => string_match_valid(names, matcher),
        Condition::Envelope { .. } if !policy.envelope => {
            Err("envelope extension must be required")
        }
        Condition::Envelope {
            part: AddressPart::User | AddressPart::Detail,
            ..
        } if !policy.subaddress => Err("subaddress extension must be required"),
        Condition::Envelope { matcher, .. } => matcher_keys_valid(matcher),
        Condition::Exists { names } if names.is_empty() => Err("empty sieve exists list"),
        Condition::Not(value) => condition_valid(value, depth.saturating_add(1), policy),
        Condition::AllOf(values) | Condition::AnyOf(values) if values.is_empty() => {
            Err("empty sieve condition list")
        }
        Condition::AllOf(values) | Condition::AnyOf(values) => values
            .iter()
            .try_for_each(|value| condition_valid(value, depth.saturating_add(1), policy)),
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

pub(super) fn safe_name(name: &str) -> bool {
    if name.is_empty() {
        return false;
    }
    name.split('/').all(safe_folder_part)
}

/// Не допускает пустой segment и переходы за пределы mail account.
fn safe_folder_part(part: &str) -> bool {
    !matches!(part, "" | "." | "..")
}
