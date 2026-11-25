//! Script-wide правила RFC 5230 для единственного vacation action.

use super::super::ast::{Action, Command, VacationAction, VacationInterval};

/// Проверяет capability, уникальность action и вложенные mailboxes.
pub(super) fn actions(
    items: &[Command],
    required: bool,
    seconds_required: bool,
    variables_required: bool,
) -> Result<(), &'static str> {
    let mut count = 0;
    visit(
        items,
        required,
        seconds_required,
        variables_required,
        &mut count,
    )
}

fn visit(
    items: &[Command],
    required: bool,
    seconds_required: bool,
    variables_required: bool,
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
                addresses(action, variables_required)?;
            }
            Command::If {
                then_commands,
                else_commands,
                ..
            } => {
                visit(
                    then_commands,
                    required,
                    seconds_required,
                    variables_required,
                    count,
                )?;
                visit(
                    else_commands,
                    required,
                    seconds_required,
                    variables_required,
                    count,
                )?;
            }
            Command::Action(_) => {}
        }
    }
    Ok(())
}

fn addresses(action: &VacationAction, variables_required: bool) -> Result<(), &'static str> {
    if let Some(from) = action.from.as_deref() {
        if unsafe_header(from) {
            return Err("invalid vacation from mailbox-list");
        }
        if !dynamic(from.as_bytes(), variables_required) && !mailbox_list(from) {
            return Err("invalid vacation from mailbox-list");
        }
    }
    for address in &action.addresses {
        if unsafe_header(address)
            || (!dynamic(address.as_bytes(), variables_required) && !single_mailbox(address))
        {
            return Err("invalid vacation address");
        }
    }
    if action.mime && !dynamic(&action.reason, variables_required) {
        super::super::vacation_mime::validate_reason(&action.reason)?;
    }
    Ok(())
}

/// Проверяет уже раскрытые mailboxes и MIME entity непосредственно перед планом.
pub(super) fn validate_expanded(action: &VacationAction) -> Result<(), &'static str> {
    addresses(action, false)
}

fn dynamic(value: &[u8], variables_required: bool) -> bool {
    variables_required && value.windows(2).any(|marker| marker == b"${")
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
