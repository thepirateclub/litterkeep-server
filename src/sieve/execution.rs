//! Bounded исполнение AST в DeliveryPlan без доступа к сети и файловой системе.

use super::address;
use super::ast::{Action, Command, Condition, Script};
use super::matching;

/// Единственный результат Sieve, который delivery transaction может применить.
#[derive(Debug, Eq, PartialEq)]
pub struct DeliveryPlan {
    pub keep: bool,
    pub discard: bool,
    pub folders: Vec<String>,
    pub redirects: Vec<String>,
}

/// Отличает неявный keep от явно запрошенного действия с тем же результатом.
enum KeepState {
    Implicit,
    Cancelled,
    Explicit,
}

/// Запускает script над заголовками и исходным размером с fixed budget команд.
pub fn execute(
    script: &Script,
    headers: &[(String, String)],
    message_size: usize,
) -> Result<DeliveryPlan, &'static str> {
    let mut plan = DeliveryPlan {
        keep: true,
        discard: false,
        folders: Vec::new(),
        redirects: Vec::new(),
    };
    let mut stopped = false;
    let mut remaining = 128;
    let mut keep_state = KeepState::Implicit;
    commands(
        &script.commands,
        headers,
        message_size,
        &mut plan,
        &mut stopped,
        &mut remaining,
        &mut keep_state,
    )?;
    Ok(plan)
}

fn commands(
    items: &[Command],
    headers: &[(String, String)],
    message_size: usize,
    plan: &mut DeliveryPlan,
    stopped: &mut bool,
    remaining: &mut usize,
    keep_state: &mut KeepState,
) -> Result<(), &'static str> {
    for command in items {
        if *stopped {
            return Ok(());
        }
        match remaining.checked_sub(1) {
            Some(value) => *remaining = value,
            None => return Err("sieve execution limit exceeded"),
        }
        match command {
            Command::Action(action) => apply(action, plan, stopped, keep_state)?,
            Command::If {
                condition,
                then_commands,
                else_commands,
            } => {
                let selected = match matches(condition, headers, message_size) {
                    true => then_commands,
                    false => else_commands,
                };
                commands(
                    selected,
                    headers,
                    message_size,
                    plan,
                    stopped,
                    remaining,
                    keep_state,
                )?;
            }
        }
    }
    Ok(())
}

fn apply(
    action: &Action,
    plan: &mut DeliveryPlan,
    stopped: &mut bool,
    keep_state: &mut KeepState,
) -> Result<(), &'static str> {
    match action {
        Action::Keep => {
            plan.keep = true;
            *keep_state = KeepState::Explicit;
        }
        Action::Discard => {
            cancel_implicit_keep(plan, keep_state);
            plan.discard = true;
        }
        Action::FileInto(folder) => {
            cancel_implicit_keep(plan, keep_state);
            plan.folders.push(folder.clone());
        }
        Action::Redirect(address) => {
            if plan.redirects.len() >= 16 {
                return Err("sieve redirect limit exceeded");
            }
            cancel_implicit_keep(plan, keep_state);
            plan.redirects.push(address.clone());
        }
        Action::Stop => *stopped = true,
    }
    Ok(())
}

/// Delivery action отменяет implicit keep, но не ранее выполненный explicit keep.
fn cancel_implicit_keep(plan: &mut DeliveryPlan, keep_state: &mut KeepState) {
    if let KeepState::Implicit = keep_state {
        plan.keep = false;
        *keep_state = KeepState::Cancelled;
    }
}

fn matches(condition: &Condition, headers: &[(String, String)], message_size: usize) -> bool {
    match condition {
        Condition::True => true,
        Condition::False => false,
        Condition::Header { names, matcher } => headers
            .iter()
            .any(|(candidate, value)| header_matches(candidate, value, names, matcher)),
        Condition::Address {
            names,
            part,
            matcher,
        } => headers
            .iter()
            .any(|(candidate, value)| address_matches(candidate, value, names, *part, matcher)),
        Condition::Exists { names } => names.iter().all(|name| {
            headers
                .iter()
                .any(|(candidate, _)| candidate.eq_ignore_ascii_case(name))
        }),
        Condition::Size { relation, limit } => match relation {
            super::ast::SizeRelation::Over => message_size > *limit,
            super::ast::SizeRelation::Under => message_size < *limit,
        },
        Condition::Not(value) => !matches(value, headers, message_size),
        Condition::AllOf(values) => values
            .iter()
            .all(|value| matches(value, headers, message_size)),
        Condition::AnyOf(values) => values
            .iter()
            .any(|value| matches(value, headers, message_size)),
    }
}

fn address_matches(
    candidate: &str,
    value: &str,
    names: &[String],
    part: super::ast::AddressPart,
    matcher: &super::ast::StringMatch,
) -> bool {
    if !names
        .iter()
        .any(|name| candidate.eq_ignore_ascii_case(name))
    {
        return false;
    }
    address::values(value, part)
        .iter()
        .any(|value| matching::matches(value, matcher))
}

/// Сначала проверяет имя header, затем его значение, не смешивая условия.
fn header_matches(
    candidate: &str,
    value: &str,
    names: &[String],
    matcher: &super::ast::StringMatch,
) -> bool {
    if !names
        .iter()
        .any(|name| candidate.eq_ignore_ascii_case(name))
    {
        return false;
    }
    matching::matches(value, matcher)
}
