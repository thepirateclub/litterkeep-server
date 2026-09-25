//! Bounded исполнение AST в DeliveryPlan без доступа к сети и файловой системе.

use super::address;
use super::ast::{Action, Command, Condition, Script};
use super::matching;
use std::fmt;

/// Единственный результат Sieve, который delivery transaction может применить.
#[derive(Debug, Eq, PartialEq)]
pub struct DeliveryPlan {
    pub keep: bool,
    pub discard: bool,
    pub folders: Vec<String>,
    pub redirects: Vec<String>,
}

/// Типизированная причина отказа bounded executor-а после успешной валидации.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ExecutionError {
    CommandLimit,
    RedirectLimit,
    RedirectRouteDenied,
    RedirectLoop,
}

impl ExecutionError {
    /// Стабильный код для долговечного журнала пользовательской диагностики.
    pub fn code(self) -> &'static str {
        match self {
            Self::CommandLimit => "command-limit",
            Self::RedirectLimit => "redirect-limit",
            Self::RedirectRouteDenied => "redirect-route-denied",
            Self::RedirectLoop => "redirect-loop",
        }
    }

    /// Разбирает только версионированные коды на границе дискового формата.
    pub fn from_code(value: &str) -> Option<Self> {
        match value {
            "command-limit" => Some(Self::CommandLimit),
            "redirect-limit" => Some(Self::RedirectLimit),
            "redirect-route-denied" => Some(Self::RedirectRouteDenied),
            "redirect-loop" => Some(Self::RedirectLoop),
            _ => None,
        }
    }
}

impl fmt::Display for ExecutionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(match self {
            Self::CommandLimit => "sieve execution limit exceeded",
            Self::RedirectLimit => "sieve redirect limit exceeded",
            Self::RedirectRouteDenied => "sieve redirect route denied",
            Self::RedirectLoop => "sieve redirect hop limit exceeded",
        })
    }
}

impl std::error::Error for ExecutionError {}

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
) -> Result<DeliveryPlan, ExecutionError> {
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
) -> Result<(), ExecutionError> {
    for command in items {
        if *stopped {
            return Ok(());
        }
        match remaining.checked_sub(1) {
            Some(value) => *remaining = value,
            None => return Err(ExecutionError::CommandLimit),
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
) -> Result<(), ExecutionError> {
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
                return Err(ExecutionError::RedirectLimit);
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
    let kind = crate::mime::header_block::HeaderName::parse(candidate);
    if !kind.is_address_list() {
        return false;
    }
    if !names
        .iter()
        .any(|name| crate::mime::header_block::HeaderName::parse(name) == kind)
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
