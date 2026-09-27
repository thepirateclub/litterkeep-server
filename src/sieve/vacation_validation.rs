//! Script-wide правила RFC 5230 для единственного vacation action.

use super::super::ast::{Action, Command, VacationAction, VacationInterval};

/// Проверяет capability, уникальность action и вложенные mailboxes.
pub(super) fn actions(
    items: &[Command],
    required: bool,
    seconds_required: bool,
) -> Result<(), &'static str> {
    let mut count = 0;
    visit(items, required, seconds_required, &mut count)
}

fn visit(
    items: &[Command],
    required: bool,
    seconds_required: bool,
    count: &mut usize,
) -> Result<(), &'static str> {
    for command in items {
        match command {
            Command::Action(Action::Vacation(action)) => {
                if !required {
                    return Err("vacation extension must be required");
                }
                if matches!(action.interval, Some(VacationInterval::Seconds(_)))
                    && !seconds_required
                {
                    return Err("vacation-seconds extension must be required");
                }
                *count = count.saturating_add(1);
                if *count > 1 {
                    return Err("vacation may appear only once per script");
                }
                addresses(action)?;
            }
            Command::If {
                then_commands,
                else_commands,
                ..
            } => {
                visit(then_commands, required, seconds_required, count)?;
                visit(else_commands, required, seconds_required, count)?;
            }
            Command::Action(_) => {}
        }
    }
    Ok(())
}

fn addresses(action: &VacationAction) -> Result<(), &'static str> {
    if let Some(from) = action.from.as_deref() {
        match (unsafe_header(from), mailbox_list(from)) {
            (false, true) => {}
            _ => return Err("invalid vacation from mailbox-list"),
        }
    }
    for address in &action.addresses {
        if unsafe_header(address) || !single_mailbox(address) {
            return Err("invalid vacation address");
        }
    }
    if action.mime {
        super::super::vacation_mime::validate_reason(action.reason.as_bytes())?;
    }
    Ok(())
}

fn mailbox_list(value: &str) -> bool {
    match crate::mime::envelope_address::parse_value(value) {
        Ok(mailboxes) => !mailboxes.is_empty(),
        Err(_) => false,
    }
}

fn single_mailbox(value: &str) -> bool {
    match crate::mime::envelope_address::parse_value(value) {
        Ok(mailboxes) => mailboxes.len() == 1,
        Err(_) => false,
    }
}

fn unsafe_header(value: &str) -> bool {
    value
        .chars()
        .any(|character| matches!(character, '\r' | '\n' | '\0'))
}
