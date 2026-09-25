//! Статическая policy validation до атомарной публикации script версии.

use super::ast::{Action, AddressPart, Capability, Command, Condition, Script};

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
    let subaddress = script
        .requires
        .iter()
        .any(|value| matches!(value, Capability::Subaddress));
    commands(&script.commands, 0, &mut count, fileinto, subaddress)
}

fn commands(
    items: &[Command],
    depth: usize,
    count: &mut usize,
    fileinto: bool,
    subaddress: bool,
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
            Command::Action(action) => action_valid(action, fileinto)?,
            Command::If {
                condition,
                then_commands,
                else_commands,
            } => {
                condition_valid(condition, depth, subaddress)?;
                commands(
                    then_commands,
                    depth.saturating_add(1),
                    count,
                    fileinto,
                    subaddress,
                )?;
                commands(
                    else_commands,
                    depth.saturating_add(1),
                    count,
                    fileinto,
                    subaddress,
                )?;
            }
        }
    }
    Ok(())
}

/// Проверяет ограничения mailbox и обязательное объявление optional fileinto.
fn action_valid(action: &Action, fileinto: bool) -> Result<(), &'static str> {
    match action {
        Action::FileInto(_) if !fileinto => Err("fileinto extension must be required"),
        Action::FileInto(name) if safe_name(name) => Ok(()),
        Action::Redirect(address) if address.contains('@') => Ok(()),
        Action::FileInto(_) | Action::Redirect(_) => Err("unsafe sieve action target"),
        Action::Keep | Action::Discard | Action::Stop => Ok(()),
    }
}

fn condition_valid(
    condition: &Condition,
    depth: usize,
    subaddress: bool,
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
        Condition::Exists { names } if names.is_empty() => Err("empty sieve exists list"),
        Condition::Not(value) => condition_valid(value, depth.saturating_add(1), subaddress),
        Condition::AllOf(values) | Condition::AnyOf(values) if values.is_empty() => {
            Err("empty sieve condition list")
        }
        Condition::AllOf(values) | Condition::AnyOf(values) => values
            .iter()
            .try_for_each(|value| condition_valid(value, depth.saturating_add(1), subaddress)),
        Condition::True | Condition::False | Condition::Exists { .. } | Condition::Size { .. } => {
            Ok(())
        }
    }
}

fn string_match_valid(
    names: &[String],
    matcher: &super::ast::StringMatch,
) -> Result<(), &'static str> {
    if matches!(
        (names.is_empty(), matcher.keys.is_empty()),
        (true, _) | (_, true)
    ) {
        return Err("empty sieve string matcher");
    }
    match (
        names.iter().any(String::is_empty),
        matcher.keys.iter().any(String::is_empty),
    ) {
        (false, false) => Ok(()),
        _ => Err("empty sieve string matcher"),
    }
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
